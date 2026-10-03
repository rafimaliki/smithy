//! The PDF viewer's toolbar: the file, the page on screen, and the zoom controls.
use super::zoom::{self, STEP};
use super::PdfView;
use crate::theme::Theme;
use gpui::{div, prelude::*, px, Context, FontWeight, SharedString, Stateful};

impl PdfView {
    pub(super) fn bar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let count = self.shared.as_ref().map_or(0, |s| s.document.count());
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
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.ink)
                    .child(SharedString::from(name)),
            )
            .when(count > 0, |d| {
                d.child(SharedString::from(format!(
                    "Page {} of {count}",
                    self.current_page(count)
                )))
            })
            .child(div().flex_1())
            .child(
                icon_button(0, "−", t)
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_by(1.0 / STEP, cx))),
            )
            .child(
                div()
                    .flex_none()
                    .min_w(px(44.))
                    .text_center()
                    .child(SharedString::from(self.zoom.label(self.scale))),
            )
            .child(
                icon_button(1, "+", t)
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_by(STEP, cx))),
            )
            .child(
                div()
                    .id("pdf-fit-width")
                    .flex_none()
                    .h(px(24.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(t.line)
                    .cursor_pointer()
                    .hover(|d| d.bg(t.hov).text_color(t.ink))
                    .on_click(cx.listener(|this, _, _, cx| this.fit_width(cx)))
                    .child("Fit width"),
            )
    }

    /// One zoom step in (`factor` above 1) or out, from the scale on screen.
    pub(super) fn zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        self.zoom = self.zoom.stepped(self.scale, factor);
        cx.notify();
    }

    pub(super) fn fit_width(&mut self, cx: &mut Context<Self>) {
        self.zoom = zoom::Zoom::Fit;
        cx.notify();
    }
}

/// A `+` / `−` button, the frame's `btn ico`.
fn icon_button(id: usize, glyph: &'static str, t: &Theme) -> Stateful<gpui::Div> {
    div()
        .id(("pdf-icon", id))
        .flex_none()
        .w(px(24.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .text_color(t.mute)
        .hover(|d| d.bg(t.hov).text_color(t.ink))
        .child(glyph)
}
