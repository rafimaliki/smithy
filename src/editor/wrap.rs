//! Wrapping lines into visual rows: pure char-column math, no gpui, so it is
//! unit-tested directly. `EditorView` draws one fixed-height row per `Row`.
use super::layout::TAB_WIDTH;

/// One visual row: char columns `start..end` of `line`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

/// Every visual row of a buffer at one wrap width, in reading order.
pub struct WrapIndex {
    rows: Vec<Row>,
    /// Row index where each line starts; one past the last line at the end.
    line_start: Vec<usize>,
}

/// Display columns one char takes in the monospace grid.
fn width(c: char) -> usize {
    if c == '\t' {
        TAB_WIDTH
    } else {
        1
    }
}

impl WrapIndex {
    /// Wrap `lines` at `cols` display columns, breaking at spaces when one is
    /// within the row and hard-breaking a word longer than the row.
    pub fn build(lines: &[String], cols: usize) -> Self {
        let cols = cols.max(1);
        let mut rows = Vec::new();
        let mut line_start = Vec::with_capacity(lines.len() + 1);
        let mut chars = Vec::new();
        for (line, text) in lines.iter().enumerate() {
            line_start.push(rows.len());
            chars.clear();
            chars.extend(text.chars());
            wrap_line(line, &chars, cols, &mut rows);
        }
        line_start.push(rows.len());
        Self { rows, line_start }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn row(&self, i: usize) -> Row {
        self.rows[i]
    }

    /// The row holding char column `col` of `line`. A column exactly at a soft
    /// break belongs to the row below, matching where the caret is drawn.
    pub fn row_of(&self, line: usize, col: usize) -> usize {
        let first = self.line_start[line];
        let last = self.line_start[line + 1];
        (first..last)
            .find(|&i| col < self.rows[i].end)
            .unwrap_or(last - 1)
    }
}

fn wrap_line(line: usize, chars: &[char], cols: usize, rows: &mut Vec<Row>) {
    let mut start = 0;
    let mut used = 0;
    // Char index of the last space at which this row could break.
    let mut space: Option<usize> = None;
    for (i, c) in chars.iter().enumerate() {
        let cw = width(*c);
        if used + cw > cols && i > start {
            // Keep the trailing space on the row that ends here.
            let brk = match space {
                Some(s) if s + 1 > start => s + 1,
                _ => i,
            };
            rows.push(Row {
                line,
                start,
                end: brk,
            });
            used = chars[brk..=i].iter().map(|c| width(*c)).sum();
            space = if *c == ' ' { Some(i) } else { None };
            start = brk;
            continue;
        }
        if *c == ' ' {
            space = Some(i);
        }
        used += cw;
    }
    rows.push(Row {
        line,
        start,
        end: chars.len(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(texts: &[&str], cols: usize) -> WrapIndex {
        let lines: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        WrapIndex::build(&lines, cols)
    }

    /// `(line, start, end)` of every row, for compact assertions.
    fn rows(idx: &WrapIndex) -> Vec<(usize, usize, usize)> {
        (0..idx.len())
            .map(|i| {
                let r = idx.row(i);
                (r.line, r.start, r.end)
            })
            .collect()
    }

    #[test]
    fn every_line_has_one_row_when_it_fits() {
        let idx = build(&["ab", "", "abcdef"], 6);
        assert_eq!(rows(&idx), vec![(0, 0, 2), (1, 0, 0), (2, 0, 6)]);
    }

    #[test]
    fn breaks_at_a_space_and_keeps_it_on_the_first_row() {
        let idx = build(&["aa bb cc dd"], 5);
        assert_eq!(rows(&idx), vec![(0, 0, 3), (0, 3, 6), (0, 6, 11)]);
    }

    #[test]
    fn a_line_that_fills_the_width_is_not_split() {
        let idx = build(&["bb cc"], 5);
        assert_eq!(rows(&idx), vec![(0, 0, 5)]);
    }

    #[test]
    fn hard_breaks_a_word_longer_than_the_row() {
        let idx = build(&["abcdefgh"], 3);
        assert_eq!(rows(&idx), vec![(0, 0, 3), (0, 3, 6), (0, 6, 8)]);
    }

    #[test]
    fn a_tab_takes_four_columns() {
        let idx = build(&["\tabc"], 4);
        assert_eq!(rows(&idx), vec![(0, 0, 1), (0, 1, 4)]);
    }

    #[test]
    fn row_of_maps_a_boundary_column_to_the_row_below() {
        let idx = build(&["aa bb cc dd"], 5);
        assert_eq!(idx.row_of(0, 0), 0);
        assert_eq!(idx.row_of(0, 2), 0);
        assert_eq!(idx.row_of(0, 3), 1);
        assert_eq!(idx.row_of(0, 5), 1);
        assert_eq!(idx.row_of(0, 6), 2);
        assert_eq!(idx.row_of(0, 11), 2);
    }

    #[test]
    fn a_zero_width_still_produces_rows() {
        let idx = build(&["ab"], 0);
        assert_eq!(idx.len(), 2);
    }
}
