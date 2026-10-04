//! The editing surface: caret, selection and input. Drawing a row lives in
//! `view/rows.rs`, wrap and horizontal scroll in `view/wrap.rs`; the gutter and
//! blame live in `view/gutter.rs`, and the Ctrl+F bar in `view/find_bar.rs`.
//! ponytail: no IME; upgrade: EntityInputHandler for IME, per-line shaped text.
mod blink;
mod find_bar;
mod gutter;
mod gutter_menu;
mod input;
mod rows;
mod wrap;

use super::find::FindState;
use super::highlight::Highlighter;
use super::lang::Lang;
use super::load::{self, Loaded};
use super::state::EditorState;
use super::wrap::{Row, WrapIndex};
use crate::actions::*;
use crate::addon::{EditorDecorations, Navigate};
use crate::settings::Settings;
use crate::theme::Theme;
use blink::Blink;
use gpui::{
    canvas, div, font, prelude::*, px, Bounds, ClipboardItem, Context, EventEmitter, FocusHandle,
    Focusable, Pixels, Render, ScrollStrategy, UniformListScrollHandle, Window,
};
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

pub const MONO: &str = "Cascadia Mono";
const LINE_H: f32 = 21.0;
const GUTTER: f32 = 64.0;

/// The editor asking the workspace to navigate — Ctrl+Click, F12, Shift+F12,
/// Alt+Left or Alt+Right. The workspace owns the add-ons that can answer it.
#[derive(Clone)]
pub enum EditorEvent {
    Navigate {
        what: Navigate,
        path: PathBuf,
        line: u32,
        character: u32,
    },
}

