//! Caret blink: pure timing, so it is tested without a window. The caret is shown
//! for one period, hidden for the next, and any move or focus change restarts the
//! cycle with it shown (so it never vanishes while typing).
use std::time::{Duration, Instant};

const PERIOD_MS: u128 = 530;

pub struct Blink {
    start: Instant,
    cursor: usize,
    focused: bool,
}

impl Blink {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            cursor: 0,
            focused: false,
        }
    }

    /// Whether the caret is drawn at `now`, given where it is and whether the
    /// editor has focus.
    pub fn visible(&mut self, cursor: usize, focused: bool, now: Instant) -> bool {
        if cursor != self.cursor || focused != self.focused {
            self.cursor = cursor;
            self.focused = focused;
            self.start = now;
        }
        ((now - self.start).as_millis() / PERIOD_MS).is_multiple_of(2)
    }

    /// The editor had focus when it was last drawn; an unfocused editor does not
    /// need redrawing for the blink.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// How long until the caret next changes state.
    pub fn until_toggle(&self, now: Instant) -> Duration {
        let into = (now - self.start).as_millis() % PERIOD_MS;
        Duration::from_millis((PERIOD_MS - into) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_alternates_and_a_move_restarts_it_shown() {
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut b = Blink::new();
        assert!(b.visible(0, true, ms(0)));
        assert!(b.visible(0, true, ms(529)));
        assert!(!b.visible(0, true, ms(530)));
        assert!(b.visible(0, true, ms(1060)));
        // Typing at the hidden moment shows it again straight away.
        assert!(!b.visible(0, true, ms(1600)));
        assert!(b.visible(1, true, ms(1700)));
        assert!(!b.visible(1, true, ms(1700 + 530)));
        assert_eq!(b.until_toggle(ms(1700 + 530)), Duration::from_millis(530));
    }
}
