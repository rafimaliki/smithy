//! Tab closing and reopening: the single close, the context menu's bulk closes,
//! the unsaved-changes prompt queue, and the `Closed <file>` toast.
//!
//! A bulk close closes highest-index-first, so each removal leaves the lower
//! indices the queue still holds valid; a dirty tab stops the queue at the prompt.
//! The queue is fixed to one group (`close_right`) when it starts, so changing
//! the focused group cannot redirect it.
use super::tab_set::{CloseGroup, TabSet};
use super::{TabContent, Workspace};
use gpui::{Context, Pixels, Point, SharedString, Timer, Window};
use std::time::Duration;

/// What the tab context menu was opened on: the tab, its group and the click.
pub(crate) struct TabMenu {
    pub index: usize,
    pub right: bool,
    pub at: Point<Pixels>,
}

/// The `Closed <file>` toast, shown until its timer runs out.
pub(crate) struct Toast {
    pub text: SharedString,
    /// The group the closed tab came from, so Reopen puts it back there.
    pub right: bool,
}

const TOAST_SECS: u64 = 6;

impl Workspace {
    /// A tab's display name: the add-on's title, or the file name.
    pub(super) fn tab_label(set: &TabSet<TabContent>, i: usize) -> SharedString {
        match set.tabs.get(i) {
            Some(tab) => match &tab.content {
                TabContent::Addon { title, .. } => title.clone(),
                _ => tab
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| tab.path.display().to_string())
                    .into(),
            },
            None => SharedString::default(),
        }
    }

    /// Close tab `i` of `right`, asking first if it has unsaved edits.
    pub fn request_close(
        &mut self,
        right: bool,
        i: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_tabs(right, vec![i], window, cx);
    }

    /// Run a bulk close from the tab menu on the group the menu was opened on.
    pub(super) fn close_group(
        &mut self,
        group: CloseGroup,
        right: bool,
        i: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tab_menu = None;
        let targets = self.group(right).close_targets(group, i);
        self.close_tabs(right, targets, window, cx);
    }

    fn close_tabs(
        &mut self,
        right: bool,
        targets: Vec<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_right = right;
        self.pending_closes = targets;
        self.advance_close(window, cx);
    }

    /// Take the next tab to close: a dirty one stops for the prompt, a clean one
    /// closes now. Targets are ascending, so popping the end closes highest-first.
    fn advance_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(i) = self.pending_closes.pop() {
            if i >= self.group(self.close_right).tabs.len() {
                continue;
            }
            if self.is_dirty_in(self.close_right, i, cx) {
                self.pending_close = Some(i);
                cx.notify();
                return;
            }
            self.close_now(i, window, cx);
        }
        self.focus_group(self.close_right);
        self.focus_tab(window, cx);
        cx.notify();
    }

    fn close_now(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let right = self.close_right;
        let name = Self::tab_label(self.group(right), i);
        self.group_mut(right).close(i);
        self.prune_right();
        super::watch::sync(self);
        self.show_toast(right, name, window, cx);
    }

    /// Answer the unsaved-changes prompt, then carry on with the queue.
    pub(crate) fn resolve_close(
        &mut self,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let right = self.close_right;
        if let Some(i) = self.pending_close.take() {
            if save {
                self.save_tab(right, i, cx);
            }
            if save && self.is_dirty_in(right, i, cx) {
                // Saving failed; keep the file and stop. The error is in the status bar.
                self.pending_closes.clear();
            } else {
                self.close_now(i, window, cx);
            }
        }
        self.advance_close(window, cx);
    }

    /// Cancel the prompt and the rest of the bulk close.
    pub(super) fn cancel_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_close = None;
        self.pending_closes.clear();
        self.focus_tab(window, cx);
        cx.notify();
    }

    /// Reopen the most recently closed tab of `right`: the toast button and
    /// Ctrl+Shift+T.
    pub(super) fn reopen_last(&mut self, right: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.toast = None;
        if let Some(p) = self.group_mut(right).pop_closed() {
            self.open_file(&p, window, cx);
        }
        cx.notify();
    }

    fn show_toast(
        &mut self,
        right: bool,
        name: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toast = Some(Toast {
            text: format!("Closed {name}").into(),
            right,
        });
        // The timer only clears the toast it was started for.
        self.toast_seq = self.toast_seq.wrapping_add(1);
        let seq = self.toast_seq;
        cx.spawn_in(window, async move |this, cx| {
            Timer::after(Duration::from_secs(TOAST_SECS)).await;
            this.update_in(cx, |this, _, cx| {
                if this.toast_seq == seq {
                    this.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn save_tab(&mut self, right: bool, i: usize, cx: &mut Context<Self>) {
        let editor = match self.group(right).tabs.get(i).map(|t| &t.content) {
            Some(TabContent::Editor(e, _)) => e.clone(),
            _ => return,
        };
        let path = self.group(right).tabs[i].path.clone();
        if let Err(err) = editor.update(cx, |e, cx| e.save(cx)) {
            self.error = Some(format!("Could not save: {err}").into());
        }
        let decorations = self.decorations_for(&path);
        editor.update(cx, |e, cx| {
            e.set_decorations(decorations);
            cx.notify();
        });
    }
}
