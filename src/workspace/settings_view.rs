//! Settings page: the section nav plus Appearance and Add-ons. Keyboard shortcuts
//! and Languages live in their own modules.
use super::Workspace;
use crate::theme::{on_accent, Theme, THEMES};
use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Stateful};

/// Which settings section the pane shows.
#[derive(Clone, Copy, PartialEq)]
pub enum SettingsSection {
    Appearance,
    Addons,
    Shortcuts,
    Languages,
}

const NAV: &[(SettingsSection, &str)] = &[
    (SettingsSection::Appearance, "Appearance"),
    (SettingsSection::Addons, "Add-ons"),
    (SettingsSection::Shortcuts, "Keyboard shortcuts"),
    (SettingsSection::Languages, "Languages"),
];

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

/// A titled choice with the accent border when picked.
fn card(id: impl Into<gpui::ElementId>, title: &str, on: bool, t: &Theme) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .w(px(240.))
        .px(px(14.))
        .py(px(12.))
        .rounded(px(8.))
        .cursor_pointer()
        .border_1()
        .border_color(if on { t.acc } else { t.line })
        .when(on, |d| d.bg(t.sel))
        .when(!on, |d| d.hover(|d| d.bg(t.hov)))
        .child(SharedString::from(title.to_string()))
}

fn heading(t: &Theme, text: &str) -> impl IntoElement {
    div()
        .text_size(px(14.))
        .text_color(t.ink)
        .mt(px(24.))
        .child(text.to_string())
}

fn hint(t: &Theme, text: &str) -> impl IntoElement {
    div()
        .text_size(px(12.))
        .text_color(t.mute)
        .mt(px(4.))
        .child(text.to_string())
}

impl Workspace {
    pub(crate) fn select_settings_section(
        &mut self,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) {
        self.settings_section = section;
        self.capture = None;
        cx.notify();
    }

    pub(super) fn settings_view(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(self.settings_nav(t, cx))
            .child(
                div()
                    .id("settings-pane")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .p(px(36.))
                    .child(self.settings_section(t, cx)),
            )
    }

    fn settings_nav(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut nav = div()
            .w(px(200.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(t.side)
            .border_r_1()
            .border_color(t.line)
            .child(
                div()
                    .px(px(14.))
                    .pt(px(14.))
                    .pb(px(10.))
                    .text_size(px(11.))
                    .text_color(t.mute)
                    .child("SETTINGS"),
            );
        for (section, label) in NAV {
            let on = self.settings_section == *section;
            nav =
                nav.child(
                    div()
                        .id(*label)
                        .mx(px(8.))
                        .px(px(8.))
                        .h(px(26.))
                        .rounded(px(4.))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_color(if on { t.ink } else { t.mute })
                        .when(on, |d| d.bg(t.sel))
                        .when(!on, |d| d.hover(|d| d.bg(t.hov)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_settings_section(*section, cx)
                        }))
                        .child(*label),
                );
        }
        nav
    }

    fn settings_section(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        match self.settings_section {
            SettingsSection::Appearance => self.appearance_section(t, cx),
            SettingsSection::Addons => self.addons_section(t, cx),
            SettingsSection::Shortcuts => self.shortcuts_section(t, cx),
            SettingsSection::Languages => self.languages_section(t, cx),
        }
    }

    fn appearance_section(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut themes = div().flex().gap(px(8.)).mt(px(8.));
        for (i, th) in THEMES.iter().enumerate() {
            let name = th.name;
            themes = themes.child(
                chip(("theme", i), name, self.settings(cx).theme == name, t).on_click(cx.listener(
                    move |this, _, _, cx| this.update_settings(cx, |s| s.theme = name.into()),
                )),
            );
        }

        let icons = self.settings(cx).file_icons.clone();
        let icon_cards = div()
            .flex()
            .gap(px(12.))
            .mt(px(8.))
            .child(
                card("icons-brand", "By file type", icons == "brand", t).on_click(cx.listener(
                    |this, _, _, cx| this.update_settings(cx, |s| s.file_icons = "brand".into()),
                )),
            )
            .child(
                card("icons-simple", "Simple", icons == "simple", t).on_click(cx.listener(
                    |this, _, _, cx| this.update_settings(cx, |s| s.file_icons = "simple".into()),
                )),
            );

        let layout = self.settings(cx).diff_layout.clone();
        let layout_pick = div()
            .flex()
            .gap(px(8.))
            .mt(px(8.))
            .child(
                chip("diff-inline", "Inline", layout == "inline", t).on_click(cx.listener(
                    |this, _, _, cx| this.update_settings(cx, |s| s.diff_layout = "inline".into()),
                )),
            )
            .child(
                chip("diff-split", "Side by side", layout == "split", t).on_click(cx.listener(
                    |this, _, _, cx| this.update_settings(cx, |s| s.diff_layout = "split".into()),
                )),
            );

        let size = self.settings(cx).font_size;
        let stepper = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .mt(px(8.))
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

        div()
            .flex()
            .flex_col()
            .child(div().text_size(px(20.)).child("Appearance"))
            .child(heading(t, "Theme"))
            .child(hint(t, "Applies instantly. Stored on this machine."))
            .child(themes)
            .child(heading(t, "File icons"))
            .child(hint(
                t,
                "Sidebar and tabs. Unlisted file types always get the generic icon.",
            ))
            .child(icon_cards)
            .child(heading(t, "Default diff layout"))
            .child(hint(t, "Can be switched per diff from its toolbar."))
            .child(layout_pick)
            .child(heading(t, "Editor font size"))
            .child(hint(t, "Applies to code and diffs."))
            .child(stepper)
            .into_any_element()
    }

    fn addons_section(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let s = self.settings(cx).clone();
        let infos = self.registry.infos();

        let mut addons = div().flex().flex_col().gap(px(10.)).mt(px(14.));
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
            .flex()
            .flex_col()
            .child(div().text_size(px(20.)).child("Add-ons"))
            .child(hint(
                t,
                "Off means not loaded and no memory; its rail button and menus are hidden. \
                 Core is always on.",
            ))
            .child(div().max_w(px(640.)).child(addons))
            .into_any_element()
    }
}
