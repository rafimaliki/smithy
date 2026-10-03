use super::{Sidebar, TabContent, Workspace};
use crate::settings::{SIDEBAR_DEFAULT, SIDEBAR_MIN};
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Render, Window,
};

const RAIL_W: f32 = 48.0;

impl Workspace {
    fn sidebar_panel(&self, t: &Theme, width: f32, collapsing: bool) -> AnyElement {
        let (title, body): (&str, AnyElement) = match self.sidebar {
            Sidebar::Files => (
                "Explorer",
                match &self.tree {
                    Some(tree) => div()
                        .flex_1()
                        .min_h_0()
                        .child(tree.clone())
                        .into_any_element(),
                    None => div().into_any_element(),
                },
            ),
            Sidebar::Addon(id) => {
                let item = self.registry.instance(id).and_then(|i| i.rail());
                let view = self.registry.instance(id).and_then(|i| i.sidebar_view());
                (
                    item.map(|i| i.title).unwrap_or(""),
                    div()
                        .flex_1()
                        .min_h_0()
                        .when_some(view, |d, v| d.child(v))
                        .into_any_element(),
                )
            }
        };
        div()
            .w(px(width))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(t.side)
            .when(collapsing, |d| d.opacity(0.4))
            .child(
                div()
                    .px(px(14.))
                    .pt(px(14.))
                    .pb(px(10.))
                    .text_size(px(11.))
                    .text_color(t.mute)
                    .child(title.to_uppercase()),
            )
            .child(body)
            .into_any_element()
    }

    fn divider(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sidebar-divider")
            .w(px(4.))
            .h_full()
            .flex_none()
            .cursor_col_resize()
            .border_l_1()
            .border_color(t.line)
            .hover(|d| d.bg(t.acc).opacity(0.5))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                    if e.click_count == 2 {
                        this.update_settings(cx, |s| s.sidebar_width = SIDEBAR_DEFAULT);
                    } else {
                        let w = this.settings(cx).sidebar_width;
                        this.drag = Some(w);
                    }
                }),
            )
    }

    fn content(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        if self.show_settings {
            return self.settings_view(t, cx).into_any_element();
        }
        match self.tabs.active_tab().map(|tab| &tab.content) {
            Some(TabContent::Editor(e, _)) => e.clone().into_any_element(),
            Some(TabContent::Viewer(v)) => v.clone().into_any_element(),
            Some(TabContent::Notice(msg)) => centered(t, msg.clone()).into_any_element(),
            None => centered(t, "Open a file from the explorer.".into()).into_any_element(),
        }
    }
}

fn centered(t: &Theme, msg: gpui::SharedString) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(t.mute)
        .child(msg)
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::by_name(&cx.global::<crate::settings::Settings>().theme);
        let root = div()
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_open_folder))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_prev_tab))
            .on_action(cx.listener(Self::on_reopen_tab))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_quit))
            .relative()
            .size_full()
            .bg(t.bg)
            .text_color(t.ink)
            .font_family("Segoe UI")
            .text_size(px(13.));

        if self.folder.is_none() {
            return root.child(self.launch(&t, cx));
        }

        let saved_width = cx.global::<crate::settings::Settings>().sidebar_width;
        let raw = self.drag.unwrap_or(saved_width);
        let width = Self::clamp_width(raw);
        let collapsing = self.drag.is_some() && raw < SIDEBAR_MIN;

        root.on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
            if this.drag.is_some() && e.pressed_button == Some(MouseButton::Left) {
                this.drag = Some(f32::from(e.position.x) - RAIL_W);
                cx.notify();
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, _, cx| {
                if let Some(raw) = this.drag.take() {
                    if raw < SIDEBAR_MIN {
                        this.sidebar_visible = false;
                        cx.notify();
                    } else {
                        let w = Self::clamp_width(raw);
                        this.update_settings(cx, |s| s.sidebar_width = w);
                    }
                }
            }),
        )
        .child(
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .child(self.rail(&t, cx))
                        .when(self.sidebar_visible && !self.show_settings, |d| {
                            d.child(self.sidebar_panel(&t, width, collapsing))
                                .child(self.divider(&t, cx))
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(self.tab_bar(&t, cx))
                                .child(div().flex_1().min_h_0().child(self.content(&t, cx))),
                        ),
                )
                .child(self.status_bar(&t, cx)),
        )
        .children(self.lang_menu(&t, cx))
        .children(self.unsaved_prompt(&t, cx))
    }
}
