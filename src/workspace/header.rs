//! The right end of the header row, the same row as the file tabs. The native title
//! bar is hidden (`appears_transparent` in `app.rs`), so the window is moved from the
//! empty part of this row and closed, minimized and maximized from the buttons here.
//! After the tabs: a drag area, then the buttons.
use super::Workspace;
use crate::theme::Theme;
use gpui::{div, prelude::*, px, rgb, Div, IntoElement, WindowControlArea};

const BUTTON_W: f32 = 46.0;

/// The empty part of the header row: dragging it moves the window.
pub(super) fn drag_area() -> Div {
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .window_control_area(WindowControlArea::Drag)
}

/// Minimize, maximize and close; Windows treats them as caption controls.
pub(super) fn window_controls(t: &Theme) -> impl IntoElement {
    // ponytail: text glyphs for the three buttons; upgrade: the board's icons.
    let button = |id: &'static str, glyph: &'static str, area, danger: bool| {
        div()
            .id(id)
            .w(px(BUTTON_W))
            .h_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .text_color(t.mute)
            .hover(move |d| {
                if danger {
                    d.bg(rgb(0xc42b1c)).text_color(rgb(0xffffff))
                } else {
                    d.bg(t.hov).text_color(t.ink)
                }
            })
            .window_control_area(area)
            .child(glyph)
    };
    div()
        .flex()
        .flex_none()
        .h_full()
        .child(button("win-min", "\u{2212}", WindowControlArea::Min, false))
        .child(button("win-max", "\u{25a1}", WindowControlArea::Max, false))
        .child(button(
            "win-close",
            "\u{2715}",
            WindowControlArea::Close,
            true,
        ))
}

impl Workspace {
    /// The window buttons: the tail of the header row.
    pub(super) fn header_tail(&self, t: &Theme) -> impl IntoElement {
        div()
            .flex()
            .flex_none()
            .items_center()
            .h_full()
            .child(window_controls(t))
    }

    /// The header row of the launch screen: nothing to show but the window buttons.
    pub(super) fn launch_header(&self, t: &Theme) -> impl IntoElement {
        div()
            .h(px(36.))
            .flex_none()
            .flex()
            .bg(t.side)
            .border_b_1()
            .border_color(t.line)
            .child(drag_area())
            .child(window_controls(t))
    }
}
