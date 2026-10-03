//! The terminal screen: a grid of styled cells plus a scrollback, and the
//! operations the escape-sequence handler drives. Pure model, so the parser tests
//! run it without a pty.
//!
//! ponytail: one cell per char (no wide CJK), no deferred wrap, scrollback capped,
//! resize keeps the top lines; upgrade: width tables, real reflow, damage tracking.

use std::collections::VecDeque;

/// Scrolled-off lines kept for reading back. Older ones are dropped.
pub const SCROLLBACK_MAX: usize = 1000;

/// A color as the pty reported it: the terminal default, an ANSI index, or true color.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// How one cell is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Pen {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub inverse: bool,
}

/// One character cell. `Default` is a blank in the default pen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub c: char,
    pub pen: Pen,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            pen: Pen::default(),
        }
    }
}

/// A slice of the buffer the view draws: whole rows, plus where the cursor sits
/// among them. `None` when the cursor is scrolled out of sight or hidden.
pub struct Window {
    pub rows: Vec<Vec<Cell>>,
    pub cursor: Option<(usize, usize)>,
}

pub struct Grid {
    cols: usize,
    rows: usize,
    lines: Vec<Vec<Cell>>,
    scrollback: VecDeque<Vec<Cell>>,
    row: usize,
    col: usize,
    pen: Pen,
    /// Scroll region, inclusive rows. Full screen unless a program set one.
    top: usize,
    bottom: usize,
    cursor_visible: bool,
    saved: Option<(usize, usize)>,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let (cols, rows) = (cols.max(1), rows.max(1));
        Self {
            cols,
            rows,
            lines: vec![blank(cols); rows],
            scrollback: VecDeque::new(),
            row: 0,
            col: 0,
            pen: Pen::default(),
            top: 0,
            bottom: rows - 1,
            cursor_visible: true,
            saved: None,
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    /// Test-only helper; the view reads [`window`](Self::window) instead.
    #[cfg(test)]
    pub fn is_cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    #[cfg(test)]
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    pub fn total_lines(&self) -> usize {
        self.scrollback.len() + self.rows
    }

    /// Buffer line `i`, oldest first (scrollback then screen). Empty when past the end.
    pub fn line(&self, i: usize) -> &[Cell] {
        if i < self.scrollback.len() {
            &self.scrollback[i]
        } else {
            self.lines.get(i - self.scrollback.len()).map_or(&[], |l| l)
        }
    }

    /// The `height` rows ending `scroll` lines above the bottom, with the cursor
    /// placed among them.
    pub fn window(&self, height: usize, scroll: usize) -> Window {
        let total = self.total_lines();
        let visible = height.clamp(1, total);
        let scroll = scroll.min(total - visible);
        let end = total - scroll;
        let start = end - visible;
        let rows = (start..end).map(|i| self.line(i).to_vec()).collect();
        let abs = self.scrollback.len() + self.row;
        let cursor =
            (self.cursor_visible && (start..end).contains(&abs)).then(|| (abs - start, self.col));
        Window { rows, cursor }
    }

    /// Grow or shrink to `cols` x `rows`, keeping the top-left of the screen.
    // ponytail: no reflow and no scrollback into the screen; upgrade: rewrap lines.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let (cols, rows) = (cols.max(1), rows.max(1));
        if cols == self.cols && rows == self.rows {
            return;
        }
        for line in self.lines.iter_mut() {
            line.resize(cols, Cell::default());
        }
        self.lines.resize(rows, blank(cols));
        self.cols = cols;
        self.rows = rows;
        self.top = 0;
        self.bottom = rows - 1;
        self.row = self.row.min(rows - 1);
        self.col = self.col.min(cols - 1);
    }

    // ---- pen ----------------------------------------------------------------

    pub fn set_pen(&mut self, pen: Pen) {
        self.pen = pen;
    }

    pub fn reset_pen(&mut self) {
        self.pen = Pen::default();
    }

