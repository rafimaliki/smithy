//! The discard confirmation, as an overlay view so it can be drawn over the diff
//! tab that owns it.
use super::changes::ChangesView;
use crate::theme::{on_accent, Theme};
use gpui::{div, prelude::*, px, Context, Entity, Render, Rgba, SharedString, Window};

pub struct DiscardDialog {
    pub(super) changes: Entity<ChangesView>,
    pub(super) name: String,
    pub(super) detail: String,
}

fn button(
    id: &'static str,
    label: &str,
    theme: &Theme,
    primary: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(28.))
        .px(px(12.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .border_1()
        .border_color(theme.line)
        .cursor_pointer()
        .when(primary, |d| {
            d.bg(theme.del)
                .border_color(theme.del)
                .text_color(on_accent())
        })
        .when(!primary, |d| {
            d.text_color(theme.ink).hover(|d| d.bg(theme.hov))
        })
        .child(SharedString::from(label.to_string()))
}

impl Render for DiscardDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::by_name(&cx.global::<crate::settings::Settings>().theme);
        let changes = self.changes.clone();
        let card = div()
            .w(px(440.))
            .p(px(22.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .bg(theme.side)
            .border_1()
            .border_color(theme.line)
            .rounded(px(10.))
            .text_color(theme.ink)
            .child(div().text_size(px(15.)).child(SharedString::from(format!(
                "Discard changes to {}?",
                self.name
            ))))
            .child(
                div()
                    .text_color(theme.mute)
                    .child(SharedString::from(self.detail.clone())),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button("cancel-discard", "Cancel", &theme, false).on_click(cx.listener({
                            let changes = changes.clone();
                            move |_, _, _, cx| {
                                changes.update(cx, |c, cx| c.cancel_discard(cx));
                            }
                        })),
                    )
                    .child(
                        button("confirm-discard", "Discard changes", &theme, true).on_click(
                            cx.listener(move |_, _, _, cx| {
                                changes.update(cx, |c, cx| c.confirm_discard(cx));
                            }),
                        ),
                    ),
            );
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(Rgba {
                a: 0.5,
                ..gpui::rgba(0x000000ff)
            })
            .occlude()
            .child(card)
    }
}
