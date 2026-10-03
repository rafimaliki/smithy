//! The peek's list rows and the marked symbol, in the frame's shapes.
use super::refs::Reference;
use crate::editor::view::MONO;
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, FontWeight, SharedString, Stateful};
use std::path::Path;

/// One file heading in the reference list.
pub(super) fn file_row(path: &Path, t: &Theme) -> AnyElement {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = path
        .parent()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_default();
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .child(SharedString::from(name))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_right()
                .text_size(px(12.))
                .text_color(t.mute)
                .child(SharedString::from(dir)),
        )
        .into_any_element()
}

/// One reference line, with the `def` mark on the declaration.
pub(super) fn reference_row(
    id: impl Into<gpui::ElementId>,
    reference: &Reference,
    selected: bool,
    t: &Theme,
) -> Stateful<gpui::Div> {
    let mut row = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(26.))
        .px(px(14.))
        .cursor_pointer()
        .font_family(MONO)
        .text_size(px(12.))
        .whitespace_nowrap()
        .overflow_hidden()
        .when(selected, |d| d.bg(t.sel))
        .when(!selected, |d| d.hover(|d| d.bg(t.hov)))
        .child(
            div()
                .w(px(28.))
                .flex_none()
                .text_right()
                .text_color(t.mute)
                .child(SharedString::from((reference.loc.line + 1).to_string())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .child(SharedString::from(reference.text.clone())),
        );
    if reference.is_def {
        row = row.child(
            div()
                .flex_none()
                .px(px(6.))
                .text_size(px(10.))
                .font_weight(FontWeight::BOLD)
                .text_color(t.acc)
                .border_1()
                .border_color(t.acc)
                .rounded(px(8.))
                .child("def"),
        );
    }
    row
}

/// `text` with `symbol` picked out, as the frame's `mark`.
pub(super) fn marked(text: &str, symbol: &str, t: &Theme) -> AnyElement {
    let at = (!symbol.is_empty()).then(|| text.find(symbol)).flatten();
    let Some(at) = at else {
        return div()
            .whitespace_nowrap()
            .child(SharedString::from(text.to_string()))
            .into_any_element();
    };
    div()
        .flex()
        .whitespace_nowrap()
        .child(SharedString::from(text[..at].to_string()))
        .child(
            div()
                .bg(t.sel)
                .text_color(t.acc)
                .child(SharedString::from(symbol.to_string())),
        )
        .child(SharedString::from(text[at + symbol.len()..].to_string()))
        .into_any_element()
}
