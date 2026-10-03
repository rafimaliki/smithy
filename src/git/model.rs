//! Diff model and the pure math over it: hunk application, side-by-side pairing
//! and word-level ranges. No git2 and no gpui here, so it is unit-tested directly.
use std::ops::Range;
use std::path::PathBuf;

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

/// Pair a hunk's lines for side-by-side display. A run of deletions followed by a
/// run of insertions is paired position by position; the longer run gets blank
/// cells on the other side.
pub fn side_by_side(hunk: &Hunk) -> Vec<SplitRow> {
    let mut rows = Vec::new();
    let mut i = 0;
    while i < hunk.lines.len() {
        match hunk.lines[i].origin {
            Origin::Context => {
                rows.push((Some(hunk.lines[i].clone()), Some(hunk.lines[i].clone())));
                i += 1;
            }
            _ => {
                let dels = take_run(hunk, &mut i, Origin::Del);
                let adds = take_run(hunk, &mut i, Origin::Add);
                for k in 0..dels.len().max(adds.len()) {
                    rows.push((dels.get(k).cloned(), adds.get(k).cloned()));
                }
            }
        }
    }
    rows
}

fn take_run(hunk: &Hunk, i: &mut usize, origin: Origin) -> Vec<DiffLine> {
    let mut out = Vec::new();
    while *i < hunk.lines.len() && hunk.lines[*i].origin == origin {
        out.push(hunk.lines[*i].clone());
        *i += 1;
    }
    out
}

/// Apply `hunk` to `lines` (a file split on `\n`). `forward` turns the old side
/// into the new; the reverse turns the new side back into the old.
pub fn apply_hunk(lines: &[String], hunk: &Hunk, forward: bool) -> Vec<String> {
    let (start, count, keep): (u32, u32, fn(Origin) -> bool) = if forward {
        (hunk.old_start, hunk.old_lines, |o| o != Origin::Del)
    } else {
        (hunk.new_start, hunk.new_lines, |o| o != Origin::Add)
    };
    let start = if start == 0 { 0 } else { (start - 1) as usize };
    let end = (start + count as usize).min(lines.len());
    let replacement: Vec<String> = hunk
        .lines
        .iter()
        .filter(|l| keep(l.origin))
        .map(|l| l.text.clone())
        .collect();
    let mut out = lines.to_vec();
    out.splice(start..end, replacement);
    out
}

/// Char ranges that changed on each side of a line pair.
pub type WordRanges = (Vec<Range<usize>>, Vec<Range<usize>>);

/// Char ranges whose tokens differ between the two lines, for word-level marks.
/// Tokens are runs of word chars; every other char is its own token.
pub fn word_ranges(old: &str, new: &str) -> WordRanges {
    let a = tokens(old);
    let b = tokens(new);
    let (keep_a, keep_b) = lcs(&a, &b);
    (
        merge_across_space(old, marked(&a, &keep_a)),
        merge_across_space(new, marked(&b, &keep_b)),
    )
}

/// Join ranges that are separated only by whitespace, so a changed run reads as
/// one mark rather than one per word.
fn merge_across_space(s: &str, ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if let Some(last) = out.last_mut() {
            if s[last.end..range.start].chars().all(char::is_whitespace) {
                last.end = range.end;
                continue;
            }
        }
        out.push(range);
    }
    out
}

fn tokens(s: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let word = c.is_alphanumeric() || c == '_';
        let mut end = i + c.len_utf8();
        if word {
            while let Some(&(j, d)) = chars.peek() {
                if d.is_alphanumeric() || d == '_' {
                    chars.next();
                    end = j + d.len_utf8();
                } else {
                    break;
                }
            }
        }
        out.push((i, s[i..end].to_string()));
    }
    out
}

/// Tokens kept by the longest common subsequence, one flag per token per side.
fn lcs(a: &[(usize, String)], b: &[(usize, String)]) -> (Vec<bool>, Vec<bool>) {
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i].1 == b[j].1 {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut keep_a = vec![false; n];
    let mut keep_b = vec![false; m];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i].1 == b[j].1 {
            keep_a[i] = true;
            keep_b[j] = true;
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    (keep_a, keep_b)
}

