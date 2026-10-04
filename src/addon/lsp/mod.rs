//! Language servers add-on: go to definition and find references with a server
//! the user already has on PATH. Servers are off per language, started the first
//! time that language asks for one, and stopped when idle or when the add-on is
//! turned off. Design: board frames `lsp-definition`, `lsp-references` and
//! `settings-languages`.
mod client;
mod manager;
mod nav;
mod peek;
mod peek_rows;
mod peek_view;
mod query;
mod refs;
mod rpc;
mod servers;
mod uri;

use super::{Addon, AddonContext, AddonInfo, AddonInstance, LanguageServerRow, Navigate};
use crate::workspace::Workspace;
use gpui::{AnyView, App, AppContext as _, Entity, WeakEntity, Window};
use manager::Manager;
use peek::LspView;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct LanguageServers;

impl Addon for LanguageServers {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "lsp",
            name: "Language servers",
            description:
                "Go to definition and find references, using a language server already installed.",
            requires: &[],
        }
    }

    fn start(&self, ctx: &AddonContext, cx: &mut App) -> Box<dyn AddonInstance> {
        let manager = Arc::new(Mutex::new(Manager::new(ctx.root.clone())));
        let view = cx.new(|cx| LspView::new(manager.clone(), ctx.workspace.clone(), cx));
        Box::new(Instance {
            manager,
            view,
            workspace: ctx.workspace.clone(),
        })
    }
}

struct Instance {
    manager: Arc<Mutex<Manager>>,
    view: Entity<LspView>,
    workspace: Option<WeakEntity<Workspace>>,
}

impl AddonInstance for Instance {
    /// The peek, or a tip, is drawn over the whole window.
    fn overlay(&self, cx: &App) -> Option<AnyView> {
        self.view
            .read(cx)
            .overlay_open()
            .then(|| self.view.clone().into())
    }

    /// Ctrl+Click, F12, Shift+F12, Alt+Left and Alt+Right.
    fn navigate(
        &self,
        what: Navigate,
        path: &Path,
        line: u32,
        character: u32,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.view.update(cx, |view, cx| {
            view.navigate(what, path, line, character, window, cx)
        })
    }

    /// Settings > Languages: the server each language has.
    fn language_servers(&self) -> Vec<LanguageServerRow> {
        self.manager.lock().unwrap().rows()
    }

    /// Settings > Languages: turn one language's server on or off.
    fn toggle_language_server(&self, id: &str, cx: &mut App) {
        self.manager.lock().unwrap().toggle(id);
        if let Some(workspace) = self.workspace.clone() {
            crate::addon::notify_workspace(&workspace, cx);
        }
    }
}

impl Drop for Instance {
    /// Turning the add-on off must not leave a server process behind.
    fn drop(&mut self) {
        self.manager.lock().unwrap().shutdown_all();
    }
}
