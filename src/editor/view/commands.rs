//! Line, word and document commands (Ctrl+/, Ctrl+Shift+K, Alt+Up, Ctrl+H, Ctrl+G, ...),
//! the Go to line bar, and Replace in the find bar. The editing itself is in
//! `editor/edit_ops.rs`; these are the gpui handlers around it.
use super::{EditorView, MONO};
use crate::actions::*;
use crate::editor::find;
use crate::theme::Theme;
use gpui::{
    div, hsla, point, prelude::*, px, AnyElement, BoxShadow, ClipboardItem, Context, Div,
    SharedString, Window,
};

/// One handler per command: run an edit op on the state, then refresh the view.
macro_rules! command {
    ($name:ident, $action:ty, |$s:ident| $body:block) => {
        fn $name(&mut self, _: &$action, _: &mut Window, cx: &mut Context<Self>) {
            if self.find.is_some() || self.goto.is_some() {
                return;
            }
            let $s = &mut self.state;
            $body
            self.changed(cx);
        }
    };
}

impl EditorView {
    fn toggle_comment(&mut self, _: &ToggleComment, _: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_some() || self.goto.is_some() {
            return;
        }
        if let Some((prefix, suffix)) = self.lang.and_then(|l| l.comment_tokens()) {
            self.state.toggle_comment(prefix, suffix);
            self.changed(cx);
        }
    }

    command!(delete_line, DeleteLine, |s| { s.delete_line() });
    command!(duplicate_down, DuplicateLineDown, |s| {
        s.duplicate_lines(true)
    });
    command!(duplicate_up, DuplicateLineUp, |s| {
        s.duplicate_lines(false)
    });
    command!(move_line_up, MoveLineUp, |s| { s.move_lines(false) });
    command!(move_line_down, MoveLineDown, |s| { s.move_lines(true) });
    command!(indent_lines, IndentLines, |s| { s.indent_lines() });
    command!(outdent, Outdent, |s| { s.outdent_lines() });
    command!(line_below, InsertLineBelow, |s| { s.open_line(true) });
    command!(line_above, InsertLineAbove, |s| { s.open_line(false) });
    command!(select_line, SelectLine, |s| { s.select_line() });
    command!(word_left, WordLeft, |s| { s.move_word(true, false) });
    command!(word_right, WordRight, |s| { s.move_word(false, false) });
    command!(select_word_left, SelectWordLeft, |s| {
        s.move_word(true, true)
    });
    command!(select_word_right, SelectWordRight, |s| {
        s.move_word(false, true)
    });
    command!(delete_word_left, DeleteWordLeft, |s| {
        s.delete_word(true)
    });
    command!(delete_word_right, DeleteWordRight, |s| {
        s.delete_word(false)
    });
    command!(doc_start, DocStart, |s| { s.doc_edge(true, false) });
    command!(doc_end, DocEnd, |s| { s.doc_edge(false, false) });
    command!(select_doc_start, SelectDocStart, |s| {
        s.doc_edge(true, true)
    });
    command!(select_doc_end, SelectDocEnd, |s| {
        s.doc_edge(false, true)
    });