fn marked(toks: &[(usize, String)], keep: &[bool]) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    for (i, (start, text)) in toks.iter().enumerate() {
        if keep[i] {
            continue;
        }
        let end = start + text.len();
        if text.chars().all(char::is_whitespace) {
            // Keep a run of changed words together across the spaces between them,
            // but do not swallow a word that both sides share.
            let next = toks[i + 1..]
                .iter()
                .enumerate()
                .find(|(_, (_, t))| !t.chars().all(char::is_whitespace))
                .map(|(k, _)| i + 1 + k);
            let runs_on = next.is_some_and(|n| !keep[n]);
            if runs_on {
                if let Some(last) = out.last_mut() {
                    last.end = end;
                }
            }
            continue;
        }
        match out.last_mut() {
            Some(last) if last.end >= *start => last.end = end,
            _ => out.push(*start..end),
        }
    }
    out
}

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiffLayout {
    Inline,
    Split,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hunk(lines: Vec<(Origin, &str)>) -> Hunk {
        let mut old = 4;
        let mut new = 4;
        let lines: Vec<DiffLine> = lines
            .into_iter()
            .map(|(origin, text)| {
                let (o, n) = match origin {
                    Origin::Context => {
                        let r = (Some(old), Some(new));
                        old += 1;
                        new += 1;
                        r
                    }
                    Origin::Del => {
                        let r = (Some(old), None);
                        old += 1;
                        r
                    }
                    Origin::Add => {
                        let r = (None, Some(new));
                        new += 1;
                        r
                    }
                };
                DiffLine {
                    origin,
                    old: o,
                    new: n,
                    text: text.to_string(),
                }
            })
            .collect();
        let old_lines = lines.iter().filter(|l| l.origin != Origin::Add).count() as u32;
        let new_lines = lines.iter().filter(|l| l.origin != Origin::Del).count() as u32;
        let old_start = lines.iter().filter_map(|l| l.old).min().unwrap_or(0);
        let new_start = lines.iter().filter_map(|l| l.new).min().unwrap_or(0);
        Hunk {
            header: "@@ -4,2 +4,3 @@".into(),
            old_start,
            old_lines,
            new_start,
            new_lines,
            lines,
        }
    }

    #[test]
    fn forward_replaces_the_old_range_and_reverse_undoes_it() {
        let lines: Vec<String> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // Replace line 5 ("e") with "e" and "x".
        let h = hunk(vec![
            (Origin::Context, "d"),
            (Origin::Del, "e"),
            (Origin::Add, "e"),
            (Origin::Add, "x"),
        ]);
        let applied = apply_hunk(&lines, &h, true);
        assert_eq!(applied, ["a", "b", "c", "d", "e", "x", "f"]);
        assert_eq!(apply_hunk(&applied, &h, false), lines);
    }

    #[test]
    fn reverse_of_a_truncated_hunk_restores_the_extra_line() {
        let lines: Vec<String> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // Context "d", then delete "e": the new side is one line shorter.
        let h = hunk(vec![(Origin::Context, "d"), (Origin::Del, "e")]);
        let applied = apply_hunk(&lines, &h, true);
        assert_eq!(applied, ["a", "b", "c", "d", "f"]);
        assert_eq!(apply_hunk(&applied, &h, false), lines);
    }

    #[test]
    fn new_file_hunk_starts_at_zero() {
        let h = Hunk {
            header: "@@ -0,0 +1,2 @@".into(),
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: 2,
            lines: vec![
                DiffLine {
                    origin: Origin::Add,
                    old: None,
                    new: Some(1),
                    text: "one".into(),
                },
                DiffLine {
                    origin: Origin::Add,
                    old: None,
                    new: Some(2),
                    text: "two".into(),
                },
            ],
        };
        assert_eq!(apply_hunk(&[], &h, true), ["one", "two"]);
        assert_eq!(
            apply_hunk(&["one".into(), "two".into()], &h, false),
            Vec::<String>::new()
        );
    }

    #[test]
    fn side_by_side_pairs_deletions_with_insertions_and_pads() {
        let h = hunk(vec![
            (Origin::Context, "keep"),
            (Origin::Del, "old1"),
            (Origin::Add, "new1"),
            (Origin::Del, "old2"),
            (Origin::Add, "new2"),
            (Origin::Add, "extra"),
            (Origin::Context, "tail"),
        ]);
        let rows = side_by_side(&h);
        let texts: Vec<(Option<String>, Option<String>)> = rows
            .iter()
            .map(|(a, b)| {
                (
                    a.as_ref().map(|l| l.text.clone()),
                    b.as_ref().map(|l| l.text.clone()),
                )
            })
            .collect();
        assert_eq!(
            texts,
            vec![
                (Some("keep".into()), Some("keep".into())),
                (Some("old1".into()), Some("new1".into())),
                (Some("old2".into()), Some("new2".into())),
                (None, Some("extra".into())),
                (Some("tail".into()), Some("tail".into())),
            ]
        );
    }

    #[test]
    fn word_ranges_mark_only_the_changed_words() {
        let (old, new) = word_ranges("let x = children;", "let x = busy || children;");
        assert_eq!(old, Vec::<Range<usize>>::new());
        assert_eq!(&"let x = busy || children;"[new[0].clone()], "busy ||");
    }

    #[test]
    fn word_ranges_on_a_rewritten_line_marks_both_sides() {
        let (old, new) = word_ranges("aaa bbb", "ccc ddd");
        assert_eq!(&"aaa bbb"[old[0].clone()], "aaa bbb");
        assert_eq!(&"ccc ddd"[new[0].clone()], "ccc ddd");
    }

    #[test]
    fn age_reads_in_human_units() {
        assert_eq!(age(100, 100), "just now");
        assert_eq!(age(100, 40), "1 minute ago");
        assert_eq!(age(86_400 * 5, 0), "5 days ago");
        assert_eq!(age(2_592_000 * 2, 0), "2 months ago");
        assert_eq!(age(31_536_000 * 3, 0), "3 years ago");
    }

    #[test]
    fn split_path_separates_name_and_dir() {
        assert_eq!(
            split_path("src/components/Button.tsx"),
            ("Button.tsx".into(), "src/components".into())
        );
        assert_eq!(split_path("README.md"), ("README.md".into(), String::new()));
    }

    #[test]
    fn layout_setting_defaults_to_inline() {
        assert_eq!(diff_layout_from_setting("split"), DiffLayout::Split);
        assert_eq!(diff_layout_from_setting("inline"), DiffLayout::Inline);
        assert_eq!(diff_layout_from_setting("nonsense"), DiffLayout::Inline);
    }
}
