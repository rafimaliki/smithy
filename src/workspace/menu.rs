//! Shared context menu: the panel, its rows and its separators. The file tree uses
//! it now; the tab bar and the other views reuse the same shapes, so a menu looks
//! the same everywhere.
use crate::theme::Theme;
use gpui::{
    div, hsla, point, prelude::*, px, AnyElement, BoxShadow, Corner, Div, ElementId,
    MouseDownEvent, Pixels, Point, SharedString, Stateful, Window,
};

pub const WIDTH: f32 = 260.0;

/// A menu row: label on the left, shortcut on the right, `danger` in the delete
/// color. Disabled rows take no clicks and stay muted. Attach `.on_click` at the
/// call site — the menu has no state of its own.
pub fn item(
    id: impl Into<ElementId>,
    label: &str,
    shortcut: Option<&'static str>,
    danger: bool,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    let text = if !enabled {
        t.mute
    } else if danger {
        t.del
    } else {
        t.ink
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .h(px(28.))
        .flex_none()
        .px(px(10.))
        .rounded(px(5.))
        .whitespace_nowrap()
        .text_color(text)
        .when(!enabled, |d| d.opacity(0.5))
        .when(enabled, |d| d.cursor_pointer().hover(|d| d.bg(t.sel)))
        .child(SharedString::from(label.to_string()))
        .when_some(shortcut.map(SharedString::from), |d, s| {
            d.child(div().text_size(px(12.)).text_color(t.mute).child(s))
        })
}

pub fn separator(t: &Theme) -> Div {
    div().h(px(1.)).flex_none().my(px(4.)).mx(px(2.)).bg(t.line)
}

/// The menu panel at window position `at`, over everything else.
///
/// `dismiss` runs on any mouse down outside the panel, so the caller clears its
/// menu state; the panel itself takes the clicks that land on it.
pub fn popup(
    t: &Theme,
    at: Point<Pixels>,
    dismiss: impl Fn(&MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
    rows: Vec<AnyElement>,
) -> AnyElement {
    gpui::anchored()
        .position(at)
        .anchor(Corner::TopLeft)
        .snap_to_window()
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(WIDTH))
                .p(px(5.))
                .bg(t.side)
                .border_1()
                .border_color(t.line)
                .rounded(px(8.))
                .shadow(vec![BoxShadow {
                    color: hsla(0., 0., 0., 0.55),
                    offset: point(px(0.), px(8.)),
                    blur_radius: px(28.),
                    spread_radius: px(0.),
                }])
                .occlude()
                .on_mouse_down_out(dismiss)
                .children(rows),
        )
        .into_any_element()
}
