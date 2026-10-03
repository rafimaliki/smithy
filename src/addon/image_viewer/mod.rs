//! Image and SVG viewer add-on. Raster images open in place of the text editor;
//! an SVG tab keeps the core editor and adds a Preview / Code toggle over it, the
//! shape the Markdown preview add-on uses. The board frames are
//! `flows/files/image-view` and `flows/files/svg-view`.
mod bar;
mod format;
mod pane;
mod view;

use super::{Addon, AddonContext, AddonInfo, AddonInstance};
use crate::editor::view::EditorView;
use gpui::{AnyView, App, AppContext as _, Entity, Window};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use view::ImageView;

const RASTER: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];
const SVG: &[&str] = &["svg"];

pub struct ImageViewer;

impl Addon for ImageViewer {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "image-viewer",
            name: "Image viewer",
            description: "Open PNG, JPG, GIF, WebP, BMP and SVG files as images.",
            requires: &[],
        }
    }

    fn start(&self, _ctx: &AddonContext, _cx: &mut App) -> Box<dyn AddonInstance> {
        Box::new(Instance::default())
    }
}

/// One view per path, so the zoom and the Preview / Code choice survive a re-render.
/// The core calls [`AddonInstance::document_view`] on every frame of an SVG tab.
#[derive(Default)]
struct Instance {
    views: RefCell<HashMap<PathBuf, Entity<ImageView>>>,
}

impl Instance {
    fn view(&self, path: &Path, editor: Option<Entity<EditorView>>, cx: &mut App) -> AnyView {
        let mut views = self.views.borrow_mut();
        let view = match views.get(path) {
            Some(view) => {
                if let Some(editor) = &editor {
                    view.update(cx, |view, _| view.editor = Some(editor.downgrade()));
                }
                view.clone()
            }
            None => {
                let view = cx.new(|cx| {
                    ImageView::new(path.to_path_buf(), editor.map(|e| e.downgrade()), cx)
                });
                views.insert(path.to_path_buf(), view.clone());
                view
            }
        };
        view.into()
    }
}

impl AddonInstance for Instance {
    fn viewer_for(&self, path: &Path, _window: &mut Window, cx: &mut App) -> Option<AnyView> {
        handles(path, RASTER).then(|| self.view(path, None, cx))
    }

    fn document_view(
        &self,
        path: &Path,
        editor: Entity<EditorView>,
        cx: &mut App,
    ) -> Option<AnyView> {
        handles(path, SVG).then(|| self.view(path, Some(editor), cx))
    }
}

fn handles(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_and_svg_go_to_their_own_side_of_the_add_on() {
        assert!(handles(Path::new("a/LOGO.PNG"), RASTER));
        assert!(handles(Path::new("x.jpeg"), RASTER));
        assert!(handles(Path::new("icon.svg"), SVG));
        assert!(!handles(Path::new("icon.SVG"), RASTER));
        assert!(!handles(Path::new("notes.md"), SVG));
        assert!(!handles(Path::new("noext"), RASTER));
    }
}
