//! The editor's vertical scrollbar: a thin track at the right edge with a thumb that
//! sizes and moves with the list. Click the track or drag the thumb to scroll.
//! Board frames: every workbench frame (`editor-scrollbar` component).
use super::{EditorView, LINE_H};
use crate::theme::Theme;
use gpui::{
    div, point, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Rgba, Window,
};

const TRACK_W: f32 = 10.0;
const MIN_THUMB: f32 = 28.0;

impl EditorView {
    /// (thumb height, thumb top) in pixels inside a track of `track` pixels, or
    /// `None` when everything fits.
    fn thumb(&self, track: f32) -> Option<(f32, f32)> {
        // Sized from the row count, not the list's own layout, so it is right on the
        // first frame; only the offset comes from the scroll handle.
        let content = self.visual_row_count() as f32 * LINE_H + 6.0;
        let max = content - track;
        if max <= 0.0 || track <= 0.0 {
            return None;
        }
        let handle = self.scroll.0.borrow().base_handle.clone();
        let len = (track * track / content).clamp(MIN_THUMB.min(track), track);
        let top = (-f32::from(handle.offset().y) / max).clamp(0.0, 1.0) * (track - len);
        Some((len, top))
    }

    /// Scroll so the thumb's centre sits at `y` pixels from the top of the track.
    fn scroll_to_thumb_centre(&mut self, y: f32, cx: &mut Context<Self>) {
        let track = f32::from(self.bounds.size.height);
        let Some((len, _)) = self.thumb(track) else {
            return;
        };
        let handle = self.scroll.0.borrow().base_handle.clone();
        let max = self.visual_row_count() as f32 * LINE_H + 6.0 - track;
        let frac = ((y - len / 2.0) / (track - len).max(1.0)).clamp(0.0, 1.0);
        handle.set_offset(point(handle.offset().x, px(-frac * max)));
        cx.notify();
    }

    pub(super) fn scrollbar_view(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let track = f32::from(self.bounds.size.height);
        let (len, top) = self.thumb(track)?;
        let thumb = Rgba {
            a: if self.sb_drag { 0.6 } else { 0.35 },
            ..t.mute
        };
        Some(
            div()
                .id("editor-scrollbar")
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(TRACK_W))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, e: &MouseDownEvent, _, cx| {
                        this.sb_drag = true;
                        let y = f32::from(e.position.y - this.bounds.origin.y);
                        this.scroll_to_thumb_centre(y, cx);
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(top))
                        .left(px(2.))
                        .w(px(TRACK_W - 4.))
                        .h(px(len))
                        .rounded(px(3.))
                        .bg(thumb)
                        .hover(|d| d.opacity(1.0)),
                )
                .into_any_element(),
        )
    }

    /// Dragging the thumb: the pointer may leave the track, so the editor root
    /// follows it while the button is down.
    pub(super) fn scrollbar_drag(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.sb_drag && e.pressed_button == Some(MouseButton::Left) {
            let y = f32::from(e.position.y - self.bounds.origin.y);
            self.scroll_to_thumb_centre(y, cx);
        }
    }

    pub(super) fn scrollbar_release(
        &mut self,
        _: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sb_drag {
            self.sb_drag = false;
            cx.notify();
        }
    }
}
