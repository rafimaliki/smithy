//! Action handlers: one method per workspace action, wired in `render.rs`.
use super::Workspace;
use crate::actions::{
    CloseTab, NextTab, OpenFolder, PrevTab, Quit, ReopenTab, Save, SearchEverywhere, SplitRight,
    ToggleSidebar, ToggleTerminal,
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
        let right = self.focus_right;
        let i = self.focused().active;
        if !self.focused().tabs.is_empty() {
            self.request_close(right, i, window, cx);
        }
    }

    pub(crate) fn on_next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        let right = self.focus_right;
        self.focused_mut().next();
        self.focus_group(right);
        self.focus_tab(window, cx);
        cx.notify();
    }

    pub(crate) fn on_prev_tab(&mut self, _: &PrevTab, window: &mut Window, cx: &mut Context<Self>) {
        let right = self.focus_right;
        self.focused_mut().prev();
        self.focus_group(right);
        self.focus_tab(window, cx);
        cx.notify();
    }

    pub(crate) fn on_reopen_tab(
        &mut self,
        _: &ReopenTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let right = self.focus_right;
        self.reopen_last(right, window, cx);
    }

    /// Ctrl+\: split the focused group to the right (Split panes add-on only).
    pub(crate) fn on_split_right(
        &mut self,
        _: &SplitRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.split_active_right(window, cx);
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

    /// Ctrl+backtick: hand the key to the add-on that owns the bottom panel.
    pub(crate) fn on_toggle_terminal(
        &mut self,
        _: &ToggleTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (_, inst) in self.registry.running() {
            inst.toggle_bottom_panel(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn on_save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<Self>) {
        let right = self.focus_right;
        let i = self.focused().active;
        self.save_tab(right, i, cx);
        cx.notify();
    }

    pub(crate) fn on_quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }
}
