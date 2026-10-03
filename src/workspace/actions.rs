//! Keymap action handlers: the small glue from a bound action to the workspace.
use super::Workspace;
use crate::actions::*;
use gpui::{Context, Window};

impl Workspace {
    pub(crate) fn on_toggle_sidebar(
        &mut self,
        _: &ToggleSidebar,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    pub(crate) fn on_open_folder(
        &mut self,
        _: &OpenFolder,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prompt_open_folder(window, cx);
    }

    pub(crate) fn on_close_tab(
        &mut self,
        _: &CloseTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.tabs.tabs.is_empty() {
            self.request_close(self.tabs.active, window, cx);
        }
    }

    pub(crate) fn on_next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.next();
        self.focus_tab(window, cx);
        cx.notify();
    }

    pub(crate) fn on_prev_tab(&mut self, _: &PrevTab, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.prev();
        self.focus_tab(window, cx);
        cx.notify();
    }

    pub(crate) fn on_reopen_tab(
        &mut self,
        _: &ReopenTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(p) = self.tabs.pop_closed() {
            self.open_file(&p, window, cx);
        }
    }

    pub(crate) fn on_save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        self.save_tab(self.tabs.active, cx);
        cx.notify();
    }

    pub(crate) fn on_quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }
}
