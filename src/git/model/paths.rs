//! Path and label helpers over the diff model: repo-relative paths, split file
//! names, and the age a timestamp reads as.

use super::types::DiffLayout;
use std::path::PathBuf;

/// Human age of a timestamp: "just now", "5 days ago", "2 months ago".
pub fn age(now: i64, then: i64) -> String {
    let secs = (now - then).max(0);
    let (n, unit) = match secs {
        s if s < 60 => return "just now".into(),
        s if s < 3600 => (s / 60, "minute"),
        s if s < 86_400 => (s / 3600, "hour"),
        s if s < 2_592_000 => (s / 86_400, "day"),
        s if s < 31_536_000 => (s / 2_592_000, "month"),
        s => (s / 31_536_000, "year"),
    };
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}

/// Split a repo-relative path into (name, dir) for the file rows.
pub fn split_path(rel: &str) -> (String, String) {
    match rel.rfind('/') {
        Some(i) => (rel[i + 1..].to_string(), rel[..i].to_string()),
        None => (rel.to_string(), String::new()),
    }
}

pub fn diff_layout_from_setting(s: &str) -> DiffLayout {
    if s.eq_ignore_ascii_case("split") {
        DiffLayout::Split
    } else {
        DiffLayout::Inline
    }
}

/// A file's absolute path turned into a repo-relative one, forward slashes.
pub fn rel_path(root: &std::path::Path, path: &std::path::Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

/// An absolute path from a repo-relative one.
pub fn abs_path(root: &std::path::Path, rel: &str) -> PathBuf {
    root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))
}
