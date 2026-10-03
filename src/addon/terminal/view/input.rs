//! Key handling and scrolling for the terminal panel: keys to the pty, and the
//! scrollback view over the active session's grid.

use super::super::keys;
use super::{TerminalView, LINE_H};
use gpui::{Context, KeyDownEvent, ScrollDelta, ScrollWheelEvent, Window};

impl TerminalView {
    fn send(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if let Some(open) = self.sessions.get_mut(self.active) {
            open.session.write(bytes);
        }
        self.scroll = 0;
        cx.notify();
    }

    pub(super) fn key_down(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let m = e.keystroke.modifiers;
        let key = e.keystroke.key.as_str();
        // Ctrl+V pastes; the terminal never claims Ctrl+C (copy needs a selection).
        if m.control && key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.send(text.replace("\r\n", "\r").as_bytes(), cx);
            }
            return;
        }
        if m.control && (key == "pageup" || key == "pagedown") {
            self.scroll_by(if key == "pageup" { 10 } else { -10 }, cx);
            return;
        }
        if let Some(bytes) = keys::encode(key, e.keystroke.key_char.as_deref(), m) {
            self.send(&bytes, cx);
        }
    }

    fn scroll_by(&mut self, lines: isize, cx: &mut Context<Self>) {
        let total = self
            .active_emulator()
            .map(|e| e.lock().map(|e| e.grid().total_lines()).unwrap_or(0))
            .unwrap_or(0);
        let max = total.saturating_sub(self.visible_rows()) as isize;
        self.scroll = (self.scroll as isize + lines).clamp(0, max.max(0)) as usize;
        cx.notify();
    }

    pub(super) fn wheel(&mut self, e: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let lines = match e.delta {
            ScrollDelta::Lines(p) => p.y,
            ScrollDelta::Pixels(p) => f32::from(p.y) / LINE_H,
        };
        if lines != 0.0 {
            self.scroll_by(lines.round() as isize, cx);
        }
    }

    pub(super) fn visible_rows(&self) -> usize {
        let (_, rows) = self.measured();
        if rows == 0 {
            12
        } else {
            rows as usize
        }
    }

    pub(super) fn focus_body(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.scroll = 0;
        self.shell_menu = false;
        window.focus(&self.focus);
        cx.notify();
    }
}
