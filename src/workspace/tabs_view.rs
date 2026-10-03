//! The tab bar, the `Closed <file>` toast and the unsaved-changes prompt. The
//! bar's right-click menu is in `tab_menu.rs`. Design: board frames
//! `files-tabs-pinned`, `reopen-toast`, `unsaved-prompt`.
use super::Workspace;
use crate::keymap;
use crate::settings::Settings;
use crate::theme::{on_accent, Theme};
use gpui::{
    div, hsla, point, prelude::*, px, AnyElement, BoxShadow, Context, Div, MouseButton,
    MouseDownEvent, SharedString, Stateful,
};

const PIN: &str = "icons/pin.svg";

fn pin_mark(t: &Theme) -> AnyElement {
    gpui::svg()
        .path(PIN)
        .w(px(12.))
        .h(px(12.))
        .flex_none()
        .text_color(t.mute)
        .into_any_element()
}

/// A dialog/toast button: accent when primary, outlined otherwise.
fn button(id: &'static str, label: &'static str, primary: bool, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.))
        .px(px(12.))
        .rounded(px(6.))
        .flex_none()
        .cursor_pointer()
        .border_1()
        .border_color(if primary { t.acc } else { t.line })
        .when(primary, |d| d.bg(t.acc).text_color(on_accent()))
        .when(!primary, |d| d.hover(|d| d.bg(t.hov)))
        .child(label)
}

impl Workspace {
    pub(super) fn tab_bar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div()
            .h(px(36.))
            .flex_none()
            .flex()
            .bg(t.side)
            .border_b_1()
            .border_color(t.line);
        let pinned_count = self.tabs.tabs.iter().filter(|t| t.pinned).count();
        for (i, tab) in self.tabs.tabs.iter().enumerate() {
            let active = i == self.tabs.active && !self.show_settings;
            let dirty = self.is_dirty(i, cx);
            let pinned = tab.pinned;
            let name = self.tab_label(i);
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
            // Pinned tabs are compact, carry a pin mark and have no close button.
            let mut item = div()
                .id(("tab", i))
                .group(group)
                .flex()
                .items_center()
                .h_full()
                .gap(px(if pinned { 6. } else { 8. }))
                .px(px(if pinned { 12. } else { 14. }))
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
                    cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                        this.tab_menu = Some(super::tabs::TabMenu {
                            index: i,
                            at: e.position,
                        });
                        cx.notify();
                    }),
                );
            if pinned {
                item = item.child(pin_mark(t)).child(name);
            } else {
                item = item.child(name).child(slot);
            }
            bar = bar.child(item);
            // A hairline between the pinned block and the rest.
            if pinned && i + 1 == pinned_count && pinned_count < self.tabs.tabs.len() {
                bar = bar.child(
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
        bar
    }

    /// The `Closed <file>` toast with its Reopen button, or `None`.
    pub(super) fn toast(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let toast = self.toast.as_ref()?;
        let text = toast.text.clone();
        let hint = self.reopen_hint(cx);
        Some(
            div()
                .absolute()
                .left(px(16.))
                .bottom(px(14.))
                .flex()
                .items_center()
                .gap(px(14.))
                .px(px(14.))
                .py(px(10.))
                .bg(t.side)
                .border_1()
                .border_color(t.line)
                .rounded(px(8.))
                .shadow(vec![BoxShadow {
                    color: hsla(0., 0., 0., 0.5),
                    offset: point(px(0.), px(8.)),
                    blur_radius: px(28.),
                    spread_radius: px(0.),
                }])
                .text_color(t.ink)
                .occlude()
                .child(text)
                .child(
                    button("toast-reopen", "Reopen", false, t)
                        .on_click(cx.listener(|this, _, window, cx| this.reopen_last(window, cx))),
                )
                .when_some(hint, |d, h| {
                    d.child(div().text_size(px(12.)).text_color(t.mute).child(h))
                })
                .into_any_element(),
        )
    }

    /// The key that reopens a closed tab right now, if it is bound.
    fn reopen_hint(&self, cx: &gpui::App) -> Option<SharedString> {
        keymap::effective("ReopenTab", &cx.global::<Settings>().shortcuts)
            .first()
            .map(|k| keymap::display(k).into())
    }

    /// Modal shown while closing a tab with unsaved edits.
    pub(super) fn unsaved_prompt(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let i = self.pending_close?;
        let name = self.tab_label(i);
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
                        .bg(t.side)
                        .border_1()
                        .border_color(t.line)
                        .rounded(px(10.))
                        .text_color(t.ink)
                        .child(
                            div()
                                .text_size(px(15.))
                                .child(format!("Do you want to save the changes to {name}?")),
                        )
                        .child(
                            div()
                                .mt(px(8.))
                                .text_color(t.mute)
                                .child("Your changes will be lost if you do not save them."),
                        )
                        .child(
                            div()
                                .mt(px(20.))
                                .flex()
                                .justify_end()
                                .gap(px(10.))
                                .child(button("dont-save", "Don't save", false, t).on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.resolve_close(false, window, cx)
                                    }),
                                ))
                                .child(button("cancel", "Cancel", false, t).on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.cancel_close(window, cx)
                                    }),
                                ))
                                .child(button("save", "Save", true, t).on_click(cx.listener(
                                    |this, _, window, cx| this.resolve_close(true, window, cx),
                                ))),
                        ),
                )
                .into_any_element(),
        )
    }
}
