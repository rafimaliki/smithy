//! How a search result is marked: the match highlight, file icons and the
//! symbol-kind badge. Layout and controls are in `chrome.rs`.
use crate::addon::search::segments::segments;
use crate::editor::layout::expand_tabs;
use crate::editor::symbols::SymbolKind;
use crate::editor::view::MONO;
use crate::theme::{on_accent, Theme};
use gpui::{div, prelude::*, px, AnyElement, FontWeight, Rgba, SharedString};

/// A tone down of a theme color, for highlight fills.
pub fn tint(c: Rgba, a: f32) -> Rgba {
    Rgba { a, ..c }
}

/// How a matched run is marked: the accent color and bold (file and symbol
/// names), or the accent as a filled chip (the matched text in a line).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Accent,
    Background,
}

/// `text` split into plain and matched runs.
pub fn marked(text: &str, ranges: &[(usize, usize)], mark: Mark, theme: &Theme) -> AnyElement {
    let mut row = div().flex().min_w_0().overflow_hidden().whitespace_nowrap();
    for (run, hot) in segments(text, ranges) {
        let mut piece = div()
            .flex_none()
            .whitespace_nowrap()
            .child(SharedString::from(expand_tabs(&run)));
        if hot {
            piece = match mark {
                Mark::Accent => piece.text_color(theme.acc).font_weight(FontWeight::BOLD),
                Mark::Background => piece.bg(tint(theme.acc, 0.38)).rounded(px(2.)),
            };
        }
        row = row.child(piece);
    }
    row.into_any_element()
}

/// `text` split into plain and matched runs, from fuzzy char positions.
pub fn marked_positions(text: &str, positions: &[usize], mark: Mark, theme: &Theme) -> AnyElement {
    let ranges: Vec<(usize, usize)> = positions.iter().map(|&i| (i, i + 1)).collect();
    marked(text, &ranges, mark, theme)
}

/// The file icon. Brand marks are the `c-files` task's; until then the simple
/// style: the extension, uppercase.
pub fn file_icon(name: &str, theme: &Theme) -> AnyElement {
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let label: String = ext.chars().take(3).collect::<String>().to_uppercase();
    div()
        .w(px(18.))
        .flex_none()
        .font_family(MONO)
        .text_size(px(10.))
        .font_weight(FontWeight::BOLD)
        .text_color(theme.mute)
        .child(SharedString::from(label))
        .into_any_element()
}

/// The symbol-kind badge: one letter on the language color.
pub fn kind_badge(kind: SymbolKind, theme: &Theme) -> AnyElement {
    let (glyph, color) = kind_style(kind, theme);
    div()
        .w(px(16.))
        .h(px(16.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .bg(color)
        .text_color(on_accent())
        .text_size(px(9.))
        .font_weight(FontWeight::BOLD)
        .child(glyph)
        .into_any_element()
}

fn kind_style(kind: SymbolKind, theme: &Theme) -> (&'static str, Rgba) {
    use SymbolKind::*;
    match kind {
        Function | Method => ("ƒ", theme.func),
        Property | Field => ("P", theme.kw),
        Constant | Variable => ("V", theme.str),
        Module | Class | Struct | Enum | Interface | Trait | Type => ("C", theme.ty),
    }
}
