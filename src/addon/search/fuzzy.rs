//! Fuzzy subsequence matching for file names and symbols. Pure.
//!
//! A query matches when its characters appear in order in the candidate, so
//! `btn` matches `Button.tsx`. Matches on a word boundary or a run of adjacent
//! characters score higher, which is enough to rank results sensibly.

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hit {
    pub score: i32,
    /// Char indices into the candidate that matched, in order.
    pub positions: Vec<usize>,
}

/// Match `needle` against `hay`, case-insensitively. `None` when the needle is
/// empty or is not a subsequence of the candidate.
pub fn fuzzy(needle: &str, hay: &str) -> Option<Hit> {
    let needle: Vec<char> = needle.chars().map(lower).collect();
    if needle.is_empty() {
        return None;
    }
    let chars: Vec<char> = hay.chars().collect();
    let lower: Vec<char> = chars.iter().copied().map(lower).collect();
    let mut next = 0;
    let mut positions = Vec::new();
    let mut score = 0;
    let mut previous: Option<usize> = None;
    for (i, c) in lower.iter().enumerate() {
        if needle.get(next) != Some(c) {
            continue;
        }
        next += 1;
        if previous == i.checked_sub(1) {
            score += 3; // adjacent to the previous match
        } else if i == 0 || is_boundary(chars[i - 1]) {
            score += 2; // start of the name, or a word boundary
        } else {
            score += 1;
        }
        previous = Some(i);
        positions.push(i);
    }
    (next == needle.len()).then_some(Hit { score, positions })
}

/// Lowercase one character, keeping the char count aligned with the source.
fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn is_boundary(c: char) -> bool {
    matches!(c, '/' | '\\' | '_' | '-' | '.' | ' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_letters_in_order_and_reports_them() {
        let hit = fuzzy("btn", "Button.tsx").unwrap();
        assert_eq!(hit.positions, vec![0, 2, 5]);
        assert!(fuzzy("xyz", "Button.tsx").is_none());
        // Order matters.
        assert!(fuzzy("ntb", "Button.tsx").is_none());
    }

    #[test]
    fn case_is_ignored() {
        assert!(fuzzy("BUSY", "setBusy").is_some());
    }

    #[test]
    fn ranks_consecutive_and_boundary_matches_higher() {
        let run = fuzzy("bus", "Busy").unwrap();
        let loose = fuzzy("bsy", "Busy").unwrap();
        assert!(run.score > loose.score);
        // A boundary match beats a match in the middle of a word.
        let boundary = fuzzy("bt", "foo_bar.ts").unwrap();
        let middle = fuzzy("oa", "foo_bar.ts").unwrap();
        assert!(boundary.score > middle.score);
    }

    #[test]
    fn empty_needle_matches_nothing() {
        assert!(fuzzy("", "anything").is_none());
    }
}
