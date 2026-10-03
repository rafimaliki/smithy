//! Embedded SVG assets for `svg()` elements: the file icons of the tree.
//!
//! The brand marks come from Simple Icons (CC0 1.0, https://simpleicons.org), the
//! app-defined list on the board's `icons` page; `file.svg` and `folder.svg` are
//! the generic marks drawn for the simple icon mode. gpui paints an SVG as an
//! alpha mask, so a mark takes its color from the element's text color.
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

const ICONS: &[(&str, &[u8])] = &[
    ("icons/file.svg", include_bytes!("../assets/icons/file.svg")),
    (
        "icons/folder.svg",
        include_bytes!("../assets/icons/folder.svg"),
    ),
    (
        "icons/folder-filled.svg",
        include_bytes!("../assets/icons/folder-filled.svg"),
    ),
    ("icons/git.svg", include_bytes!("../assets/icons/git.svg")),
    (
        "icons/github-actions.svg",
        include_bytes!("../assets/icons/github-actions.svg"),
    ),
    ("icons/json.svg", include_bytes!("../assets/icons/json.svg")),
    (
        "icons/kotlin.svg",
        include_bytes!("../assets/icons/kotlin.svg"),
    ),
    (
        "icons/markdown.svg",
        include_bytes!("../assets/icons/markdown.svg"),
    ),
    ("icons/npm.svg", include_bytes!("../assets/icons/npm.svg")),
    ("icons/pin.svg", include_bytes!("../assets/icons/pin.svg")),
    (
        "icons/react.svg",
        include_bytes!("../assets/icons/react.svg"),
    ),
    ("icons/rust.svg", include_bytes!("../assets/icons/rust.svg")),
    ("icons/toml.svg", include_bytes!("../assets/icons/toml.svg")),
    (
        "icons/typescript.svg",
        include_bytes!("../assets/icons/typescript.svg"),
    ),
    ("icons/yaml.svg", include_bytes!("../assets/icons/yaml.svg")),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
