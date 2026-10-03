//! Diff model and the pure math over it: hunk application, side-by-side pairing
//! and word-level ranges. No git2 and no gpui here, so it is unit-tested directly.
//!
//! The types live in [`types`], the maths in [`diff`], and the path and label
//! helpers in [`paths`]; this file only wires them together.

mod diff;
mod paths;
mod types;

#[cfg(test)]
mod tests;

pub use diff::{apply_hunk, side_by_side, word_ranges};
pub use paths::{abs_path, age, diff_layout_from_setting, rel_path, split_path};
pub use types::{
    BlameLine, ChangeEntry, DiffLayout, DiffLine, FileDiff, Hunk, LineMark, Origin, Section,
    WordRanges,
};
