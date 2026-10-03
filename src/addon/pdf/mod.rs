//! PDF viewer add-on: a `.pdf` tab renders through the Windows built-in PDF component,
//! so nothing is bundled. Pages rasterize on a worker thread, one at a time, and only
//! for the pages the view can see; the rest are empty paper. The board frame is
//! `flows/files/pdf-view`.
mod cache;
mod pages;
mod render;
mod toolbar;
mod zoom;

use super::{Addon, AddonContext, AddonInfo, AddonInstance};
use crate::settings::Settings;
use crate::theme::Theme;
use cache::PageCache;
use gpui::{
    div, prelude::*, px, AnyElement, AnyView, App, Context, ListAlignment, ListState, Pixels,
    Render, SharedString, Size, Window,
};
use render::{Document, Msg, Request};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use zoom::Zoom;

const EXTENSIONS: [&str; 1] = ["pdf"];
/// How many page bitmaps stay in memory. A page at 1000x1300 is 5 MB of pixels.
const CACHE_PAGES: usize = 8;
/// Pixels rendered above and below the viewport, so a short scroll is not a blank page.
const OVERDRAW: f32 = 240.0;
/// Pages that may wait for the renderer at once. A page takes up to a second to draw, so
/// a longer queue would rasterize pages the reader has already scrolled past.
const QUEUE_LIMIT: usize = 4;

pub struct PdfViewer;

impl Addon for PdfViewer {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "pdf-viewer",
            name: "PDF viewer",
            description: "Open PDF files with page scrolling, zoom and fit width.",
            requires: &[],
        }
    }

    fn start(&self, _ctx: &AddonContext, _cx: &mut App) -> Box<dyn AddonInstance> {
        Box::new(Instance)
    }
}

struct Instance;

impl AddonInstance for Instance {
    fn viewer_for(&self, path: &Path, _window: &mut Window, cx: &mut App) -> Option<AnyView> {
        if !handles(path) {
            return None;
        }
        let view = cx.new(|cx| PdfView::new(path.to_path_buf(), cx));
        Some(view.into())
    }
}

fn handles(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// What the view knows about the file so far.
enum Load {
    Loading,
    Ready,
    Failed(SharedString),
}

/// State the list's item builder needs. It runs during layout, where the view itself
/// cannot be borrowed, so it reaches its state through this.
struct Shared {
    document: Document,
    zoom: Cell<Zoom>,
    /// The scale pages draw at, refreshed every frame from the zoom mode and viewport.
    scale: Cell<f32>,
    cache: RefCell<PageCache>,
    /// Pages asked for but not answered yet, so a page is asked for once.
    pending: RefCell<HashSet<usize>>,
    /// Pages the renderer could not draw, so the paper can say so.
    failed: RefCell<HashSet<usize>>,
    /// False until the viewport is measured, so the first page is not rasterized twice:
    /// once at the natural size and again at the width fit width works out.
    settled: Cell<bool>,
    requests: async_channel::Sender<Request>,
}

impl Shared {
    fn request(&self, page: usize, width: u32) {
        // Do not queue up the whole document: scrolling past fifty pages would otherwise
        // rasterize every one of them. A page still on screen asks again next frame, and
        // every finished page redraws the view.
        if self.requests.len() >= QUEUE_LIMIT {
            return;
        }
        if !self.pending.borrow_mut().insert(page) {
            return;
        }
        if self.requests.try_send(Request { page, width }).is_err() {
            // The render thread is gone; the view is told separately.
            self.pending.borrow_mut().remove(&page);
        }
    }
}

struct PdfView {
    path: PathBuf,
    load: Load,
    shared: Option<Rc<Shared>>,
    list: ListState,
    zoom: Zoom,
    /// The scale the last frame laid the pages out at.
    scale: f32,
    /// The pages area's size as last measured, which fit width divides.
    viewport: Size<Pixels>,
    requests: async_channel::Sender<Request>,
}

impl PdfView {
    fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let (msgs, requests) = render::start(&path);
        cx.spawn(async move |this, cx| {
            while let Ok(msg) = msgs.recv().await {
                if this.update(cx, |this, cx| this.received(msg, cx)).is_err() {
                    return;
                }
            }
            // The channel closes when the render thread ends, the only way a page can
            // never arrive.
            let _ = this.update(cx, |this, cx| {
                if !matches!(this.load, Load::Failed(_)) {
                    this.load = Load::Failed("The PDF renderer stopped.".into());
                    cx.notify();
                }
            });
        })
        .detach();
        Self {
            path,
            load: Load::Loading,
            shared: None,
            list: ListState::new(0, ListAlignment::Top, px(OVERDRAW)),
            zoom: Zoom::Fit,
            scale: 1.0,
            viewport: Size {
                width: px(0.),
                height: px(0.),
            },
            requests,
        }
    }

    fn received(&mut self, msg: Msg, cx: &mut Context<Self>) {
        match msg {
            Msg::Loaded(Ok(document)) => {
                let count = document.count();
                self.list = ListState::new(count, ListAlignment::Top, px(OVERDRAW));
                self.shared = Some(Rc::new(Shared {
                    document,
                    zoom: Cell::new(self.zoom),
                    scale: Cell::new(self.scale),
                    cache: RefCell::new(PageCache::new(CACHE_PAGES)),
                    pending: RefCell::new(HashSet::new()),
                    failed: RefCell::new(HashSet::new()),
                    settled: Cell::new(false),
                    requests: self.requests.clone(),
                }));
                self.load = Load::Ready;
            }
            Msg::Loaded(Err(reason)) => self.load = Load::Failed(reason.into()),
            Msg::Page { page, width, image } => {
                if let Some(shared) = &self.shared {
                    shared.pending.borrow_mut().remove(&page);
                    match image {
                        Ok(image) => shared.cache.borrow_mut().insert(page, width, image),
                        Err(_) => {
                            shared.failed.borrow_mut().insert(page);
                        }
                    }
                }
            }
        }
        cx.notify();
    }

    /// The page at the top of the viewport, for the toolbar's `Page n of m`.
    fn current_page(&self, count: usize) -> usize {
        (self.list.logical_scroll_top().item_ix + 1)
            .min(count)
            .max(1)
    }

    fn set_viewport(&mut self, size: Size<Pixels>) -> bool {
        // A sub-pixel difference is not worth another frame.
        let changed = (f32::from(size.width) - f32::from(self.viewport.width)).abs() > 0.5;
        self.viewport = size;
        changed
    }
}

impl Render for PdfView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::by_name(&cx.global::<Settings>().theme);
        let body = match &self.load {
            Load::Loading => notice(&t, "Reading the PDF…"),
            Load::Failed(reason) => notice(&t, reason.as_ref()),
            Load::Ready => self.pages(&t, cx),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.bg)
            .text_color(t.ink)
            .child(self.bar(&t, cx))
            .child(body)
    }
}

/// A one-line state in the middle of the tab, like the other viewers' empty states.
fn notice(t: &Theme, text: &str) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .p_6()
        .text_color(t.mute)
        .child(SharedString::from(text.to_string()))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_pdf_case_insensitively_and_nothing_else() {
        assert!(handles(Path::new("a/Spec.PDF")));
        assert!(!handles(Path::new("x.pdfx")));
        assert!(!handles(Path::new("noext")));
    }
}
