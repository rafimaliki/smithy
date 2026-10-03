//! The second editor group: how it is created, focused and collapsed, plus the
//! lookups the rest of the workspace uses.
//!
//! `self.tabs` is always the left group and `self.right` exists only while the
//! Split panes add-on is on and the user has split. With no split, every method
//! here reduces to the single-group case and allocates nothing. Design: board
//! frame `split-panes`; two groups maximum in v1.
use super::tab_set::TabSet;
use super::{TabContent, Workspace};
use gpui::{Context, Window};
use std::path::{Path, PathBuf};

impl Workspace {
    /// Whether a running add-on allows a second group at all.
    pub(crate) fn split_enabled(&self) -> bool {
        self.registry.running().any(|(_, i)| i.enables_split())
    }

    pub(crate) fn is_split(&self) -> bool {
        self.right.is_some()
    }

    /// The group the keyboard, the status bar and a file open act on.
    pub(crate) fn focused(&self) -> &TabSet<TabContent> {
        match &self.right {
            Some(r) if self.focus_right => r,
            _ => &self.tabs,
        }
    }

    pub(crate) fn focused_mut(&mut self) -> &mut TabSet<TabContent> {
        if self.focus_right {
            if let Some(r) = self.right.as_mut() {
                return r;
            }
        }
        &mut self.tabs
    }

    /// The left or right group by side.
    pub(crate) fn group(&self, right: bool) -> &TabSet<TabContent> {
        if right {
            self.right.as_ref().unwrap_or(&self.tabs)
        } else {
            &self.tabs
        }
    }

    pub(crate) fn group_mut(&mut self, right: bool) -> &mut TabSet<TabContent> {
        if right {
            if let Some(r) = self.right.as_mut() {
                return r;
            }
        }
        &mut self.tabs
    }

    /// Move the focus to a group; asking for a right group that is not there
    /// leaves the focus on the left one.
    pub(crate) fn focus_group(&mut self, right: bool) {
        self.focus_right = right && self.right.is_some();
    }

    /// The group and index of an open tab, in the left group first.
    pub(crate) fn find_tab(&self, path: &Path) -> Option<(bool, usize)> {
        if let Some(i) = self.tabs.position(path) {
            return Some((false, i));
        }
        self.right
            .as_ref()
            .and_then(|r| r.position(path))
            .map(|i| (true, i))
    }

    /// Focus the group already showing `path` and activate its tab. True when the
    /// path was open somewhere, so a caller does not open a second editor for it.
    pub(crate) fn reveal_open_tab(&mut self, path: &Path) -> bool {
        match self.find_tab(path) {
            Some((right, i)) => {
                self.focus_group(right);
                self.group_mut(right).active = i;
                true
            }
            None => false,
        }
    }

    /// Ctrl+\ and "Split right": move the active tab of the focused group into a
    /// new right group and focus it. A no-op while the add-on is off, while there
    /// is already a split, or with nothing open.
    pub(crate) fn split_active_right(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.right.is_some() || !self.split_enabled() || self.focused().tabs.is_empty() {
            return;
        }
        let from = self.focus_right;
        let i = self.focused().active;
        let Some(tab) = self.group_mut(from).take(i) else {
            return;
        };
        let mut set = TabSet::default();
        set.tabs.push(tab);
        set.active = 0;
        self.right = Some(set);
        self.focus_right = true;
        self.focus_tab(window, cx);
        cx.notify();
    }

    /// "Close group when its last tab closes": drop the right group once it holds
    /// no tabs. The left group always stays.
    pub(crate) fn prune_right(&mut self) {
        if self.right.as_ref().is_some_and(|r| r.tabs.is_empty()) {
            self.right = None;
            self.focus_right = false;
        }
    }

    /// Turn the split off (the add-on was switched off or a folder was opened):
    /// keep every editor by moving the right group's tabs into the left group.
    pub(crate) fn collapse_split(&mut self) {
        if let Some(mut r) = self.right.take() {
            for tab in r.tabs.drain(..) {
                if self.tabs.position(&tab.path).is_none() {
                    self.tabs.tabs.push(tab);
                }
            }
            self.focus_right = false;
        }
    }

    /// Move a tab between groups, e.g. dropped on the other group. `from_right`
    /// says which group it came from; the other one takes it.
    pub(crate) fn move_tab_between(
        &mut self,
        path: &Path,
        from_right: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let to_right = !from_right;
        if self.group(to_right).position(path).is_some() {
            self.reveal_open_tab(path);
            self.focus_tab(window, cx);
            cx.notify();
            return;
        }
        let Some(i) = self.group(from_right).position(path) else {
            return;
        };
        let Some(tab) = self.group_mut(from_right).take(i) else {
            return;
        };
        self.focus_group(to_right);
        self.group_mut(to_right).add(tab);
        self.prune_right();
        self.focus_tab(window, cx);
        cx.notify();
    }

    /// Every path open in either group, for the filesystem watcher.
    pub(crate) fn open_paths(&self) -> Vec<PathBuf> {
        self.tabs
            .tabs
            .iter()
            .chain(self.right.iter().flat_map(|r| r.tabs.iter()))
            .map(|t| t.path.clone())
            .collect()
    }
}
