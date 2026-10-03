//! Line, hunk and toolbar rendering for the diff view. Split from `diff_view.rs`
//! to stay under the file ceiling.
use super::diff_view::{action, DiffAction, DiffOptions, DiffView, HunkActions};
use crate::git::{
    side_by_side, word_ranges, DiffLayout, DiffLine, FileDiff, Hunk, Origin, WordRanges,
};
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, Context, ElementId, Rgba, SharedString, Stateful};
use std::ops::Range;

const LINE_H: f32 = 21.0;
const NUM_W: f32 = 44.0;
const SIGN_W: f32 = 22.0;

fn tint(c: Rgba, a: f32) -> Rgba {
    Rgba { a, ..c }
}

fn btn(id: ElementId, label: &str, theme: &Theme, primary: bool) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(10.))
        .h(px(22.))
        .flex_none()
        .flex()
        .items_center()
        .rounded(px(4.))
        .cursor_pointer()
        .border_1()
        .border_color(theme.line)
        .text_color(theme.ink)
        .mr(px(8.))
        .text_size(px(12.))
        .when(primary, |d| {
            d.bg(theme.acc).text_color(crate::theme::on_accent())
        })
        .when(!primary, |d| d.hover(|d| d.bg(theme.hov)))
        .child(SharedString::from(label.to_string()))
}

/// Char ranges that differ, per hunk line, for inline mode.
fn inline_marks(hunk: &Hunk) -> Vec<WordRanges> {
    let mut out = vec![(Vec::new(), Vec::new()); hunk.lines.len()];
    let mut i = 0;
    while i < hunk.lines.len() {
        if hunk.lines[i].origin == Origin::Context {
            i += 1;
            continue;
        }
        let dels = i;
        while i < hunk.lines.len() && hunk.lines[i].origin == Origin::Del {
            i += 1;
        }
        let adds = i;
        while i < hunk.lines.len() && hunk.lines[i].origin == Origin::Add {
            i += 1;
        }
        for k in 0..(adds - dels).min(i - adds) {
            let (old, new) = word_ranges(&hunk.lines[dels + k].text, &hunk.lines[adds + k].text);
            out[dels + k].0 = old;
            out[adds + k].1 = new;
        }
    }
    out
}

fn marked_text(text: &str, ranges: &[Range<usize>], mark: Rgba) -> AnyElement {
    if ranges.is_empty() {
        return div()
            .whitespace_nowrap()
            .child(SharedString::from(text.to_string()))
            .into_any_element();
    }
    let mut row = div().flex().whitespace_nowrap();
    let mut at = 0;
    for range in ranges {
        if range.start > at {
            row = row.child(SharedString::from(text[at..range.start].to_string()));
        }
        row = row.child(
            div()
                .bg(mark)
                .rounded(px(2.))
                .child(SharedString::from(text[range.clone()].to_string())),
        );
        at = range.end;
    }
    if at < text.len() {
        row = row.child(SharedString::from(text[at..].to_string()));
    }
    row.into_any_element()
}

fn number(text: Option<u32>, theme: &Theme) -> AnyElement {
    div()
        .w(px(NUM_W))
        .flex_none()
        .pr(px(8.))
        .text_right()
        .text_color(theme.mute)
        .opacity(0.7)
        .child(SharedString::from(
            text.map(|n| n.to_string()).unwrap_or_default(),
        ))
        .into_any_element()
}

/// One diff line. `nums` picks which line numbers to show: old first, then new.
fn line_row(
    theme: &Theme,
    line: &DiffLine,
    show_old: bool,
    show_new: bool,
    marks: &WordRanges,
) -> AnyElement {
    let (bg, fg, sign) = match line.origin {
        Origin::Add => (tint(theme.add, 0.14), theme.add, "+"),
        Origin::Del => (tint(theme.del, 0.14), theme.del, "-"),
        Origin::Context => (Rgba { a: 0.0, ..theme.bg }, theme.ink, ""),
    };
    let mark = match line.origin {
        Origin::Del => tint(theme.del, 0.34),
        _ => tint(theme.add, 0.34),
    };
    let ranges = match line.origin {
        Origin::Del => &marks.0,
        _ => &marks.1,
    };
    let mut row = div().flex().h(px(LINE_H)).w_full().bg(bg);
    if show_old {
        row = row.child(number(line.old, theme));
    }
    if show_new {
        row = row.child(number(line.new, theme));
    }
    row.child(
        div()
            .w(px(SIGN_W))
            .flex_none()
            .text_center()
            .text_color(fg)
            .child(sign),
    )
    .child(
        div()
            .flex_1()
            .min_w_0()
            .font_family(crate::editor::view::MONO)
            .whitespace_nowrap()
            .child(marked_text(&line.text, ranges, mark)),
    )
    .into_any_element()
}

