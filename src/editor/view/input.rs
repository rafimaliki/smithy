//! Keyboard input for the editor: printable characters, and the Backspace,
//! Delete, Enter and Tab actions, which double as find-bar keys while that bar
//! is open. The act! handlers for everything else live in `view.rs`.
use super::EditorView;
use crate::actions::{Backspace, Delete, Enter, Tab};
use crate::editor::edit_ops::indent_unit;
use gpui::{Context, KeyDownEvent, Window};

impl EditorView {
    pub(super) fn key_down(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let m = e.keystroke.modifiers;
        if m.control || m.alt || m.platform {
            return;
        }
        // Windows reports the space bar as key "space" with no key_char.
        let typed = match (e.keystroke.key_char.as_deref(), e.keystroke.key.as_str()) {
            (Some(ch), _) => Some(ch),
            (None, "space") => Some(" "),
            _ => None,
        };
        if self.goto.is_some() {
            if e.keystroke.key == "escape" {
                self.goto_close(cx);
            } else if let Some(ch) = typed {
                self.goto_type(ch, cx);
            }
            return;
        }
        if self.find.is_some() {
            match e.keystroke.key.as_str() {
                "escape" => {
                    self.find_close(cx);
                    return;
                }
                // Plain Enter is the `Enter` action; only Shift+Enter arrives here.
                "enter" => {
                    if m.shift {
                        self.find_step(-1, cx);
                    }
                    return;
                }
                _ => {}
            }
            if let Some(ch) = typed {
                if !ch.chars().any(|c| c.is_control()) {
                    self.find_type(ch, cx);
                }
            }
            return;
        }
        if let Some(ch) = typed {
            if !ch.chars().any(|c| c.is_control()) {
                self.state.insert(ch);
                self.changed(cx);
            }
        }
    }

    pub(super) fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.goto.is_some() {
            self.goto_backspace(cx);
            return;
        }
        if self.find.is_some() {
            self.find_backspace(cx);
            return;
        }
        self.state.backspace();
        self.changed(cx);
    }

    /// The query has no caret, so Delete does nothing while finding.
    pub(super) fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_some() {
            return;
        }
        self.state.delete();
        self.changed(cx);
    }

    pub(super) fn enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        if self.goto.is_some() {
            self.goto_commit(cx);
            return;
        }
        if let Some(f) = self.find.as_ref() {
            if f.replace_open && f.in_replace {
                self.replace_current(cx);
            } else {
                self.find_step(1, cx);
            }
            return;
        }
        self.state.newline_keeping_indent();
        self.changed(cx);
    }

    pub(super) fn tab(&mut self, _: &Tab, _: &mut Window, cx: &mut Context<Self>) {
        if self.goto.is_some() {
            return;
        }
        if let Some(f) = self.find.as_mut() {
            // Tab moves between the find and replace fields.
            if f.replace_open {
                f.in_replace = !f.in_replace;
                cx.notify();
            }
            return;
        }
        let (a, b) = self.state.selection();
        if self.state.line_col(a).0 != self.state.line_col(b).0 {
            self.state.indent_lines();
        } else {
            let unit = indent_unit(&self.state.buffer.text());
            self.state.insert(unit);
        }
        self.changed(cx);
    }
}
