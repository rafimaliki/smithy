//! Appearance previews: a theme card is a miniature window in that theme's own
//! colors, a file-icon card lists sample files with that icon style. Board frame
//! `settings-appearance`.
use crate::theme::Theme;
use gpui::{div, prelude::*, px, relative, Div, ElementId, Rgba, SharedString, Stateful};
use std::path::Path;

const CARD_W: f32 = 200.0;

/// A card shell: bordered, accent border when picked, a name row under `preview`.
fn card(
    id: impl Into<ElementId>,
    name: &str,
    on: bool,
    t: &Theme,
    preview: impl IntoElement,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(CARD_W))
        .flex_none()
        .rounded(px(10.))
        .overflow_hidden()
        .cursor_pointer()
        .border_1()
        .border_color(if on { t.acc } else { t.line })
        .bg(t.side)
        .child(preview)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.))
                .py(px(10.))
                .child(SharedString::from(name.to_string()))
                .when(on, |d| d.child(div().text_color(t.acc).child("\u{2713}"))),
        )
}

fn bar(color: Rgba, width: f32) -> Div {
    div()
        .h(px(5.))
        .my(px(6.))
        .rounded(px(3.))
        .w(relative(width))
        .bg(color)
}

/// `th` drawn as a tiny window: rail, sidebar with three rows, editor with five
/// syntax-colored lines. `t` is the theme the settings page itself is drawn in.
pub fn theme_card(id: impl Into<ElementId>, th: &Theme, on: bool, t: &Theme) -> Stateful<Div> {
    let preview = div()
        .h(px(112.))
        .flex()
        .bg(th.bg)
        .child(div().w(px(14.)).flex_none().bg(th.rail))
        .child(
            div()
                .w(px(52.))
                .flex_none()
                .px(px(6.))
                .py(px(8.))
                .bg(th.side)
                .child(bar(th.mute, 0.7))
                .child(bar(th.acc, 0.5))
                .child(bar(th.mute, 0.6)),
        )
        .child(
            div()
                .flex_1()
                .px(px(10.))
                .py(px(8.))
                .child(bar(th.kw, 0.6))
                .child(bar(th.func, 0.8))
                .child(bar(th.str, 0.45))
                .child(bar(th.ty, 0.7))
                .child(bar(th.comment, 0.3)),
        );
    card(id, th.name, on, t, preview)
}

/// Five sample files drawn with the brand or the simple icon style.
pub fn icon_card(
    id: impl Into<ElementId>,
    name: &str,
    simple: bool,
    on: bool,
    t: &Theme,
) -> Stateful<Div> {
    let mut rows = div().h(px(150.)).px(px(12.)).py(px(10.)).flex().flex_col();
    for file in [
        "Button.tsx",
        "Routes.kt",
        "package.json",
        "README.md",
        "notes.txt",
    ] {
        rows = rows.child(
            div()
                .h(px(26.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .w(px(16.))
                        .h(px(16.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::tree::view::icon(t, simple, Path::new(file), false)),
                )
                .child(SharedString::from(file)),
        );
    }
    card(id, name, on, t, rows)
}
