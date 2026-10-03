//! Rendering for the Search everywhere modal. State is in `everywhere.rs`.
use super::chrome::{self, Mark};
use super::{EverywhereView, Item, TABS};
use crate::keymap;
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent, Render, SharedString,
    Window,
};

impl Render for EverywhereView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        let chip = keymap::effective("SearchEverywhere", &cx.global::<Settings>().shortcuts)
            .first()
            .map(|keys| keymap::display(keys));

        // Tabs.
        let mut tabs = div()
            .flex()
            .gap(px(4.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(theme.line);
        for (kind, id, label) in TABS {
            tabs = tabs.child(
                div()
                    .id(id)
                    .px(px(12.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(self.tab == kind, |d| d.bg(theme.sel).text_color(theme.ink))
                    .when(self.tab != kind, |d| {
                        d.text_color(theme.mute).hover(|d| d.text_color(theme.ink))
                    })
                    .child(SharedString::from(label))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_tab(kind, cx))),
            );
        }

        // Results.
        let mut results = div()
            .id("se-results")
            .flex()
            .flex_col()
            .max_h(px(520.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll);
        let mut flat = 0;
        for (label, kind) in self.groups() {
            results = results.child(group_label(label, &theme));
            for item in self.items(kind) {
                let active = flat == self.selected;
                let row = match item {
                    Item::File(file) => {
                        let path = file.file.path.clone();
                        chrome::row(("se", flat), active, &theme)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                    this.open(&path, 0, window, cx)
                                }),
                            )
                            .child(chrome::file_icon(&file.file.name, &theme))
                            .child(chrome::marked_positions(
                                &file.file.name,
                                &file.positions,
                                Mark::Accent,
                                &theme,
                            ))
                            .child(location(&file.file.dir, false, &theme))
                    }
                    Item::Symbol(symbol) => {
                        let path = symbol.file.path.clone();
                        let line = symbol.line;
                        chrome::row(("se", flat), active, &theme)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                    this.open(&path, line, window, cx)
                                }),
                            )
                            .child(chrome::kind_badge(symbol.kind, &theme))
                            .child(chrome::marked_positions(
                                &symbol.name,
                                &symbol.positions,
                                Mark::Accent,
                                &theme,
                            ))
                            .child(location(
                                &format!("{}:{}", symbol.file.name, symbol.line + 1),
                                false,
                                &theme,
                            ))
                    }
                    Item::Hit(file, hit) => {
                        let path = file.file.path.clone();
                        let line = hit.line;
                        chrome::row(("se", flat), active, &theme)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                    this.open(&path, line, window, cx)
                                }),
                            )
                            .child(chrome::marked(
                                &hit.text,
                                &hit.ranges,
                                Mark::Background,
                                &theme,
                            ))
                            .child(location(
                                &format!("{}:{}", file.file.name, hit.line + 1),
                                true,
                                &theme,
                            ))
                    }
                };
                results = results.child(row);
                flat += 1;
            }
        }

        let footer = div()
            .flex()
            .gap(px(18.))
            .h(px(30.))
            .items_center()
            .px(px(16.))
            .border_t_1()
            .border_color(theme.line)
            .text_size(px(12.))
            .text_color(theme.mute)
            .child("Tab switch type")
            .child("Enter open")
            .child("Esc close");

        let panel = div()
            .mt(px(84.))
            .w(px(760.))
            .flex()
            .flex_col()
            .bg(theme.side)
            .border_1()
            .border_color(theme.line)
            .rounded(px(10.))
            .overflow_hidden()
            .text_color(theme.ink)
            .font_family("Segoe UI")
            .text_size(px(13.))
            .child(
                chrome::input_row("se-input", &self.focus, false, &theme, Self::input_key, cx)
                    .mx(px(0.))
                    .mb(px(0.))
                    .h(px(46.))
                    .px(px(16.))
                    .gap(px(10.))
                    .border_color(theme.line)
                    .child(div().text_color(theme.mute).child("⌕"))
                    .child(chrome::input_text(
                        &self.query,
                        "Search files, symbols and text",
                        13.,
                        &theme,
                    ))
                    .when_some(chip, |d, keys| {
                        d.child(
                            div()
                                .px(px(8.))
                                .rounded(px(10.))
                                .text_size(px(12.))
                                .text_color(theme.mute)
                                .child(SharedString::from(keys)),
                        )
                    }),
            )
            .child(tabs)
            .child(self.hint(&theme))
            .child(results)
            .child(footer);

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .id("se-backdrop")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .bg(chrome::tint(theme.bg, 0.55))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, _, cx| this.close(cx)),
                    ),
            )
            .child(panel)
            .into_any_element()
    }
}

/// A group heading inside the modal.
fn group_label(label: &str, theme: &Theme) -> AnyElement {
    div()
        .px(px(16.))
        .pt(px(10.))
        .pb(px(4.))
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(theme.mute)
        .child(SharedString::from(label.to_uppercase()))
        .into_any_element()
}

/// The muted location at the right of a row.
fn location(location: &str, align_right: bool, theme: &Theme) -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .text_size(px(12.))
        .text_color(theme.mute)
        .when(align_right, |d| d.text_right())
        .child(SharedString::from(location.to_string()))
        .into_any_element()
}
