//! The language picker: the status-bar language opens a list that overrides the
//! language for the current file only. Design: board frame `lang-picker`.
use super::{TabContent, Workspace};
use crate::editor::lang::Lang;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyElement, Context, Div, MouseButton, MouseDownEvent, SharedString,
    Stateful,
};

const PANEL_W: f32 = 240.0;

impl Workspace {
    /// The status-bar language label; clicking it opens the picker.
    pub(super) fn status_language(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("status-language")
            .cursor_pointer()
            .hover(|d| d.text_color(t.ink))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.lang_menu = !this.lang_menu;
                    cx.notify();
                }),
            )
            .child(SharedString::from(Lang::display(self.active_lang(cx))))
            .into_any_element()
    }

    fn active_lang(&self, cx: &Context<Self>) -> Option<Lang> {
        match self.tabs.active_tab().map(|t| &t.content) {
            Some(TabContent::Editor(e, _)) => e.read(cx).lang(),
            _ => None,
        }
    }

    fn choose_lang(&mut self, lang: Option<Lang>, cx: &mut Context<Self>) {
        if let Some(TabContent::Editor(e, _)) = self.tabs.active_tab().map(|t| &t.content) {
            e.update(cx, |editor, cx| editor.set_lang(lang, cx));
        }
        self.lang_menu = false;
        cx.notify();
    }

    /// The picker overlay, or `None` while it is closed.
    pub(super) fn lang_menu(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.lang_menu {
            return None;
        }
        let current = self.active_lang(cx);
        let mut list = div()
            .id("lang-menu-list")
            .flex()
            .flex_col()
            .max_h(px(560.))
            .overflow_y_scroll()
            .py(px(4.));
        list = list.child(
            item(
                SharedString::from("lang-plain"),
                "Plain text".into(),
                current.is_none(),
                t,
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| this.choose_lang(None, cx)),
            ),
        );
        for lang in Lang::ALL {
            list = list.child(
                item(
                    SharedString::from(format!("lang-{}", lang.name())),
                    lang.name().into(),
                    current == Some(lang),
                    t,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.choose_lang(Some(lang), cx)
                    }),
                ),
            );
        }
        let panel = div()
            .absolute()
            .right(px(12.))
            .bottom(px(28.))
            .w(px(PANEL_W))
            .flex()
            .flex_col()
            .bg(t.side)
            .border_1()
            .border_color(t.line)
            .rounded(px(8.))
            .child(
                div()
                    .h(px(32.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .text_color(t.mute)
                    .child("Select language mode"),
            )
            .child(div().border_t_1().border_color(t.line))
            .child(list);
        // Clicking anywhere else closes the menu without reaching what is behind it.
        let backdrop = div()
            .id("lang-menu-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.lang_menu = false;
                    cx.notify();
                }),
            );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(backdrop)
                .child(panel)
                .into_any_element(),
        )
    }
}

/// One row of the picker: the language name, and `current` on the active one.
fn item(id: SharedString, label: SharedString, current: bool, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(28.))
        .px(px(12.))
        .cursor_pointer()
        .when(current, |d| d.bg(t.sel))
        .hover(|d| d.bg(t.hov))
        .child(label)
        .when(current, |d| {
            d.child(div().ml_auto().text_color(t.mute).child("current"))
        })
}
