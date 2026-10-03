//! The page area of the PDF viewer: one list item per page, and the bitmap request that
//! makes rendering lazy — the item builder runs only for the pages the list can see.
use super::zoom::Zoom;
use super::{PdfView, Shared};
use crate::theme::Theme;
use gpui::{
    canvas, div, img, list, prelude::*, px, rgb, AnyElement, Context, ListOffset, RenderImage,
    SharedString,
};
use std::rc::Rc;
use std::sync::Arc;

/// Space between pages.
const GAP: f32 = 18.0;
/// The paper a page is drawn on. Not a theme token: it is the document's own background.
const PAPER: u32 = 0xffffff;

impl PdfView {
    /// The page area: one item per page, laid out at the current scale. Fit width needs
    /// the measured viewport, so a canvas reports it and asks for another frame when it
    /// differs from the one the pages were laid out at.
    pub(super) fn pages(&mut self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let Some(shared) = self.shared.clone() else {
            return div().into_any_element();
        };
        let count = shared.document.count();
        let scale = self
            .zoom
            .scale(f32::from(self.viewport.width), shared.document.widest());
        if (scale - self.scale).abs() > 0.001 {
            // Page heights follow the scale, so the list has to measure them again.
            let top = self.list.logical_scroll_top().item_ix;
            self.scale = scale;
            self.list.reset(count);
            self.list.scroll_to(ListOffset {
                item_ix: top,
                offset_in_item: px(0.),
            });
        }
        shared.zoom.set(self.zoom);
        shared.scale.set(scale);
        // A fixed zoom needs no measurement; fit width waits for one, so the first page
        // is not rasterized at the natural size and then again at the fitted width.
        shared
            .settled
            .set(!matches!(self.zoom, Zoom::Fit) || f32::from(self.viewport.width) > 0.0);
        let theme = *t;
        let entity = cx.entity();
        let measured = canvas(
            move |bounds, window, cx| {
                if entity.update(cx, |this, _| this.set_viewport(bounds.size)) {
                    window.request_animation_frame();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let items = list(self.list.clone(), move |page, window, _| {
            page_item(&shared, page, window.scale_factor(), &theme)
        });
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .pt(px(24.))
            .bg(t.sel)
            .child(measured)
            .child(items.size_full())
            .into_any_element()
    }
}

/// One page: paper the size the page draws at, with its bitmap on top once the renderer
/// has produced it.
fn page_item(shared: &Rc<Shared>, page: usize, dpr: f32, t: &Theme) -> AnyElement {
    let (width, height) = shared.document.size(page);
    let scale = shared.scale.get();
    let width = width * scale;
    let height = height * scale;
    let wanted = ((width * dpr).round() as u32).max(1);
    let bitmap: Option<Arc<RenderImage>> = shared.cache.borrow_mut().get(page, wanted);
    // A page the renderer could not draw is not asked for again; it stays blank paper
    // with the reason on it.
    let failed = bitmap.is_none() && shared.failed.borrow().contains(&page);
    if bitmap.is_none() && !failed && shared.settled.get() {
        shared.request(page, wanted);
    }
    let paper = div()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .bg(rgb(PAPER))
        .shadow_sm()
        .when_some(bitmap, |d, bitmap| {
            d.child(img(bitmap).w(px(width)).h(px(height)))
        })
        .when(failed, |d| {
            d.flex()
                .items_center()
                .justify_center()
                .p_4()
                .text_color(t.mute)
                .child(SharedString::from(format!(
                    "Page {} could not be rendered.",
                    page + 1
                )))
        });
    div()
        .w_full()
        .flex()
        .justify_center()
        .pb(px(GAP))
        .child(paper)
        .into_any_element()
}
