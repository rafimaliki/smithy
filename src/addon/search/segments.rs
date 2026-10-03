//! Split text into plain and highlighted runs for rendering. Pure.
//!
//! Ranges are char indices; they are clamped to the text and may overlap.

/// `(run text, is_highlighted)` pairs covering `text` in order.
pub fn segments(text: &str, ranges: &[(usize, usize)]) -> Vec<(String, bool)> {
    let chars: Vec<char> = text.chars().collect();
    let mut marked = vec![false; chars.len()];
    for &(start, end) in ranges {
        for slot in marked.iter_mut().take(end.min(chars.len())).skip(start) {
            *slot = true;
        }
    }
    let mut out: Vec<(String, bool)> = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        match out.last_mut() {
            Some((run, hot)) if *hot == marked[i] => run.push(*c),
            _ => out.push((c.to_string(), marked[i])),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(pairs: &[(String, bool)]) -> Vec<(&str, bool)> {
        pairs.iter().map(|(s, h)| (s.as_str(), *h)).collect()
    }

    #[test]
    fn no_ranges_is_one_plain_run() {
        assert_eq!(plain(&segments("let x", &[])), vec![("let x", false)]);
    }

    #[test]
    fn a_range_splits_into_three_runs() {
        assert_eq!(
            plain(&segments("let busy = 1", &[(4, 8)])),
            vec![("let ", false), ("busy", true), (" = 1", false)]
        );
    }

    #[test]
    fn overlapping_and_adjacent_ranges_merge_and_clamp() {
        assert_eq!(
            plain(&segments("abcd", &[(1, 3), (2, 4), (9, 12)])),
            vec![("a", false), ("bcd", true)]
        );
    }

    #[test]
    fn ranges_are_char_offsets() {
        assert_eq!(
            plain(&segments("aébc", &[(1, 2)])),
            vec![("a", false), ("é", true), ("bc", false)]
        );
    }
}
