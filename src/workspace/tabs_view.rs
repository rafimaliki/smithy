//! The tab bar: one per editor group, its drag source and drop target. The bar's
//! right-click menu is in `tab_menu.rs`; the toast and unsaved prompt it raises
//! are in `tab_prompt.rs`. Design: board frames `files-tabs-pinned`,
//! `reopen-toast`, `unsaved-prompt`, `split-panes`.
use super::Workspace;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent, Render, Rgba,
    ScrollHandle, SharedString, Window,
};
use std::cell::Cell;
use std::path::PathBuf;

const PIN: &str = "icons/pin.svg";

/// Horizontal scroll of each group's tab strip, and what it was last scrolled to,
/// so a tab that becomes active (or is added) is brought into view once.
#[derive(Default)]
pub(super) struct TabScroll {
    handles: [ScrollHandle; 2],
    seen: Cell<[Option<(usize, usize)>; 2]>,
}

/// What a dragged tab carries: its path and the group it came from, so a drop on
/// the other group can move it across.
#[derive(Clone)]
pub(super) struct DraggedTab {
    pub path: PathBuf,
    pub from_right: bool,
}

/// The label that follows the cursor while a tab is dragged.
struct TabDragPreview {
    label: SharedString,
    bg: Rgba,
    line: Rgba,
    ink: Rgba,
}

impl Render for TabDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .h(px(26.))
            .flex()
            .items_center()
            .bg(self.bg)
            .border_1()
            .border_color(self.line)
            .rounded(px(6.))
            .text_color(self.ink)
            .child(self.label.clone())
    }
}

fn pin_mark(t: &Theme) -> AnyElement {
    gpui::svg()
        .path(PIN)
        .w(px(12.))
        .h(px(12.))
        .flex_none()
        .text_color(t.mute)
        .into_any_element()
}

impl Workspace {
    pub(super) fn tab_bar(
        &self,
        right: bool,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let set = self.group(right);
        let focused = !right || self.focus_right;
        let (bg, line, ink) = (t.bg, t.line, t.ink);
        let mut bar = div()
            .id(SharedString::from(format!("tabbar-{right}")))
            .h(px(36.))
            .flex_none()
            .flex()
            .bg(t.side)
            .border_b_1()
            .border_color(t.line);
        // The tabs sit in their own strip that clips and scrolls, so many tabs never
        // push the drag area, the title actions or the window buttons out of the row.
        let handle = self.tab_scroll.handles[usize::from(right)].clone();
        let mut tabs = div()
            .id(SharedString::from(format!("tabs-{right}")))
            .flex()
            .flex_initial()
            .min_w_0()
            .h_full()
            .overflow_x_scroll()
            .track_scroll(&handle);
        let pinned_count = set.tabs.iter().filter(|t| t.pinned).count();
        for (i, tab) in set.tabs.iter().enumerate() {
            let active = i == set.active;
            let dirty = self.is_dirty_in(right, i, cx);
            let pinned = tab.pinned;
            let name = Self::tab_label(set, i);
            let group: SharedString = format!("tabgrp-{right}-{i}").into();
            let slot = div()
                .relative()
                .w(px(16.))
                .h(px(16.))
                .ml(px(4.))
                .flex_none()
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(dirty, |d| {
                            d.child(div().size(px(6.)).rounded_full().bg(t.acc))
                        })
                        .group_hover(group.clone(), |d| d.invisible()),
                )
                .child(
                    div()
                        .id((SharedString::from(format!("close-{right}")), i))
                        .absolute()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(t.mute)
                        .when(!dirty, |d| d.invisible())
                        .group_hover(group.clone(), |d| d.visible().text_color(t.ink))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                this.request_close(right, i, window, cx)
                            }),
                        )
                        .child("×"),
                );
            // Pinned tabs are compact, carry a pin mark and have no close button.
            let mut item = div()
                .id((SharedString::from(format!("tab-{right}")), i))
                .group(group)
                .flex()
                .flex_none()
                .items_center()
                .h_full()
                .gap(px(if pinned { 6. } else { 8. }))
                .px(px(if pinned { 12. } else { 14. }))
                .cursor_pointer()
                .text_color(if active { t.ink } else { t.mute })
                .when(active, |d| {
                    d.bg(t.bg)
                        .border_t_1()
                        .border_color(if focused { t.acc } else { t.line })
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                        this.show_settings = false;
                        this.focus_group(right);
                        this.group_mut(right).active = i;
                        this.focus_tab(window, cx);
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                        this.focus_group(right);
                        this.tab_menu = Some(super::tabs::TabMenu {
                            index: i,
                            right,
                            at: e.position,
                        });
                        cx.notify();
                    }),
                );
            if pinned {
                item = item.child(pin_mark(t)).child(name.clone());
            } else {
                item = item.child(name.clone()).child(slot);
            }
            // Dragging a tab onto the other group moves it there. Only worth
            // wiring while there are two groups, so the single-group path pays
            // nothing.
            if self.is_split() {
                let drag_label = name;
                let drag_path = tab.path.clone();
                item = item.on_drag(
                    DraggedTab {
                        path: drag_path,
                        from_right: right,
                    },
                    move |_, _offset, _window, cx| {
                        cx.new(|_| TabDragPreview {
                            label: drag_label.clone(),
                            bg,
                            line,
                            ink,
                        })
                    },
                );
            }
            tabs = tabs.child(item);
            // A hairline between the pinned block and the rest.
            if pinned && i + 1 == pinned_count && pinned_count < set.tabs.len() {
                tabs = tabs.child(
                    div()
                        .w(px(1.))
                        .h(px(20.))
                        .my(px(8.))
                        .mx(px(2.))
                        .flex_none()
                        .bg(t.line),
                );
            }
        }
        // The empty part of the row moves the window; the window buttons belong to the
        // group at the right edge only.
        // Bring the active tab into view when it changes or the count does.
        let key = (set.active, set.tabs.len());
        let mut seen = self.tab_scroll.seen.get();
        if !self.show_settings && seen[usize::from(right)] != Some(key) && !set.tabs.is_empty() {
            seen[usize::from(right)] = Some(key);
            self.tab_scroll.seen.set(seen);
            // The hairline after the pinned block is a child of its own.
            let hairline =
                pinned_count > 0 && pinned_count < set.tabs.len() && set.active >= pinned_count;
            handle.scroll_to_item(set.active + usize::from(hairline));
        }
        bar = bar.child(tabs);
        bar = bar.child(super::header::drag_area());
        if !self.is_split() || right {
            bar = bar.child(self.header_tail(t, cx));
        }
        // Accept a tab dragged from the other group; only relevant while split.
        if self.is_split() {
            let entity = cx.entity();
            bar = bar.on_drop(move |dragged: &DraggedTab, window, cx| {
                if dragged.from_right != right {
                    entity.update(cx, |this, cx| {
                        this.move_tab_between(&dragged.path, dragged.from_right, window, cx)
                    });
                }
            });
        }
        bar
    }
}
