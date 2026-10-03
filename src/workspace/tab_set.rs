//! Tab order, pinning and the reopen stack. Generic over the tab content so the
//! rules are tested without a window.
use std::path::{Path, PathBuf};

pub struct Tab<C> {
    pub path: PathBuf,
    pub pinned: bool,
    pub content: C,
}

pub struct TabSet<C> {
    pub tabs: Vec<Tab<C>>,
    pub active: usize,
    closed: Vec<PathBuf>,
}

impl<C> Default for TabSet<C> {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
            closed: Vec::new(),
        }
    }
}

const MAX_CLOSED: usize = 20;

/// A bulk close from the tab context menu. Pinned tabs are skipped by all three,
/// so a pinned tab never disappears from a menu that names a neighbouring tab.
#[derive(Clone, Copy, PartialEq)]
pub enum CloseGroup {
    /// Every tab but this one.
    Others,
    /// Every tab after this one.
    Right,
    /// Every tab.
    All,
}

impl<C> TabSet<C> {
    pub fn active_tab(&self) -> Option<&Tab<C>> {
        self.tabs.get(self.active)
    }

    pub fn position(&self, path: &Path) -> Option<usize> {
        self.tabs.iter().position(|t| t.path == path)
    }

    /// Activate the tab for `path`, creating it after the pinned tabs if absent.
    /// Returns true when a new tab was created.
    pub fn open(&mut self, path: &Path, make: impl FnOnce() -> C) -> bool {
        if let Some(i) = self.position(path) {
            self.active = i;
            return false;
        }
        let at = self
            .tabs
            .iter()
            .filter(|t| t.pinned)
            .count()
            .max(self.active + 1);
        let at = at.min(self.tabs.len());
        self.tabs.insert(
            at,
            Tab {
                path: path.to_path_buf(),
                pinned: false,
                content: make(),
            },
        );
        self.active = at;
        true
    }

    /// Close tab `i` and remember it for reopen.
    pub fn close(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        let t = self.tabs.remove(i);
        self.closed.push(t.path);
        if self.closed.len() > MAX_CLOSED {
            self.closed.remove(0);
        }
        if i < self.active || (i == self.active && self.active == self.tabs.len()) {
            self.active = self.active.saturating_sub(1);
        }
    }

    /// Path of the most recently closed tab, removed from the stack.
    pub fn pop_closed(&mut self) -> Option<PathBuf> {
        self.closed.pop()
    }

    /// Follow a rename: every tab at `from` or under it moves to `to`.
    pub fn remap_paths(&mut self, from: &Path, to: &Path) {
        let moved = |p: &Path| -> PathBuf {
            match p.strip_prefix(from) {
                Ok(rest) => to.join(rest),
                Err(_) => p.to_path_buf(),
            }
        };
        for tab in &mut self.tabs {
            tab.path = moved(&tab.path);
        }
        for path in &mut self.closed {
            *path = moved(path);
        }
    }

    /// Close every tab at `path` or under it. A deleted path is not remembered for
    /// reopen. Returns how many closed.
    pub fn close_under(&mut self, path: &Path) -> usize {
        let before = self.tabs.len();
        self.tabs.retain(|t| !t.path.starts_with(path));
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
        before - self.tabs.len()
    }

    /// Which tabs a tab-menu bulk close takes. Pinned tabs are never included.
    pub fn close_targets(&self, group: CloseGroup, i: usize) -> Vec<usize> {
        (0..self.tabs.len())
            .filter(|&j| {
                !self.tabs[j].pinned
                    && match group {
                        CloseGroup::All => true,
                        CloseGroup::Others => j != i,
                        CloseGroup::Right => j > i,
                    }
            })
            .collect()
    }

    /// Pin or unpin; pinned tabs sit at the left, the toggled tab lands on the
    /// pinned/unpinned boundary.
    pub fn toggle_pin(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        let active_path = self.tabs[self.active.min(self.tabs.len() - 1)].path.clone();
        let mut tab = self.tabs.remove(i);
        tab.pinned = !tab.pinned;
        let pinned = self.tabs.iter().filter(|t| t.pinned).count();
        self.tabs.insert(pinned, tab);
        self.active = self.position(&active_path).unwrap_or(0);
    }

