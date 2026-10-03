//! Drawing the visible editor rows: gutter ticks and blame, selection, caret
//! and the syntax colored text of each row. With word wrap on a logical line
//! is several fixed-height rows; its number and marks stay on the first one.
use super::gutter::{blame_cell, blame_group_start, inline_blame, mark_color};
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
        let scroll_x = self.scroll_x;
        let this = &*self;
        let blame_column = this.blame_column;
        let decorations = &this.decorations;
        range
            .map(|i| {
                let r = this.row_at(i);
                let line = r.line;
                let text = this.state.buffer.line(line);
                let line_start = this.state.buffer.line_start(line);
                let len = text.chars().count();
                let first = r.start == 0;
                let last = r.end == len;
                let vbase = visual_col(&text, r.start);
                let mut row = div()
                    .id(("row", i))
                    .relative()
                    .flex()
                    .h(px(LINE_H))
                    .w_full()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                            this.focus(window);
                            // Ctrl+Click goes to the definition; the caret does not move.
                            if e.modifiers.control {
                                this.ctrl_click(r, e.position.x, cx);
                            } else {
                                this.click(r, e.position.x, e.modifiers.shift, cx);
                            }
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                        if e.pressed_button == Some(MouseButton::Left) {
                            this.click(r, e.position.x, true, cx);
                        }
                    }))
                    .when(blame_column && first, |d| {
                        // Only the first line of a commit group carries the label.
                        let blame = match blame_group_start(&decorations.blame, line) {
                            true => decorations.blame.get(line).and_then(|b| b.as_ref()),
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
                            .child(SharedString::from(if first {
                                (line + 1).to_string()
                            } else {
                                String::new()
                            })),
                    );
                if first {
                    if let Some(color) =
                        mark_color(decorations.marks.get(line).copied().flatten(), &theme)
                    {
                        row = row.child(
                            div()
                                .absolute()
                                .left(px(48.))
                                .top(px(1.))
                                .bottom(px(1.))
                                .w(px(3.))
                                .rounded(px(2.))
                                .bg(color),
                        );
                    }
                }
                let mut body = div().relative().flex_1().h_full().overflow_hidden();
                // Selection part on this row (a selected line break extends one cell).
                if sel_a != sel_b && sel_b > line_start && sel_a <= line_start + len {
                    let a = sel_a.saturating_sub(line_start).min(len);
                    let b = (sel_b - line_start).min(len + 1);
                    let lo = a.max(r.start).min(r.end);
                    let hi = b.min(r.end);
                    let newline = b > len && last;
                    let v0 = visual_col(&text, lo) - vbase;
                    let v1 = visual_col(&text, hi) - vbase + usize::from(newline);
                    if v1 > v0 {
                        body = body.child(
                            div()
                                .absolute()
                                .top_0()
                                .h_full()
                                .left(px(v0 as f32 * char_w - scroll_x))
                                .w(px((v1 - v0) as f32 * char_w))
                                .bg(theme.sel)
                                .border_1()
                                .border_color(theme.acc)
                                .opacity(0.5),
                        );
                    }
                }
                body = body.child(this.text_row(&text, line, r.start..r.end, &theme));
                if !blame_column && last && line == cursor_line {
                    if let Some(Some(blame)) = decorations.blame.get(line) {
                        body = body.child(inline_blame(blame, &theme));
                    }
                }
                if focused
                    && line == cursor_line
                    && cursor_col >= r.start
                    && (cursor_col < r.end || last)
                {
                    body = body.child(
                        div()
                            .absolute()
                            .top(px(2.))
                            .h(px(LINE_H - 4.))
                            .w(px(2.))
                            .left(px(visual_col(&text, cursor_col) as f32 * char_w - scroll_x))
                            .bg(theme.acc),
                    );
                }
                row = row.child(body);
                row
            })
            .collect()
    }

    /// Char columns `cols` of a line as colored runs: the highlight spans, with
    /// the gaps and any unhighlighted part in the plain text color.
    fn text_row(&self, text: &str, line: usize, cols: Range<usize>, theme: &Theme) -> Div {
        let len = text.chars().count();
        let spans = self
            .highlighter
            .as_ref()
            .map(|h| h.line_spans(line))
            .unwrap_or(&[]);
        let mut row = div().flex().whitespace_nowrap().ml(px(-self.scroll_x));
        let end = cols.end.min(len);
        let mut at = cols.start;
        for (range, kind) in highlight::segments(len, spans) {
            if range.end <= cols.start {
                continue;
            }
            if range.start >= end {
                break;
            }
            let from = range.start.max(cols.start);
            let to = range.end.min(end);
            if from > at {
                row = row.child(piece(text, at..from, None, theme));
            }
            row = row.child(piece(text, from..to, Some(kind), theme));
            at = to;
        }
        if at < end {
            row = row.child(piece(text, at..end, None, theme));
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
