//! The references of one symbol: the rows the peek shows, how they are ordered,
//! and where each row sits in the list. Pure, so the ordering and the list
//! geometry are tested rather than eyeballed.
use super::nav::Loc;
use std::path::Path;

/// One reference row of the peek.
pub struct Reference {
    pub loc: Loc,
    /// The line's text, as the row shows it.
    pub text: String,
    /// The row is the declaration (the frame's `def` mark).
    pub is_def: bool,
}

pub struct Peek {
    pub symbol: String,
    pub refs: Vec<Reference>,
    pub selected: usize,
    /// Where the gesture was made, so Escape can put the user back.
    pub origin: Loc,
}

/// Keep the selection inside the list after a move.
pub fn clamp_selection(len: usize, selected: usize, delta: isize) -> usize {
    let last = len.saturating_sub(1) as isize;
    (selected as isize + delta).clamp(0, last) as usize
}

/// The list child index of a reference: one file heading before each new file.
pub fn row_index(peek: &Peek, selected: usize) -> usize {
    let mut index = 0;
    let mut last: Option<&Path> = None;
    for (i, reference) in peek.refs.iter().enumerate() {
        if last != Some(reference.loc.path.as_path()) {
            last = Some(reference.loc.path.as_path());
            index += 1;
        }
        if i == selected {
            return index;
        }
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn peek(lines: &[(&str, u32)]) -> Peek {
        Peek {
            symbol: "Button".into(),
            refs: lines
                .iter()
                .map(|(path, line)| Reference {
                    loc: Loc::new(PathBuf::from(path), *line, 0),
                    text: String::new(),
                    is_def: false,
                })
                .collect(),
            selected: 0,
            origin: Loc::new(PathBuf::from("C:/a.rs"), 0, 0),
        }
    }

    #[test]
    fn selection_stays_inside_the_list() {
        assert_eq!(clamp_selection(3, 0, -1), 0);
        assert_eq!(clamp_selection(3, 2, 1), 2);
        assert_eq!(clamp_selection(3, 1, 1), 2);
        assert_eq!(clamp_selection(0, 0, 1), 0);
    }

    #[test]
    fn the_list_index_counts_file_headings() {
        let peek = peek(&[("C:/a.rs", 10), ("C:/a.rs", 20), ("C:/b.rs", 3)]);
        assert_eq!(row_index(&peek, 0), 1);
        assert_eq!(row_index(&peek, 1), 2);
        assert_eq!(row_index(&peek, 2), 4);
    }
}
