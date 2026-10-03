//! The window's own title bar. The native one is hidden (`appears_transparent` in
//! `app.rs`), so this bar appears over the top edge when the pointer comes near it
//! and goes away when the pointer leaves it. It also carries the drag area and the
//! minimize, maximize and close buttons, which Windows treats as caption controls.
use super::Workspace;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, rgb, AnyElement, Context, MouseMoveEvent, SharedString, WindowControlArea,
};

/// Pointer distance from the top edge that reveals the bar.
const REVEAL_ZONE: f32 = 6.0;
const BAR_H: f32 = 32.0;
const BUTTON_W: f32 = 46.0;

impl Workspace {
    /// Reveal the bar in the top strip; keep it while the pointer is on it.
    pub(super) fn track_title_bar(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let y = f32::from(e.position.y);
        let show = y < REVEAL_ZONE || (self.title_bar && y < BAR_H);
        if show != self.title_bar {
            self.title_bar = show;
            cx.notify();
        }
    }

    pub(super) fn hide_title_bar(&mut self, cx: &mut Context<Self>) {
        if self.title_bar {
            self.title_bar = false;
            cx.notify();
        }
    }

    pub(super) fn title_bar(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.title_bar {
            return None;
        }
        let title = match self.folder.as_ref().and_then(|f| f.file_name()) {
            Some(name) => format!("{} \u{2014} Smithy", name.to_string_lossy()),
            None => "Smithy".to_string(),
        };
        let this = cx.entity();
        // ponytail: text glyphs for the three buttons; upgrade: the board's icons.
        let button = |id: &'static str, glyph: &'static str, area, danger: bool| {
            div()
                .id(id)
                .w(px(BUTTON_W))
                .h_full()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .text_color(t.mute)
                .hover(move |d| {
                    if danger {
                        d.bg(rgb(0xc42b1c)).text_color(rgb(0xffffff))
                    } else {
                        d.bg(t.hov).text_color(t.ink)
                    }
                })
                .window_control_area(area)
                .child(glyph)
        };
        Some(
            div()
                .id("title-bar")
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(BAR_H))
                .flex()
                .items_center()
                .bg(t.rail)
                .border_b_1()
                .border_color(t.line)
                .occlude()
                // The pointer left the bar (through the top edge or the sides).
                .on_hover(move |hovered, _, app| {
                    if !*hovered {
                        this.update(app, |ws, cx| ws.hide_title_bar(cx));
                    }
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .items_center()
                        .px(px(12.))
                        .text_color(t.mute)
                        .window_control_area(WindowControlArea::Drag)
                        .child(SharedString::from(title)),
                )
                .child(button("win-min", "\u{2212}", WindowControlArea::Min, false))
                .child(button("win-max", "\u{25a1}", WindowControlArea::Max, false))
                .child(button(
                    "win-close",
                    "\u{2715}",
                    WindowControlArea::Close,
                    true,
                ))
                .into_any_element(),
        )
    }
}
