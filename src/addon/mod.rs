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
pub mod lsp;
pub mod markdown;
pub mod pdf;
pub mod problems;
pub mod pull_requests;
mod registry;
pub mod search;
pub mod source_control;
pub mod split_panes;
pub mod terminal;

pub use deps::{resolve_disable, resolve_enable};
pub use registry::Registry;

use crate::editor::view::EditorView;
use crate::workspace::Workspace;
use gpui::{AnyView, App, Entity, WeakEntity, Window};
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
    /// The window's workspace. Lets an add-on open its view as a tab and ask for a
    /// repaint after it changes something the chrome shows.
    pub workspace: Option<WeakEntity<Workspace>>,
}

/// A button on the left rail that opens an add-on's sidebar view.
pub struct RailItem {
    /// Asset path of the rail icon, e.g. `icons/rail-search.svg` (see `assets.rs`).
    pub icon: &'static str,
    pub title: &'static str,
}

/// A mark the editor draws in the gutter beside a line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineMark {
    Added,
    Modified,
}

/// Blame for one line, as the editor shows it.
#[derive(Clone)]
pub struct BlameLine {
    pub author: String,
    pub age: String,
    pub subject: String,
}

/// Per-line decorations for an open file, indexed from line 0.
#[derive(Clone, Default)]
pub struct EditorDecorations {
    pub marks: Vec<Option<LineMark>>,
    pub blame: Vec<Option<BlameLine>>,
}

/// Extra text for the status bar's left side.
#[derive(Clone)]
pub struct StatusInfo {
    /// Branch name, or an explanation like "No repository".
    pub branch: String,
    /// A short detail, e.g. "3 changes"; may be empty.
    pub detail: String,
}

/// A navigation gesture in the editor that an add-on may answer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Navigate {
    /// Ctrl+Click, or F12.
    Definition,
    /// Shift+F12, or Ctrl+Click on a definition.
    References,
    /// Alt+Left.
    Back,
    /// Alt+Right.
    Forward,
}

/// How far a language server is, as Settings > Languages shows it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ServerState {
    /// On and found on PATH.
    On,
    /// Off in Settings > Languages.
    Off,
    /// On, but the command is not on PATH.
    NotInstalled,
}

/// One row an add-on contributes to Settings > Languages.
#[derive(Clone)]
pub struct LanguageServerRow {
    /// Id `toggle_language_server` takes.
    pub id: &'static str,
    /// Language name, matching `editor::lang::Lang::name`.
    pub language: &'static str,
    /// Command shown in the Language server column.
    pub server: &'static str,
    pub state: ServerState,
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
    /// A count drawn on the rail button, e.g. the number of changes.
    fn rail_badge(&self, _cx: &App) -> Option<String> {
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
    /// Gutter marks and blame for a file the editor has open. The core calls it when
    /// the file opens and after a save.
    fn editor_decorations(&self, _path: &Path) -> Option<EditorDecorations> {
        None
    }
    /// Extra text for the status bar's left side.
    fn status(&self, _cx: &App) -> Option<StatusInfo> {
        None
    }
    /// A full-window overlay (e.g. Search everywhere), drawn over the editor
    /// while the add-on says it is open. Only one is shown.
    fn overlay(&self, _cx: &App) -> Option<AnyView> {
        None
    }
    /// Ctrl+Shift+O reached the workspace: open or close the add-on's overlay.
    fn toggle_search_everywhere(&self, _window: &mut Window, _cx: &mut App) {}
    /// The add-on's view for the bottom panel, or `None` while it has none open.
    /// The core draws it under the editor, above the status bar.
    fn bottom_panel(&self, _cx: &App) -> Option<AnyView> {
        None
    }
    /// Ctrl+backtick reached the workspace: show or hide the bottom panel.
    fn toggle_bottom_panel(&self, _window: &mut Window, _cx: &mut App) {}
    /// The explorer asked to open a terminal in `dir` (folder menu).
    fn open_terminal_at(&self, _dir: &Path, _window: &mut Window, _cx: &mut App) {}
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
    /// True while this add-on wants the core's second editor group to be available
    /// (Split panes). Off by default, and the only thing that decides whether the
    /// tab menu offers "Split right": no running add-on says yes, no split.
    fn enables_split(&self) -> bool {
        false
    }
    /// The editor saw a navigation gesture at `path`, `line`, `character` (0-based;
    /// `character` in UTF-16 units). Return true when the add-on took it. Back and
    /// forward carry no position, so ignore the last three there.
    fn navigate(
        &self,
        _what: Navigate,
        _path: &Path,
        _line: u32,
        _character: u32,
        _window: &mut Window,
        _cx: &mut App,
    ) -> bool {
        false
    }
    /// Per-language server rows for Settings > Languages. Empty while the add-on is off.
    fn language_servers(&self) -> Vec<LanguageServerRow> {
        Vec::new()
    }
    /// Settings > Languages: turn the server for `id` on or off.
    fn toggle_language_server(&self, _id: &str, _cx: &mut App) {}
}

/// Every add-on Smithy ships. One line per add-on.
pub fn all() -> Vec<Box<dyn Addon>> {
    vec![
        Box::new(image_viewer::ImageViewer),
        Box::new(lsp::LanguageServers),
        Box::new(markdown::Markdown),
        Box::new(pdf::PdfViewer),
        Box::new(problems::Problems),
        Box::new(pull_requests::PullRequests),
        Box::new(search::Search),
        Box::new(source_control::SourceControl),
        Box::new(split_panes::SplitPanes),
        Box::new(terminal::Terminal),
    ]
}
