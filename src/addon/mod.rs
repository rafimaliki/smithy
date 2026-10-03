//! Add-on interface and registry.
//!
//! An add-on is a first-party module compiled into the binary. It is described by a
//! zero-sized [`Addon`] value that is always registered but holds no state. Only when
//! the user turns it on does the registry call [`Addon::start`], which builds the
//! running [`AddonInstance`]. Off add-ons are never started, so they cost no memory.
//!
//! To add one: create `src/addon/<name>.rs`, implement both traits, and add it to
//! [`all`]. `image_viewer.rs` is the reference.
mod deps;
pub mod image_viewer;
pub mod markdown;
mod registry;

pub use deps::{resolve_disable, resolve_enable};
pub use registry::Registry;

use crate::editor::view::EditorView;
use gpui::{AnyView, App, Entity, Window};
use std::path::{Path, PathBuf};

/// Static facts about an add-on, shown in Settings > Add-ons.
#[derive(Clone, Copy)]
pub struct AddonInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Ids of add-ons that must be on for this one to work.
    pub requires: &'static [&'static str],
}

/// What the core hands an add-on when it starts.
pub struct AddonContext {
    /// The open folder, if any.
    #[allow(dead_code)] // read by add-ons that work on the folder
    pub root: Option<PathBuf>,
}

/// A button on the left rail that opens an add-on's sidebar view.
pub struct RailItem {
    /// Short label drawn in the rail button (no icon assets yet).
    pub glyph: &'static str,
    pub title: &'static str,
}

pub trait Addon {
    fn info(&self) -> AddonInfo;
    /// Build the running add-on. Called once when it is turned on.
    fn start(&self, ctx: &AddonContext, cx: &mut App) -> Box<dyn AddonInstance>;
}

pub trait AddonInstance {
    /// Rail button and sidebar view, if the add-on has a sidebar.
    fn rail(&self) -> Option<RailItem> {
        None
    }
    /// The sidebar content. Shown while this add-on's rail button is active.
    fn sidebar_view(&self) -> Option<AnyView> {
        None
    }
    /// A viewer that replaces the text editor for `path`, or `None` when this add-on
    /// does not handle that file. The core caches the view per open tab.
    fn viewer_for(&self, _path: &Path, _window: &mut Window, _cx: &mut App) -> Option<AnyView> {
        None
    }
    /// A view that wraps the tab's editor for `path`, or `None` to leave the tab alone.
    /// The core keeps the editor (saving, dirty state and the unsaved prompt are
    /// unchanged) and renders this view instead of it; the view draws its own chrome
    /// around the editor it is handed (Markdown: Code / Side by side / Preview).
    fn document_view(
        &self,
        _path: &Path,
        _editor: Entity<EditorView>,
        _cx: &mut App,
    ) -> Option<AnyView> {
        None
    }
}

/// Every add-on Smithy ships. One line per add-on.
pub fn all() -> Vec<Box<dyn Addon>> {
    vec![
        Box::new(image_viewer::ImageViewer),
        Box::new(markdown::Markdown),
    ]
}
