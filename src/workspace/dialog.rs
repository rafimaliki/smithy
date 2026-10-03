//! Delete confirmation. Delete means the Recycle Bin, never permanent, so it
//! always asks first; the text says when unsaved changes go with it.
use super::Workspace;
use crate::theme::{on_accent, Theme};
use crate::tree::ops;
use gpui::{div, prelude::*, px, AnyElement, Context, Div, Stateful, Window};

impl Workspace {
    /// The dialog while a delete waits for an answer.
    pub(super) fn delete_dialog(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let path = self.pending_delete.clone()?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let dirty = (0..self.tabs.tabs.len())
            .any(|i| self.tabs.tabs[i].path.starts_with(&path) && self.is_dirty(i, cx));
        let body = if dirty {
            "You can restore it from the Recycle Bin. It has unsaved changes, which go with it."
        } else {
            "You can restore it from the Recycle Bin."
        };

        let button = |id: &'static str, label: &'static str, danger: bool| -> Stateful<Div> {
            div()
                .id(id)
                .px(px(14.))
                .py(px(7.))
                .rounded(px(6.))
                .cursor_pointer()
                .border_1()
                .border_color(if danger { t.del } else { t.line })
                .when(danger, |d| d.bg(t.del).text_color(on_accent()))
                .when(!danger, |d| d.hover(|d| d.bg(t.hov)))
                .child(label)
        };

        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000099))
                .occlude()
                .child(
                    div()
                        .w(px(440.))
                        .p(px(22.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .bg(t.side)
                        .border_1()
                        .border_color(t.line)
                        .rounded(px(10.))
                        .text_color(t.ink)
                        .child(
                            div()
                                .text_size(px(15.))
                                .child(format!("Move {name} to the Recycle Bin?")),
                        )
                        .child(div().text_color(t.mute).child(body))
                        .child(
                            div()
                                .mt(px(12.))
                                .flex()
                                .justify_end()
                                .gap(px(10.))
                                .child(button("del-cancel", "Cancel", false).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.pending_delete = None;
                                        cx.notify();
                                    },
                                )))
                                .child(
                                    button("del-confirm", "Move to Recycle Bin", true).on_click(
                                        cx.listener(|this, _, window: &mut Window, cx| {
                                            this.confirm_delete(window, cx)
                                        }),
                                    ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Send `pending_delete` to the Recycle Bin, then close the tabs it leaves
    /// behind and re-read the tree.
    pub(super) fn confirm_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.pending_delete.take() else {
            return;
        };
        match ops::delete(&path) {
            Ok(()) => {
                self.tabs.close_under(&path);
                if let Some(tree) = self.tree.clone() {
                    tree.update(cx, |tree, cx| tree.reload(cx));
                }
                self.focus_tab(window, cx);
            }
            Err(e) => self.error = Some(format!("Could not delete: {e}").into()),
        }
        cx.notify();
    }
}
