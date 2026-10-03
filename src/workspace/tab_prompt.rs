//! The `Closed <file>` toast and the unsaved-changes prompt: the two overlays the
//! tab bar raises. Design: board frames `reopen-toast`, `unsaved-prompt`. The bar
//! itself is in `tabs_view.rs`.
use super::Workspace;
use crate::keymap;
use crate::settings::Settings;
use crate::theme::{on_accent, Theme};
use gpui::{
    div, hsla, point, prelude::*, px, AnyElement, BoxShadow, Context, Div, SharedString, Stateful,
};

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
    /// The `Closed <file>` toast with its Reopen button, or `None`.
    pub(super) fn toast(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let toast = self.toast.as_ref()?;
        let text = toast.text.clone();
        let right = toast.right;
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
                .child(button("toast-reopen", "Reopen", false, t).on_click(
                    cx.listener(move |this, _, window, cx| this.reopen_last(right, window, cx)),
                ))
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
        let name = Self::tab_label(self.group(self.close_right), i);
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