    pub fn next(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> TabSet<()> {
        let mut s = TabSet::default();
        for n in names {
            s.open(Path::new(n), || ());
        }
        s
    }

    fn order(s: &TabSet<()>) -> Vec<String> {
        s.tabs
            .iter()
            .map(|t| t.path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn opening_an_open_file_activates_it_without_duplicating() {
        let mut s = set(&["a", "b", "c"]);
        assert!(!s.open(Path::new("a"), || ()));
        assert_eq!(s.active, 0);
        assert_eq!(s.tabs.len(), 3);
    }

    #[test]
    fn pinned_tabs_move_left_and_active_tab_survives() {
        let mut s = set(&["a", "b", "c"]);
        s.active = 1; // b
        s.toggle_pin(2); // pin c
        assert_eq!(order(&s), ["c", "a", "b"]);
        assert_eq!(s.active_tab().unwrap().path, Path::new("b"));
        s.toggle_pin(0); // unpin c
        assert!(!s.tabs[0].pinned);
        assert_eq!(s.active_tab().unwrap().path, Path::new("b"));
    }

    #[test]
    fn reopen_is_lifo() {
        let mut s = set(&["a", "b", "c", "d"]);
        s.toggle_pin(3); // pin d -> [d,a,b,c]
        s.close(3); // c
        s.close(2); // b
        assert_eq!(s.pop_closed(), Some(PathBuf::from("b")));
        assert_eq!(s.pop_closed(), Some(PathBuf::from("c")));
        assert_eq!(s.pop_closed(), None);
    }

    #[test]
    fn closing_the_last_active_tab_activates_its_neighbour() {
        let mut s = set(&["a", "b"]);
        s.close(1);
        assert_eq!(s.active, 0);
        s.close(0);
        assert!(s.active_tab().is_none());
    }

    #[test]
    fn a_rename_carries_the_tabs_under_the_moved_folder() {
        let mut s = set(&["src/a.rs", "src/deep/b.rs", "other.rs"]);
        s.remap_paths(Path::new("src"), Path::new("code"));
        assert_eq!(order(&s), ["code/a.rs", "code/deep/b.rs", "other.rs"]);
        // A sibling with the same prefix is not part of it.
        let mut s = set(&["src/a.rs", "src2/b.rs"]);
        s.remap_paths(Path::new("src"), Path::new("code"));
        assert_eq!(order(&s), ["code/a.rs", "src2/b.rs"]);
    }

    #[test]
    fn close_under_takes_the_folder_and_its_files_and_clamps_active() {
        let mut s = set(&["src/a.rs", "src/b.rs", "other.rs"]);
        s.active = 2;
        assert_eq!(s.close_under(Path::new("src")), 2);
        assert_eq!(order(&s), ["other.rs"]);
        assert_eq!(s.active, 0);
        // Deleted files are not offered for reopen.
        assert_eq!(s.pop_closed(), None);
    }

    #[test]
    fn bulk_close_skips_pinned_tabs() {
        // d is pinned, so it sits first and no group ever names it.
        let mut s = set(&["a", "b", "c", "d"]);
        s.toggle_pin(3); // [d, a, b, c]
        assert_eq!(s.close_targets(CloseGroup::All, 1), vec![1, 2, 3]);
        assert_eq!(s.close_targets(CloseGroup::Others, 2), vec![1, 3]);
        assert_eq!(s.close_targets(CloseGroup::Right, 1), vec![2, 3]);
        assert!(s.close_targets(CloseGroup::Right, 3).is_empty());
    }

    #[test]
    fn bulk_close_indices_stay_valid_when_removed_from_the_end() {
        // The caller closes highest-first, so each removal only shifts indices that
        // are already gone.
        let mut s = set(&["a", "b", "c", "d", "e"]);
        let targets = s.close_targets(CloseGroup::All, 0);
        assert_eq!(targets, vec![0, 1, 2, 3, 4]);
        for i in targets.into_iter().rev() {
            let name = order(&s)[i].clone();
            s.close(i);
            assert_eq!(s.position(Path::new(&name)), None);
        }
        assert!(s.tabs.is_empty());
        // Closing remembers each one for reopen.
        assert_eq!(s.pop_closed(), Some(PathBuf::from("a")));
    }
}
