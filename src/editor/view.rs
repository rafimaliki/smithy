//! The editing surface: caret, selection and input. Drawing a row lives in
//! `view/rows.rs`, wrap and horizontal scroll in `view/wrap.rs`.
//! ponytail: no IME; upgrade: EntityInputHandler for IME, per-line shaped text.
mod gutter;
mod rows;
mod wrap;

use super::highlight::Highlighter;
use super::lang::Lang;
use super::state::EditorState;
use super::wrap::WrapIndex;
use crate::actions::*;
use crate::addon::EditorDecorations;
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    canvas, div, font, prelude::*, px, Bounds, ClipboardItem, Context, FocusHandle, Focusable,
    KeyDownEvent, Pixels, Render, UniformListScrollHandle, Window,
};

pub const MONO: &str = "Cascadia Mono";
const LINE_H: f32 = 21.0;
const GUTTER: f32 = 64.0;

pub struct EditorView {
    pub state: EditorState,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    bounds: Bounds<Pixels>,
    char_w: f32,
    /// Gutter marks and blame supplied by an add-on; empty without one.
    decorations: EditorDecorations,
    /// Ctrl+Alt+B: show the full blame column instead of the caret-line label.
    blame_column: bool,
    /// The file's language: detected from the extension, or the picker's override.
    lang: Option<Lang>,
    /// `None` for plain text and for large files, which skip highlighting.
    highlighter: Option<Highlighter>,
    /// Buffer revision the spans were built from; `u64::MAX` forces the first pass.
    hl_revision: u64,
    /// Alt+Z or the status-bar item flips this; last choice is in `Settings`.
    wrap: bool,
    /// Rows for the buffer at the current width; `None` when wrap is off.
    wrap_index: Option<WrapIndex>,
    /// Buffer revision and column count `wrap_index` was built from.
    wrap_key: (u64, usize),
    /// Horizontal scroll in pixels; only used when wrap is off.
    scroll_x: f32,
    /// Widest logical line in display columns, bounding horizontal scroll.
    max_cols: usize,
    /// Buffer revision `max_cols` was measured at.
    max_key: u64,
}

impl Focusable for EditorView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EditorView {
    pub fn new(state: EditorState, cx: &mut Context<Self>) -> Self {
        // Large (read-only) files still show their language, but skip highlighting.
        let lang = state.path.as_deref().and_then(Lang::for_path);
        let highlighter = if state.read_only {
            None
        } else {
            lang.and_then(Highlighter::new)
        };
        let wrap = cx.global::<Settings>().word_wrap;
        Self {
            state,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            bounds: Bounds::default(),
            char_w: 8.0,
            decorations: EditorDecorations::default(),
            blame_column: false,
            lang,
            highlighter,
            hl_revision: u64::MAX,
            wrap,
            wrap_index: None,
            wrap_key: (u64::MAX, 0),
            scroll_x: 0.0,
            max_cols: 0,
            max_key: u64::MAX,
        }
    }

    /// Replace the gutter marks and blame, e.g. after an add-on refreshed.
    pub fn set_decorations(&mut self, decorations: EditorDecorations) {
        self.decorations = decorations;
    }

    fn toggle_blame(&mut self, _: &ToggleBlame, _: &mut Window, cx: &mut Context<Self>) {
        self.blame_column = !self.blame_column;
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus);
    }

    /// The language shown in the status bar, override included.
    pub fn lang(&self) -> Option<Lang> {
        self.lang
    }

    /// Language picker choice for this file; `None` means plain text.
    pub fn set_lang(&mut self, lang: Option<Lang>, cx: &mut Context<Self>) {
        self.lang = lang;
        self.highlighter = if self.state.read_only {
            None
        } else {
            lang.and_then(Highlighter::new)
        };
        self.hl_revision = u64::MAX;
        cx.notify();
    }

    /// Re-parse when the buffer changed; incremental after the first parse.
    // ponytail: the whole buffer is materialized per edit to feed the parser;
    // upgrade: hand tree-sitter the rope's chunks via a TextProvider.
    fn refresh_highlight(&mut self) {
        let rev = self.state.buffer.revision;
        if let Some(h) = self.highlighter.as_mut() {
            if rev != self.hl_revision {
                h.update(&self.state.buffer.text());
                self.hl_revision = rev;
            }
        }
    }

    /// Write the buffer to its path and clear the dirty mark.
    pub fn save(&mut self, cx: &mut Context<Self>) -> std::io::Result<()> {
        if let Some(path) = self.state.path.clone() {
            std::fs::write(path, self.state.buffer.text())?;
            self.state.mark_saved();
            cx.notify();
        }
        Ok(())
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.refresh_highlight();
        self.reveal_cursor();
        cx.notify();
    }

    fn key_down(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let m = e.keystroke.modifiers;
        if m.control || m.alt || m.platform {
            return;
        }
        // Windows reports the space bar as key "space" with no key_char.
        let typed = match (e.keystroke.key_char.as_deref(), e.keystroke.key.as_str()) {
            (Some(ch), _) => Some(ch),
            (None, "space") => Some(" "),
            _ => None,
        };
        if let Some(ch) = typed {
            if !ch.chars().any(|c| c.is_control()) {
                self.state.insert(ch);
                self.changed(cx);
            }
        }
    }
}

