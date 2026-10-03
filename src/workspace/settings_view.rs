//! Settings page: appearance (theme, editor font size) and add-ons.
//! Keyboard shortcuts, GitHub token and languages sections are tasks in docs/tasks.md.
use super::Workspace;
use crate::theme::{on_accent, Theme, THEMES};
use gpui::{div, prelude::*, px, Context, SharedString, Stateful};

fn chip(id: impl Into<gpui::ElementId>, label: &str, on: bool, t: &Theme) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(12.))
        .py(px(6.))
        .rounded(px(4.))
        .cursor_pointer()
        .border_1()
        .border_color(if on { t.acc } else { t.line })
        .when(on, |d| d.bg(t.acc).text_color(on_accent()))
        .when(!on, |d| d.hover(|d| d.bg(t.hov)))
        .child(SharedString::from(label.to_string()))
}

fn heading(t: &Theme, text: &str) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .text_color(t.mute)
        .mt(px(24.))
        .mb(px(8.))
        .child(text.to_uppercase())
}

impl Workspace {
    pub(super) fn settings_view(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.settings(cx).clone();
        let infos = self.registry.infos();

        let mut themes = div().flex().gap(px(8.));
        for (i, th) in THEMES.iter().enumerate() {
            let name = th.name;
            themes = themes.child(chip(("theme", i), name, s.theme == name, t).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.update_settings(cx, |s| s.theme = name.into())
                }),
            ));
        }

        let size = s.font_size;
        let stepper = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(chip("font-minus", "−", false, t).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.update_settings(cx, |s| s.font_size = (size - 1.0).max(9.0))
                },
            )))
            .child(format!("{size:.0} px"))
            .child(chip("font-plus", "+", false, t).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.update_settings(cx, |s| s.font_size = (size + 1.0).min(28.0))
                },
            )));

        let mut addons = div().flex().flex_col().gap(px(10.));
        for (i, info) in infos.iter().enumerate() {
            let on = s.addon_enabled(info.id);
            let id = info.id;
            let needs = if info.requires.is_empty() {
                String::new()
            } else {
                format!(" Needs: {}.", info.requires.join(", "))
            };
            addons = addons.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(
                        div().flex().flex_col().child(info.name).child(
                            div()
                                .text_color(t.mute)
                                .child(SharedString::from(format!("{}{needs}", info.description))),
                        ),
                    )
                    .child(
                        chip(("addon", i), if on { "On" } else { "Off" }, on, t).on_click(
                            cx.listener(move |this, _, _, cx| this.set_addon(id, !on, cx)),
                        ),
                    ),
            );
        }

        div()
            .size_full()
            .id("settings-scroll")
            .overflow_y_scroll()
            .p(px(32.))
            .child(div().text_size(px(20.)).child("Settings"))
            .child(heading(t, "Theme"))
            .child(themes)
            .child(heading(t, "Editor font size"))
            .child(stepper)
            .child(heading(t, "Add-ons (off by default)"))
            .child(div().max_w(px(640.)).child(addons))
    }
}