fn empty_cell(theme: &Theme) -> AnyElement {
    div()
        .flex()
        .h(px(LINE_H))
        .w_full()
        .bg(tint(theme.mute, 0.08))
        .child(number(None, theme))
        .child(div().w(px(SIGN_W)))
        .child(div().flex_1())
        .into_any_element()
}

fn hunk_header(
    theme: &Theme,
    hunk: &Hunk,
    rel: &str,
    file: usize,
    index: usize,
    actions: Option<HunkActions>,
    view: &mut Context<DiffView>,
) -> AnyElement {
    let mut row = div()
        .flex()
        .items_center()
        .h(px(28.))
        .pl(px(14.))
        .bg(tint(theme.acc, 0.1))
        .font_family(crate::editor::view::MONO)
        .text_color(theme.mute)
        .child(
            div()
                .flex_1()
                .child(SharedString::from(hunk.header.clone())),
        );
    match actions {
        Some(HunkActions::Unstaged) => {
            let (r1, r2) = (rel.to_string(), rel.to_string());
            row = row
                .child(
                    btn(
                        ElementId::named_usize("stage-hunk", file * 10_000 + index),
                        "Stage hunk",
                        theme,
                        false,
                    )
                    .on_click(view.listener(move |this, _, window, cx| {
                        action(
                            this,
                            DiffAction::StageHunk {
                                rel: r1.clone(),
                                hunk: index,
                            },
                            window,
                            cx,
                        )
                    })),
                )
                .child(
                    btn(
                        ElementId::named_usize("discard-hunk", file * 10_000 + index),
                        "Discard",
                        theme,
                        false,
                    )
                    .on_click(view.listener(move |this, _, window, cx| {
                        action(
                            this,
                            DiffAction::DiscardHunk {
                                rel: r2.clone(),
                                hunk: index,
                            },
                            window,
                            cx,
                        )
                    })),
                );
        }
        Some(HunkActions::Staged) => {
            let r1 = rel.to_string();
            row = row.child(
                btn(
                    ElementId::named_usize("unstage-hunk", file * 10_000 + index),
                    "Unstage hunk",
                    theme,
                    false,
                )
                .on_click(view.listener(move |this, _, window, cx| {
                    action(
                        this,
                        DiffAction::UnstageHunk {
                            rel: r1.clone(),
                            hunk: index,
                        },
                        window,
                        cx,
                    )
                })),
            );
        }
        None => {}
    }
    row.into_any_element()
}

fn segmented(
    theme: &Theme,
    file: usize,
    layout: DiffLayout,
    view: &mut Context<DiffView>,
) -> AnyElement {
    let mut seg = div()
        .flex()
        .flex_none()
        .border_1()
        .border_color(theme.line)
        .rounded(px(6.))
        .overflow_hidden();
    for (i, (label, value)) in [
        ("Inline", DiffLayout::Inline),
        ("Side by side", DiffLayout::Split),
    ]
    .into_iter()
    .enumerate()
    {
        seg = seg.child(
            div()
                .id(ElementId::named_usize("mode", file * 10 + i))
                .px(px(12.))
                .py(px(4.))
                .cursor_pointer()
                .when(layout == value, |d| d.bg(theme.sel).text_color(theme.ink))
                .when(layout != value, |d| d.text_color(theme.mute))
                .child(SharedString::from(label))
                .on_click(view.listener(move |this, _, _, cx| {
                    this.layout = value;
                    cx.notify();
                })),
        );
    }
    seg.into_any_element()
}

