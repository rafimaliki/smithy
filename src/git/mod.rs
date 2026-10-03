//! Git domain: a git2 wrapper, the diff model and the pure diff maths.
//! The Source control add-on is the only consumer; nothing here touches gpui.
pub mod apply;
pub mod inspect;
pub mod model;
pub mod pull;
pub mod repo;

pub use model::{
    abs_path, diff_layout_from_setting, rel_path, side_by_side, split_path, word_ranges,
    ChangeEntry, DiffLayout, DiffLine, FileDiff, Hunk, LineMark, Origin, Section, WordRanges,
};
pub use repo::{Compare, Repo};
