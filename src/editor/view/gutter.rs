//! Gutter and blame drawing for the editor rows: the change marks and the
//! author/age labels an add-on supplies.
use crate::addon::{BlameLine, LineMark};
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, SharedString};

pub(super) const BLAME_W: f32 = 170.0;

/// True when line `i` starts a run of lines from the same commit.
pub(super) fn blame_group_start(blame: &[Option<BlameLine>], i: usize) -> bool {
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
pub(super) fn blame_cell(blame: Option<&BlameLine>, theme: &Theme) -> AnyElement {
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

pub(super) fn inline_blame(blame: &BlameLine, theme: &Theme) -> AnyElement {
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

pub(super) fn mark_color(mark: Option<LineMark>, theme: &Theme) -> Option<gpui::Rgba> {
    match mark {
        Some(LineMark::Added) => Some(theme.add),
        Some(LineMark::Modified) => Some(theme.acc),
        None => None,
    }
}