    /// Every command above, plus Replace and Go to line, as editor actions.
    pub(super) fn command_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(Self::toggle_comment))
            .on_action(cx.listener(Self::delete_line))
            .on_action(cx.listener(Self::duplicate_down))
            .on_action(cx.listener(Self::duplicate_up))
            .on_action(cx.listener(Self::move_line_up))
            .on_action(cx.listener(Self::move_line_down))
            .on_action(cx.listener(Self::indent_lines))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(Self::line_below))
            .on_action(cx.listener(Self::line_above))
            .on_action(cx.listener(Self::select_line))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::doc_start))
            .on_action(cx.listener(Self::doc_end))
            .on_action(cx.listener(Self::select_doc_start))
            .on_action(cx.listener(Self::select_doc_end))
            .on_action(cx.listener(Self::on_replace))
            .on_action(cx.listener(Self::on_go_to_line))
    }

    /// Ctrl+C with nothing selected copies the whole line.
    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let text = if self.state.has_selection() {
            self.state.selected_text()
        } else {
            self.state.line_for_clipboard()
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    /// Ctrl+X with nothing selected cuts the whole line.
    pub(super) fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if self.state.read_only {
            return;
        }
        let selected = self.state.has_selection();
        let text = if selected {
            self.state.selected_text()
        } else {
            self.state.line_for_clipboard()
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        if selected {
            self.state.insert("");
        } else {
            self.state.delete_line();
        }
        self.changed(cx);
    }

    // ---- Replace -----------------------------------------------------------

    /// Ctrl+H: the find bar with the replace row, the keyboard in the replace field.
    fn on_replace(&mut self, _: &Replace, window: &mut Window, cx: &mut Context<Self>) {
        self.goto = None;
        self.on_find(&Find, window, cx);
        if let Some(f) = self.find.as_mut() {
            f.replace_open = true;
            f.in_replace = true;
        }
        cx.notify();
    }

    /// Replace the current match, then move to the next.
    pub(super) fn replace_current(&mut self, cx: &mut Context<Self>) {
        if self.state.read_only {
            return;
        }
        let Some(f) = self.find.as_ref() else {
            return;
        };
        let Some(m) = f.current.and_then(|i| f.matches.get(i)).cloned() else {
            return;
        };
        let (query, replacement) = (f.query.clone(), f.replace.clone());
        let start = self.state.buffer.line_start(m.line) + m.col.start;
        let matched = self.state.buffer.slice(start, start + m.col.len());
        let new = find::replace_one(&query, &matched, &replacement);
        let cursor = self.state.cursor;
        let at = self
            .state
            .buffer
            .replace(start, start + m.col.len(), &new, cursor);
        self.state.set_cursor(at, false);
        let (line, col) = self.state.line_col(at);
        if let Some(f) = self.find.as_mut() {
            f.set_anchor(line, col);
        }
        self.changed(cx);
        self.find_recompute();
        self.find_step(1, cx);
    }

    /// Replace every match in one edit, so one Undo reverts it all.
    pub(super) fn replace_all(&mut self, cx: &mut Context<Self>) {
        if self.state.read_only {
            return;
        }
        let Some(f) = self.find.as_ref() else {
            return;
        };
        let (query, replacement) = (f.query.clone(), f.replace.clone());
        let text = self.state.buffer.text();
        let Ok((new, n)) = find::replace_all(&text, &query, &replacement) else {
            return;
        };
        if n == 0 {
            return;
        }
        let (len, cursor) = (self.state.buffer.len_chars(), self.state.cursor);
        self.state.buffer.replace(0, len, &new, cursor);
        self.state
            .set_cursor(cursor.min(self.state.buffer.len_chars()), false);
        self.changed(cx);
        self.find_recompute();
    }

    // ---- Go to line --------------------------------------------------------

    /// Ctrl+G: ask for a line number.
    fn on_go_to_line(&mut self, _: &GoToLine, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        self.goto = Some(String::new());
        window.focus(&self.focus);
        cx.notify();
    }

    pub(super) fn goto_type(&mut self, ch: &str, cx: &mut Context<Self>) {
        if let Some(text) = self.goto.as_mut() {
            if ch.chars().all(|c| c.is_ascii_digit()) && text.len() < 9 {
                text.push_str(ch);
                cx.notify();
            }
        }
    }

    pub(super) fn goto_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.goto.as_mut() {
            text.pop();
            cx.notify();
        }
    }

    /// Enter: jump to the typed line and close the bar.
    pub(super) fn goto_commit(&mut self, cx: &mut Context<Self>) {
        let line = self.goto.take().and_then(|t| t.parse::<usize>().ok());
        if let Some(line) = line {
            self.state.go_to_line(line);
            self.changed(cx);
        }
        cx.notify();
    }

    pub(super) fn goto_close(&mut self, cx: &mut Context<Self>) {
        self.goto = None;
        cx.notify();
    }

    /// The small bar Ctrl+G opens, top right like the find bar. Board frame `go-to-line`.
    pub(super) fn goto_bar(&self, t: &Theme) -> Option<AnyElement> {
        let text = self.goto.as_ref()?;
        let last = self.state.buffer.len_lines();
        Some(
            div()
                .id("goto-bar")
                .absolute()
                .right(px(28.))
                .top(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .pl(px(12.))
                .pr(px(10.))
                .py(px(5.))
                .rounded(px(8.))
                .bg(t.side)
                .border_1()
                .border_color(t.line)
                .shadow(vec![BoxShadow {
                    color: hsla(0., 0., 0., 0.5),
                    offset: point(px(0.), px(6.)),
                    blur_radius: px(20.),
                    spread_radius: px(0.),
                }])
                .occlude()
                .font_family("Segoe UI")
                .text_size(px(13.))
                .child(div().text_color(t.mute).child("Go to line"))
                .child(
                    div()
                        .w(px(70.))
                        .px(px(8.))
                        .py(px(3.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(t.acc)
                        .bg(t.bg)
                        .font_family(MONO)
                        .text_size(px(12.))
                        .text_color(t.ink)
                        .child(SharedString::from(text.clone())),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(t.mute)
                        .child(SharedString::from(format!("of {last}"))),
                )
                .into_any_element(),
        )
    }
}