    pub fn pen(&self) -> Pen {
        self.pen
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    pub fn save_cursor(&mut self) {
        self.saved = Some((self.row, self.col));
    }

    pub fn restore_cursor(&mut self) {
        if let Some((row, col)) = self.saved {
            self.set_cursor(row, col);
        }
    }

    // ---- writing ------------------------------------------------------------

    /// Draw `c` at the cursor and advance, wrapping to the next line at the edge.
    pub fn put(&mut self, c: char) {
        if self.col >= self.cols {
            self.col = 0;
            self.linefeed();
        }
        let (row, col, pen) = (self.row, self.col, self.pen);
        let cell = &mut self.lines[row][col];
        *cell = Cell { c, pen };
        self.col += 1;
    }

    pub fn linefeed(&mut self) {
        if self.row == self.bottom {
            self.scroll_up(1);
        } else if self.row < self.rows - 1 {
            self.row += 1;
        }
    }

    pub fn reverse_index(&mut self) {
        if self.row == self.top {
            self.scroll_down(1);
        } else if self.row > 0 {
            self.row -= 1;
        }
    }

    pub fn carriage_return(&mut self) {
        self.col = 0;
    }

    pub fn backspace(&mut self) {
        self.col = self.col.saturating_sub(1);
    }

    pub fn tab(&mut self) {
        let next = (self.col / 8 + 1) * 8;
        self.col = next.min(self.cols - 1);
    }

    pub fn move_up(&mut self, n: usize) {
        self.row = self.row.saturating_sub(n.max(1));
    }

    pub fn move_down(&mut self, n: usize) {
        self.row = (self.row + n.max(1)).min(self.bottom);
    }

    pub fn move_forward(&mut self, n: usize) {
        self.col = (self.col + n.max(1)).min(self.cols - 1);
    }

    pub fn move_back(&mut self, n: usize) {
        self.col = self.col.saturating_sub(n.max(1));
    }

    /// 1-based, as the cursor reports it; out-of-range values clamp.
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.row = row.saturating_sub(1).min(self.rows - 1);
        self.col = col.saturating_sub(1).min(self.cols - 1);
    }

    pub fn set_row(&mut self, row: usize) {
        self.row = row.saturating_sub(1).min(self.rows - 1);
    }

    pub fn set_col(&mut self, col: usize) {
        self.col = col.saturating_sub(1).min(self.cols - 1);
    }

    /// DECSTBM: rows are 1-based, `bottom` is exclusive on the wire (`0` = last row).
    pub fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        let top = top.max(1) - 1;
        let bottom = if bottom == 0 {
            self.rows - 1
        } else {
            (bottom - 1).min(self.rows - 1)
        };
        if top < bottom {
            self.top = top;
            self.bottom = bottom;
            self.set_cursor(1, 1);
        }
    }

    pub fn clear_line(&mut self, mode: u16) {
        let (cols, row, col) = (self.cols, self.row, self.col);
        let line = &mut self.lines[row];
        match mode {
            0 => line[col..].iter_mut().for_each(|c| *c = Cell::default()),
            1 => line[..=col.min(cols - 1)]
                .iter_mut()
                .for_each(|c| *c = Cell::default()),
            _ => line.iter_mut().for_each(|c| *c = Cell::default()),
        }
    }

    pub fn clear_screen(&mut self, mode: u16) {
        let blank = blank(self.cols);
        match mode {
            1 => {
                for r in 0..self.row {
                    self.lines[r] = blank.clone();
                }
                self.clear_line(1);
            }
            2 | 3 => {
                self.lines.iter_mut().for_each(|l| *l = blank.clone());
                if mode == 3 {
                    self.scrollback.clear();
                }
            }
            _ => {
                self.clear_line(0);
                for r in self.row + 1..self.rows {
                    self.lines[r] = blank.clone();
                }
            }
        }
    }

    /// ECH: blank `n` cells from the cursor, without moving it.
    pub fn erase_chars(&mut self, n: usize) {
        let end = (self.col + n.max(1)).min(self.cols);
        for cell in &mut self.lines[self.row][self.col..end] {
            *cell = Cell::default();
        }
    }

    pub fn insert_lines(&mut self, n: usize) {
        if !(self.top..=self.bottom).contains(&self.row) {
            return;
        }
        for _ in 0..n.max(1) {
            self.lines.remove(self.bottom);
            self.lines.insert(self.row, blank(self.cols));
        }
    }

    pub fn delete_lines(&mut self, n: usize) {
        if !(self.top..=self.bottom).contains(&self.row) {
            return;
        }
        for _ in 0..n.max(1) {
            self.lines.remove(self.row);
            self.lines.insert(self.bottom, blank(self.cols));
        }
    }

    /// Scroll the region up by `n`, feeding the scrollback when it is the full screen.
    pub fn scroll_up(&mut self, n: usize) {
        for _ in 0..n {
            let line = self.lines.remove(self.top);
            self.lines.insert(self.bottom, blank(self.cols));
            // Only the full-screen region feeds the scrollback.
            if self.top == 0 && self.bottom == self.rows - 1 {
                self.scrollback.push_back(line);
                while self.scrollback.len() > SCROLLBACK_MAX {
                    self.scrollback.pop_front();
                }
            }
        }
    }

    /// Scroll the region down by `n`.
    pub fn scroll_down(&mut self, n: usize) {
        for _ in 0..n {
            self.lines.remove(self.bottom);
            self.lines.insert(self.top, blank(self.cols));
        }
    }
}

