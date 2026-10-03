//! Presentation shared by the Search sidebar and the Search everywhere modal:
//! the segmented tabs, the query fields, result rows, and the marks that show
//! why a row matched. Styling follows the board frames.
use super::segments::segments;
use crate::editor::layout::expand_tabs;
use crate::editor::symbols::SymbolKind;
use crate::editor::view::MONO;
use crate::theme::{on_accent, Theme};
use gpui::{
    div, prelude::*, px, AnyElement, Context, FocusHandle, FontWeight, KeyDownEvent, MouseButton,
    MouseDownEvent, Rgba, SharedString, Stateful, Window,
};

/// A tone down of a theme color, for highlight fills.
pub fn tint(c: Rgba, a: f32) -> Rgba {
    Rgba { a, ..c }
}

/// One clickable tab of a segmented control.
pub type Segment<T> = (
    &'static str,
    &'static str,
    bool,
    Box<dyn Fn(&mut T, &mut Context<T>)>,
);

/// A bordered segmented control filling the sidebar width.
pub fn segmented<T: 'static>(
    items: Vec<Segment<T>>,
    theme: &Theme,
    cx: &mut Context<T>,
) -> AnyElement {
    let mut seg = div()
        .flex()
        .flex_none()
        .mx(px(12.))
        .mb(px(10.))
        .border_1()
        .border_color(theme.line)
        .rounded(px(6.))
        .overflow_hidden();
    for (id, label, on, run) in items {
        seg = seg.child(
            div()
                .id(id)
                .flex_1()
                .py(px(4.))
                .text_center()
                .cursor_pointer()
                .when(on, |d| d.bg(theme.sel).text_color(theme.ink))
                .when(!on, |d| d.text_color(theme.mute))
                .hover(|d| d.text_color(theme.ink))
                .child(SharedString::from(label))
                .on_click(cx.listener(move |this, _, _, cx| run(this, cx))),
        );
    }
    seg.into_any_element()
}

/// A focusable, borderless input row. Callers set the border color and add the
/// text and any toggles. `small` is the include field; the rest is the query.
pub fn input_row<T: 'static>(
    id: &'static str,
    focus: &FocusHandle,
    small: bool,
    theme: &Theme,
    on_key: fn(&mut T, &KeyDownEvent, &mut Window, &mut Context<T>),
    cx: &mut Context<T>,
) -> Stateful<gpui::Div> {
    let handle = focus.clone();
    div()
        .id(id)
        .track_focus(focus)
        .on_key_down(cx.listener(on_key))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_, _: &MouseDownEvent, window, _| window.focus(&handle)),
        )
        .flex()
        .items_center()
        .gap(px(4.))
        .mx(px(12.))
        .mb(px(8.))
        .when(small, |d| d.h(px(28.)).px(px(10.)).mb(px(10.)))
        .when(!small, |d| d.h(px(32.)).pl(px(10.)).pr(px(6.)))
        .flex_none()
        .border_1()
        .border_color(theme.line)
        .rounded(px(6.))
        .bg(theme.bg)
}

/// The query text, or its placeholder in the muted color.
pub fn input_text(text: &str, placeholder: &str, size: f32, theme: &Theme) -> AnyElement {
    let empty = text.is_empty();
    let shown = if empty { placeholder } else { text }.to_string();
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(MONO)
        .text_size(px(size))
        .when(!empty, |d| d.text_color(theme.ink))
        .when(empty, |d| d.text_color(theme.mute))
        .child(SharedString::from(shown))
        .into_any_element()
}

/// One of the three Text-tab toggles (Aa, ab, .*).
pub fn toggle(
    id: &'static str,
    label: &'static str,
    on: bool,
    theme: &Theme,
) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .w(px(22.))
        .h(px(22.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .cursor_pointer()
        .font_family(MONO)
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .when(on, |d| d.bg(theme.sel).text_color(theme.ink))
        .when(!on, |d| d.text_color(theme.mute).hover(|d| d.bg(theme.hov)))
        .child(SharedString::from(label))
}

/// The small muted line under the fields, e.g. "7 results in 3 files".
pub fn count_line(text: &str, theme: &Theme) -> AnyElement {
    div()
        .px(px(14.))
        .pb(px(8.))
        .text_size(px(12.))
        .text_color(theme.mute)
        .child(SharedString::from(text.to_string()))
        .into_any_element()
}

/// A line explaining why nothing is shown.
pub fn error_line(text: &str, theme: &Theme) -> AnyElement {
    div()
        .px(px(14.))
        .pb(px(8.))
        .text_size(px(12.))
        .text_color(theme.del)
        .child(SharedString::from(text.to_string()))
        .into_any_element()
}

/// A result row: 28px, clickable, hover and active backgrounds.
pub fn row(id: (&'static str, usize), active: bool, theme: &Theme) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .pl(px(14.))
        .pr(px(10.))
        .flex_none()
        .whitespace_nowrap()
        .cursor_pointer()
        .when(active, |d| d.bg(theme.sel))
        .hover(|d| d.bg(theme.hov))
}

/// A hit row under a file header: indented, monospace, one line of code.
pub fn hit_row(id: (&'static str, usize), theme: &Theme) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(24.))
        .pl(px(34.))
        .pr(px(10.))
        .flex_none()
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(MONO)
        .text_size(px(12.))
        .cursor_pointer()
        .hover(|d| d.bg(theme.hov))
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

/// The muted block under the results, e.g. the Symbols footnote.
pub fn footnote(text: &str, theme: &Theme) -> AnyElement {
    div()
        .p(px(14.))
        .text_size(px(12.))
        .text_color(theme.mute)
        .child(SharedString::from(text.to_string()))
        .into_any_element()
}

/// Apply one keystroke to a plain text field. Returns true when it changed.
/// ponytail: no IME and no caret; upgrade: EntityInputHandler, as the editor notes.
pub fn typed(text: &mut String, e: &KeyDownEvent) -> bool {
    let m = e.keystroke.modifiers;
    if m.control || m.alt || m.platform {
        return false;
    }
    if e.keystroke.key == "backspace" {
        return text.pop().is_some();
    }
    let ch = match (e.keystroke.key_char.as_deref(), e.keystroke.key.as_str()) {
        (Some(ch), _) => ch,
        // Windows reports the space bar as key "space" with no key_char.
        (None, "space") => " ",
        (None, _) => return false,
    };
    if ch.chars().any(char::is_control) {
        return false;
    }
    text.push_str(ch);
    true
}
