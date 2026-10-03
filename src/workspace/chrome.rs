//! Workbench chrome: activity rail, tab bar, status bar, unsaved-changes prompt.
use super::{Sidebar, TabContent, Workspace};
use crate::editor::lang::language_for;
use crate::theme::{on_accent, Theme};
use gpui::{
    div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent, SharedString, Stateful,
    Window,
};

fn rail_button(id: &'static str, glyph: &'static str, on: bool, t: &Theme) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .relative()
        .w(px(48.))
        .h(px(40.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(if on { t.ink } else { t.mute })
        .when(on, |d| d.border_l_2().border_color(t.acc))
        .hover(|d| d.text_color(t.ink))
        .child(glyph)
}

/// A small count on the rail button, e.g. the number of changes.
fn badge(text: SharedString, t: &Theme) -> impl IntoElement {
    div()
        .absolute()
        .top(px(4.))
        .right(px(6.))
        .px(px(4.))
        .rounded(px(8.))
        .bg(t.acc)
        .text_color(on_accent())
        .text_size(px(9.))
        .child(text)
}

impl Workspace {
    pub(super) fn rail(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self.sidebar_visible && !self.show_settings;
        let mut rail = div()
            .w(px(48.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .py(px(8.))
            .gap(px(4.))
            .bg(t.rail)
            .border_r_1()
            .border_color(t.line)
            .child(
                rail_button(
                    "rail-files",
                    "F",
                    shown && self.sidebar == Sidebar::Files,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| this.select_sidebar(Sidebar::Files, cx))),
            );
        for (id, inst) in self.registry.running() {
            if let Some(item) = inst.rail() {
                let count = inst.rail_badge(cx);
                let mut button = rail_button(
                    id,
                    item.glyph,
                    shown && self.sidebar == Sidebar::Addon(id),
                    t,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.select_sidebar(Sidebar::Addon(id), cx)),
                );
                if let Some(text) = count {
                    button = button.child(badge(text.into(), t));
                }
                rail = rail.child(button);
            }
        }
        rail.child(div().flex_1()).child(
            rail_button("rail-settings", "S", self.show_settings, t).on_click(cx.listener(
                |this, _, _, cx| {
                    this.show_settings = !this.show_settings;
                    cx.notify();
                },
            )),
        )
    }

    pub(super) fn tab_bar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div()
            .h(px(36.))
            .flex_none()
            .flex()
            .bg(t.side)
            .border_b_1()
            .border_color(t.line);
        for (i, tab) in self.tabs.tabs.iter().enumerate() {
            let active = i == self.tabs.active && !self.show_settings;
            let dirty = self.is_dirty(i, cx);
            let pinned = tab.pinned;
            let name: SharedString = match &tab.content {
                TabContent::Addon { title, .. } => title.clone(),
                _ => tab
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
                    .into(),
            };
            let group: SharedString = format!("tab-{i}").into();
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
                        .id(("close", i))
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
                                this.request_close(i, window, cx)
                            }),
                        )
                        .child("×"),
                );
            bar = bar.child(
                div()
                    .id(("tab", i))
                    .group(group)
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h_full()
                    .px(px(14.))
                    .cursor_pointer()
                    .text_color(if active { t.ink } else { t.mute })
                    .when(active, |d| d.bg(t.bg).border_t_1().border_color(t.acc))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            this.show_settings = false;
                            this.tabs.active = i;
                            this.focus_tab(window, cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        // ponytail: right-click toggles pin until the tab context menu lands.
                        cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                            this.tabs.toggle_pin(i);
                            cx.notify();
                        }),
                    )
                    .when(pinned, |d| d.child(div().text_color(t.acc).child("▪")))
                    .child(name)
                    .when(!pinned, |d| d.child(slot)),
            );
        }
        bar
    }

    pub(super) fn status_bar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut right = div().ml_auto().flex().gap(px(18.));
        if let Some(tab) = self.tabs.active_tab() {
            if let TabContent::Editor(e, _) = &tab.content {
                let s = &e.read(cx).state;
                let (l, c) = s.line_col(s.cursor);
                right = right.child(format!("Ln {}, Col {}", l + 1, c + 1));
                if s.read_only {
                    right = right.child("Read-only");
                }
            }
            right = right.child(language_for(&tab.path)).child("UTF-8");
        }
        let folder = self
            .folder
            .as_ref()
            .and_then(|f| f.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut bar = div()
            .h(px(24.))
            .flex_none()
            .flex()
            .items_center()
            .px(px(12.))
            .gap(px(18.))
            .bg(t.rail)
            .border_t_1()
            .border_color(t.line)
            .text_size(px(12.))
            .text_color(t.mute)
            .child(div().text_color(t.ink).child(SharedString::from(folder)));
        for (_, inst) in self.registry.running() {
            let Some(status) = inst.status(cx) else {
                continue;
            };
            bar = bar.child(
                div()
                    .flex()
                    .gap(px(5.))
                    .text_color(t.ink)
                    .child(SharedString::from(status.branch)),
            );
            if !status.detail.is_empty() {
                bar = bar.child(SharedString::from(status.detail));
            }
        }
        bar.when_some(self.error.clone(), |d, e| {
            d.child(div().text_color(t.del).child(e))
        })
        .child(right)
    }

    /// Modal shown when closing a tab with unsaved edits.
    pub(super) fn unsaved_prompt(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let i = self.pending_close?;
        let name = self
            .tabs
            .tabs
            .get(i)?
            .path
            .file_name()?
            .to_string_lossy()
            .into_owned();
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .px(px(14.))
                .py(px(6.))
                .rounded(px(4.))
                .cursor_pointer()
                .border_1()
                .border_color(t.line)
                .when(primary, |d| {
                    d.bg(t.acc).text_color(on_accent()).border_color(t.acc)
                })
                .when(!primary, |d| d.hover(|d| d.bg(t.hov)))
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
                        .w(px(420.))
                        .p(px(20.))
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .bg(t.side)
                        .border_1()
                        .border_color(t.line)
                        .rounded(px(8.))
                        .text_color(t.ink)
                        .child(
                            div()
                                .text_size(px(15.))
                                .child(format!("Save changes to {name}?")),
                        )
                        .child(
                            div()
                                .text_color(t.mute)
                                .child("Your changes will be lost if you don't save them."),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .child(button("cancel", "Cancel", false).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.pending_close = None;
                                        cx.notify();
                                    },
                                )))
                                .child(button("discard", "Don't save", false).on_click(
                                    cx.listener(|this, _, window: &mut Window, cx| {
                                        this.resolve_close(false, window, cx)
                                    }),
                                ))
                                .child(button("save", "Save", true).on_click(cx.listener(
                                    |this, _, window: &mut Window, cx| {
                                        this.resolve_close(true, window, cx)
                                    },
                                ))),
                        ),
                )
                .into_any_element(),
        )
    }
}
