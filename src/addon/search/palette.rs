//! The command palette (Ctrl+Shift+P): every bound action with its shortcut,
//! filtered by fuzzy text. Enter runs the action where the caret was. Board frame
//! `command-palette`. Part of the Search add-on, so it costs nothing while Search is off.
use super::chrome::{self, Mark};
use super::fuzzy::fuzzy;
use crate::keymap::{self, Shortcut};
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyElement, Context, FocusHandle, KeyDownEvent, MouseButton,
    MouseDownEvent, Render, ScrollHandle, SharedString, Window,
};

/// Plain movement and typing keys are not commands worth listing.
const HIDDEN: &[&str] = &[
    "MoveLeft",
    "MoveRight",
    "MoveUp",
    "MoveDown",
    "SelectLeft",
    "SelectRight",
    "SelectUp",
    "SelectDown",
    "Home",
    "End",
    "SelectHome",
    "SelectEnd",
    "Backspace",
    "Delete",
    "Enter",
    "Tab",
    "CommandPalette",
];

pub struct PaletteView {
    open: bool,
    query: String,
    selected: usize,
    focus: FocusHandle,
    scroll: ScrollHandle,
    /// What had the keyboard before the palette opened; the action runs there.
    previous: Option<FocusHandle>,
}

impl PaletteView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            previous: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    pub fn show(&mut self, previous: Option<FocusHandle>, cx: &mut Context<Self>) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.previous = previous;
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
    }

    /// Close and hand the keyboard back to what had it, so shortcuts keep working.
    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close(cx);
        if let Some(previous) = self.previous.take() {
            window.focus(&previous);
        }
    }

    /// The commands that match the query, best first (catalog order when empty).
    fn rows(&self) -> Vec<(&'static Shortcut, Vec<usize>)> {
        let mut out: Vec<(i32, &'static Shortcut, Vec<usize>)> = keymap::CATALOG
            .iter()
            .filter(|s| !HIDDEN.contains(&s.action))
            .filter_map(|s| {
                if self.query.is_empty() {
                    return Some((0, s, Vec::new()));
                }
                fuzzy(&self.query, s.label).map(|h| (h.score, s, h.positions))
            })
            .collect();
        if !self.query.is_empty() {
            out.sort_by_key(|row| std::cmp::Reverse(row.0));
        }
        out.into_iter().map(|(_, s, p)| (s, p)).collect()
    }

    fn input_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "escape" => self.dismiss(window, cx),
            "enter" => self.run_selected(window, cx),
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            _ => {
                if chrome::typed(&mut self.query, e) {
                    self.selected = 0;
                }
                cx.notify();
            }
        }
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.rows().len();
        if n == 0 {
            return;
        }
        self.selected =
            (self.selected.min(n - 1) as isize + delta).clamp(0, n as isize - 1) as usize;
        self.scroll.scroll_to_item(self.selected);
        cx.notify();
    }

    fn run_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows();
        if let Some((shortcut, _)) = rows.get(self.selected) {
            self.run(shortcut.action, window, cx);
        }
    }

    fn run(&mut self, action: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
        if let Some(previous) = self.previous.take() {
            window.focus(&previous);
        }
        if let Ok(action) = cx.build_action(&format!("smithy::{action}"), None) {
            window.dispatch_action(action, cx);
        }
    }
}

impl Render for PaletteView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let settings = cx.global::<Settings>();
        let theme = Theme::by_name(&settings.theme);
        let overrides = settings.shortcuts.clone();
        let chip = keymap::effective("CommandPalette", &overrides)
            .first()
            .map(|keys| keymap::display(keys));

        let rows = self.rows();
        let mut list = div()
            .id("palette-results")
            .flex()
            .flex_col()
            .max_h(px(520.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(group_label("Commands", &theme));
        for (i, (shortcut, positions)) in rows.iter().enumerate() {
            let action = shortcut.action;
            let keys = keymap::effective(action, &overrides)
                .first()
                .map(|k| keymap::display(k))
                .unwrap_or_default();
            list = list.child(
                chrome::row(("pal", i), i == self.selected, &theme)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            this.run(action, window, cx)
                        }),
                    )
                    .child(chrome::marked_positions(
                        shortcut.label,
                        positions,
                        Mark::Accent,
                        &theme,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(theme.mute)
                            .child(SharedString::from(shortcut.group)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.mute)
                            .child(SharedString::from(keys)),
                    ),
            );
        }
        if rows.is_empty() {
            list = list.child(chrome::count_line(
                &format!("No command matches “{}”", self.query.trim()),
                &theme,
            ));
        }

        let footer = div()
            .flex()
            .gap(px(18.))
            .h(px(30.))
            .items_center()
            .px(px(16.))
            .border_t_1()
            .border_color(theme.line)
            .text_size(px(12.))
            .text_color(theme.mute)
            .child("Up and Down choose")
            .child("Enter run")
            .child("Esc close");

        let panel = div()
            .mt(px(84.))
            .w(px(760.))
            .flex()
            .flex_col()
            .bg(theme.side)
            .border_1()
            .border_color(theme.line)
            .rounded(px(10.))
            .overflow_hidden()
            .text_color(theme.ink)
            .font_family("Segoe UI")
            .text_size(px(13.))
            .child(
                chrome::input_row(
                    "palette-input",
                    &self.focus,
                    false,
                    &theme,
                    Self::input_key,
                    cx,
                )
                .mx(px(0.))
                .mb(px(0.))
                .h(px(46.))
                .px(px(16.))
                .gap(px(10.))
                .border_color(theme.line)
                .child(div().text_color(theme.mute).child("⌕"))
                .child(chrome::input_text(
                    &self.query,
                    "Type a command",
                    13.,
                    &theme,
                ))
                .when_some(chip, |d, keys| {
                    d.child(
                        div()
                            .px(px(8.))
                            .rounded(px(10.))
                            .text_size(px(12.))
                            .text_color(theme.mute)
                            .child(SharedString::from(keys)),
                    )
                }),
            )
            .child(list)
            .child(footer);

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .id("palette-backdrop")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .bg(chrome::tint(theme.bg, 0.55))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, window, cx| {
                            this.dismiss(window, cx)
                        }),
                    ),
            )
            .child(panel)
            .into_any_element()
    }
}

fn group_label(label: &str, theme: &Theme) -> AnyElement {
    div()
        .px(px(16.))
        .pt(px(10.))
        .pb(px(4.))
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(theme.mute)
        .child(SharedString::from(label.to_uppercase()))
        .into_any_element()
}
