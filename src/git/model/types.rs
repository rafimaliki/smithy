//! The diff data types: sections, lines, hunks and file diffs. Plain data, so the
//! views and the git wrapper can share one shape.

use std::ops::Range;

/// Which side of the index a change lives on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Staged,
    Unstaged,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Staged => "Staged changes",
            Section::Unstaged => "Changes",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    Context,
    Add,
    Del,
}

#[derive(Clone, Debug)]
pub struct DiffLine {
    pub origin: Origin,
    /// 1-based line number on the old side, if the line exists there.
    pub old: Option<u32>,
    /// 1-based line number on the new side, if the line exists there.
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Hunk {
    /// The `@@ -3,6 +3,7 @@ section` header as git prints it.
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug)]
pub struct FileDiff {
    /// Repo-relative path, forward slashes.
    pub rel: String,
    pub name: String,
    pub dir: String,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    pub fn added(&self) -> usize {
        self.count(Origin::Add)
    }

    pub fn removed(&self) -> usize {
        self.count(Origin::Del)
    }

    fn count(&self, origin: Origin) -> usize {
        self.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| l.origin == origin)
            .count()
    }

    pub fn hunk_count(&self) -> usize {
        self.hunks.len()
    }
}

/// A blame summary for one line.
#[derive(Clone, Debug)]
pub struct BlameLine {
    pub author: String,
    pub age: String,
    /// Short hash; the blame hover card (a later change) shows and copies it.
    // ponytail: no blame card yet, upgrade: hover card with Copy hash.
    #[allow(dead_code)]
    pub hash: String,
    pub subject: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineMark {
    Added,
    Modified,
}

#[derive(Clone, Debug)]
pub struct ChangeEntry {
    pub rel: String,
    pub name: String,
    pub dir: String,
    pub status: char,
    pub section: Section,
}

/// One row of the side-by-side view: old side and new side, either may be blank.
pub type SplitRow = (Option<DiffLine>, Option<DiffLine>);

/// Char ranges that changed on each side of a line pair.
pub type WordRanges = (Vec<Range<usize>>, Vec<Range<usize>>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiffLayout {
    Inline,
    Split,
}
