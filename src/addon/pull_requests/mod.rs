//! Pull requests add-on: the GitHub pull request list and detail, and a local
//! read-only review diff of the fetched PR ref. Needs Source control, whose
//! diff view it reuses. The board frames are `pr-list`, `pr-diff`,
//! `prs-no-token` and `prs-error`.
mod detail;
mod panels;
mod review;
mod sidebar;
mod state;

pub use state::PrView;

use super::{Addon, AddonContext, AddonInfo, AddonInstance, RailItem, StatusInfo};
use crate::git::Repo;
use crate::github::RepoSlug;
use gpui::{AnyView, App, AppContext as _, Entity};

pub struct PullRequests;

impl Addon for PullRequests {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "pull-requests",
            name: "Pull requests",
            description: "List GitHub pull requests and review their changes locally.",
            requires: &["source-control"],
        }
    }

    fn start(&self, ctx: &AddonContext, cx: &mut App) -> Box<dyn AddonInstance> {
        let root = ctx.root.clone();
        let workspace = ctx.workspace.clone();
        let (has_repo, slug) = root
            .as_deref()
            .and_then(Repo::discover)
            .map(|repo| {
                let slug = repo
                    .remote_url("origin")
                    .and_then(|url| RepoSlug::parse(&url));
                (true, slug)
            })
            .unwrap_or((false, None));
        let view = cx.new(|cx| PrView::new(root, slug, has_repo, workspace, cx));
        Box::new(Instance { view })
    }
}

struct Instance {
    view: Entity<PrView>,
}

impl AddonInstance for Instance {
    fn rail(&self) -> Option<RailItem> {
        Some(RailItem {
            glyph: "P",
            title: "Pull requests",
        })
    }

    fn sidebar_view(&self) -> Option<AnyView> {
        Some(self.view.clone().into())
    }

    fn status(&self, cx: &App) -> Option<StatusInfo> {
        self.view.read(cx).status_info()
    }
}
