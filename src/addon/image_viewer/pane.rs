//! The pane the image sits on: a checkerboard that shows through transparency, and
//! the image itself — shrunk to fit, or a scrollable box at the bar's zoom.
use super::format::Dimensions;
use super::view::{ImageView, Zoom};
use crate::theme::Theme;
use gpui::{canvas, div, fill, img, point, prelude::*, px, size, AnyElement, Bounds};

/// Checkerboard cell, the board's 20px tile.
const CELL: f32 = 20.0;

pub(super) fn pane(view: &ImageView, t: &Theme, dims: Option<Dimensions>) -> AnyElement {
    let mute = t.mute;
    let content = match (view.zoom, dims) {
        (Zoom::Percent(percent), Some(dims)) => {
            let (w, h) = dims.scaled(percent);
            div()
                .id("image-scroll")
                .size_full()
                .overflow_scroll()
                .child(
                    div()
                        .min_w_full()
                        .min_h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(img(view.path.clone()).w(px(w)).h(px(h))),
                )
                .into_any_element()
        }
        _ => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                img(view.path.clone())
                    .max_w_full()
                    .max_h_full()
                    .with_fallback(move || {
                        div()
                            .text_color(mute)
                            .child("Could not open this image.")
                            .into_any_element()
                    }),
            )
            .into_any_element(),
    };
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(checker(t))
        .child(content)
        .into_any_element()
}

/// `bg` with `sel` squares, so the board reads in every theme.
// ponytail: the board's literal cell colours (`#2a2c30`, `#222428`) are not Theme
// tokens, and `sel`/`hov` are equal in three of the four themes, so the two tones
// are derived from `bg` and `sel`; upgrade: a checker token on the themes page.
fn checker(t: &Theme) -> AnyElement {
    let square = t.sel;
    let base = t.bg;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            window.paint_quad(fill(bounds, base));
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let cols = (width / CELL).ceil() as i32;
            let rows = (height / CELL).ceil() as i32;
            for row in 0..rows {
                for col in 0..cols {
                    if (row + col) % 2 != 0 {
                        continue;
                    }
                    let origin = point(
                        bounds.origin.x + px(col as f32 * CELL),
                        bounds.origin.y + px(row as f32 * CELL),
                    );
                    window.paint_quad(fill(Bounds::new(origin, size(px(CELL), px(CELL))), square));
                }
            }
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}
