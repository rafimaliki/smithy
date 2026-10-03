//! Unit tests for the diff maths and the path helpers.

use super::*;
use std::ops::Range;

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
