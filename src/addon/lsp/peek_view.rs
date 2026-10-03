//! Drawing the Language servers view: the references peek (modal, references
//! grouped by file, a code preview of the selected one) and the tip that
//! explains a miss. Board frames `lsp-references` and `lsp-definition` (tip).
use super::peek::LspView;
use super::peek_rows::{file_row, marked, reference_row};
use super::refs::Peek;
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyElement, Context, FontWeight, MouseButton, MouseDownEvent, Render,
    SharedString, Window,
};
use std::path::Path;

const PANEL_W: f32 = 980.0;
const PANEL_H: f32 = 540.0;
const LIST_W: f32 = 340.0;
/// Lines of context on each side of the selected reference.
const CONTEXT: u32 = 9;

impl Render for LspView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::by_name(&cx.global::<Settings>().theme);
        match (&self.peek, &self.tip) {
            (Some(peek), _) => self.panel(peek, &t, cx).into_any_element(),
            (None, Some(tip)) => tip_box(tip, &t).into_any_element(),
            (None, None) => div().into_any_element(),
        }
    }
}

impl LspView {
    fn panel(&self, peek: &Peek, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let files = peek
            .refs
            .iter()
            .map(|r| r.loc.path.to_string_lossy().to_lowercase())
            .collect::<std::collections::BTreeSet<_>>()
            .len();

        let header = div()
            .flex()
            .items_center()
            .h(px(42.))
            .flex_none()
            .px(px(16.))
            .border_b_1()
            .border_color(t.line)
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(SharedString::from(format!("References to {}", peek.symbol))),
            )
            .child(
                div()
                    .ml(px(10.))
                    .flex_1()
                    .text_color(t.mute)
                    .child(SharedString::from(format!(
                        "{} in {} files",
                        peek.refs.len(),
                        files
                    ))),
            )
            .child(
                div()
                    .id("lsp-peek-close")
                    .w(px(24.))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_color(t.mute)
                    .hover(|d| d.bg(t.hov).text_color(t.ink))
                    .child("×")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, _, cx| {
                            this.close(cx);
                        }),
                    ),
            );

        let mut list = div()
            .id("lsp-refs")
            .w(px(LIST_W))
            .flex_none()
            .py(px(6.))
            .overflow_y_scroll()
            .track_scroll(self.scroll())
            .border_r_1()
            .border_color(t.line);
        let mut last: Option<&Path> = None;
        for (i, reference) in peek.refs.iter().enumerate() {
            if last != Some(reference.loc.path.as_path()) {
                last = Some(reference.loc.path.as_path());
                list = list.child(file_row(&reference.loc.path, t));
            }
            let selected = i == peek.selected;
            list = list.child(
                reference_row(("lsp-ref", i), reference, selected, t).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| this.select(i, cx)),
                ),
            );
        }

        let body = div()
            .flex()
            .flex_1()
            .min_h_0()
            .child(list)
            .child(self.preview(peek, t));

        let footer = div()
            .flex()
            .items_center()
            .gap(px(18.))
            .h(px(30.))
            .flex_none()
            .px(px(16.))
            .border_t_1()
            .border_color(t.line)
            .text_size(px(12.))
            .text_color(t.mute)
            .child("Enter open file")
            .child("Esc close")
            .child("Up / Down move");

        let panel = div()
            .w(px(PANEL_W))
            .h(px(PANEL_H))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(t.side)
            .border_1()
            .border_color(t.line)
            .rounded(px(10.))
            .text_color(t.ink)
            .font_family("Segoe UI")
            .text_size(px(13.))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .child(header)
            .child(body)
            .child(footer);

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("lsp-peek-backdrop")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, _, cx| this.close(cx)),
                    ),
            )
            .child(panel)
            .into_any_element()
    }

    /// The code around the selected reference.
    fn preview(&self, peek: &Peek, t: &Theme) -> AnyElement {
        let Some(reference) = peek.refs.get(peek.selected) else {
            return div().into_any_element();
        };
        let Some(lines) = self.sources().get(&reference.loc.path) else {
            return div()
                .flex_1()
                .min_w_0()
                .bg(t.bg)
                .p(px(16.))
                .text_color(t.mute)
                .child("This file is too large to preview.")
                .into_any_element();
        };
        let first = reference.loc.line.saturating_sub(CONTEXT);
        let last = (reference.loc.line + CONTEXT).min(lines.len().saturating_sub(1) as u32);
        let mut body = div()
            .flex_1()
            .min_w_0()
            .bg(t.bg)
            .pt(px(8.))
            .overflow_hidden();
        body = body.child(
            div()
                .px(px(16.))
                .pb(px(8.))
                .text_size(px(12.))
                .text_color(t.mute)
                .child(SharedString::from(
                    reference.loc.path.to_string_lossy().into_owned(),
                )),
        );
        for number in first..=last {
            let Some(text) = lines.get(number as usize) else {
                continue;
            };
            let current = number == reference.loc.line;
            let mut row = div()
                .flex()
                .h(px(21.))
                .when(current, |d| d.bg(t.sel))
                .child(
                    div()
                        .w(px(56.))
                        .flex_none()
                        .pr(px(18.))
                        .text_right()
                        .text_color(t.mute)
                        .opacity(0.6)
                        .child(SharedString::from((number + 1).to_string())),
                );
            row = row.child(if current {
                marked(text, &peek.symbol, t)
            } else {
                div()
                    .whitespace_nowrap()
                    .child(SharedString::from(text.clone()))
                    .into_any_element()
            });
            body = body.child(row);
        }
        body.into_any_element()
    }
}

/// A short message near the bottom of the window: a starting server, a miss.
fn tip_box(tip: &SharedString, t: &Theme) -> AnyElement {
    div()
        .absolute()
        .bottom(px(56.))
        .left_0()
        .w_full()
        .flex()
        .justify_center()
        .child(
            div()
                .bg(t.side)
                .border_1()
                .border_color(t.line)
                .rounded(px(6.))
                .px(px(10.))
                .py(px(6.))
                .text_size(px(12.))
                .text_color(t.mute)
                .font_family("Segoe UI")
                .child(tip.clone()),
        )
        .into_any_element()
}
