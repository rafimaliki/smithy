//! Syntax error support in the editor view: switching collection on with the
//! Error highlighting add-on, the count for the status bar, and F8 / Shift+F8.
use super::EditorView;
use crate::actions::{NextError, PrevError};
use crate::addon::problems;
use crate::settings::Settings;
use gpui::{Context, Window};

impl EditorView {
    /// Follow the add-on's on/off state; a change re-parses so errors appear or go.
    pub(super) fn sync_error_highlight(&mut self, cx: &mut Context<Self>) {
        let on = cx.global::<Settings>().addon_enabled(problems::ID);
        if on != self.errors_on {
            self.errors_on = on;
            if let Some(h) = self.highlighter.as_mut() {
                h.set_collect_errors(on);
            }
            self.hl_revision = u64::MAX;
        }
    }

    /// How many syntax errors the file has, or `None` when the add-on is off or the
    /// language has no grammar.
    pub fn error_count(&self) -> Option<usize> {
        if !self.errors_on {
            return None;
        }
        self.highlighter.as_ref().map(|h| h.errors().len())
    }

    pub(super) fn next_error(&mut self, _: &NextError, _: &mut Window, cx: &mut Context<Self>) {
        self.step_error(true, cx);
    }

    pub(super) fn prev_error(&mut self, _: &PrevError, _: &mut Window, cx: &mut Context<Self>) {
        self.step_error(false, cx);
    }

    /// Move the caret to the next or previous error, wrapping around.
    fn step_error(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(h) = self.highlighter.as_ref().filter(|_| self.errors_on) else {
            return;
        };
        let errors = h.errors();
        if errors.is_empty() {
            return;
        }
        let (line, col) = self.state.line_col(self.state.cursor);
        let here = (line as u32, col as u32);
        let target = if forward {
            errors
                .iter()
                .find(|e| (e.line, e.start) > here)
                .or(errors.first())
        } else {
            errors
                .iter()
                .rev()
                .find(|e| (e.line, e.start) < here)
                .or(errors.last())
        };
        if let Some(e) = target.copied() {
            self.state
                .set_cursor_line_col(e.line as usize, e.start as usize, false);
            self.reveal_cursor();
            cx.notify();
        }
    }
}
