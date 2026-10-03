//! Word wrap and horizontal scroll for the editor: build the wrap index at the
//! current width, remember the choice, keep the caret visible, and map a click
//! to a caret position. Row drawing is in `rows.rs`.
use super::gutter::BLAME_W;
use super::{EditorView, GUTTER, LINE_H};
use crate::actions::ToggleWrap;
use crate::editor::layout::{col_at_visual, visual_col};
use crate::editor::wrap::{Row, WrapIndex};
use crate::settings::Settings;
use gpui::{Context, Pixels, ScrollDelta, ScrollStrategy, ScrollWheelEvent, Window};

impl EditorView {
    /// True when this editor wraps long lines.
    pub fn wraps(&self) -> bool {
        self.wrap
    }

    /// The width available to text; wrapping measures against it.
    pub(super) fn text_width(&self) -> f32 {
        let blame = if self.blame_column { BLAME_W } else { 0.0 };
        (f32::from(self.bounds.size.width) - GUTTER - blame).max(0.0)
    }

    /// The number of rows `uniform_list` shows: wrapped rows, or one per line.
    pub(super) fn visual_row_count(&self) -> usize {
        match &self.wrap_index {
            Some(idx) => idx.len(),
            None => self.state.buffer.len_lines(),
        }
    }

    /// The visual row at index `i`; the whole logical line when wrap is off.
    pub(super) fn row_at(&self, i: usize) -> Row {
        match &self.wrap_index {
            Some(idx) => idx.row(i),
            None => Row {
                line: i,
                start: 0,
                end: self.state.buffer.line_len(i),
            },
        }
    }

    /// Rebuild the wrap index when the buffer revision or the width changed.
    // ponytail: the whole buffer is re-wrapped after every edit, the same order
    // as the highlighter's per-edit pass; upgrade: re-wrap the touched lines only.
    pub(super) fn ensure_wrap(&mut self, text_w: f32) {
        if !self.wrap {
            self.wrap_index = None;
            // The widest line bounds horizontal scroll, so measure it while wrapping is off.
            if self.max_key != self.state.buffer.revision {
                let mut max = 0;
                for i in 0..self.state.buffer.len_lines() {
                    let text = self.state.buffer.line(i);
                    max = max.max(visual_col(&text, text.chars().count()));
                }
                self.max_cols = max;
                self.max_key = self.state.buffer.revision;
            }
            return;
        }
        if text_w < self.char_w {
            self.wrap_index = None;
            return;
        }
        let cols = (text_w / self.char_w).floor() as usize;
        let key = (self.state.buffer.revision, cols);
        if self.wrap_index.is_some() && self.wrap_key == key {
            return;
        }
        let lines: Vec<String> = (0..self.state.buffer.len_lines())
            .map(|i| self.state.buffer.line(i))
            .collect();
        self.wrap_index = Some(WrapIndex::build(&lines, cols));
        self.wrap_key = key;
    }

    /// Keep `scroll_x` inside the content: no scrolling into empty space.
    fn clamp_scroll_x(&mut self, text_w: f32) {
        let max = (self.max_cols as f32 * self.char_w - text_w).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max);
    }

    /// Alt+Z: flip wrap on this editor.
    pub(super) fn toggle_wrap(&mut self, _: &ToggleWrap, _: &mut Window, cx: &mut Context<Self>) {
        let on = !self.wrap;
        self.set_wrap(on, cx);
    }

    /// Turn wrap on or off and remember the choice for the next file.
    pub fn set_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        self.wrap = wrap;
        self.scroll_x = 0.0;
        self.wrap_index = None;
        let s = cx.global_mut::<Settings>();
        s.word_wrap = wrap;
        s.save();
        cx.notify();
    }

    /// Shift+wheel, or a trackpad's horizontal delta, scrolls a long line when
    /// wrap is off. Windows sends a horizontal delta only, so the list's own
    /// vertical wheel is untouched.
    pub(super) fn scroll_wheel(
        &mut self,
        e: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.wrap {
            return;
        }
        let dx = match e.delta {
            ScrollDelta::Pixels(p) => f32::from(p.x),
            ScrollDelta::Lines(p) => p.x * self.char_w,
        };
        if dx == 0.0 {
            return;
        }
        self.scroll_x -= dx;
        self.clamp_scroll_x(self.text_width());
        cx.notify();
    }

    /// The visual row holding `(line, col)`; the logical line when wrap is off.
    pub(super) fn row_of_caret(&self, line: usize, col: usize) -> usize {
        match &self.wrap_index {
            Some(idx) => idx.row_of(line, col),
            None => line,
        }
    }

    /// Scroll so the caret is on screen, horizontally too when wrap is off.
    pub(super) fn reveal_cursor(&mut self) {
        let (line, col) = self.state.line_col(self.state.cursor);
        let row = self.row_of_caret(line, col);
        let top = self.scroll.0.borrow().base_handle.logical_scroll_top().0;
        let rows = ((f32::from(self.bounds.size.height) / LINE_H) as usize).max(1);
        if row < top {
            self.scroll.scroll_to_item(row, ScrollStrategy::Top);
        } else if row + 1 >= top + rows {
            self.scroll.scroll_to_item(row, ScrollStrategy::Bottom);
        }
        let width = self.text_width();
        if self.wrap || width < self.char_w {
            return;
        }
        let text = self.state.buffer.line(line);
        let caret = visual_col(&text, col) as f32 * self.char_w;
        let line_w = visual_col(&text, text.chars().count()) as f32 * self.char_w;
        if line_w <= width {
            self.scroll_x = 0.0;
        } else if caret < self.scroll_x {
            self.scroll_x = caret;
        } else if caret + self.char_w > self.scroll_x + width {
            self.scroll_x = caret + self.char_w - width;
        }
        self.clamp_scroll_x(width);
    }

    /// The buffer column a click at `x` on `row` lands on; with wrap on the row
    /// is a slice of its logical line, so the column is measured inside the slice.
    pub(super) fn col_at(&self, row: Row, x: Pixels) -> usize {
        let rel = f32::from(x - self.bounds.left()) - GUTTER + self.scroll_x;
        let vcol = (rel / self.char_w + 0.5).max(0.0) as usize;
        let text = self.state.buffer.line(row.line);
        let slice: String = text
            .chars()
            .skip(row.start)
            .take(row.end - row.start)
            .collect();
        row.start + col_at_visual(&slice, vcol)
    }

    /// Place the caret on the clicked row.
    pub(super) fn click(&mut self, row: Row, x: Pixels, extend: bool, cx: &mut Context<Self>) {
        let col = self.col_at(row, x);
        self.state.set_cursor_line_col(row.line, col, extend);
        cx.notify();
    }
}
