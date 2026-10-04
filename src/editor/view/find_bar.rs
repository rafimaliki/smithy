//! The find bar drawn over the editor, and the key/click behavior behind it.
//! Matching itself lives in `editor/find.rs`; this file is state triggers and
//! gpui elements. The bar floats top-right of the editor, per the board's
//! `find-in-file` frame.
use super::{EditorView, MONO};
use crate::actions::Find;
use crate::editor::find::{FindQuery, FindState, Status};
use crate::theme::Theme;
use gpui::{
    div, hsla, point, prelude::*, px, AnyElement, BoxShadow, Context, Div, MouseButton,
    MouseDownEvent, SharedString, Stateful, Window,
};

const FIELD_W: f32 = 150.0;

/// Which query toggle a click flips.
#[derive(Clone, Copy)]
enum Toggle {
    Case,
    Word,
    Regex,
}

fn toggle(id: &'static str, glyph: &'static str, on: bool, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(22.))
        .h(px(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .font_family(MONO)
        .text_size(px(11.))
        .cursor_pointer()
        .text_color(if on { t.ink } else { t.mute })
        .when(on, |d| d.bg(t.sel))
        .when(!on, |d| d.hover(|d| d.text_color(t.ink).bg(t.hov)))
        .child(glyph)
}

fn nav(id: &'static str, glyph: &'static str, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(22.))
        .h(px(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .text_size(px(12.))
        .cursor_pointer()
        .text_color(t.mute)
        .hover(|d| d.text_color(t.ink).bg(t.hov))
        .child(glyph)
}

impl EditorView {
    /// Ctrl+F: open the bar, or leave it as it is when already open.
    pub(super) fn on_find(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            let (line, col) = self.state.line_col(self.state.cursor);
            let query = FindQuery {
                text: self.find_seed(),
                ..Default::default()
            };
            self.find = Some(FindState::new(query, (line, col)));
            self.find_rev = self.state.buffer.revision;
            self.find_recompute();
        }
        window.focus(&self.focus);
        cx.notify();
    }

    /// The selected text when it is one line, so Ctrl+F on a word searches it.
    fn find_seed(&self) -> String {
        if !self.state.has_selection() {
            return String::new();
        }
        let (a, b) = self.state.selection();
        if self.state.line_col(a).0 == self.state.line_col(b).0 {
            self.state.selected_text()
        } else {
            String::new()
        }
    }

    pub(super) fn find_recompute(&mut self) {
        let text = self.state.buffer.text();
        let Some(f) = self.find.as_mut() else {
            return;
        };
        f.recompute(&text);
    }

    pub(super) fn find_type(&mut self, ch: &str, cx: &mut Context<Self>) {
        if let Some(f) = self.find.as_mut() {
            if f.replace_open && f.in_replace {
                f.replace.push_str(ch);
                cx.notify();
                return;
            }
            f.query.text.push_str(ch);
        }
        self.find_recompute();
        cx.notify();
    }

    pub(super) fn find_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(f) = self.find.as_mut() {
            if f.replace_open && f.in_replace {
                f.replace.pop();
                cx.notify();
                return;
            }
            f.query.text.pop();
        }
        self.find_recompute();
        cx.notify();
    }

    pub(super) fn find_close(&mut self, cx: &mut Context<Self>) {
        self.find = None;
        cx.notify();
    }

    /// Enter / Shift+Enter: jump to the next or previous match and show it.
    pub(super) fn find_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(m) = self.find.as_mut().and_then(|f| f.step(delta)) else {
            return;
        };
        let at = self.state.buffer.line_start(m.line) + m.col.start;
        self.state.set_cursor(at, false);
        self.reveal_cursor();
        cx.notify();
    }

    fn find_flip(&mut self, which: Toggle, window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = self.find.as_mut() else {
            return;
        };
        match which {
            Toggle::Case => f.query.case = !f.query.case,
            Toggle::Word => f.query.word = !f.query.word,
            Toggle::Regex => f.query.regex = !f.query.regex,
        }
        self.find_recompute();
        window.focus(&self.focus);
        cx.notify();
    }

    /// The floating find bar, or `None` when the editor is not finding.
    pub(super) fn find_bar(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let f = self.find.as_ref()?;
        let (label, color) = match f.status() {
            Status::Idle => (String::new(), t.mute),
            Status::Count(i, n) => (format!("{i} of {n}"), t.mute),
            Status::NoResults => ("No results".to_string(), t.mute),
            Status::BadRegex => ("Invalid regex".to_string(), t.del),
        };
        let replacing = f.replace_open && f.in_replace;
        let field = div()
            .id("find-field")
            .w(px(FIELD_W))
            .flex_none()
            .px(px(8.))
            .py(px(3.))
            .rounded(px(4.))
            .border_1()
            .border_color(if replacing { t.line } else { t.acc })
            .bg(t.bg)
            .font_family(MONO)
            .text_size(px(12.))
            .text_color(t.ink)
            .whitespace_nowrap()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if let Some(f) = this.find.as_mut() {
                        f.in_replace = false;
                    }
                    cx.notify();
                }),
            )
            .child(SharedString::from(f.query.text.clone()));
        let count = div()
            .w(px(48.))
            .flex_none()
            .text_size(px(12.))
            .text_center()
            .text_color(color)
            .child(SharedString::from(label));
        let mut bar = div()
            .id("find-bar")
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(12.))
            .pr(px(6.))
            .py(px(5.))
            .rounded(px(8.))
            .bg(t.side)
            .border_1()
            .border_color(t.line)
            .shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.5),
                offset: point(px(0.), px(6.)),
                blur_radius: px(20.),
                spread_radius: px(0.),
            }])
            .occlude()
            .font_family("Segoe UI")
            .text_size(px(13.))
            .child(field)
            .child(count);
        for (id, glyph, on, which) in [
            ("find-case", "Aa", f.query.case, Toggle::Case),
            ("find-word", "ab", f.query.word, Toggle::Word),
            ("find-regex", ".*", f.query.regex, Toggle::Regex),
        ] {
            bar = bar.child(toggle(id, glyph, on, t).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.find_flip(which, window, cx)
                }),
            ));
        }
        bar = bar
            .child(nav("find-prev", "↑", t).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.find_step(-1, cx)
                }),
            ))
            .child(nav("find-next", "↓", t).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.find_step(1, cx)
                }),
            ))
            .child(nav("find-close", "×", t).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.find_close(cx)
                }),
            ));
        let replace = f.replace_open.then(|| self.replace_panel(t, cx));
        Some(
            div()
                .id("find-wrap")
                .absolute()
                .right(px(28.))
                .top(px(8.))
                .flex()
                .flex_col()
                .items_end()
                .gap(px(4.))
                .child(bar)
                .children(replace)
                .into_any_element(),
        )
    }

    /// The row under the find bar: the replacement text, Replace and Replace all.
    fn replace_panel(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let (text, active) = self
            .find
            .as_ref()
            .map(|f| (f.replace.clone(), f.in_replace))
            .unwrap_or_default();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px(px(8.))
                .h(px(22.))
                .flex()
                .items_center()
                .rounded(px(4.))
                .text_size(px(12.))
                .cursor_pointer()
                .text_color(t.mute)
                .hover(|d| d.text_color(t.ink).bg(t.hov))
                .child(label)
        };
        div()
            .id("replace-bar")
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(12.))
            .pr(px(6.))
            .py(px(5.))
            .rounded(px(8.))
            .bg(t.side)
            .border_1()
            .border_color(t.line)
            .shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.5),
                offset: point(px(0.), px(6.)),
                blur_radius: px(20.),
                spread_radius: px(0.),
            }])
            .occlude()
            .font_family("Segoe UI")
            .text_size(px(13.))
            .child(
                div()
                    .id("replace-field")
                    .w(px(FIELD_W))
                    .flex_none()
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(if active { t.acc } else { t.line })
                    .bg(t.bg)
                    .font_family(MONO)
                    .text_size(px(12.))
                    .text_color(t.ink)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            if let Some(f) = this.find.as_mut() {
                                f.in_replace = true;
                            }
                            cx.notify();
                        }),
                    )
                    .child(SharedString::from(text)),
            )
            .child(button("replace-one", "Replace").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.replace_current(cx)
                }),
            ))
            .child(button("replace-all", "All").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.replace_all(cx)
                }),
            ))
            .into_any_element()
    }
}
