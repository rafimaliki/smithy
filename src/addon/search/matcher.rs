//! Text-search query: the three toggles, how a query is compiled, and the
//! character ranges it matches on one line. Pure, so it is tested directly.
use regex::{Regex, RegexBuilder};

/// The Text tab's toggles.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Options {
    /// Aa: match case (off means case-insensitive).
    pub case: bool,
    /// ab: whole word.
    pub word: bool,
    /// .*: treat the query as a regular expression.
    pub regex: bool,
}

/// Compile `pattern` with `opts`. `Ok(None)` means the query is empty (no
/// search); `Err` carries the message shown for a broken regular expression.
pub fn compile(pattern: &str, opts: Options) -> Result<Option<Regex>, String> {
    if pattern.is_empty() {
        return Ok(None);
    }
    let mut body = if opts.regex {
        pattern.to_string()
    } else {
        regex::escape(pattern)
    };
    if opts.word {
        body = format!(r"\b(?:{body})\b");
    }
    RegexBuilder::new(&body)
        .case_insensitive(!opts.case)
        .build()
        .map(Some)
        .map_err(|e| e.to_string())
}

/// Character ranges of every match of `re` in `line`.
pub fn ranges(re: &Regex, line: &str) -> Vec<(usize, usize)> {
    re.find_iter(line)
        .filter(|m| m.end() > m.start())
        .map(|m| (chars_before(line, m.start()), chars_before(line, m.end())))
        .collect()
}

fn chars_before(line: &str, byte: usize) -> usize {
    line[..byte.min(line.len())].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(pattern: &str, opts: Options, line: &str) -> Vec<(usize, usize)> {
        let re = compile(pattern, opts).unwrap().unwrap();
        ranges(&re, line)
    }

    #[test]
    fn empty_query_compiles_to_nothing() {
        assert!(compile("", Options::default()).unwrap().is_none());
    }

    #[test]
    fn literal_is_case_insensitive_until_case_is_on() {
        let loose = Options::default();
        assert_eq!(text("busy", loose, "let Busy = 1"), vec![(4, 8)]);
        let strict = Options {
            case: true,
            ..Options::default()
        };
        assert!(text("busy", strict, "let Busy = 1").is_empty());
        assert_eq!(text("Busy", strict, "let Busy = 1"), vec![(4, 8)]);
    }

    #[test]
    fn whole_word_skips_substrings() {
        let opts = Options {
            word: true,
            ..Options::default()
        };
        assert!(text("busy", opts, "busyness rules").is_empty());
        assert_eq!(text("busy", opts, "is busy now"), vec![(3, 7)]);
    }

    #[test]
    fn regex_mode_uses_the_pattern_and_reports_bad_ones() {
        let opts = Options {
            regex: true,
            ..Options::default()
        };
        assert_eq!(text("b.sy", opts, "busy busy"), vec![(0, 4), (5, 9)]);
        // Without regex mode the dots are literal.
        assert!(text("b.sy", Options::default(), "busy").is_empty());
        assert!(compile("(", opts).is_err());
    }

    #[test]
    fn ranges_are_char_offsets_not_bytes() {
        let loose = Options::default();
        assert_eq!(text("é", loose, "aébc"), vec![(1, 2)]);
    }
}
