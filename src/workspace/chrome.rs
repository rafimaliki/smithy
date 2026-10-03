//! Workbench chrome: the activity rail and the status bar. The tab bar, its menu,
//! the reopen toast and the unsaved-changes prompt live in `tabs_view.rs`.
use super::{Sidebar, TabContent, Workspace};
use crate::theme::{on_accent, Theme};
use gpui::{
    div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent, SharedString, Stateful,
};

fn rail_button(id: &'static str, icon: &'static str, on: bool, t: &Theme) -> Stateful<gpui::Div> {
    // An svg is painted in its own text color, not the parent's, so the hover tone
    // is set on the svg through the group.
    let color = if on { t.ink } else { t.mute };
    div()
        .id(id)
        .group(id)
        .relative()
        .w(px(48.))
        .h(px(40.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(on, |d| d.border_l_2().border_color(t.acc))
        .child(
            gpui::svg()
                .path(icon)
                .w(px(20.))
                .h(px(20.))
                .flex_none()
                .text_color(color)
                .group_hover(id, |s| s.text_color(t.ink)),
        )
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
    /// The first add-on overlay that is open, drawn over the whole window.
    pub(super) fn addon_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.registry
            .running()
            .find_map(|(_, inst)| inst.overlay(cx))
            .map(IntoElement::into_any_element)
    }

    pub(super) fn rail(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self.sidebar_visible;
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
                    "icons/rail-files.svg",
                    shown && self.sidebar == Sidebar::Files,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| this.select_sidebar(Sidebar::Files, cx))),
            );
        // Registry order is not stable (a map), so the rail order is set here, as on the board.
        let mut running: Vec<_> = self.registry.running().collect();
        running.sort_by_key(|(id, _)| match *id {
            "search" => 0,
            "source-control" => 1,
            "pull-requests" => 2,
            _ => 3,
        });
        for (id, inst) in running {
            if let Some(item) = inst.rail() {
                let count = inst.rail_badge(cx);
                let mut button = rail_button(
                    id,
                    item.icon,
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
            rail_button(
                "rail-settings",
                "icons/rail-settings.svg",
                self.show_settings,
                t,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.show_settings = !this.show_settings;
                this.capture = None;
                cx.notify();
            })),
        )
    }

    pub(super) fn status_bar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut right = div().ml_auto().flex().gap(px(18.));
        if let Some(tab) = self.focused().active_tab() {
            if let TabContent::Editor(e, _) = &tab.content {
                let wraps = e.read(cx).wraps();
                let editor = e.clone();
                right = right.child(
                    div()
                        .id("status-wrap")
                        .cursor_pointer()
                        .text_color(t.mute)
                        .hover(|d| d.text_color(t.ink))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_, _: &MouseDownEvent, _, cx| {
                                editor.update(cx, |view, cx| {
                                    let on = !view.wraps();
                                    view.set_wrap(on, cx);
                                });
                            }),
                        )
                        .child(if wraps { "Wrap on" } else { "Wrap off" }),
                );
                let s = &e.read(cx).state;
                let (l, c) = s.line_col(s.cursor);
                right = right.child(format!("Ln {}, Col {}", l + 1, c + 1));
                if s.read_only {
                    right = right.child("Read-only");
                }
            }
            right = right.child(self.status_language(t, cx)).child("UTF-8");
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
}
