//! VT escape handling: the pty's bytes go through the `vte` parser into a [`Grid`].
//!
//! A program (ConPTY at startup, and anything else) can ask the terminal to report
//! its cursor or size; those requests are collected into [`Emulator::take_reply`]
//! for the session to write back. ConPTY stalls until it gets its `ESC[6n` answer.
//!
//! ponytail: no alt screen, so a full-screen program draws over the session's
//! scrollback; upgrade: a second grid swapped by `ESC[?1049h/l`.

use super::grid::{Color, Grid, Pen};
use std::io::Write as _;
use vte::{Params, Perform};

/// Owns the parser and the screen the pty writes into. Not `Sync`; the session
/// wraps it in a mutex to share it with its reader thread.
pub struct Emulator {
    parser: vte::Parser,
    grid: Grid,
    /// Bytes of an OSC string in progress (window titles); discarded.
    osc: Vec<u8>,
    /// What the program asked the terminal to send back, e.g. a cursor report.
    reply: Vec<u8>,
}

impl Emulator {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            parser: vte::Parser::new(),
            grid: Grid::new(cols, rows),
            osc: Vec::new(),
            reply: Vec::new(),
        }
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn grid_mut(&mut self) -> &mut Grid {
        &mut self.grid
    }

    /// Take the bytes the program asked for, leaving the buffer empty.
    pub fn take_reply(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.reply)
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        let mut screen = Screen {
            grid: &mut self.grid,
            osc: &mut self.osc,
            reply: &mut self.reply,
        };
        self.parser.advance(&mut screen, bytes);
    }
}

/// The `vte` callbacks, writing straight into the grid.
struct Screen<'a> {
    grid: &'a mut Grid,
    osc: &'a mut Vec<u8>,
    reply: &'a mut Vec<u8>,
}

