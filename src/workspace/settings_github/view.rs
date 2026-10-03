//! Markup for Settings > GitHub, split from the entity so each file stays small.
use super::GithubSettings;
use crate::theme::{on_accent, Theme};
use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton, Rgba, SharedString};

fn hint(t: &Theme, text: &str) -> AnyElement {
    div()
        .mb(px(18.))
        .text_size(px(12.))
        .text_color(t.mute)
        .child(SharedString::from(text.to_string()))
        .into_any_element()
}

/// Paste a token and connect.
pub(super) fn form(
    view: &GithubSettings,
    t: &Theme,
    cx: &mut Context<GithubSettings>,
) -> AnyElement {
    let masked: String = "•".repeat(view.input.chars().count());
    let shown = if masked.is_empty() {
        SharedString::from("Paste a token")
    } else {
        SharedString::from(masked)
    };
    div()
        .flex()
        .flex_col()
        .child(div().text_size(px(20.)).child("GitHub"))
        .child(hint(
            t,
            "Connect to list pull requests and fetch their branches.",
        ))
        .child(div().mb(px(8.)).child("Personal access token"))
        .child(
            div()
                .id("token-field")
                .track_focus(&view.focus)
                .on_key_down(cx.listener(GithubSettings::token_key))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, _cx| window.focus(&this.focus)),
                )
                .w(px(560.))
                .h(px(34.))
                .px(px(12.))
                .flex()
                .items_center()
                .border_1()
                .border_color(t.line)
                .rounded(px(6.))
                .bg(t.bg)
                .font_family(crate::editor::view::MONO)
                .text_color(if view.input.is_empty() { t.mute } else { t.ink })
                .child(shown),
        )
        .child(
            div()
                .w(px(560.))
                .my(px(18.))
                .text_size(px(12.))
                .text_color(t.mute)
                .child("A fine-grained token with read access to Pull requests and Contents. Saved in Windows Credential Manager, never in files or git."),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .id("connect")
                        .h(px(28.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .bg(t.acc)
                        .text_color(on_accent())
                        .when(!view.connecting, |d| {
                            d.on_click(cx.listener(|this, _, _, cx| this.connect(cx)))
                        })
                        .child(if view.connecting {
                            "Connecting…"
                        } else {
                            "Connect"
                        }),
                )
                .child(
                    div()
                        .id("create-token")
                        .h(px(28.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(t.line)
                        .cursor_pointer()
                        .hover(|d| d.bg(t.hov))
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.open_url("https://github.com/settings/tokens")
                        }))
                        .child("Create token on GitHub"),
                ),
        )
        .when_some(view.error.clone(), |d, e| {
            d.child(div().mt(px(14.)).text_color(t.del).child(e))
        })
        .into_any_element()
}

/// The account the token belongs to, with Replace and Remove.
pub(super) fn connected(
    view: &GithubSettings,
    t: &Theme,
    login: &str,
    cx: &mut Context<GithubSettings>,
) -> AnyElement {
    let initial = login
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase().to_string())
        .unwrap_or_default();
    let tail = view.tail.clone().unwrap_or_default();
    div()
        .flex()
        .flex_col()
        .child(div().text_size(px(20.)).child("GitHub"))
        .child(hint(
            t,
            "Used to list pull requests and fetch their branches.",
        ))
        .child(
            div()
                .w(px(640.))
                .p(px(16.))
                .border_1()
                .border_color(t.line)
                .rounded(px(10.))
                .bg(t.side)
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(
                            div()
                                .w(px(36.))
                                .h(px(36.))
                                .flex_none()
                                .rounded_full()
                                .bg(t.acc)
                                .text_color(on_accent())
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(initial),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(SharedString::from(if login.is_empty() {
                                    "Connected".to_string()
                                } else {
                                    login.to_string()
                                }))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(t.mute)
                                        .child(SharedString::from(format!(
                                            "Token ending {tail} · stored in Windows Credential Manager"
                                        ))),
                                ),
                        )
                        .child(
                            div()
                                .px(px(8.))
                                .h(px(20.))
                                .flex()
                                .items_center()
                                .flex_none()
                                .rounded(px(10.))
                                .bg(Rgba { a: 0.22, ..t.add })
                                .text_color(t.add)
                                .text_size(px(12.))
                                .child("Connected"),
                        ),
                )
                .child(
                    div()
                        .mt(px(16.))
                        .flex()
                        .gap(px(10.))
                        .child(
                            div()
                                .id("replace-token")
                                .h(px(28.))
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .rounded(px(6.))
                                .border_1()
                                .border_color(t.line)
                                .cursor_pointer()
                                .hover(|d| d.bg(t.hov))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.replacing = true;
                                    this.input.clear();
                                    cx.notify();
                                }))
                                .child("Replace token"),
                        )
                        .child(
                            div()
                                .id("remove-token")
                                .h(px(28.))
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .rounded(px(6.))
                                .border_1()
                                .border_color(t.line)
                                .cursor_pointer()
                                .text_color(t.del)
                                .hover(|d| d.bg(t.hov))
                                .on_click(cx.listener(|this, _, _, cx| this.remove(cx)))
                                .child("Remove"),
                        ),
                ),
        )
        .into_any_element()
}
