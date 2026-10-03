//! The image view: which mode and zoom it is in, and where its pixel size comes
//! from. The bar it draws is `bar.rs`, the checkerboard pane under it `pane.rs`.
use super::bar;
use super::format::{self, Dimensions};
use super::pane;
use crate::editor::view::EditorView;
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, Context, FocusHandle, ImgResourceLoader, Render, Resource, WeakEntity, Window,
};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Mode {
    Preview,
    Code,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Zoom {
    /// Shrink to fit the pane; never larger than the image's own pixels.
    Fit,
    Percent(f32),
}

pub struct ImageView {
    /// The file drawn in the pane and named in the bar.
    pub(super) path: PathBuf,
    /// SVG only: the core's editor, shown on the Code side of the toggle.
    pub(super) editor: Option<WeakEntity<EditorView>>,
    pub(super) mode: Mode,
    pub(super) zoom: Zoom,
    focus: FocusHandle,
    /// Size on disk, read once when the view is built.
    pub(super) bytes: Option<u64>,
    /// SVG dimensions, with the buffer revision they were parsed from.
    parsed: Option<(u64, Option<Dimensions>)>,
}

impl ImageView {
    pub(super) fn new(
        path: PathBuf,
        editor: Option<WeakEntity<EditorView>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let bytes = std::fs::metadata(&path).ok().map(|m| m.len());
        Self {
            path,
            editor,
            mode: Mode::Preview,
            zoom: Zoom::Fit,
            focus: cx.focus_handle(),
            bytes,
            parsed: None,
        }
    }

    pub(super) fn is_svg(&self) -> bool {
        self.editor.is_some()
    }

    pub(super) fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = mode;
        match mode {
            Mode::Code => {
                if let Some(editor) = self.editor.as_ref().and_then(|e| e.upgrade()) {
                    editor.read(cx).focus(window);
                }
            }
            Mode::Preview => window.focus(&self.focus),
        }
        cx.notify();
    }

    pub(super) fn set_zoom(&mut self, zoom: Zoom, cx: &mut Context<Self>) {
        self.zoom = zoom;
        cx.notify();
    }

    /// Fit has no percentage of its own, so both zoom buttons leave it at 100%.
    // ponytail: the fit scale is never measured; upgrade: measure the pane and
    // step from the percentage it works out to.
    pub(super) fn zoom_step(&mut self, up: bool, cx: &mut Context<Self>) {
        let next = match self.zoom {
            Zoom::Fit => Zoom::Percent(100.0),
            Zoom::Percent(p) => Zoom::Percent(format::zoom_step(p, up)),
        };
        self.set_zoom(next, cx);
    }

    /// The image's pixel size: parsed from the SVG buffer, or read back from the image
    /// gpui decoded. `None` while a raster image is still loading, or if it cannot be
    /// decoded at all.
    fn dimensions(&mut self, window: &mut Window, cx: &mut gpui::App) -> Option<Dimensions> {
        if let Some(editor) = self.editor.as_ref().and_then(|e| e.upgrade()) {
            let buffer = &editor.read(cx).state.buffer;
            if self.parsed.map(|(revision, _)| revision) != Some(buffer.revision) {
                self.parsed = Some((buffer.revision, format::svg_dimensions(&buffer.text())));
            }
            return self.parsed.and_then(|(_, dims)| dims);
        }
        let source = Resource::from(self.path.clone());
        window
            .use_asset::<ImgResourceLoader>(&source, cx)
            .and_then(Result::ok)
            .map(|image| {
                let size = image.size(0);
                Dimensions {
                    width: size.width.0 as f32,
                    height: size.height.0 as f32,
                }
            })
    }
}

impl Render for ImageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::by_name(&cx.global::<Settings>().theme);
        let dims = self.dimensions(window, cx);
        let editor = self.editor.as_ref().and_then(|e| e.upgrade());
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .child(bar::bar(self, &t, dims, cx));
        if self.mode == Mode::Code {
            root = root.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when_some(editor, |d, editor| d.child(editor)),
            );
        } else {
            root = root.child(pane::pane(self, &t, dims));
        }
        root
    }
}
