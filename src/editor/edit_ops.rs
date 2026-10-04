//! Line and word editing commands (the VS Code basics): comment, delete, duplicate
//! and move lines, indent, select line, word moves. Pure functions over
//! `EditorState`, so they are unit-tested without a window. Each command is one
//! buffer edit, so one Undo reverts it.
use super::state::EditorState;

/// The indent unit: a tab when the file already indents with tabs, else four spaces.
pub fn indent_unit(text: &str) -> &'static str {
    let first = text
        .lines()
        .find(|l| l.starts_with(' ') || l.starts_with('\t'));
    match first {
        Some(l) if l.starts_with('\t') => "\t",
        _ => "    ",
    }
}

impl EditorState {
    /// First and last line a command acts on: the caret line, or every line the
    /// selection touches (a selection ending at column 0 leaves that last line out).
    fn line_span(&self) -> (usize, usize) {
        let (a, b) = self.selection();
        let first = self.buffer.line_of(a);
        let mut last = self.buffer.line_of(b);
        if b > a && last > first && b == self.buffer.line_start(last) {
            last -= 1;
        }
        (first, last)
    }

    /// Char range of whole lines `first..=last`, including the final line break.
    fn block_range(&self, first: usize, last: usize) -> (usize, usize) {
        let start = self.buffer.line_start(first);
        let end = if last + 1 < self.buffer.len_lines() {
            self.buffer.line_start(last + 1)
        } else {
            self.buffer.len_chars()
        };
        (start, end)
    }

    /// The lines of `first..=last` as strings without terminators.
    fn lines_of(&self, first: usize, last: usize) -> Vec<String> {
        (first..=last).map(|i| self.buffer.line(i)).collect()
    }

    /// Replace the block of lines, keeping the caret and selection on those lines.
    fn replace_block(&mut self, first: usize, last: usize, lines: &[String]) {
        let (start, end) = self.block_range(first, last);
        let had_break = end > start && self.buffer.slice(end - 1, end) == "\n";
        let single = first == last && !self.has_selection();
        let (old_col, old_len) = (
            self.cursor.saturating_sub(start),
            self.buffer.line_len(first),
        );
        let mut text = lines.join("\n");
        if had_break {
            text.push('\n');
        }
        let cursor_before = self.cursor;
        let at = self.buffer.replace(start, end, &text, cursor_before);
        let block_end = if had_break { at - 1 } else { at };
        if single {
            // Keep the caret where it was in the line, shifted by what changed before it.
            let new_len = lines[0].chars().count();
            let col = (old_col as isize + new_len as isize - old_len as isize)
                .clamp(0, new_len as isize) as usize;
            self.set_cursor(start + col, false);
        } else {
            self.anchor = start;
            self.cursor = block_end;
        }
    }

