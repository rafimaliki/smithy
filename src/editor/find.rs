//! Find-in-file matching. Pure: the buffer text and a query in, matches in char
//! coordinates out, so the view can highlight and navigate without gpui. Regex
//! matching goes through the `regex` crate for every mode, so case folding and
//! word boundaries behave the same whether the regex toggle is on or off.
use regex::Regex;
use std::ops::Range;

/// A find-bar query. `case` is match-case (the Aa toggle), off by default.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct FindQuery {
    pub text: String,
    pub case: bool,
    pub word: bool,
    pub regex: bool,
}

/// One match: the line it starts on and its char columns in that line. A match
/// that spans a line break keeps the columns of its first line only.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Match {
    pub line: usize,
    pub col: Range<usize>,
}

impl FindQuery {
    /// The regular expression the query stands for: a literal is escaped, the
    /// word toggle wraps it in boundaries, and the case toggle decides `(?i)`.
    fn pattern(&self) -> String {
        let core = if self.regex {
            self.text.clone()
        } else {
            regex::escape(&self.text)
        };
        let core = if self.word {
            format!(r"\b(?:{core})\b")
        } else {
            core
        };
        if self.case {
            core
        } else {
            format!("(?i){core}")
        }
    }
}

/// Run `q` over `text`. An invalid pattern is the only error.
pub fn find_all(text: &str, q: &FindQuery) -> Result<Vec<Match>, regex::Error> {
    if q.text.is_empty() {
        return Ok(Vec::new());
    }
    let re = Regex::new(&q.pattern())?;
    let mut out = Vec::new();
    let mut line = 0usize;
    let mut col = 0usize;
    let mut byte = 0usize;
    let mut chars = text.char_indices();
    for m in re.find_iter(text) {
        // An empty match (e.g. `a*`) has nothing to highlight or count.
        if m.start() == m.end() {
            continue;
        }
        while byte < m.start() {
            let (_, ch) = chars.next().expect("match offset inside text");
            byte += ch.len_utf8();
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        let start = (line, col);
        while byte < m.end() {
            let (_, ch) = chars.next().expect("match offset inside text");
            byte += ch.len_utf8();
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        out.push(Match {
            line: start.0,
            col: start.1..start.1 + col_span(text, m.start(), m.end()),
        });
    }
    Ok(out)
}

/// Number of chars between two byte offsets; counted here to keep `find_all`'s
/// walk over matches already advanced.
fn col_span(text: &str, start: usize, end: usize) -> usize {
    text[start..end].chars().count()
}

/// Index of the first match at or after the caret, or 0 when the caret is past
/// every match (so Enter wraps to the top).
pub fn first_from(matches: &[Match], line: usize, col: usize) -> usize {
    matches
        .iter()
        .position(|m| m.line > line || (m.line == line && m.col.end > col))
        .unwrap_or(0)
}

/// The matches that start on `line`, with the index of the first one.
pub fn on_line(matches: &[Match], line: usize) -> (usize, &[Match]) {
    let start = matches.partition_point(|m| m.line < line);
    let end = start + matches[start..].partition_point(|m| m.line <= line);
    (start, &matches[start..end])
}

/// What the bar's count area shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Empty query: nothing to say.
    Idle,
    /// The match the bar is on and the total, both 1-based.
    Count(usize, usize),
    NoResults,
    BadRegex,
}

/// Find-bar state for one editor: the query, the caret the search started from,
/// and the matches. `moved` tells the first Enter to jump to the match at the
/// caret instead of skipping past it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FindState {
    pub query: FindQuery,
    /// (line, column) of the caret when the bar opened; the search anchor.
    anchor: (usize, usize),
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    pub error: bool,
    moved: bool,
}

impl FindState {
    pub fn new(query: FindQuery, anchor: (usize, usize)) -> Self {
        Self {
            query,
            anchor,
            matches: Vec::new(),
            current: None,
            error: false,
            moved: false,
        }
    }

    /// Re-run the query; the bar goes back to its match at the anchor.
    pub fn recompute(&mut self, text: &str) {
        self.moved = false;
        match find_all(text, &self.query) {
            Ok(matches) => {
                self.error = false;
                self.matches = matches;
                self.current = (!self.matches.is_empty())
                    .then(|| first_from(&self.matches, self.anchor.0, self.anchor.1));
            }
            Err(_) => {
                self.error = true;
                self.matches.clear();
                self.current = None;
            }
        }
    }

