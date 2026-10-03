//! Launch screen: shown while no folder is open.
use super::Workspace;
use crate::theme::{on_accent, Theme};
use gpui::{div, prelude::*, px, Context, SharedString, Window};

impl Workspace {
    pub(super) fn launch(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let recent: Vec<_> = cx.global::<crate::settings::Settings>().recent.clone();
        let mut list = div().flex().flex_col().w_full().gap(px(2.));
        for (i, path) in recent.into_iter().enumerate() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let full = path.display().to_string();
            list = list.child(
                div()
                    .id(("recent", i))
                    .flex()
                    .justify_between()
                    .gap(px(16.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|d| d.bg(t.hov))
                    .on_click(cx.listener(move |this, _, window: &mut Window, cx| {
                        this.open_folder(path.clone(), window, cx)
                    }))
                    .child(SharedString::from(name))
                    .child(div().text_color(t.mute).child(SharedString::from(full))),
            );
        }
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(520.))
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .child(div().text_size(px(28.)).child("Smithy"))
                    .child(
                        div()
                            .text_color(t.mute)
                            .child("A light code editor. Open a folder to start."),
                    )
                    .child(
                        div()
                            .id("open-folder")
                            .w(px(160.))
                            .py(px(8.))
                            .flex()
                            .justify_center()
                            .rounded(px(4.))
                            .cursor_pointer()
                            .bg(t.acc)
                            .text_color(on_accent())
                            .on_click(cx.listener(|this, _, window: &mut Window, cx| {
                                this.prompt_open_folder(window, cx)
                            }))
                            .child("Open folder (Ctrl+O)"),
                    )
                    .when_some(self.error.clone(), |d, e| {
                        d.child(div().text_color(t.del).child(e))
                    })
                    .child(div().text_size(px(11.)).text_color(t.mute).child("RECENT"))
                    .child(list),
            )
    }
}