impl Perform for Screen<'_> {
    fn print(&mut self, c: char) {
        self.grid.put(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x08 => self.grid.backspace(),
            0x09 => self.grid.tab(),
            // LF, VT and FF all move down; the carriage return is separate.
            0x0a..=0x0c => self.grid.linefeed(),
            0x0d => self.grid.carriage_return(),
            _ => {} // BEL and the rest are ignored.
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let p: Vec<u16> = params
            .iter()
            .map(|s| s.first().copied().unwrap_or(0))
            .collect();
        // A parameter of 0 or a missing one means the same as 1 for movement.
        let n = |i: usize| -> usize { p.get(i).copied().filter(|v| *v != 0).unwrap_or(1) as usize };
        let private = intermediates.first() == Some(&b'?');
        match action {
            'A' => self.grid.move_up(n(0)),
            'B' => self.grid.move_down(n(0)),
            'C' => self.grid.move_forward(n(0)),
            'D' => self.grid.move_back(n(0)),
            'E' => {
                self.grid.move_down(n(0));
                self.grid.carriage_return();
            }
            'F' => {
                self.grid.move_up(n(0));
                self.grid.carriage_return();
            }
            'G' => self.grid.set_col(n(0)),
            'd' => self.grid.set_row(n(0)),
            'H' | 'f' => self.grid.set_cursor(n(0), n(1)),
            'J' => self.grid.clear_screen(p.first().copied().unwrap_or(0)),
            'K' => self.grid.clear_line(p.first().copied().unwrap_or(0)),
            'X' => self.grid.erase_chars(n(0)),
            'L' => self.grid.insert_lines(n(0)),
            'M' => self.grid.delete_lines(n(0)),
            'S' => self.grid.scroll_up(n(0)),
            'T' => self.grid.scroll_down(n(0)),
            'r' => self.grid.set_scroll_region(
                p.first().copied().unwrap_or(1) as usize,
                p.get(1).copied().unwrap_or(0) as usize,
            ),
            'm' => sgr(self.grid, &p),
            // Device status report: the program is waiting for this answer.
            'n' => match p.first().copied().unwrap_or(0) {
                5 => self.reply.extend_from_slice(b"\x1b[0n"),
                6 => {
                    let (row, col) = self.grid.cursor();
                    let _ = write!(self.reply, "\x1b[{};{}R", row + 1, col + 1);
                }
                _ => {}
            },
            // Window size in characters.
            't' if p.first() == Some(&18) => {
                let _ = write!(
                    self.reply,
                    "\x1b[8;{};{}t",
                    self.grid.rows(),
                    self.grid.cols()
                );
            }
            // Private modes: only the cursor's visibility is used.
            'h' | 'l' if private && p.first() == Some(&25) => {
                self.grid.set_cursor_visible(action == 'h');
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'7' => self.grid.save_cursor(),
            b'8' => self.grid.restore_cursor(),
            b'D' => self.grid.linefeed(),
            b'M' => self.grid.reverse_index(),
            b'c' => {
                self.grid.clear_screen(2);
                self.grid.reset_pen();
                self.grid.set_cursor(1, 1);
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {
        // Window titles and shell integration marks: dropped.
        self.osc.clear();
    }
}

/// SGR: the pen the next `print` uses.
fn sgr(grid: &mut Grid, p: &[u16]) {
    let mut pen = if p.is_empty() {
        Pen::default()
    } else {
        grid.pen()
    };
    let mut i = 0;
    while i < p.len() {
        match p[i] {
            0 => pen = Pen::default(),
            1 => pen.bold = true,
            22 => pen.bold = false,
            7 => pen.inverse = true,
            27 => pen.inverse = false,
            30..=37 => pen.fg = Color::Indexed((p[i] - 30) as u8),
            90..=97 => pen.fg = Color::Indexed((p[i] - 90 + 8) as u8),
            39 => pen.fg = Color::Default,
            40..=47 => pen.bg = Color::Indexed((p[i] - 40) as u8),
            100..=107 => pen.bg = Color::Indexed((p[i] - 100 + 8) as u8),
            49 => pen.bg = Color::Default,
            38 | 48 => {
                let (color, used) = extended(&p[i..]);
                if p[i] == 38 {
                    pen.fg = color;
                } else {
                    pen.bg = color;
                }
                i += used;
            }
            _ => {}
        }
        i += 1;
    }
    grid.set_pen(pen);
}

/// `38`/`48` with their `5;n` or `2;r;g;b` tail: the color and how many extra
/// parameters it consumed.
fn extended(p: &[u16]) -> (Color, usize) {
    match p.get(1).copied() {
        Some(5) => (Color::Indexed(p.get(2).copied().unwrap_or(0) as u8), 2),
        Some(2) => (
            Color::Rgb(
                p.get(2).copied().unwrap_or(0) as u8,
                p.get(3).copied().unwrap_or(0) as u8,
                p.get(4).copied().unwrap_or(0) as u8,
            ),
            4,
        ),
        _ => (Color::Default, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emu() -> Emulator {
        Emulator::new(10, 3)
    }

    fn row(e: &Emulator, i: usize) -> String {
        e.grid()
            .line(e.grid().scrollback_len() + i)
            .iter()
            .map(|c| c.c)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn plain_text_lands_on_the_screen() {
        let mut e = emu();
        e.feed(b"hello");
        assert_eq!(row(&e, 0), "hello");
        assert_eq!(e.grid().cursor(), (0, 5));
    }

    #[test]
    fn clear_and_home_start_a_fresh_screen() {
        let mut e = emu();
        e.feed(b"junk\x1b[2J\x1b[Hok");
        assert_eq!(row(&e, 0), "ok");
        assert_eq!(row(&e, 1), "");
    }

    #[test]
    fn newline_and_carriage_return_move_the_cursor() {
        let mut e = emu();
        e.feed(b"a\r\nb");
        assert_eq!(row(&e, 0), "a");
        assert_eq!(row(&e, 1), "b");
    }

    #[test]
    fn cursor_position_places_the_next_char() {
        let mut e = emu();
        e.feed(b"\x1b[2;5Hx");
        assert_eq!(row(&e, 1), "    x");
    }

    #[test]
    fn sgr_sets_indexed_and_true_colors() {
        let mut e = emu();
        e.feed(b"\x1b[31;1mR\x1b[0m");
        let cell = e.grid().line(0)[0];
        assert_eq!(cell.c, 'R');
        assert_eq!(cell.pen.fg, Color::Indexed(1));
        assert!(cell.pen.bold);
        e.feed(b"\x1b[38;5;196mX");
        assert_eq!(e.grid().line(0)[1].pen.fg, Color::Indexed(196));
        e.feed(b"\x1b[48;2;10;20;30mY");
        assert_eq!(e.grid().line(0)[2].pen.bg, Color::Rgb(10, 20, 30));
    }

    #[test]
    fn erase_line_clears_the_whole_line_from_any_column() {
        let mut e = emu();
        e.feed(b"abc\x1b[2K\x1b[Hz");
        assert_eq!(row(&e, 0), "z");
    }

    #[test]
    fn erase_to_start_keeps_what_follows_the_cursor() {
        let mut e = emu();
        e.feed(b"abcd\x1b[1;3H\x1b[1J");
        assert_eq!(row(&e, 0), "   d");
    }

    #[test]
    fn cursor_visibility_and_osc_titles_are_consumed() {
        let mut e = emu();
        e.feed(b"\x1b]0;a title\x07\x1b[?25l");
        assert!(!e.grid().is_cursor_visible());
        e.feed(b"\x1b[?25h");
        assert!(e.grid().is_cursor_visible());
        assert_eq!(row(&e, 0), "");
    }

    #[test]
    fn cursor_report_answers_with_one_based_position() {
        let mut e = Emulator::new(10, 3);
        e.feed(b"\x1b[2;5H\x1b[6n");
        assert_eq!(e.take_reply(), b"\x1b[2;5R".to_vec());
        // Taken, not repeated.
        assert!(e.take_reply().is_empty());
        e.feed(b"\x1b[18t");
        assert_eq!(e.take_reply(), b"\x1b[8;3;10t".to_vec());
    }

    #[test]
    fn scroll_region_limits_where_output_scrolls() {
        let mut e = Emulator::new(5, 4);
        e.feed(b"top\x1b[3;4r\x1b[4;1Hone\r\ntwo\r\nthree");
        assert_eq!(row(&e, 0), "top");
        assert_eq!(row(&e, 3), "three");
    }
}
