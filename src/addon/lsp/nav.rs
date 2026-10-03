//! A place in a file, and the back/forward trail through them. Pure, so the
//! truncate-on-a-new-jump rule is tested rather than discovered in the editor.
use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Loc {
    pub path: PathBuf,
    /// 0-based.
    pub line: u32,
    /// 0-based, UTF-16 units (what a language server counts in).
    pub character: u32,
}

impl Loc {
    pub fn new(path: PathBuf, line: u32, character: u32) -> Self {
        Self {
            path,
            line,
            character,
        }
    }
}

/// The places visited, most recent last, with a cursor for back and forward.
#[derive(Default)]
pub struct History {
    entries: Vec<Loc>,
    index: usize,
}

impl History {
    pub fn current(&self) -> Option<&Loc> {
        self.entries.get(self.index)
    }

    /// Record arriving at `loc`; a jump made after going back drops the trail
    /// that was ahead of the cursor.
    pub fn visit(&mut self, loc: Loc) {
        if self.current() == Some(&loc) {
            return;
        }
        self.entries.truncate(self.index + 1);
        self.entries.push(loc);
        self.index = self.entries.len() - 1;
    }

    pub fn back(&mut self) -> Option<Loc> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        self.current().cloned()
    }

    pub fn forward(&mut self) -> Option<Loc> {
        if self.index + 1 >= self.entries.len() {
            return None;
        }
        self.index += 1;
        self.current().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(n: u32) -> Loc {
        Loc::new(PathBuf::from(format!("C:/f{n}.rs")), n, 0)
    }

    #[test]
    fn back_and_forward_walk_the_trail() {
        let mut h = History::default();
        h.visit(loc(1));
        h.visit(loc(2));
        h.visit(loc(3));
        assert_eq!(h.back(), Some(loc(2)));
        assert_eq!(h.back(), Some(loc(1)));
        assert_eq!(h.back(), None);
        assert_eq!(h.forward(), Some(loc(2)));
        assert_eq!(h.forward(), Some(loc(3)));
        assert_eq!(h.forward(), None);
    }

    #[test]
    fn a_new_jump_after_going_back_drops_the_future() {
        let mut h = History::default();
        h.visit(loc(1));
        h.visit(loc(2));
        h.visit(loc(3));
        h.back();
        h.back();
        h.visit(loc(9));
        assert_eq!(h.forward(), None);
        assert_eq!(h.back(), Some(loc(1)));
    }

    #[test]
    fn visiting_the_current_place_does_not_grow_the_trail() {
        let mut h = History::default();
        h.visit(loc(1));
        h.visit(loc(1));
        assert_eq!(h.back(), None);
    }
}
