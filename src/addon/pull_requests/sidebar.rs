//! The Pull requests sidebar: shared widgets, the no-token and unavailable
//! states, and the view that picks which panel to draw (`panels.rs`).
use super::state::{Load, PrView};
use crate::github::CheckState;
use crate::settings::Settings;
use crate::theme::{on_accent, Theme};
use gpui::{div, prelude::*, px, AnyElement, Context, Render, Rgba, SharedString, Stateful};

pub(crate) fn tint(c: Rgba, a: f32) -> Rgba {
    Rgba { a, ..c }
}

/// The dot beside a pull request, from its checks.
pub(crate) fn dot_color(state: Option<CheckState>, t: &Theme) -> Rgba {
    match state {
        Some(CheckState::Success) => t.add,
        Some(CheckState::Failure) => t.del,
        Some(CheckState::Pending) => t.acc,
        _ => t.mute,
    }
}

pub(crate) fn small_button(
    id: &'static str,
    label: &str,
    t: &Theme,
    primary: bool,
) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(28.))
        .px(px(12.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .border_1()
        .border_color(t.line)
        .cursor_pointer()
        .when(primary, |d| {
            d.bg(t.acc).border_color(t.acc).text_color(on_accent())
        })
        .when(!primary, |d| d.text_color(t.ink).hover(|d| d.bg(t.hov)))
        .child(SharedString::from(label.to_string()))
}

pub(crate) fn chip(t: &Theme, label: &str, positive: bool) -> AnyElement {
    div()
        .px(px(8.))
        .h(px(20.))
        .flex()
        .items_center()
        .flex_none()
        .rounded(px(10.))
        .text_size(px(12.))
        .bg(if positive { tint(t.add, 0.22) } else { t.sel })
        .text_color(if positive { t.add } else { t.ink })
        .child(SharedString::from(label.to_string()))
        .into_any_element()
}

/// No repository, or its origin is not on GitHub: explain, no retry.
fn unavailable(t: &Theme, message: &str) -> AnyElement {
    div()
        .px(px(14.))
        .pt(px(16.))
        .flex()
        .flex_col()
        .text_color(t.mute)
        .child(
            div()
                .mb(px(6.))
                .text_color(t.ink)
                .child("Pull requests need a GitHub repository"),
        )
        .child(SharedString::from(message.to_string()))
        .into_any_element()
}

/// A message card, e.g. GitHub unreachable, with an optional action button.
pub(crate) fn notice(
    t: &Theme,
    title: &str,
    detail: &str,
    button: Option<AnyElement>,
) -> AnyElement {
    let mut card = div()
        .mx(px(12.))
        .p(px(12.))
        .rounded(px(6.))
        .bg(tint(t.del, 0.14))
        .flex()
        .flex_col()
        .child(
            div()
                .text_color(t.ink)
                .child(SharedString::from(title.to_string())),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(t.mute)
                .my(px(6.))
                .child(SharedString::from(detail.to_string())),
        );
    if let Some(button) = button {
        card = card.child(button);
    }
    card.into_any_element()
}

impl PrView {
    fn connect_panel(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div()
            .px(px(20.))
            .pt(px(30.))
            .flex()
            .flex_col()
            .text_color(t.mute)
            .child(div().mb(px(6.)).text_color(t.ink).child("Connect GitHub"))
            .child("Pull requests need a personal access token.");
        if let Some(ws) = self.workspace.clone() {
            col = col.child(div().mt(px(14.)).child(
                small_button("open-gh-settings", "Open GitHub settings", t, true).on_click(
                    cx.listener(move |_, _, _, cx| {
                        ws.update(cx, |w, cx| w.open_github_settings(cx)).ok();
                    }),
                ),
            ));
        }
        col.into_any_element()
    }
}

impl Render for PrView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::by_name(&cx.global::<Settings>().theme);
        self.sync(cx);
        let body = if self.review.is_some() {
            self.review_panel(&t, cx)
        } else if self.load == Load::NoToken {
            self.connect_panel(&t, cx)
        } else if let Load::Unavailable(message) = &self.load {
            unavailable(&t, message)
        } else if let Load::Error(message) = &self.load {
            let message = message.clone();
            let retry = small_button("retry-list", "Retry", &t, false)
                .on_click(cx.listener(|this, _, _, cx| this.reload(cx)))
                .into_any_element();
            notice(&t, "Could not reach GitHub", &message, Some(retry))
        } else {
            self.list_panel(&t, cx)
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .pt(px(2.))
            .text_color(t.ink)
            .text_size(px(13.))
            .font_family("Segoe UI")
            .child(body)
    }
}
