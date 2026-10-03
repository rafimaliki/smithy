//! Drawing the visible editor lines: gutter, selection, caret and the syntax
//! colored text of each line.
use super::{EditorView, GUTTER, LINE_H};
use crate::editor::highlight::{self, Kind};
use crate::editor::layout::{expand_tabs, visual_col};
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, SharedString,
    Stateful, Window,
};
use std::ops::Range;

impl EditorView {
    /// The rows `uniform_list` asks for.
    pub(super) fn rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Stateful<Div>> {
        self.refresh_highlight();
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        let focused = self.focus.is_focused(window);
        let (cursor_line, cursor_col) = self.state.line_col(self.state.cursor);
        let (sel_a, sel_b) = self.state.selection();
        let char_w = self.char_w;
        let this = &*self;
        range
            .map(|i| {
                let text = this.state.buffer.line(i);
                let start = this.state.buffer.line_start(i);
                let len = text.chars().count();
                let mut row = div()
                    .id(("line", i))
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
                    .child(
                        div()
                            .w(px(GUTTER))
                            .flex_none()
                            .pr(px(18.))
                            .text_right()
                            .text_color(theme.mute)
                            .opacity(0.6)
                            .child(SharedString::from((i + 1).to_string())),
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
                body = body.child(this.text_row(&text, i, &theme));
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

    /// One line as colored runs: the highlight spans, with the gaps and any
    /// unhighlighted line in the plain text color.
    fn text_row(&self, text: &str, line: usize, theme: &Theme) -> Div {
        let len = text.chars().count();
        let spans = self
            .highlighter
            .as_ref()
            .map(|h| h.line_spans(line))
            .unwrap_or(&[]);
        let mut row = div().flex().whitespace_nowrap();
        let mut at = 0;
        for (range, kind) in highlight::segments(len, spans) {
            if range.start > at {
                row = row.child(piece(text, at..range.start, None, theme));
            }
            row = row.child(piece(text, range.clone(), Some(kind), theme));
            at = range.end;
        }
        if at < len {
            row = row.child(piece(text, at..len, None, theme));
        }
        row
    }
}

/// A colored slice of a line. Tabs expand inside the slice, the same as in the
/// whole line, because expansion does not depend on the column.
fn piece(text: &str, range: Range<usize>, kind: Option<Kind>, theme: &Theme) -> impl IntoElement {
    let s: String = text
        .chars()
        .skip(range.start)
        .take(range.end - range.start)
        .collect();
    div()
        .flex_none()
        .whitespace_nowrap()
        .when_some(kind, |d, k| d.text_color(highlight::color(k, theme)))
        .child(SharedString::from(expand_tabs(&s)))
}
