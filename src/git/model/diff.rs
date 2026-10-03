//! The pure diff maths: hunk application, side-by-side pairing and word-level
//! ranges. No git2 and no gpui, so it is unit-tested directly.

use super::types::{DiffLine, Hunk, Origin, SplitRow, WordRanges};
use std::ops::Range;

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
    // Clamp so a hunk that no longer fits the text edits the end, never panics.
    let start = (start.saturating_sub(1) as usize).min(lines.len());
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
