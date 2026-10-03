//! Tab closing and reopening: the single close, the context menu's bulk closes,
//! the unsaved-changes prompt queue, and the `Closed <file>` toast.
//!
//! A bulk close closes highest-index-first, so each removal leaves the lower
//! indices the queue still holds valid; a dirty tab stops the queue at the prompt.
use super::tab_set::CloseGroup;
use super::{TabContent, Workspace};
use gpui::{Context, Pixels, Point, SharedString, Timer, Window};
use std::time::Duration;

/// What the tab context menu was opened on: the tab and the click position.
pub(crate) struct TabMenu {
    pub index: usize,
    pub at: Point<Pixels>,
}

/// The `Closed <file>` toast, shown until its timer runs out.
pub(crate) struct Toast {
    pub text: SharedString,
}

const TOAST_SECS: u64 = 6;

impl Workspace {
    /// A tab's display name: the add-on's title, or the file name.
    pub(super) fn tab_label(&self, i: usize) -> SharedString {
        match self.tabs.tabs.get(i) {
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

    /// Close tab `i`, asking first if it has unsaved edits.
    pub fn request_close(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.close_tabs(vec![i], window, cx);
    }

    /// Run a bulk close from the tab menu.
    pub(super) fn close_group(
        &mut self,
        group: CloseGroup,
        i: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tab_menu = None;
        let targets = self.tabs.close_targets(group, i);
        self.close_tabs(targets, window, cx);
    }

    fn close_tabs(&mut self, targets: Vec<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_closes = targets;
        self.advance_close(window, cx);
    }

    /// Take the next tab to close: a dirty one stops for the prompt, a clean one
    /// closes now. Targets are ascending, so popping the end closes highest-first.
    fn advance_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(i) = self.pending_closes.pop() {
            if i >= self.tabs.tabs.len() {
                continue;
            }
            if self.is_dirty(i, cx) {
                self.pending_close = Some(i);
                cx.notify();
                return;
            }
            self.close_now(i, window, cx);
        }
        self.focus_tab(window, cx);
        cx.notify();
    }

    fn close_now(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.tab_label(i);
        self.tabs.close(i);
        super::watch::sync(self);
        self.show_toast(name, window, cx);
    }

    /// Answer the unsaved-changes prompt, then carry on with the queue.
    pub(crate) fn resolve_close(
        &mut self,
        save: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(i) = self.pending_close.take() {
            if save {
                self.save_tab(i, cx);
            }
            if save && self.is_dirty(i, cx) {
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

    /// Reopen the most recently closed tab: the toast button and Ctrl+Shift+T.
    pub(super) fn reopen_last(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toast = None;
        if let Some(p) = self.tabs.pop_closed() {
            self.open_file(&p, window, cx);
        }
        cx.notify();
    }

    fn show_toast(&mut self, name: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.toast = Some(Toast {
            text: format!("Closed {name}").into(),
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

    pub(super) fn save_tab(&mut self, i: usize, cx: &mut Context<Self>) {
        let editor = match self.tabs.tabs.get(i).map(|t| &t.content) {
            Some(TabContent::Editor(e, _)) => e.clone(),
            _ => return,
        };
        let path = self.tabs.tabs[i].path.clone();
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
