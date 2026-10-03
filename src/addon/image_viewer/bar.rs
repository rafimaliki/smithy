//! The viewer bar, as the board frames `flows/files/image-view` and
//! `flows/files/svg-view` draw it: the file's name, its dimensions and size, then
//! — for an SVG — the Preview / Code toggle and the zoom controls.
use super::format::{self, Dimensions};
use super::view::{ImageView, Mode, Zoom};
use crate::theme::Theme;
use gpui::{div, prelude::*, px, Context, FontWeight, SharedString};

/// The controls on the right of the bar.
#[derive(Clone, Copy)]
enum ZoomAction {
    Out,
    In,
    /// `100%`, drawn only for raster images (the SVG frame has no such button).
    Actual,
}

pub(super) fn bar(
    view: &ImageView,
    t: &Theme,
    dims: Option<Dimensions>,
    cx: &mut Context<ImageView>,
) -> impl IntoElement {
    let name = view
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut info = String::new();
    if let Some(dims) = dims {
        info.push_str(&dims.bar_text());
    }
    if let Some(bytes) = view.bytes {
        if !info.is_empty() {
            info.push_str(" · ");
        }
        info.push_str(&format::format_size(bytes));
    }
    let previewing = view.mode == Mode::Preview;
    div()
        .h(px(40.))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(14.))
        .border_b_1()
        .border_color(t.line)
        .text_color(t.mute)
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.ink)
                .child(SharedString::from(name)),
        )
        .when(!info.is_empty(), |d| d.child(SharedString::from(info)))
        .child(div().flex_1())
        .when(view.is_svg(), |d| d.child(mode_switch(view, t, cx)))
        .when(previewing && dims.is_some(), |d| {
            d.child(zoom_button(ZoomAction::Out, t, cx))
                .child(
                    div()
                        .flex_none()
                        .min_w(px(36.))
                        .text_center()
                        .child(SharedString::from(match view.zoom {
                            Zoom::Fit => "Fit".to_string(),
                            Zoom::Percent(p) => format!("{}%", p as i64),
                        })),
                )
                .child(zoom_button(ZoomAction::In, t, cx))
                .when(!view.is_svg(), |d| {
                    d.child(zoom_button(ZoomAction::Actual, t, cx))
                })
        })
}

fn mode_switch(view: &ImageView, t: &Theme, cx: &mut Context<ImageView>) -> impl IntoElement {
    div()
        .flex_none()
        .flex()
        .rounded(px(6.))
        .border_1()
        .border_color(t.line)
        .overflow_hidden()
        .child(mode_button(view, "Preview", Mode::Preview, t, cx))
        .child(mode_button(view, "Code", Mode::Code, t, cx))
}

fn mode_button(
    view: &ImageView,
    label: &'static str,
    mode: Mode,
    t: &Theme,
    cx: &mut Context<ImageView>,
) -> impl IntoElement {
    let on = view.mode == mode;
    div()
        .id(("image-mode", matches!(mode, Mode::Code) as usize))
        .px(px(12.))
        .py(px(4.))
        .cursor_pointer()
        .text_color(if on { t.ink } else { t.mute })
        .when(on, |d| d.bg(t.sel))
        .when(!on, |d| d.hover(|d| d.bg(t.hov).text_color(t.ink)))
        .on_click(cx.listener(move |view, _, window, cx| view.set_mode(mode, window, cx)))
        .child(SharedString::from(label))
}

fn zoom_button(action: ZoomAction, t: &Theme, cx: &mut Context<ImageView>) -> impl IntoElement {
    let icon = !matches!(action, ZoomAction::Actual);
    let (id, label) = match action {
        ZoomAction::Out => ("image-zoom-out", "−"),
        ZoomAction::In => ("image-zoom-in", "+"),
        ZoomAction::Actual => ("image-zoom-actual", "100%"),
    };
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .h(px(if icon { 24. } else { 28. }))
        .when(icon, |d| d.w(px(24.)))
        .when(!icon, |d| {
            d.px(px(12.))
                .rounded(px(6.))
                .border_1()
                .border_color(t.line)
        })
        .cursor_pointer()
        .text_color(t.mute)
        .hover(|d| d.bg(t.hov).text_color(t.ink))
        .on_click(cx.listener(move |view, _, _, cx| match action {
            ZoomAction::Out => view.zoom_step(false, cx),
            ZoomAction::In => view.zoom_step(true, cx),
            ZoomAction::Actual => view.set_zoom(Zoom::Percent(100.0), cx),
        }))
        .child(SharedString::from(label))
}
