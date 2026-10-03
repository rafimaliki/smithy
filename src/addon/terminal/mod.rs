//! Terminal add-on: a bottom panel with PowerShell, cmd or Git Bash sessions.
//! Design: framery frame `flows/files/terminal`; the panel hook is
//! [`AddonInstance::bottom_panel`](super::AddonInstance::bottom_panel).
//!
//! Nothing runs until the panel is first opened, and a session's process lives only
//! as long as its tab: closing the tab kills it (`pty::Session`'s `Drop`).
mod ansi;
mod grid;
mod keys;
mod pty;
mod view;

use super::{Addon, AddonContext, AddonInfo, AddonInstance};
use gpui::{AnyView, App, AppContext as _, Entity, Window};
use std::path::Path;
use view::TerminalView;

pub struct Terminal;

impl Addon for Terminal {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "terminal",
            name: "Terminal",
            description: "Bottom panel with PowerShell, cmd or Git Bash sessions.",
            requires: &[],
        }
    }

    fn start(&self, ctx: &AddonContext, cx: &mut App) -> Box<dyn AddonInstance> {
        let view = cx.new(|cx| TerminalView::new(ctx.root.clone(), ctx.workspace.clone(), cx));
        Box::new(Instance { view })
    }
}

/// The running add-on: just the panel view, which owns its sessions.
struct Instance {
    view: Entity<TerminalView>,
}

impl AddonInstance for Instance {
    fn bottom_panel(&self, cx: &App) -> Option<AnyView> {
        self.view
            .read(cx)
            .is_visible()
            .then(|| self.view.clone().into())
    }

    fn toggle_bottom_panel(&self, window: &mut Window, cx: &mut App) {
        self.view.update(cx, |view, cx| view.toggle(window, cx));
    }

    fn open_terminal_at(&self, dir: &Path, window: &mut Window, cx: &mut App) {
        self.view
            .update(cx, |view, cx| view.open_here(dir, window, cx));
    }
}
