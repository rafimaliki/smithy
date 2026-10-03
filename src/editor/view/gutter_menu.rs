//! The menu opened by right-clicking a line number. Today it holds one option, Git
//! blame; other line actions can be added as rows. Board frame `gutter-menu`.
use super::EditorView;
use crate::theme::Theme;
use crate::workspace::menu;
use gpui::{prelude::*, AnyElement, Context};

impl EditorView {
    /// Whether the line menu has anything to offer: blame comes from the Source
    /// control add-on, so without it (or outside a repository) the menu stays shut.
    pub(super) fn has_line_menu(&self) -> bool {
        !self.decorations.blame.is_empty()
    }

    pub(super) fn gutter_menu_view(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let at = self.gutter_menu?;
        let this = cx.entity();
        let label = if self.blame_column {
            "Hide git blame"
        } else {
            "Git blame"
        };
        let rows = vec![
            menu::item("line-blame", label, Some("Ctrl+Alt+B"), false, true, t)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.blame_column = !this.blame_column;
                    this.gutter_menu = None;
                    cx.notify();
                }))
                .into_any_element(),
        ];
        Some(menu::popup(
            t,
            at,
            move |_, _, app| {
                this.update(app, |view, cx| {
                    view.gutter_menu = None;
                    cx.notify();
                })
            },
            rows,
        ))
    }
}
