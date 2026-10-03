//! Keyboard input for the editor: printable characters, and the Backspace,
//! Delete, Enter and Tab actions, which double as find-bar keys while that bar
//! is open. The act! handlers for everything else live in `view.rs`.
use super::EditorView;
use crate::actions::{Backspace, Delete, Enter, Tab};
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
        if self.find.is_some() {
            self.find_step(1, cx);
            return;
        }
        self.state.insert("\n");
        self.changed(cx);
    }

    pub(super) fn tab(&mut self, _: &Tab, _: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_some() {
            return;
        }
        self.state.insert("\t");
        self.changed(cx);
    }
}
