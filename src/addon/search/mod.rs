//! Search add-on: the sidebar's Text, Files and Symbols tabs, and the
//! Search everywhere modal (Ctrl+Shift+O). The search itself runs on a
//! background task; see `engine.rs`. Design: board frames `search-results`,
//! `search-files`, `search-symbols`, `search-everywhere`.
mod chrome;
mod engine;
mod everywhere;
mod fuzzy;
mod matcher;
mod palette;
mod segments;
mod sidebar;

use super::{Addon, AddonContext, AddonInfo, AddonInstance, RailItem};
use engine::Kind;
use everywhere::EverywhereView;
use gpui::{AnyView, App, AppContext as _, Entity, FocusHandle, Window};
use palette::PaletteView;
use sidebar::SearchView;

pub struct Search;

impl Addon for Search {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "search",
            name: "Search",
            description: "Search the folder by text, file name or symbol, and Search everywhere.",
            requires: &[],
        }
    }

    fn start(&self, ctx: &AddonContext, cx: &mut App) -> Box<dyn AddonInstance> {
        let root = ctx.root.clone();
        let workspace = ctx.workspace.clone();
        let sidebar = cx.new(|cx| SearchView::new(root.clone(), workspace.clone(), cx));
        let everywhere = cx.new(|cx| EverywhereView::new(root, workspace, cx));
        let palette = cx.new(PaletteView::new);
        Box::new(Instance {
            sidebar,
            everywhere,
            palette,
        })
    }
}

struct Instance {
    sidebar: Entity<SearchView>,
    everywhere: Entity<EverywhereView>,
    palette: Entity<PaletteView>,
}

impl AddonInstance for Instance {
    fn rail(&self) -> Option<RailItem> {
        Some(RailItem {
            icon: "icons/rail-search.svg",
            title: "Search",
        })
    }

    fn sidebar_view(&self) -> Option<AnyView> {
        Some(self.sidebar.clone().into())
    }

    fn overlay(&self, cx: &App) -> Option<AnyView> {
        if self.palette.read(cx).is_open() {
            return Some(self.palette.clone().into());
        }
        self.everywhere
            .read(cx)
            .is_open()
            .then(|| self.everywhere.clone().into())
    }

    fn toggle_search_everywhere(&self, window: &mut Window, cx: &mut App) {
        self.palette.update(cx, |view, cx| view.close(cx));
        if self.everywhere.read(cx).is_open() {
            self.everywhere.update(cx, |view, cx| view.close(cx));
        } else {
            let previous = window.focused(cx);
            self.everywhere
                .update(cx, |view, cx| view.show(previous, cx));
            let focus: FocusHandle = self.everywhere.read(cx).focus().clone();
            window.focus(&focus);
        }
    }

    fn quick_open(&self, window: &mut Window, cx: &mut App) {
        self.palette.update(cx, |view, cx| view.close(cx));
        let previous = window.focused(cx);
        self.everywhere
            .update(cx, |view, cx| view.show_tab(Kind::Files, previous, cx));
        let focus: FocusHandle = self.everywhere.read(cx).focus().clone();
        window.focus(&focus);
    }

    fn toggle_command_palette(&self, window: &mut Window, cx: &mut App) {
        if self.palette.read(cx).is_open() {
            self.palette.update(cx, |view, cx| view.close(cx));
            return;
        }
        self.everywhere.update(cx, |view, cx| view.close(cx));
        let previous = window.focused(cx);
        self.palette.update(cx, |view, cx| view.show(previous, cx));
        let focus: FocusHandle = self.palette.read(cx).focus().clone();
        window.focus(&focus);
    }
}