fn blank(cols: usize) -> Vec<Cell> {
    vec![Cell::default(); cols]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(grid: &Grid, row: usize) -> String {
        grid.line(grid.scrollback_len() + row)
            .iter()
            .map(|c| c.c)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn put_wraps_at_the_edge() {
        let mut g = Grid::new(4, 3);
        for c in "abcdef".chars() {
            g.put(c);
        }
        assert_eq!(text(&g, 0), "abcd");
        assert_eq!(text(&g, 1), "ef");
        assert_eq!(g.cursor(), (1, 2));
    }

    #[test]
    fn linefeed_at_the_bottom_scrolls_into_the_scrollback() {
        let mut g = Grid::new(4, 2);
        g.put('a');
        g.carriage_return();
        g.linefeed();
        g.put('b');
        g.carriage_return();
        g.linefeed();
        g.put('c');
        assert_eq!(g.scrollback_len(), 1);
        assert_eq!(text(&g, 0), "b");
        assert_eq!(text(&g, 1), "c");
    }

    #[test]
    fn clear_line_modes() {
        let mut g = Grid::new(4, 1);
        for c in "abcd".chars() {
            g.put(c);
        }
        g.set_cursor(1, 3);
        g.clear_line(0);
        assert_eq!(text(&g, 0), "ab");
        g.clear_line(2);
        assert_eq!(text(&g, 0), "");
    }

    #[test]
    fn window_places_the_cursor_and_respects_scroll() {
        let mut g = Grid::new(3, 2);
        for c in "abcdefghij".chars() {
            g.put(c);
        }
        // 10 chars over two 3-wide rows: lines "abc", "def", ..., scrollback full.
        let w = g.window(2, 0);
        assert_eq!(w.rows.len(), 2);
        assert_eq!(w.cursor, Some((1, 1)));
        let scrolled = g.window(2, 1);
        assert_eq!(scrolled.rows.len(), 2);
        assert_eq!(scrolled.cursor, None);
    }

    #[test]
    fn scroll_region_keeps_the_screen_above_it() {
        let mut g = Grid::new(4, 4);
        g.put('x');
        g.set_scroll_region(3, 4);
        g.set_cursor(4, 1);
        g.linefeed();
        // The region scrolled; the top two rows are untouched.
        assert_eq!(text(&g, 0), "x");
        assert_eq!(g.scrollback_len(), 0);
    }
}
