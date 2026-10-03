//! Settings > Languages: the built-in languages, picked from the file extension,
//! and the language server each one may use. The server column is the Language
//! servers add-on's, so it is empty while that add-on is off.
use super::Workspace;
use crate::addon::{LanguageServerRow, ServerState};
use crate::editor::lang::built_in;
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Stateful};

impl Workspace {
    /// The server rows every running add-on offers.
    fn language_server_rows(&self) -> Vec<LanguageServerRow> {
        self.registry
            .running()
            .find_map(|(_, instance)| {
                let rows = instance.language_servers();
                (!rows.is_empty()).then_some(rows)
            })
            .unwrap_or_default()
    }

    fn toggle_language_server(&mut self, id: &'static str, cx: &mut Context<Self>) {
        for (_, instance) in self.registry.running() {
            if instance.language_servers().iter().any(|r| r.id == id) {
                instance.toggle_language_server(id, cx);
                break;
            }
        }
    }

    pub(super) fn languages_section(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let langs = built_in();
        let servers = self.language_server_rows();
        let count = langs.len();

        let header = div()
            .flex()
            .items_center()
            .h(px(28.))
            .px(px(12.))
            .text_size(px(11.))
            .text_color(t.mute)
            .child(div().w(px(160.)).child("Language".to_uppercase()))
            .child(div().w(px(150.)).child("Files".to_uppercase()))
            .child(div().flex_1().child("Language server".to_uppercase()))
            .child(
                div()
                    .w(px(100.))
                    .text_right()
                    .child("Status".to_uppercase()),
            );

        let mut rows = div().flex().flex_col();
        for (i, (name, exts)) in langs.iter().enumerate() {
            let server = servers.iter().find(|s| s.language == *name);
            let mut row = div()
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
                        .w(px(150.))
                        .font_family("Cascadia Mono")
                        .text_size(px(12.))
                        .text_color(t.mute)
                        .child(SharedString::from(exts.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .font_family("Cascadia Mono")
                        .text_size(px(12.))
                        .text_color(t.mute)
                        .child(SharedString::from(
                            server.map(|s| s.server).unwrap_or_default(),
                        )),
                );
            let status = div().w(px(100.)).flex().justify_end();
            row = match server {
                Some(server) => {
                    let id = server.id;
                    row.child(status.child(state_chip(id, server.state, t).on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_language_server(id, cx)),
                    )))
                }
                None => row.child(status),
            };
            rows = rows.child(row);
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
                        "Syntax highlighting for {count} languages, plus optional language servers."
                    ))),
            )
            .child(div().w(px(860.)).mt(px(22.)).child(header).child(rows))
            .child(
                div()
                    .max_w(px(860.))
                    .mt(px(22.))
                    .text_size(px(13.))
                    .text_color(t.mute)
                    .child(
                        "The language of a file is picked automatically from its extension. \
                         Highlighting is built in and loads when a file of that type opens. \
                         Language servers add go to definition, find references and go back and \
                         forward. Smithy does not bundle servers: it uses ones already installed, \
                         starts one only when a file of that language asks for it, and stops it \
                         when it has been idle.",
                    ),
            )
            .into_any_element()
    }
}

/// On / Not installed / Off, in the frame's chip shape.
fn state_chip(id: &'static str, state: ServerState, t: &Theme) -> Stateful<gpui::Div> {
    let (label, color) = match state {
        ServerState::On => ("On", t.add),
        ServerState::NotInstalled => ("Not installed", t.del),
        ServerState::Off => ("Off", t.mute),
    };
    div()
        .id((id, 0u64))
        .px(px(8.))
        .rounded(px(10.))
        .bg(t.sel)
        .text_size(px(12.))
        .text_color(color)
        .cursor_pointer()
        .hover(|d| d.bg(t.hov))
        .child(SharedString::from(label))
}