pub(super) fn file_header(
    theme: &Theme,
    file: &FileDiff,
    index: usize,
    layout: DiffLayout,
    options: &DiffOptions,
    view: &mut Context<DiffView>,
) -> AnyElement {
    let mut bar = div()
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(40.))
        .px(px(14.))
        .flex_none()
        .border_b_1()
        .border_color(theme.line)
        .text_color(theme.mute)
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .whitespace_nowrap()
                .child(SharedString::from(match file.dir.is_empty() {
                    true => String::new(),
                    false => format!("{}/", file.dir),
                }))
                .child(
                    div()
                        .text_color(theme.ink)
                        .child(SharedString::from(file.name.clone())),
                ),
        )
        .child(
            div()
                .flex()
                .gap(px(6.))
                .flex_none()
                .font_family(crate::editor::view::MONO)
                .child(
                    div()
                        .text_color(theme.add)
                        .child(SharedString::from(format!("+{}", file.added()))),
                )
                .child(
                    div()
                        .text_color(theme.del)
                        .child(SharedString::from(format!("-{}", file.removed()))),
                ),
        );
    if let Some(chip) = options.chip.clone() {
        bar = bar.child(
            div()
                .px(px(8.))
                .h(px(20.))
                .flex_none()
                .flex()
                .items_center()
                .rounded(px(10.))
                .bg(theme.sel)
                .text_color(theme.ink)
                .text_size(px(12.))
                .child(chip),
        );
    }
    bar = bar.child(segmented(theme, index, layout, view));
    if options.open_file {
        let rel = file.rel.clone();
        bar = bar.child(
            btn(
                ElementId::named_usize("open-file", index),
                "Open file",
                theme,
                false,
            )
            .mr(px(0.))
            .on_click(view.listener(move |this, _, window, cx| {
                action(this, DiffAction::OpenFile { rel: rel.clone() }, window, cx)
            })),
        );
    }
    bar.into_any_element()
}

pub(super) fn body(
    theme: &Theme,
    file: &FileDiff,
    index: usize,
    layout: DiffLayout,
    options: &DiffOptions,
    view: &mut Context<DiffView>,
) -> AnyElement {
    let actions = if file.hunks.is_empty() {
        None
    } else {
        options.hunk_actions
    };
    let mut out = div().flex().flex_col();
    if file.binary {
        return out
            .child(
                div()
                    .p(px(24.))
                    .text_color(theme.mute)
                    .child("Binary file, not shown."),
            )
            .into_any_element();
    }
    match layout {
        DiffLayout::Inline => {
            for (i, hunk) in file.hunks.iter().enumerate() {
                out = out
                    .child(hunk_header(theme, hunk, &file.rel, index, i, actions, view))
                    .children(
                        hunk.lines
                            .iter()
                            .zip(inline_marks(hunk))
                            .map(|(line, marks)| line_row(theme, line, true, true, &marks)),
                    );
            }
        }
        DiffLayout::Split => {
            for hunk in &file.hunks {
                out = out.child(
                    div()
                        .flex()
                        .h(px(28.))
                        .pl(px(14.))
                        .bg(tint(theme.acc, 0.1))
                        .font_family(crate::editor::view::MONO)
                        .text_color(theme.mute)
                        .child(SharedString::from(hunk.header.clone())),
                );
                for (old, new) in side_by_side(hunk) {
                    let (old_marks, new_marks) = match (&old, &new) {
                        (Some(a), Some(b)) => word_ranges(&a.text, &b.text),
                        _ => (Vec::new(), Vec::new()),
                    };
                    let left = match &old {
                        Some(l) => line_row(theme, l, true, false, &(old_marks, Vec::new())),
                        None => empty_cell(theme),
                    };
                    let right = match &new {
                        Some(l) => line_row(theme, l, false, true, &(Vec::new(), new_marks)),
                        None => empty_cell(theme),
                    };
                    out = out.child(
                        div()
                            .flex()
                            .w_full()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .border_r_1()
                                    .border_color(theme.line)
                                    .child(left),
                            )
                            .child(div().flex_1().min_w_0().child(right)),
                    );
                }
            }
        }
    }
    out.into_any_element()
}
