//! The tab bar's right-click menu: Close, Close others / to the right / all,
//! Pin or Unpin, Split right, Copy path, Copy relative path and Reveal. Design:
//! board frames `files-tab-context` (the Split right row) and `split-panes`. Rows
//! and the panel come from the shared `menu` module.
use super::menu;
use super::tab_set::CloseGroup;
use super::Workspace;
use crate::theme::Theme;
use gpui::{prelude::*, AnyElement, ClipboardItem, Context, MouseDownEvent, Window};

impl Workspace {
    /// The tab's right-click menu, or `None` while it is closed.
    pub(super) fn tab_menu(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.tab_menu.as_ref()?;
        let i = open.index;
        let right = open.right;
        let at = open.at;
        let set = self.group(right);
        let tab = set.tabs.get(i)?;
        let pinned = tab.pinned;
        let path = tab.path.clone();
        // `<addon>/…` is not a path on disk: no copy or reveal for those.
        let real = !path.starts_with("<addon>");
        let can_others = !set.close_targets(CloseGroup::Others, i).is_empty();
        let can_right = !set.close_targets(CloseGroup::Right, i).is_empty();
        let can_all = !set.close_targets(CloseGroup::All, i).is_empty();
        // Only offered while the Split panes add-on is on, there is no split yet
        // (two groups maximum) and there is a tab to move over. With the add-on
        // off the row is absent, not disabled.
        let can_split = self.split_enabled() && !self.is_split() && !set.tabs.is_empty();

        let mut rows: Vec<AnyElement> = Vec::new();
        rows.push(
            menu::item("m-close", "Close", Some("Ctrl+W"), false, true, t)
                .on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.request_close(right, i, window, cx)
                    }),
                )
                .into_any_element(),
        );
        rows.push(
            menu::item("m-close-others", "Close others", None, false, can_others, t)
                .when(can_others, |d| {
                    d.on_click(cx.listener(move |this, _, window, cx| {
                        this.close_group(CloseGroup::Others, right, i, window, cx)
                    }))
                })
                .into_any_element(),
        );
        rows.push(
            menu::item(
                "m-close-right",
                "Close to the right",
                None,
                false,
                can_right,
                t,
            )
            .when(can_right, |d| {
                d.on_click(cx.listener(move |this, _, window, cx| {
                    this.close_group(CloseGroup::Right, right, i, window, cx)
                }))
            })
            .into_any_element(),
        );
        rows.push(
            menu::item("m-close-all", "Close all", None, false, can_all, t)
                .when(can_all, |d| {
                    d.on_click(cx.listener(move |this, _, window, cx| {
                        this.close_group(CloseGroup::All, right, i, window, cx)
                    }))
                })
                .into_any_element(),
        );
        rows.push(menu::separator(t).into_any_element());

        let pin_label = if pinned { "Unpin tab" } else { "Pin tab" };
        rows.push(
            menu::item("m-pin", pin_label, None, false, true, t)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tab_menu = None;
                    this.focus_group(right);
                    this.group_mut(right).toggle_pin(i);
                    cx.notify();
                }))
                .into_any_element(),
        );
        if can_split {
            rows.push(
                menu::item("m-split", "Split right", Some("Ctrl+\\"), false, true, t)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.tab_menu = None;
                        this.focus_group(right);
                        this.split_active_right(window, cx);
                    }))
                    .into_any_element(),
            );
        }
        rows.push(menu::separator(t).into_any_element());

        let abs = path.clone();
        rows.push(
            menu::item("m-copy-path", "Copy path", None, false, real, t)
                .when(real, |d| {
                    d.on_click(cx.listener(move |this, _, _, cx| {
                        this.tab_menu = None;
                        cx.write_to_clipboard(ClipboardItem::new_string(abs.display().to_string()));
                        cx.notify();
                    }))
                })
                .into_any_element(),
        );
        let rel = path.clone();
        let has_root = self.folder.is_some();
        rows.push(
            menu::item(
                "m-copy-rel",
                "Copy relative path",
                None,
                false,
                real && has_root,
                t,
            )
            .when(real && has_root, |d| {
                d.on_click(cx.listener(move |this, _, _, cx| {
                    this.tab_menu = None;
                    if let Some(root) = this.folder.clone() {
                        let text = crate::tree::ops::relative(&rel, &root);
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                    cx.notify();
                }))
            })
            .into_any_element(),
        );
        let reveal = path.clone();
        rows.push(
            menu::item("m-reveal", "Reveal in File Explorer", None, false, real, t)
                .when(real, |d| {
                    d.on_click(cx.listener(move |this, _, _, cx| {
                        this.tab_menu = None;
                        cx.reveal_path(&reveal);
                        cx.notify();
                    }))
                })
                .into_any_element(),
        );

        let entity = cx.entity();
        Some(menu::popup(
            t,
            at,
            move |_: &MouseDownEvent, _: &mut Window, cx| {
                entity.update(cx, |this, cx| {
                    this.tab_menu = None;
                    cx.notify();
                });
            },
            rows,
        ))
    }
}
