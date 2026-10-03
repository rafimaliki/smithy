//! The panel header: session tabs, the new / shell / hide buttons, and the shell
//! picker drawn under its button.

use super::super::pty::Shell;
use super::{TerminalView, HEAD_H};
use crate::theme::Theme;
use crate::workspace::menu;
use gpui::{div, prelude::*, px, AnyElement, Context, Div, MouseButton, SharedString, Stateful};

impl TerminalView {
    /// The session tabs and the new / shell / hide buttons.
    pub(super) fn header(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut header = div()
            .h(px(HEAD_H))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(10.))
            .bg(t.side)
            .border_b_1()
            .border_color(t.line);
        for i in 0..self.sessions.len() {
            let active = i == self.active;
            let label: SharedString = self.tab_label(i).into();
            header = header.child(
                div()
                    .id(("term-tab", i))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(24.))
                    .px(px(10.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .whitespace_nowrap()
                    .when(active, |d| d.bg(t.sel).text_color(t.ink))
                    .when(!active, |d| {
                        d.text_color(t.mute).hover(|d| d.text_color(t.ink))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.active = i;
                            this.scroll = 0;
                            this.shell_menu = false;
                            window.focus(&this.focus);
                            cx.notify();
                        }),
                    )
                    .child(label)
                    .child(
                        div()
                            .id(("term-close-tab", i))
                            .px(px(3.))
                            .text_color(t.mute)
                            .hover(|d| d.text_color(t.del))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.close_session(i, cx);
                                }),
                            )
                            .child("×"),
                    ),
            );
        }
        header
            .child(head_button("term-new", "+", t).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    let cwd = this.root.clone();
                    this.shell_menu = false;
                    this.open_session(Shell::PowerShell, cwd, window, cx);
                }),
            ))
            .child(head_button("term-shell", "⌄", t).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.shell_menu = !this.shell_menu;
                    cx.notify();
                }),
            ))
            .child(div().flex_1())
            .child(head_button("term-hide", "×", t).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close_panel(cx)),
            ))
    }

    /// The name on the tab: the shell, numbered when that shell is already open.
    fn tab_label(&self, i: usize) -> String {
        let shell = self.sessions[i].session.shell();
        let before = self.sessions[..i]
            .iter()
            .filter(|o| o.session.shell() == shell)
            .count();
        if before == 0 {
            shell.label().to_string()
        } else {
            format!("{} {}", shell.label(), before + 1)
        }
    }

    /// The shell picker, drawn under its header button.
    pub(super) fn shell_menu(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.shell_menu {
            return None;
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        for shell in Shell::ALL {
            rows.push(
                menu::item(
                    ("term-shell-item", shell as usize),
                    shell.label(),
                    None,
                    false,
                    true,
                    t,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let cwd = this.root.clone();
                    this.shell_menu = false;
                    this.open_session(shell, cwd, window, cx);
                }))
                .into_any_element(),
            );
        }
        Some(
            div()
                .absolute()
                .left(px(52.))
                .top(px(HEAD_H - 2.))
                .w(px(menu::WIDTH))
                .p(px(5.))
                .bg(t.side)
                .border_1()
                .border_color(t.line)
                .rounded(px(8.))
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.shell_menu = false;
                    cx.notify();
                }))
                .children(rows)
                .into_any_element(),
        )
    }
}

/// A header icon button: a text glyph, as the rail and the tab bar use.
fn head_button(id: &'static str, glyph: &'static str, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .cursor_pointer()
        .text_color(t.mute)
        .hover(|d| d.bg(t.hov).text_color(t.ink))
        .child(glyph)
}
