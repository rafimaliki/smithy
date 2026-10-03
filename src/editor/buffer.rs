//! Text storage with undo/redo. Positions are char indices into the rope.
use ropey::Rope;

struct Edit {
    at: usize,
    removed: String,
    inserted: String,
    cursor_before: usize,
    cursor_after: usize,
}

pub struct Buffer {
    rope: Rope,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Bumped on every change, so views can tell when to re-read.
    pub revision: u64,
}

impl Buffer {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
        }
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Number of lines; an empty buffer and a buffer ending in `\n` both have a last empty line.
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// The line without its line terminator.
    pub fn line(&self, i: usize) -> String {
        let s = self.rope.line(i).to_string();
        s.trim_end_matches(['\n', '\r']).to_string()
    }

    pub fn line_len(&self, i: usize) -> usize {
        self.line(i).chars().count()
    }

    pub fn line_start(&self, i: usize) -> usize {
        self.rope.line_to_char(i)
    }

    pub fn line_of(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx)
    }

    pub fn slice(&self, from: usize, to: usize) -> String {
        self.rope.slice(from..to).to_string()
    }

    /// Replace `from..to` with `text`, recording it for undo. Cursor positions are
    /// stored so undo/redo can put the caret back.
    pub fn replace(&mut self, from: usize, to: usize, text: &str, cursor_before: usize) -> usize {
        let removed = self.rope.slice(from..to).to_string();
        self.rope.remove(from..to);
        self.rope.insert(from, text);
        let cursor_after = from + text.chars().count();
        self.undo.push(Edit {
            at: from,
            removed,
            inserted: text.to_string(),
            cursor_before,
            cursor_after,
        });
        self.redo.clear();
        self.revision += 1;
        cursor_after
    }

    /// Undo the last edit; returns where the caret goes.
    pub fn undo(&mut self) -> Option<usize> {
        let e = self.undo.pop()?;
        let end = e.at + e.inserted.chars().count();
        self.rope.remove(e.at..end);
        self.rope.insert(e.at, &e.removed);
        let cursor = e.cursor_before;
        self.redo.push(e);
        self.revision += 1;
        Some(cursor)
    }

    pub fn redo(&mut self) -> Option<usize> {
        let e = self.redo.pop()?;
        let end = e.at + e.removed.chars().count();
        self.rope.remove(e.at..end);
        self.rope.insert(e.at, &e.inserted);
        let cursor = e.cursor_after;
        self.undo.push(e);
        self.revision += 1;
        Some(cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_and_redo_restore_text_and_caret() {
        let mut b = Buffer::new("hello world");
        let c = b.replace(5, 11, "!", 11);
        assert_eq!(b.text(), "hello!");
        assert_eq!(c, 6);
        assert_eq!(b.undo(), Some(11));
        assert_eq!(b.text(), "hello world");
        assert_eq!(b.redo(), Some(6));
        assert_eq!(b.text(), "hello!");
        assert_eq!(b.redo(), None);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut b = Buffer::new("a");
        b.replace(1, 1, "b", 1);
        b.undo();
        b.replace(1, 1, "c", 1);
        assert_eq!(b.redo(), None);
        assert_eq!(b.text(), "ac");
    }

    #[test]
    fn lines_exclude_terminators_and_handle_crlf() {
        let b = Buffer::new("one\r\ntwo\n");
        assert_eq!(b.len_lines(), 3);
        assert_eq!(b.line(0), "one");
        assert_eq!(b.line(1), "two");
        assert_eq!(b.line(2), "");
    }
}