impl EventEmitter<EditorEvent> for EditorView {}

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
    /// Ctrl+F bar; `None` when it is closed.
    find: Option<FindState>,
    /// Buffer revision the matches were built from.
    find_rev: u64,
    /// Caret blink phase; restarts on every move.
    blink: Blink,
    /// Where the line-number menu is open, in window coordinates.
    gutter_menu: Option<gpui::Point<gpui::Pixels>>,
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
    /// Size on disk, shown by the large-file bar.
    size_bytes: u64,
    /// The file changed on disk while this buffer had unsaved edits.
    disk_changed: bool,
    /// Modification time of the content we last loaded or wrote, so the watcher
    /// can tell an external change from our own save.
    disk_mtime: Option<SystemTime>,
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
        let on_disk = state
            .path
            .as_deref()
            .and_then(|p| std::fs::metadata(p).ok());
        let size_bytes = on_disk.as_ref().map(|m| m.len()).unwrap_or(0);
        let disk_mtime = on_disk.and_then(|m| m.modified().ok());
        // Redraw at each blink toggle, only while the editor has focus.
        cx.spawn(async move |this, cx| loop {
            let wait = this.update(cx, |this, _| this.blink.until_toggle(Instant::now()));
            let Ok(wait) = wait else { break };
            cx.background_executor().timer(wait).await;
            let alive = this.update(cx, |this, cx| {
                if this.blink.is_focused() {
                    cx.notify();
                }
            });
            if alive.is_err() {
                break;
            }
        })
        .detach();
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
            find: None,
            find_rev: u64::MAX,
            blink: Blink::new(),
            gutter_menu: None,
            wrap,
            wrap_index: None,
            wrap_key: (u64::MAX, 0),
            scroll_x: 0.0,
            max_cols: 0,
            max_key: u64::MAX,
            size_bytes,
            disk_changed: false,
            disk_mtime,
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
        if self.find.is_some() && rev != self.find_rev {
            self.find_rev = rev;
            self.find_recompute();
        }
    }

    /// Put the caret at the start of `line` (0-based) and centre it,
    /// e.g. when a Search result is opened.
    pub fn goto_line(&mut self, line: usize, cx: &mut Context<Self>) {
        self.state.set_cursor_line_col(line, 0, false);
        let (line, col) = self.state.line_col(self.state.cursor);
        // With wrap on, a logical line spans several rows; the list is indexed by row.
        let row = self.row_of_caret(line, col);
        self.scroll.scroll_to_item(row, ScrollStrategy::Center);
        cx.notify();
    }

    /// Write the buffer to its path and clear the dirty mark.
    pub fn save(&mut self, cx: &mut Context<Self>) -> std::io::Result<()> {
        if let Some(path) = self.state.path.clone() {
            std::fs::write(&path, self.state.buffer.text())?;
            self.state.mark_saved();
            self.disk_mtime = std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok());
            self.disk_changed = false;
            cx.notify();
        }
        Ok(())
    }

    /// Size on disk, for the large-file bar.
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// The file changed on disk while this buffer had unsaved edits.
    pub fn disk_changed(&self) -> bool {
        self.disk_changed
    }

    /// Act on an external change to the open file. Reloads silently when the buffer
    /// is clean; raises the Reload / Keep mine bar when it has unsaved edits.
    pub fn external_change(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.state.path.clone() else {
            return;
        };
        let mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        if mtime.is_none() || mtime == self.disk_mtime {
            // Gone, or the write we just made ourselves.
            return;
        }
        if self.state.is_dirty() {
            self.disk_mtime = mtime;
            self.disk_changed = true;
            cx.notify();
            return;
        }
        self.reload_from_disk(cx);
    }

    /// Reload the file from disk, discarding the buffer (Reload on the bar).
    pub fn reload_from_disk(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.state.path.clone() else {
            return;
        };
        let mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        if let Loaded::Text { text, read_only } = load::load(&path) {
            self.state.read_only = read_only;
            self.state.reload(&text);
            self.highlighter = if read_only {
                None
            } else {
                self.lang.and_then(Highlighter::new)
            };
            self.hl_revision = u64::MAX;
            self.refresh_highlight();
            self.disk_changed = false;
            self.disk_mtime = mtime;
            cx.notify();
        }
    }

    /// Keep the buffer after an external change; the next save overwrites the disk.
    pub fn keep_mine(&mut self, cx: &mut Context<Self>) {
        self.disk_changed = false;
        cx.notify();
    }

    /// Large file: open editable, with highlighting, after the deliberate click.
    pub fn open_normally(&mut self, cx: &mut Context<Self>) {
        self.state.read_only = false;
        self.highlighter = self.lang.and_then(Highlighter::new);
        self.hl_revision = u64::MAX;
        self.refresh_highlight();
        cx.notify();
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.refresh_highlight();
        self.reveal_cursor();
        cx.notify();
    }
    /// Column `col` of `line` as UTF-16 units, the way a language server counts.
    fn character(&self, line: usize, col: usize) -> u32 {
        self.state
            .buffer
            .line(line)
            .chars()
            .take(col)
            .map(|c| c.len_utf16() as u32)
            .sum()
    }

    /// Ctrl+Click: a navigation gesture, not a caret move. Under wrap the click
    /// lands on a slice of the logical line, so the column comes from `col_at`.
    fn ctrl_click(&mut self, row: Row, x: Pixels, cx: &mut Context<Self>) {
        let character = self.character(row.line, self.col_at(row, x));
        self.emit_navigate(Navigate::Definition, row.line as u32, character, cx);
    }

    fn emit_navigate(&mut self, what: Navigate, line: u32, character: u32, cx: &mut Context<Self>) {
        if let Some(path) = self.state.path.clone() {
            cx.emit(EditorEvent::Navigate {
                what,
                path,
                line,
                character,
            });
        }
    }

    /// F12, Shift+F12: act on the symbol at the caret.
    fn at_caret(&mut self, what: Navigate, cx: &mut Context<Self>) {
        let (line, col) = self.state.line_col(self.state.cursor);
        let character = self.character(line, col);
        self.emit_navigate(what, line as u32, character, cx);
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

    fn goto_definition(&mut self, _: &GoToDefinition, _: &mut Window, cx: &mut Context<Self>) {
        self.at_caret(Navigate::Definition, cx);
    }

    fn find_references(&mut self, _: &FindReferences, _: &mut Window, cx: &mut Context<Self>) {
        self.at_caret(Navigate::References, cx);
    }

    fn nav_back(&mut self, _: &NavBack, _: &mut Window, cx: &mut Context<Self>) {
        self.at_caret(Navigate::Back, cx);
    }

    fn nav_forward(&mut self, _: &NavForward, _: &mut Window, cx: &mut Context<Self>) {
        self.at_caret(Navigate::Forward, cx);
    }
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
            .on_action(cx.listener(Self::on_find))
            .on_action(cx.listener(Self::goto_definition))
            .on_action(cx.listener(Self::find_references))
            .on_action(cx.listener(Self::nav_back))
            .on_action(cx.listener(Self::nav_forward))
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
            .children(self.find_bar(&theme, cx))
            .children(self.gutter_menu_view(&theme, cx))
    }
}
