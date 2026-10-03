//! The editing surface: a virtualized, monospace line list with caret and selection.
//! ponytail: no IME, no horizontal scroll, no wrap yet (word wrap is a core task in
//! docs/tasks.md); upgrade: EntityInputHandler for IME, per-line shaped text.
use super::layout::{col_at_visual, expand_tabs, visual_col};
use super::state::EditorState;
use crate::actions::*;
use crate::addon::{BlameLine, EditorDecorations, LineMark};
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    canvas, div, font, prelude::*, px, AnyElement, Bounds, ClipboardItem, Context, FocusHandle,
    Focusable, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Render,
    ScrollStrategy, SharedString, UniformListScrollHandle, Window,
};
use std::ops::Range;

pub const MONO: &str = "Cascadia Mono";
const LINE_H: f32 = 21.0;
const GUTTER: f32 = 64.0;
const BLAME_W: f32 = 170.0;

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
}

impl Focusable for EditorView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EditorView {
    pub fn new(state: EditorState, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            bounds: Bounds::default(),
            char_w: 8.0,
            decorations: EditorDecorations::default(),
            blame_column: false,
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
        self.reveal_cursor();
        cx.notify();
    }

    fn reveal_cursor(&self) {
        let (line, _) = self.state.line_col(self.state.cursor);
        let top = self.scroll.0.borrow().base_handle.logical_scroll_top().0;
        let rows = ((f32::from(self.bounds.size.height) / LINE_H) as usize).max(1);
        if line < top {
            self.scroll.scroll_to_item(line, ScrollStrategy::Top);
        } else if line + 1 >= top + rows {
            self.scroll.scroll_to_item(line, ScrollStrategy::Bottom);
        }
    }

    fn click(&mut self, line: usize, x: Pixels, extend: bool, cx: &mut Context<Self>) {
        let rel = f32::from(x - self.bounds.left()) - GUTTER;
        let vcol = (rel / self.char_w + 0.5).max(0.0) as usize;
        let text = self.state.buffer.line(line);
        self.state
            .set_cursor_line_col(line, col_at_visual(&text, vcol), extend);
        cx.notify();
    }

    fn rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::Stateful<gpui::Div>> {
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        let focused = self.focus.is_focused(window);
        let (cursor_line, cursor_col) = self.state.line_col(self.state.cursor);
        let (sel_a, sel_b) = self.state.selection();
        let char_w = self.char_w;
        let blame_column = self.blame_column;
        let decorations = &self.decorations;
        range
            .map(|i| {
                let text = self.state.buffer.line(i);
                let start = self.state.buffer.line_start(i);
                let len = text.chars().count();
                let mut row = div()
                    .id(("line", i))
                    .relative()
                    .flex()
                    .h(px(LINE_H))
                    .w_full()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                            this.focus(window);
                            this.click(i, e.position.x, e.modifiers.shift, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                        if e.pressed_button == Some(MouseButton::Left) {
                            this.click(i, e.position.x, true, cx);
                        }
                    }))
                    .when(blame_column, |d| {
                        // Only the first line of a commit group carries the label.
                        let blame = match blame_group_start(&decorations.blame, i) {
                            true => decorations.blame.get(i).and_then(|b| b.as_ref()),
                            false => None,
                        };
                        d.child(blame_cell(blame, &theme))
                    })
                    .child(
                        div()
                            .w(px(GUTTER))
                            .flex_none()
                            .pr(px(18.))
                            .text_right()
                            .text_color(theme.mute)
                            .opacity(0.6)
                            .child(SharedString::from((i + 1).to_string())),
                    )
                    .when_some(
                        mark_color(decorations.marks.get(i).copied().flatten(), &theme),
                        |d, color| {
                            d.child(
                                div()
                                    .absolute()
                                    .left(px(48.))
                                    .top(px(1.))
                                    .bottom(px(1.))
                                    .w(px(3.))
                                    .rounded(px(2.))
                                    .bg(color),
                            )
                        },
                    );
                let mut body = div().relative().flex_1().h_full().overflow_hidden();
                // Selection part on this line (a selected line break extends one cell).
                if sel_a != sel_b && sel_a <= start + len && sel_b > start {
                    let from = sel_a.saturating_sub(start).min(len);
                    let to = (sel_b - start).min(len + 1);
                    let v0 = visual_col(&text, from);
                    let v1 = if to > len {
                        visual_col(&text, len) + 1
                    } else {
                        visual_col(&text, to)
                    };
                    body = body.child(
                        div()
                            .absolute()
                            .top_0()
                            .h_full()
                            .left(px(v0 as f32 * char_w))
                            .w(px((v1 - v0) as f32 * char_w))
                            .bg(theme.sel)
                            .border_1()
                            .border_color(theme.acc)
                            .opacity(0.5),
                    );
                }
                body = body.child(
                    div()
                        .whitespace_nowrap()
                        .child(SharedString::from(expand_tabs(&text))),
                );
                if !blame_column && i == cursor_line {
                    if let Some(Some(blame)) = decorations.blame.get(i) {
                        body = body.child(inline_blame(blame, &theme));
                    }
                }
                if focused && i == cursor_line {
                    body = body.child(
                        div()
                            .absolute()
                            .top(px(2.))
                            .h(px(LINE_H - 4.))
                            .w(px(2.))
                            .left(px(visual_col(&text, cursor_col) as f32 * char_w))
                            .bg(theme.acc),
                    );
                }
                row = row.child(body);
                row
            })
            .collect()
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

/// True when line `i` starts a run of lines from the same commit.
fn blame_group_start(blame: &[Option<BlameLine>], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    match (blame.get(i - 1), blame.get(i)) {
        (Some(Some(a)), Some(Some(b))) => {
            a.author != b.author || a.age != b.age || a.subject != b.subject
        }
        _ => true,
    }
}

/// A three-column blame label: author and age on the first line of a group.
fn blame_cell(blame: Option<&BlameLine>, theme: &Theme) -> AnyElement {
    div()
        .w(px(BLAME_W))
        .flex_none()
        .pr(px(8.))
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family("Segoe UI")
        .text_size(px(11.))
        .text_color(theme.mute)
        .opacity(0.85)
        .child(SharedString::from(match blame {
            Some(b) => format!("{} · {}", b.author, b.age),
            None => String::new(),
        }))
        .into_any_element()
}

fn inline_blame(blame: &BlameLine, theme: &Theme) -> AnyElement {
    div()
        .ml(px(28.))
        .flex_none()
        .whitespace_nowrap()
        .font_family("Segoe UI")
        .text_size(px(12.))
        .text_color(theme.mute)
        .opacity(0.75)
        .child(SharedString::from(format!(
            "{}, {} · {}",
            blame.author, blame.age, blame.subject
        )))
        .into_any_element()
}

fn mark_color(mark: Option<LineMark>, theme: &Theme) -> Option<gpui::Rgba> {
    match mark {
        Some(LineMark::Added) => Some(theme.add),
        Some(LineMark::Modified) => Some(theme.acc),
        None => None,
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
        let count = self.state.buffer.len_lines();
        div()
            .key_context("Editor")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
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
