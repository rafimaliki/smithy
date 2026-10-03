//! The rename field's state: text, caret and selection. No gpui here, so the
//! editing rules are tested directly.
use super::ops;

pub struct Input {
    chars: Vec<char>,
    /// Caret and the other end of the selection, in chars.
    cursor: usize,
    anchor: usize,
    /// True for an entry created by New file / New folder: Esc removes it again.
    pub created: bool,
    /// Why the last Enter was refused, shown inline under the field.
    pub error: Option<String>,
}

impl Input {
    /// Start on `name`, with the base name selected (not the extension).
    pub fn for_name(name: &str, created: bool) -> Self {
        let chars: Vec<char> = name.chars().collect();
        Self {
            anchor: 0,
            cursor: stem_len(name),
            chars,
            created,
            error: None,
        }
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    /// Caret position in chars, where the blinking bar is drawn.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn set_cursor(&mut self, at: usize, extend: bool) {
        self.cursor = at.min(self.chars.len());
        if !extend {
            self.anchor = self.cursor;
        }
    }

    fn replace_selection(&mut self, insert: &[char]) -> usize {
        let (a, b) = self.selection();
        self.chars.splice(a..b, insert.iter().copied());
        a + insert.len()
    }

    pub fn insert(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().filter(|c| !c.is_control()).collect();
        let at = self.replace_selection(&chars);
        self.set_cursor(at, false);
    }

    pub fn backspace(&mut self) {
        if self.selection().0 == self.selection().1 {
            let (a, _) = self.selection();
            if a == 0 {
                return;
            }
            self.anchor = a - 1;
        }
        let at = self.replace_selection(&[]);
        self.set_cursor(at, false);
    }

    pub fn delete(&mut self) {
        if self.selection().0 == self.selection().1 {
            let (a, b) = self.selection();
            if b >= self.chars.len() {
                return;
            }
            self.anchor = a;
            self.cursor = b + 1;
        }
        let at = self.replace_selection(&[]);
        self.set_cursor(at, false);
    }

    pub fn move_left(&mut self, extend: bool) {
        let (a, _) = self.selection();
        let at = if self.selection().0 != self.selection().1 && !extend {
            a
        } else {
            self.cursor.saturating_sub(1)
        };
        self.set_cursor(at, extend);
    }

    pub fn move_right(&mut self, extend: bool) {
        let (_, b) = self.selection();
        let at = if self.selection().0 != self.selection().1 && !extend {
            b
        } else {
            self.cursor + 1
        };
        self.set_cursor(at, extend);
    }

    pub fn home(&mut self, extend: bool) {
        self.set_cursor(0, extend);
    }

    pub fn end(&mut self, extend: bool) {
        self.set_cursor(self.chars.len(), extend);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.chars.len();
    }

    /// Check the typed name, for `Enter`. Clears the previous error.
    pub fn check(&mut self) -> Result<(), String> {
        match ops::validate_name(&self.text()) {
            Ok(()) => {
                self.error = None;
                Ok(())
            }
            Err(why) => {
                self.error = Some(why.to_string());
                Err(why.to_string())
            }
        }
    }
}

/// Chars of `name` before its extension; the whole name when there is none.
fn stem_len(name: &str) -> usize {
    match name.rfind('.') {
        Some(i) if i > 0 => name[..i].chars().count(),
        _ => name.chars().count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_name_starts_selected_and_the_extension_is_kept() {
        let mut input = Input::for_name("Card.tsx", false);
        assert_eq!(input.selection(), (0, 4));
        assert_eq!(input.text(), "Card.tsx");
        input.insert("Button");
        assert_eq!(input.text(), "Button.tsx");
        assert_eq!(input.cursor, 6);
    }

    #[test]
    fn a_dotfile_selects_its_whole_name() {
        assert_eq!(Input::for_name(".gitignore", false).selection(), (0, 10));
        assert_eq!(Input::for_name("src", false).selection(), (0, 3));
    }

    #[test]
    fn editing_after_the_first_key() {
        let mut input = Input::for_name("Card.tsx", false);
        input.end(false);
        input.backspace();
        assert_eq!(input.text(), "Card.ts");
        input.home(false);
        input.delete();
        assert_eq!(input.text(), "ard.ts");
        input.select_all();
        input.insert("x");
        assert_eq!(input.text(), "x");
        input.backspace();
        assert_eq!(input.text(), "");
        input.backspace(); // nothing left to remove
        assert_eq!(input.text(), "");
    }

    #[test]
    fn moving_collapses_the_selection() {
        let mut input = Input::for_name("Card.tsx", false);
        input.move_right(false);
        assert_eq!(input.selection(), (4, 4));
        input.select_all();
        input.move_left(false);
        assert_eq!(input.selection(), (0, 0));
        input.end(true);
        assert_eq!(input.selection(), (0, 8));
    }

    #[test]
    fn check_reports_what_the_file_system_would_refuse() {
        let mut input = Input::for_name("Card.tsx", false);
        assert!(input.check().is_ok());
        input.select_all();
        input.insert("a/b");
        assert!(input.check().is_err());
        assert!(input.error.is_some());
        input.select_all();
        input.insert("ok.txt");
        assert!(input.check().is_ok());
        assert!(input.error.is_none());
    }
}
