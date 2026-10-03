//! Settings > Languages: the built-in languages, picked from the file extension.
//! The optional language-server column belongs to the Language servers add-on.
use super::Workspace;
use crate::editor::lang::built_in;
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};

impl Workspace {
    pub(super) fn languages_section(&self, t: &Theme, _cx: &mut Context<Self>) -> AnyElement {
        let langs = built_in();
        let count = langs.len();

        let header = div()
            .flex()
            .items_center()
            .h(px(28.))
            .px(px(12.))
            .text_size(px(11.))
            .text_color(t.mute)
            .child(div().w(px(160.)).child("Language".to_uppercase()))
            .child(div().flex_1().child("Files".to_uppercase()));

        let mut rows = div().flex().flex_col();
        for (i, (name, exts)) in langs.iter().enumerate() {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .h(px(30.))
                    .px(px(12.))
                    .border_t_1()
                    .border_color(t.line)
                    .when(i % 2 == 1, |d| d.bg(t.side))
                    .child(
                        div()
                            .w(px(160.))
                            .text_color(t.ink)
                            .child(SharedString::from(*name)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_family("Cascadia Mono")
                            .text_size(px(12.))
                            .text_color(t.mute)
                            .child(SharedString::from(exts.clone())),
                    ),
            );
        }

        div()
            .flex()
            .flex_col()
            .child(div().text_size(px(20.)).child("Languages"))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(t.mute)
                    .mt(px(4.))
                    .child(SharedString::from(format!(
                        "Syntax highlighting for {count} languages, chosen by file extension."
                    ))),
            )
            .child(div().w(px(720.)).mt(px(22.)).child(header).child(rows))
            .child(
                div()
                    .max_w(px(720.))
                    .mt(px(22.))
                    .text_size(px(13.))
                    .text_color(t.mute)
                    .child(
                        "The language of a file is picked automatically from its extension. \
                         Highlighting is built in and loads when a file of that type opens. \
                         Unknown extensions open as plain text.",
                    ),
            )
            .into_any_element()
    }
}
