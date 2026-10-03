//! Image viewer add-on: opens raster images in place of the text editor.
//! This is the reference add-on: copy its shape (a descriptor, an instance, a view).
//! SVG is left to a later change; PNG, JPG, GIF, WebP and BMP are decoded by gpui.
use super::{Addon, AddonContext, AddonInfo, AddonInstance};
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{div, img, prelude::*, AnyView, App, Context, Entity, Window};
use std::path::{Path, PathBuf};

const EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];

pub struct ImageViewer;

impl Addon for ImageViewer {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "image-viewer",
            name: "Image viewer",
            description: "Open PNG, JPG, GIF, WebP and BMP files as images.",
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
        let view: Entity<ImageView> = cx.new(|_| ImageView {
            path: path.to_path_buf(),
        });
        Some(view.into())
    }
}

fn handles(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

struct ImageView {
    path: PathBuf,
}

impl Render for ImageView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.bg)
            .p_6()
            .child(img(self.path.clone()).max_w_full().max_h_full())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_images_case_insensitively_and_nothing_else() {
        assert!(handles(Path::new("a/B.PNG")));
        assert!(handles(Path::new("x.webp")));
        assert!(!handles(Path::new("x.svg")));
        assert!(!handles(Path::new("noext")));
    }
}