    /// Ctrl+/: comment the lines out, or uncomment them when all are commented.
    /// `suffix` is for languages with only block comments (`/* */`, `<!-- -->`).
    pub fn toggle_comment(&mut self, prefix: &str, suffix: Option<&str>) {
        if self.read_only {
            return;
        }
        let (first, last) = self.line_span();
        let lines = self.lines_of(first, last);
        let blank = |l: &String| l.trim().is_empty();
        let commented = |l: &String| {
            let t = l.trim_start();
            t.starts_with(prefix) && suffix.is_none_or(|s| t.trim_end().ends_with(s))
        };
        let all =
            lines.iter().filter(|l| !blank(l)).all(commented) && lines.iter().any(|l| !blank(l));
        let indent = lines
            .iter()
            .filter(|l| !blank(l))
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);
        let out: Vec<String> = lines
            .iter()
            .map(|l| {
                if blank(l) {
                    return l.clone();
                }
                if all {
                    let body = l.trim_start();
                    let lead = &l[..l.len() - body.len()];
                    let mut body = body.strip_prefix(prefix).unwrap_or(body);
                    body = body.strip_prefix(' ').unwrap_or(body);
                    if let Some(s) = suffix {
                        let t = body.trim_end();
                        body = t.strip_suffix(s).unwrap_or(t);
                        body = body.strip_suffix(' ').unwrap_or(body);
                    }
                    format!("{lead}{body}")
                } else {
                    let (lead, rest) = l.split_at(indent);
                    match suffix {
                        Some(s) => format!("{lead}{prefix} {rest} {s}"),
                        None => format!("{lead}{prefix} {rest}"),
                    }
                }
            })
            .collect();
        self.replace_block(first, last, &out);
    }

    /// Ctrl+Shift+K: remove the lines the caret or selection is on.
    pub fn delete_line(&mut self) {
        if self.read_only {
            return;
        }
        let (first, last) = self.line_span();
        let (start, end) = self.block_range(first, last);
        // The last line has no break after it; take the one before instead.
        let start = if end == self.buffer.len_chars() && start > 0 && first > 0 {
            start - 1
        } else {
            start
        };
        let cursor_before = self.cursor;
        let at = self.buffer.replace(start, end, "", cursor_before);
        self.set_cursor(at, false);
    }

    /// Shift+Alt+Down / Up: copy the lines below or above themselves.
    pub fn duplicate_lines(&mut self, down: bool) {
        if self.read_only {
            return;
        }
        let (first, last) = self.line_span();
        let lines = self.lines_of(first, last);
        let mut doubled = lines.clone();
        doubled.extend(lines);
        let n = last - first + 1;
        self.replace_block(first, last, &doubled);
        // The selection is on both copies now: put it on the copy that should stay.
        let first_new = if down { first + n } else { first };
        let last_new = first_new + n - 1;
        self.select_lines(first_new, last_new);
    }

    /// Alt+Up / Alt+Down: swap the lines with their neighbour.
    pub fn move_lines(&mut self, down: bool) {
        if self.read_only {
            return;
        }
        let (first, last) = self.line_span();
        let total = self.buffer.len_lines();
        // A trailing empty line (text ends in "\n") is not a line to move over.
        let real = if self.buffer.line_len(total - 1) == 0 && total > 1 {
            total - 1
        } else {
            total
        };
        if (!down && first == 0) || (down && last + 1 >= real) {
            return;
        }
        let (lo, hi) = if down {
            (first, last + 1)
        } else {
            (first - 1, last)
        };
        let mut lines = self.lines_of(lo, hi);
        if down {
            let moved = lines.remove(lines.len() - 1);
            lines.insert(0, moved);
        } else {
            let moved = lines.remove(0);
            lines.push(moved);
        }
        self.replace_block(lo, hi, &lines);
        let shift = |n: usize| if down { n + 1 } else { n - 1 };
        self.select_lines(shift(first), shift(last));
    }

    /// Select lines `first..=last` whole (without the final break).
    pub fn select_lines(&mut self, first: usize, last: usize) {
        self.anchor = self.buffer.line_start(first);
        self.cursor = self.buffer.line_start(last) + self.buffer.line_len(last);
    }

    /// Ctrl+L: select the current line, then the next ones on repeat.
    pub fn select_line(&mut self) {
        let (first, last) = self.line_span();
        let (start, end) = self.block_range(first, last);
        if self.selection() == (start, end) {
            let (_, e2) = self.block_range(first, (last + 1).min(self.buffer.len_lines() - 1));
            self.cursor = e2;
        } else {
            self.anchor = start;
            self.cursor = end;
        }
    }

    /// Ctrl+] / Tab on a multi-line selection: indent every line.
    pub fn indent_lines(&mut self) {
        if self.read_only {
            return;
        }
        let unit = indent_unit(&self.buffer.text());
        let (first, last) = self.line_span();
        let out: Vec<String> = self
            .lines_of(first, last)
            .into_iter()
            .map(|l| {
                if l.is_empty() {
                    l
                } else {
                    format!("{unit}{l}")
                }
            })
            .collect();
        self.replace_block(first, last, &out);
    }

    /// Ctrl+[ / Shift+Tab: remove one indent unit from every line.
    pub fn outdent_lines(&mut self) {
        if self.read_only {
            return;
        }
        let (first, last) = self.line_span();
        let out: Vec<String> = self
            .lines_of(first, last)
            .into_iter()
            .map(|l| {
                if let Some(rest) = l.strip_prefix('\t') {
                    rest.to_string()
                } else {
                    let spaces = l.chars().take_while(|c| *c == ' ').count().min(4);
                    l[spaces..].to_string()
                }
            })
            .collect();
        self.replace_block(first, last, &out);
    }

    /// Ctrl+Enter / Ctrl+Shift+Enter: open a new line below or above, indented like this one.
    pub fn open_line(&mut self, below: bool) {
        if self.read_only {
            return;
        }
        let (line, _) = self.line_col(self.cursor);
        let text = self.buffer.line(line);
        let lead: String = text
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        if below {
            let at = self.buffer.line_start(line) + self.buffer.line_len(line);
            self.set_cursor(at, false);
            self.insert(&format!("\n{lead}"));
        } else {
            let at = self.buffer.line_start(line);
            self.set_cursor(at, false);
            self.insert(&format!("{lead}\n"));
            let up = self.buffer.line_start(line) + lead.chars().count();
            self.set_cursor(up, false);
        }
    }

    /// Enter: break the line and keep its indentation.
    pub fn newline_keeping_indent(&mut self) {
        let (line, col) = self.line_col(self.cursor);
        let text = self.buffer.line(line);
        let lead: String = text
            .chars()
            .take(col)
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        self.insert(&format!("\n{lead}"));
    }

    /// Ctrl+Left / Ctrl+Right: to the start of the previous word or end of the next.
    pub fn move_word(&mut self, left: bool, extend: bool) {
        let at = self.word_edge(left);
        self.set_cursor(at, extend);
    }

    /// Ctrl+Backspace / Ctrl+Delete.
    pub fn delete_word(&mut self, left: bool) {
        if self.read_only {
            return;
        }
        if !self.has_selection() {
            self.anchor = self.word_edge(left);
        }
        self.insert("");
    }

    fn word_edge(&self, left: bool) -> usize {
        let word = |c: char| c.is_alphanumeric() || c == '_';
        let text = self.buffer.text();
        let chars: Vec<char> = text.chars().collect();
        let mut i = self.cursor.min(chars.len());
        if left {
            while i > 0 && chars[i - 1].is_whitespace() {
                i -= 1;
            }
            if i > 0 {
                let w = word(chars[i - 1]);
                while i > 0 && !chars[i - 1].is_whitespace() && word(chars[i - 1]) == w {
                    i -= 1;
                }
            }
        } else {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() {
                let w = word(chars[i]);
                while i < chars.len() && !chars[i].is_whitespace() && word(chars[i]) == w {
                    i += 1;
                }
            }
        }
        i
    }

    /// Ctrl+Home / Ctrl+End.
    pub fn doc_edge(&mut self, start: bool, extend: bool) {
        let at = if start { 0 } else { self.buffer.len_chars() };
        self.set_cursor(at, extend);
    }

    /// The text Ctrl+C / Ctrl+X take when nothing is selected: the whole line and its break.
    pub fn line_for_clipboard(&self) -> String {
        let (line, _) = self.line_col(self.cursor);
        let (start, end) = self.block_range(line, line);
        let mut text = self.buffer.slice(start, end);
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text
    }

    /// Jump to a 1-based line, clamped to the file.
    pub fn go_to_line(&mut self, line: usize) {
        let last = self.buffer.len_lines().saturating_sub(1);
        let target = line.saturating_sub(1).min(last);
        self.set_cursor(self.buffer.line_start(target), false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(text: &str) -> EditorState {
        EditorState::new(text, None, false)
    }

    #[test]
    fn comment_toggles_on_and_off_and_keeps_blank_lines() {
        let mut e = ed("a\n\n  b\n");
        e.select_all();
        e.toggle_comment("//", None);
        assert_eq!(e.buffer.text(), "// a\n\n//   b\n");
        e.select_all();
        e.toggle_comment("//", None);
        assert_eq!(e.buffer.text(), "a\n\n  b\n");
    }

    #[test]
    fn block_comment_languages_wrap_each_line() {
        let mut e = ed("a { }");
        e.toggle_comment("/*", Some("*/"));
        assert_eq!(e.buffer.text(), "/* a { } */");
        e.toggle_comment("/*", Some("*/"));
        assert_eq!(e.buffer.text(), "a { }");
    }

    #[test]
    fn delete_line_removes_the_line_and_its_break() {
        let mut e = ed("one\ntwo\nthree");
        e.set_cursor_line_col(1, 1, false);
        e.delete_line();
        assert_eq!(e.buffer.text(), "one\nthree");
        e.set_cursor_line_col(1, 0, false);
        e.delete_line();
        assert_eq!(e.buffer.text(), "one");
    }

    #[test]
    fn duplicate_and_move_lines() {
        let mut e = ed("a\nb\nc");
        e.set_cursor_line_col(1, 0, false);
        e.duplicate_lines(true);
        assert_eq!(e.buffer.text(), "a\nb\nb\nc");
        e.move_lines(false);
        assert_eq!(e.buffer.text(), "a\nb\nb\nc");
        e.set_cursor_line_col(0, 0, false);
        e.move_lines(true);
        assert_eq!(e.buffer.text(), "b\na\nb\nc");
        e.set_cursor_line_col(0, 0, false);
        e.move_lines(false);
        assert_eq!(e.buffer.text(), "b\na\nb\nc");
    }

    #[test]
    fn indent_and_outdent_a_selection() {
        let mut e = ed("a\n  b\n");
        e.select_all();
        e.indent_lines();
        assert_eq!(e.buffer.text(), "    a\n      b\n");
        e.select_all();
        e.outdent_lines();
        assert_eq!(e.buffer.text(), "a\n  b\n");
    }

    #[test]
    fn enter_keeps_indentation_and_open_line_works() {
        let mut e = ed("    foo");
        e.set_cursor(7, false);
        e.newline_keeping_indent();
        assert_eq!(e.buffer.text(), "    foo\n    ");
        let mut e = ed("  x\ny");
        e.open_line(true);
        assert_eq!(e.buffer.text(), "  x\n  \ny");
        let mut e = ed("  x\ny");
        e.open_line(false);
        assert_eq!(e.buffer.text(), "  \n  x\ny");
        assert_eq!(e.line_col(e.cursor), (0, 2));
    }

    #[test]
    fn word_moves_and_deletes() {
        let mut e = ed("foo_bar baz");
        e.set_cursor(11, false);
        e.move_word(true, false);
        assert_eq!(e.cursor, 8);
        e.move_word(true, false);
        assert_eq!(e.cursor, 0);
        e.move_word(false, false);
        assert_eq!(e.cursor, 7);
        e.set_cursor(11, false);
        e.delete_word(true);
        assert_eq!(e.buffer.text(), "foo_bar ");
    }

    #[test]
    fn select_line_grows_and_clipboard_line_has_a_break() {
        let mut e = ed("a\nb\nc");
        e.set_cursor_line_col(0, 0, false);
        e.select_line();
        assert_eq!(e.selected_text(), "a\n");
        e.select_line();
        assert_eq!(e.selected_text(), "a\nb\n");
        e.set_cursor_line_col(2, 0, false);
        assert_eq!(e.line_for_clipboard(), "c\n");
    }

    #[test]
    fn go_to_line_clamps() {
        let mut e = ed("a\nb\nc");
        e.go_to_line(2);
        assert_eq!(e.line_col(e.cursor), (1, 0));
        e.go_to_line(99);
        assert_eq!(e.line_col(e.cursor).0, 2);
        e.go_to_line(0);
        assert_eq!(e.cursor, 0);
    }

    #[test]
    fn indent_unit_follows_the_file() {
        assert_eq!(indent_unit("fn x() {\n\treturn;\n}"), "\t");
        assert_eq!(indent_unit("fn x() {\n    return;\n}"), "    ");
        assert_eq!(indent_unit("plain"), "    ");
    }
}