    /// Advance to the next (`delta > 0`) or previous match and return it.
    pub fn step(&mut self, delta: isize) -> Option<Match> {
        if self.matches.is_empty() {
            return None;
        }
        if self.moved {
            let n = self.matches.len();
            let cur = self.current.unwrap_or(0);
            let next = if delta >= 0 {
                (cur + 1) % n
            } else {
                (cur + n - 1) % n
            };
            self.current = Some(next);
        } else {
            self.moved = true;
        }
        self.current.map(|i| self.matches[i].clone())
    }

    pub fn status(&self) -> Status {
        if self.error {
            return Status::BadRegex;
        }
        if self.query.text.is_empty() {
            return Status::Idle;
        }
        if self.matches.is_empty() {
            return Status::NoResults;
        }
        match self.current {
            Some(i) => Status::Count(i + 1, self.matches.len()),
            None => Status::NoResults,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str) -> FindQuery {
        FindQuery {
            text: text.to_string(),
            ..Default::default()
        }
    }

    fn cols(text: &str, query: &FindQuery) -> Vec<(usize, Range<usize>)> {
        find_all(text, query)
            .unwrap()
            .into_iter()
            .map(|m| (m.line, m.col))
            .collect()
    }

    #[test]
    fn literal_is_case_insensitive_by_default() {
        let text = "Busy busy BUSY";
        assert_eq!(
            cols(text, &q("busy")),
            vec![(0, 0..4), (0, 5..9), (0, 10..14)]
        );
        let mut cased = q("busy");
        cased.case = true;
        assert_eq!(cols(text, &cased), vec![(0, 5..9)]);
    }

    #[test]
    fn whole_word_skips_substrings() {
        let mut query = q("bus");
        query.word = true;
        assert_eq!(cols("busy bus", &query), vec![(0, 5..8)]);
    }

    #[test]
    fn regex_mode_uses_the_pattern_and_reports_bad_ones() {
        let mut query = q(r"[A-Z]\w+");
        query.regex = true;
        query.case = true;
        assert_eq!(cols("let Foo = Bar;", &query), vec![(0, 4..7), (0, 10..13)]);
        let mut bad = q("(");
        bad.regex = true;
        assert!(find_all("x", &bad).is_err());
    }

    #[test]
    fn matches_carry_line_and_column() {
        assert_eq!(
            cols("one\ntwo one\n", &q("one")),
            vec![(0, 0..3), (1, 4..7)]
        );
    }

    #[test]
    fn empty_query_and_empty_matches_are_skipped() {
        assert!(cols("aaa", &q("")).is_empty());
        let mut query = q("a*");
        query.regex = true;
        // `a*` matches empty strings too; only the real run is kept.
        assert_eq!(cols("baaac", &query), vec![(0, 1..4)]);
    }

    #[test]
    fn navigation_wraps_and_first_enter_uses_the_anchor_match() {
        let mut state = FindState::new(q("x"), (0, 3));
        state.recompute("x x x");
        assert_eq!(state.status(), Status::Count(3, 3));
        // First step lands on the anchor match, later ones advance and wrap.
        assert_eq!(state.step(1).unwrap().col, 4..5);
        assert_eq!(state.step(1).unwrap().col, 0..1);
        assert_eq!(state.step(-1).unwrap().col, 4..5);
    }

    #[test]
    fn status_reports_empty_no_results_and_bad_regex() {
        let mut state = FindState::new(q(""), (0, 0));
        state.recompute("abc");
        assert_eq!(state.status(), Status::Idle);

        state.query = q("zzz");
        state.recompute("abc");
        assert_eq!(state.status(), Status::NoResults);
        assert!(state.step(1).is_none());

        state.query = q("(");
        state.query.regex = true;
        state.recompute("abc");
        assert_eq!(state.status(), Status::BadRegex);
    }

    #[test]
    fn on_line_slices_one_line_with_its_offset() {
        let matches = find_all("a\na\na\n", &q("a")).unwrap();
        let (base, slice) = on_line(&matches, 1);
        assert_eq!(base, 1);
        assert_eq!(slice.len(), 1);
        assert_eq!(slice[0].col, 0..1);
        assert_eq!(on_line(&matches, 9).1.len(), 0);
    }
}
