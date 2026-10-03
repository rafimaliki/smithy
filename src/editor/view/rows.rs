//! Drawing the visible editor lines: gutter ticks and blame, selection, caret
//! and the syntax colored text of each line.
use super::{EditorView, GUTTER, LINE_H};
use crate::addon::{BlameLine, LineMark};
use crate::editor::find;
use crate::editor::highlight::{self, Kind};
use crate::editor::layout::{expand_tabs, visual_col};
use crate::settings::Settings;
use crate::theme::{on_accent, Theme};
use gpui::{
    div, prelude::*, px, AnyElement, Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent,
    Rgba, SharedString, Stateful, Window,
};
use std::ops::Range;

const BLAME_W: f32 = 170.0;

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
        let blame_column = this.blame_column;
        let decorations = &this.decorations;
        let finder = this.find.as_ref();
        let current_line = finder.and_then(|f| f.current.map(|i| f.matches[i].line));
        range
            .map(|i| {
                let text = this.state.buffer.line(i);
                let start = this.state.buffer.line_start(i);
                let len = text.chars().count();
                let line_matches: Vec<(Range<usize>, bool)> = match finder {
                    Some(f) => {
                        let (base, matches) = find::on_line(&f.matches, i);
                        matches
                            .iter()
                            .enumerate()
                            .map(|(j, m)| (m.col.clone(), f.current == Some(base + j)))
                            .collect()
                    }
                    None => Vec::new(),
                };
                let mut row = div()
                    .id(("line", i))
                    .relative()
                    .flex()
                    .h(px(LINE_H))
                    .w_full()
                    // The line holding the current match, drawn under the highlights.
                    .when(current_line == Some(i), |d| d.bg(fade(theme.sel, 0.7)))
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
                body = body.child(this.text_row(&text, i, &theme, &line_matches));
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

    /// One line as colored runs: the syntax spans with the find matches painted
    /// over them. The current match gets the stronger background and dark text.
    fn text_row(
        &self,
        text: &str,
        line: usize,
        theme: &Theme,
        matches: &[(Range<usize>, bool)],
    ) -> Div {
        let len = text.chars().count();
        let mut row = div().flex().whitespace_nowrap();
        if len == 0 {
            return row;
        }
        // ponytail: per-char arrays like `highlight::segments`; lines over 10k
        // chars (minified files) draw plain, matches included.
        if len > 10_000 {
            return row.child(piece(text, 0..len, None, None, theme));
        }
        let spans = self
            .highlighter
            .as_ref()
            .map(|h| h.line_spans(line))
            .unwrap_or(&[]);
        let mut kind = vec![None; len];
        for (range, k) in highlight::segments(len, spans) {
            for slot in &mut kind[range] {
                *slot = Some(k);
            }
        }
        let marks = if matches.is_empty() {
            Vec::new()
        } else {
            let mut marks = vec![None; len];
            for (range, current) in matches {
                let start = range.start.min(len);
                let end = range.end.min(len);
                for slot in &mut marks[start..end] {
                    *slot = Some(*current);
                }
            }
            marks
        };
        let mark_at = |i: usize| marks.get(i).copied().flatten();
        let mut i = 0;
        while i < len {
            let (k, m) = (kind[i], mark_at(i));
            let mut j = i + 1;
            while j < len && kind[j] == k && mark_at(j) == m {
                j += 1;
            }
            row = row.child(piece(text, i..j, k, m, theme));
            i = j;
        }
        row
    }
}

/// A colored slice of a line. Tabs expand inside the slice, the same as in the
/// whole line, because expansion does not depend on the column. `mark` paints a
/// find match: `Some(true)` is the current one.
fn piece(
    text: &str,
    range: Range<usize>,
    kind: Option<Kind>,
    mark: Option<bool>,
    theme: &Theme,
) -> impl IntoElement {
    let s: String = text
        .chars()
        .skip(range.start)
        .take(range.end - range.start)
        .collect();
    div()
        .flex_none()
        .whitespace_nowrap()
        .when_some(kind, |d, k| d.text_color(highlight::color(k, theme)))
        .when_some(mark, |d, current| {
            if current {
                d.bg(fade(theme.acc, 0.75)).text_color(on_accent())
            } else {
                d.bg(fade(theme.acc, 0.38))
            }
        })
        .child(SharedString::from(expand_tabs(&s)))
}

/// `color` at a different alpha, for a background that lets the row show through.
fn fade(color: Rgba, alpha: f32) -> Rgba {
    Rgba { a: alpha, ..color }
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
