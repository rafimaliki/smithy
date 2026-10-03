//! Editor model: buffer, caret, selection, dirty tracking. No gpui here, so it is
//! unit-tested directly.
use super::buffer::Buffer;
use std::path::PathBuf;

pub struct EditorState {
    pub buffer: Buffer,
    pub path: Option<PathBuf>,
    pub read_only: bool,
    /// Caret as a char index; `anchor` is the other end of the selection.
    pub cursor: usize,
    pub anchor: usize,
    /// Column the caret tries to keep while moving vertically.
    want_col: usize,
    saved_revision: u64,
}

impl EditorState {
    pub fn new(text: &str, path: Option<PathBuf>, read_only: bool) -> Self {
        Self {
            buffer: Buffer::new(text),
            path,
            read_only,
            cursor: 0,
            anchor: 0,
            want_col: 0,
            saved_revision: 0,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.buffer.revision != self.saved_revision
    }

    pub fn mark_saved(&mut self) {
        self.saved_revision = self.buffer.revision;
    }

    pub fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    pub fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    pub fn selected_text(&self) -> String {
        let (a, b) = self.selection();
        self.buffer.slice(a, b)
    }

    /// (line, column) of a char index, both 0-based.
    pub fn line_col(&self, idx: usize) -> (usize, usize) {
        let line = self.buffer.line_of(idx);
        (line, idx - self.buffer.line_start(line))
    }

    fn index_at(&self, line: usize, col: usize) -> usize {
        let line = line.min(self.buffer.len_lines() - 1);
        self.buffer.line_start(line) + col.min(self.buffer.line_len(line))
    }

    pub fn set_cursor(&mut self, idx: usize, extend: bool) {
        self.cursor = idx.min(self.buffer.len_chars());
        if !extend {
            self.anchor = self.cursor;
        }
        self.want_col = self.line_col(self.cursor).1;
    }

    pub fn set_cursor_line_col(&mut self, line: usize, col: usize, extend: bool) {
        let idx = self.index_at(line, col);
        self.set_cursor(idx, extend);
    }

    pub fn move_left(&mut self, extend: bool) {
        let (a, _) = self.selection();
        if self.has_selection() && !extend {
            self.set_cursor(a, false);
        } else {
            self.set_cursor(self.cursor.saturating_sub(1), extend);
        }
    }

    pub fn move_right(&mut self, extend: bool) {
        let (_, b) = self.selection();
        if self.has_selection() && !extend {
            self.set_cursor(b, false);
        } else {
            self.set_cursor(self.cursor + 1, extend);
        }
    }

    pub fn move_vertical(&mut self, delta: isize, extend: bool) {
        let (line, _) = self.line_col(self.cursor);
        let target = line as isize + delta;
        let want = self.want_col;
        if target < 0 {
            self.set_cursor(0, extend);
        } else if target as usize >= self.buffer.len_lines() {
            self.set_cursor(self.buffer.len_chars(), extend);
        } else {
            self.cursor = self.index_at(target as usize, want);
            if !extend {
                self.anchor = self.cursor;
            }
        }
    }

    pub fn home(&mut self, extend: bool) {
        let (line, _) = self.line_col(self.cursor);
        self.set_cursor(self.buffer.line_start(line), extend);
    }

    pub fn end(&mut self, extend: bool) {
        let (line, _) = self.line_col(self.cursor);
        let idx = self.index_at(line, usize::MAX);
        self.set_cursor(idx, extend);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.buffer.len_chars();
    }

    /// Insert at the caret, replacing the selection.
    pub fn insert(&mut self, text: &str) {
        if self.read_only {
            return;
        }
        let (a, b) = self.selection();
        let at = self.buffer.replace(a, b, text, self.cursor);
        self.set_cursor(at, false);
    }

    pub fn backspace(&mut self) {
        if self.read_only {
            return;
        }
        if !self.has_selection() {
            if self.cursor == 0 {
                return;
            }
            self.anchor = self.cursor - 1;
        }
        self.insert("");
    }

    pub fn delete(&mut self) {
        if self.read_only {
            return;
        }
        if !self.has_selection() {
            if self.cursor >= self.buffer.len_chars() {
                return;
            }
            self.anchor = self.cursor + 1;
        }
        self.insert("");
    }

    pub fn undo(&mut self) {
        if let Some(c) = self.buffer.undo() {
            self.set_cursor(c, false);
        }
    }

    pub fn redo(&mut self) {
        if let Some(c) = self.buffer.redo() {
            self.set_cursor(c, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_replaces_selection_and_dirty_tracks_save() {
        let mut e = EditorState::new("abc", None, false);
        assert!(!e.is_dirty());
        e.select_all();
        e.insert("x");
        assert_eq!(e.buffer.text(), "x");
        assert!(e.is_dirty());
        e.mark_saved();
        assert!(!e.is_dirty());
        e.undo();
        assert!(e.is_dirty());
        assert_eq!(e.buffer.text(), "abc");
    }

    #[test]
    fn vertical_move_keeps_wanted_column_across_short_lines() {
        let mut e = EditorState::new("abcdef\nab\nabcdef", None, false);
        e.set_cursor_line_col(0, 5, false);
        e.move_vertical(1, false);
        assert_eq!(e.line_col(e.cursor), (1, 2));
        e.move_vertical(1, false);
        assert_eq!(e.line_col(e.cursor), (2, 5));
    }

    #[test]
    fn read_only_ignores_edits() {
        let mut e = EditorState::new("abc", None, true);
        e.insert("x");
        e.set_cursor(1, false);
        e.backspace();
        assert_eq!(e.buffer.text(), "abc");
    }

    #[test]
    fn backspace_at_start_and_delete_at_end_do_nothing() {
        let mut e = EditorState::new("a", None, false);
        e.backspace();
        e.set_cursor(1, false);
        e.delete();
        assert_eq!(e.buffer.text(), "a");
    }
}
