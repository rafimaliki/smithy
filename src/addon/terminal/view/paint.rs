//! Turning terminal grid cells into colored spans: the row text, the pen colors,
//! and the 256-color palette.

use super::super::grid::{Cell, Color, Pen};
use super::LINE_H;
use crate::editor::view::MONO;
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, FontWeight, Rgba, SharedString};

/// The 16 ANSI colors. These are the VT protocol's own palette, not app theming, so
/// they do not come from `Theme`; everything around them — background, header,
/// borders, default text — does.
const ANSI: [u32; 16] = [
    0x1c1f22, 0xc74e39, 0x81b88b, 0xa28a59, 0x70949e, 0x9a7fb8, 0x4aa3a8, 0xc8cdd3, 0x5c6167,
    0xe07a6b, 0xa6d189, 0xd8b56a, 0x8ba0b6, 0xb49ede, 0x66c2c6, 0xf2f3f5,
];

/// The screen text and colors of one row.
pub(super) fn row_element(cells: &[Cell], t: &Theme) -> AnyElement {
    let end = cells
        .iter()
        .rposition(|c| *c != Cell::default())
        .map_or(0, |i| i + 1);
    let mut row = div()
        .flex()
        .flex_none()
        .h(px(LINE_H))
        .whitespace_nowrap()
        .font_family(MONO);
    let mut i = 0;
    while i < end {
        let pen = cells[i].pen;
        let mut j = i;
        while j < end && cells[j].pen == pen {
            j += 1;
        }
        let text: String = cells[i..j].iter().map(|c| c.c).collect();
        let (fg, bg) = colors(pen, t);
        row = row.child(
            div()
                .when_some(bg, |d, bg| d.bg(bg))
                .text_color(fg)
                .when(pen.bold, |d| d.font_weight(FontWeight::BOLD))
                .child(SharedString::from(text)),
        );
        i = j;
    }
    row.into_any_element()
}

/// The foreground and optional background for a pen, with inverse swapped.
fn colors(pen: Pen, t: &Theme) -> (Rgba, Option<Rgba>) {
    let (mut fg, mut bg) = (
        paint(pen.fg, t.ink),
        match pen.bg {
            Color::Default => None,
            c => Some(paint(c, t.ink)),
        },
    );
    if pen.inverse {
        let base = bg.unwrap_or(t.bg);
        bg = Some(fg);
        fg = base;
    }
    (fg, bg)
}

fn paint(c: Color, default: Rgba) -> Rgba {
    match c {
        Color::Default => default,
        Color::Indexed(i) if (i as usize) < ANSI.len() => rgb(ANSI[i as usize]),
        Color::Indexed(i) => indexed(i),
        Color::Rgb(r, g, b) => Rgba {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        },
    }
}

/// The 6x6x6 color cube and the 24-step gray ramp above the 16 named colors.
fn indexed(i: u8) -> Rgba {
    let i = i as u32;
    if i < 232 {
        let n = i - 16;
        let level = |v: u32| if v == 0 { 0 } else { 55 + 40 * v };
        rgb((level(n / 36) << 16) | (level((n / 6) % 6) << 8) | level(n % 6))
    } else {
        let v = 8 + 10 * (i - 232);
        rgb((v << 16) | (v << 8) | v)
    }
}

fn rgb(v: u32) -> Rgba {
    Rgba {
        r: ((v >> 16) & 0xff) as f32 / 255.0,
        g: ((v >> 8) & 0xff) as f32 / 255.0,
        b: (v & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}
