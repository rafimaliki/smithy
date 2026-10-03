//! Source control add-on: Changes (staged/unstaged, stage, discard, commit),
//! Compare against a local base, and blame / gutter marks in the editor.
mod changes;
mod compare;
mod dialog;
mod diff_lines;
pub mod diff_view;
mod sidebar;
mod views;

pub use changes::{decorations, ChangesView};
#[allow(unused_imports)] // re-exported for the Pull requests add-on (`a-pr`)
pub use diff_view::{DiffAction, DiffOptions, DiffView, HunkActions};

use super::{
    Addon, AddonContext, AddonInfo, AddonInstance, EditorDecorations, RailItem, StatusInfo,
};
use gpui::{AnyView, App, AppContext as _, Entity};
use std::path::{Path, PathBuf};

pub struct SourceControl;

impl Addon for SourceControl {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "source-control",
            name: "Source control",
            description: "Changes view, diffs, staging, commits, compare and blame.",
            requires: &[],
        }
    }

    fn start(&self, ctx: &AddonContext, cx: &mut App) -> Box<dyn AddonInstance> {
        let root = ctx.root.clone();
        let workspace = ctx.workspace.clone();
        let changes = cx.new(|cx| ChangesView::new(root.clone(), workspace, cx));
        Box::new(Instance { root, changes })
    }
}

struct Instance {
    root: Option<PathBuf>,
    changes: Entity<ChangesView>,
}

impl AddonInstance for Instance {
    fn rail(&self) -> Option<RailItem> {
        Some(RailItem {
            icon: "icons/rail-changes.svg",
            title: "Source control",
        })
    }

    fn rail_badge(&self, cx: &App) -> Option<String> {
        self.changes.read(cx).rail_badge()
    }

    fn sidebar_view(&self) -> Option<AnyView> {
        Some(self.changes.clone().into())
    }

    fn editor_decorations(&self, path: &Path) -> Option<EditorDecorations> {
        decorations(self.root.as_deref()?, path)
    }

    fn status(&self, cx: &App) -> Option<StatusInfo> {
        Some(self.changes.read(cx).status_info())
    }
}