macro_rules! act {
    ($name:ident, $action:ty, |$s:ident, $cx:ident| $body:block) => {
        fn $name(&mut self, _: &$action, _: &mut Window, $cx: &mut Context<Self>) {
            let $s = &mut self.state;
            $body
            self.changed($cx);
        }
    };
}

impl EditorView {
    act!(move_left, MoveLeft, |s, _cx| { s.move_left(false) });
    act!(move_right, MoveRight, |s, _cx| { s.move_right(false) });
    act!(move_up, MoveUp, |s, _cx| { s.move_vertical(-1, false) });
    act!(move_down, MoveDown, |s, _cx| { s.move_vertical(1, false) });
    act!(select_left, SelectLeft, |s, _cx| { s.move_left(true) });
    act!(select_right, SelectRight, |s, _cx| { s.move_right(true) });
    act!(select_up, SelectUp, |s, _cx| { s.move_vertical(-1, true) });
    act!(select_down, SelectDown, |s, _cx| {
        s.move_vertical(1, true)
    });
    act!(home, Home, |s, _cx| { s.home(false) });
    act!(end, End, |s, _cx| { s.end(false) });
    act!(select_home, SelectHome, |s, _cx| { s.home(true) });
    act!(select_end, SelectEnd, |s, _cx| { s.end(true) });
    act!(backspace, Backspace, |s, _cx| { s.backspace() });
    act!(delete, Delete, |s, _cx| { s.delete() });
    act!(enter, Enter, |s, _cx| { s.insert("\n") });
    act!(tab, Tab, |s, _cx| { s.insert("\t") });
    act!(undo, Undo, |s, _cx| { s.undo() });
    act!(redo, Redo, |s, _cx| { s.redo() });
    act!(select_all, SelectAll, |s, _cx| { s.select_all() });
    act!(copy, Copy, |s, cx| {
        if s.has_selection() {
            cx.write_to_clipboard(ClipboardItem::new_string(s.selected_text()));
        }
    });
    act!(cut, Cut, |s, cx| {
        if s.has_selection() && !s.read_only {
            cx.write_to_clipboard(ClipboardItem::new_string(s.selected_text()));
            s.insert("");
        }
    });
    act!(paste, Paste, |s, cx| {
        if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
            s.insert(&text.replace("\r\n", "\n"));
        }
    });
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = cx.global::<Settings>();
        let theme = Theme::by_name(&settings.theme);
        let size = px(settings.font_size);
        let f = font(MONO);
        let id = window.text_system().resolve_font(&f);
        if let Ok(adv) = window.text_system().advance(id, size, 'm') {
            self.char_w = f32::from(adv.width);
        }
        let entity = cx.entity();
        self.ensure_wrap(self.text_width());
        let count = self.visual_row_count();
        div()
            .key_context("Editor")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_scroll_wheel(cx.listener(Self::scroll_wheel))
            .on_action(cx.listener(Self::move_left))
            .on_action(cx.listener(Self::move_right))
            .on_action(cx.listener(Self::move_up))
            .on_action(cx.listener(Self::move_down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::tab))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::toggle_blame))
            .on_action(cx.listener(Self::toggle_wrap))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .relative()
            .size_full()
            .bg(theme.bg)
            .text_color(theme.ink)
            .font_family(MONO)
            .text_size(size)
            .pt(px(6.))
            .child(
                canvas(
                    move |bounds, _, cx| {
                        entity.update(cx, |this, _| this.bounds = bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                gpui::uniform_list("lines", count, cx.processor(Self::rows))
                    .track_scroll(self.scroll.clone())
                    .size_full(),
            )
    }
}
