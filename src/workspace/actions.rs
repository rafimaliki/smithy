//! Action handlers: one method per workspace action, wired in `render.rs`.
use super::Workspace;
use crate::actions::{
    CloseTab, NextTab, OpenFolder, PrevTab, Quit, ReopenTab, Save, SearchEverywhere, ToggleSidebar,
};
use crate::editor::view::EditorEvent;
use gpui::{Context, Window};

impl Workspace {
    /// The editor asked to navigate (Ctrl+Click, F12, Shift+F12, Alt+Left/Right):
    /// the first add-on that knows what to do with it takes it.
    pub(crate) fn on_editor_navigate(
        &mut self,
        event: &EditorEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let EditorEvent::Navigate {
            what,
            path,
            line,
            character,
        } = event;
        for (_, instance) in self.registry.running() {
            if instance.navigate(*what, path, *line, *character, window, cx) {
                break;
            }
        }
    }
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
        self.reopen_last(window, cx);
    }

    /// Ctrl+Shift+O: hand the key to the add-on that owns Search everywhere.
    pub(crate) fn on_search_everywhere(
        &mut self,
        _: &SearchEverywhere,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (_, inst) in self.registry.running() {
            inst.toggle_search_everywhere(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn on_save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        self.save_tab(self.tabs.active, cx);
        cx.notify();
    }

    pub(crate) fn on_quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }
}
