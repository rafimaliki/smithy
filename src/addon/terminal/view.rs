//! The bottom panel: session tabs, the shell picker, and the active session's grid.
//! Design: framery frame `flows/files/terminal`.
//!
//! ponytail: the panel is a fixed height and the cursor is a plain block (no blink),
//! and rows are drawn as styled spans, not shaped runs; upgrade: a draggable divider,
//! per-cell font shaping with a grid damage model.

use super::grid::{Cell, Color, Pen};
use super::keys;
use super::pty::{Session, Shell};
use crate::editor::view::MONO;
use crate::settings::Settings;
use crate::theme::Theme;
use crate::workspace::menu;
use crate::workspace::Workspace;
use async_channel::Sender;
use gpui::{
    div, font, prelude::*, px, AnyElement, App, Bounds, Context, Div, FocusHandle, Focusable,
    FontWeight, KeyDownEvent, MouseButton, Rgba, ScrollDelta, ScrollWheelEvent, SharedString,
    Stateful, WeakEntity, Window,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const PANEL_H: f32 = 280.0;
const HEAD_H: f32 = 34.0;
const LINE_H: f32 = 20.0;
const PAD_X: f32 = 16.0;
const PAD_Y: f32 = 10.0;
const CURSOR_H: f32 = 15.0;

/// The 16 ANSI colors. These are the VT protocol's own palette, not app theming, so
/// they do not come from `Theme`; everything around them — background, header,
/// borders, default text — does.
const ANSI: [u32; 16] = [
    0x1c1f22, 0xc74e39, 0x81b88b, 0xa28a59, 0x70949e, 0x9a7fb8, 0x4aa3a8, 0xc8cdd3, 0x5c6167,
    0xe07a6b, 0xa6d189, 0xd8b56a, 0x8ba0b6, 0xb49ede, 0x66c2c6, 0xf2f3f5,
];

/// One open session in the panel.
struct Open {
    session: Session,
    emulator: Arc<Mutex<super::ansi::Emulator>>,
}

pub struct TerminalView {
    root: Option<PathBuf>,
    workspace: Option<WeakEntity<Workspace>>,
    sessions: Vec<Open>,
    active: usize,
    visible: bool,
    shell_menu: bool,
    focus: FocusHandle,
    /// Body bounds as last painted; they size the pty.
    body: Bounds<gpui::Pixels>,
    char_w: f32,
    /// Lines scrolled up from the bottom; 0 follows the output.
    scroll: usize,
    error: Option<SharedString>,
    /// Held so the wake-up task lives exactly as long as the view.
    wake: Sender<()>,
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TerminalView {
    pub fn new(
        root: Option<PathBuf>,
        workspace: Option<WeakEntity<Workspace>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (wake, signals) = async_channel::bounded(1);
        // Output arrives on a reader thread; this only turns it into a repaint.
        cx.spawn(async move |this, cx| {
            while signals.recv().await.is_ok() {
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
        })
        .detach();
        Self {
            root,
            workspace,
            sessions: Vec::new(),
            active: 0,
            visible: false,
            shell_menu: false,
            focus: cx.focus_handle(),
            body: Bounds::default(),
            char_w: 0.0,
            scroll: 0,
            error: None,
            wake,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Ctrl+backtick: show or hide the panel. A session opens on first show, so an
    /// idle add-on has no process running.
    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.visible {
            self.visible = false;
            self.shell_menu = false;
        } else {
            if self.sessions.is_empty() && self.error.is_none() {
                self.open_session(Shell::PowerShell, self.root.clone(), window, cx);
            }
            self.visible = true;
            window.focus(&self.focus);
        }
        cx.notify();
    }

    /// The folder menu's Open in Terminal: a session in `dir`, panel shown.
    pub fn open_here(&mut self, dir: &Path, window: &mut Window, cx: &mut Context<Self>) {
        self.open_session(Shell::PowerShell, Some(dir.to_path_buf()), window, cx);
    }

    fn open_session(
        &mut self,
        shell: Shell,
        cwd: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match Session::spawn(shell, cwd.as_deref(), self.wake.clone()) {
            Ok(session) => {
                let emulator = session.emulator();
                self.sessions.push(Open { session, emulator });
                self.active = self.sessions.len() - 1;
                self.visible = true;
                self.error = None;
                self.scroll = 0;
                window.focus(&self.focus);
            }
            Err(err) => {
                self.error = Some(format!("Could not start {}: {err}", shell.label()).into());
                self.visible = true;
            }
        }
        cx.notify();
    }

    /// Close a session: dropping it kills the shell process.
    fn close_session(&mut self, i: usize, cx: &mut Context<Self>) {
        if i >= self.sessions.len() {
            return;
        }
        self.sessions.remove(i);
        if self.sessions.is_empty() {
            self.visible = false;
            self.notify_workspace(cx);
        } else if self.active >= self.sessions.len() {
            self.active = self.sessions.len() - 1;
        }
        cx.notify();
    }

    /// The panel's own close button: hide it, keeping the sessions alive.
    fn close_panel(&mut self, cx: &mut Context<Self>) {
        self.visible = false;
        self.shell_menu = false;
        self.notify_workspace(cx);
        cx.notify();
    }

    /// The workspace draws the panel from what [`is_visible`](Self::is_visible) says,
    /// so it has to repaint when the panel hides itself.
    fn notify_workspace(&self, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspace.clone() {
            workspace.update(cx, |_, cx| cx.notify()).ok();
        }
    }

    fn measured(&self) -> (u16, u16) {
        let char_w = if self.char_w > 0.0 { self.char_w } else { 8.0 };
        let cols = ((f32::from(self.body.size.width) - 2.0 * PAD_X) / char_w).floor();
        let rows = ((f32::from(self.body.size.height) - 2.0 * PAD_Y) / LINE_H).floor();
        if cols < 8.0 || rows < 2.0 {
            return (0, 0);
        }
        (cols as u16, rows as u16)
    }

    fn active_emulator(&self) -> Option<Arc<Mutex<super::ansi::Emulator>>> {
        self.sessions.get(self.active).map(|o| o.emulator.clone())
    }

    fn send(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if let Some(open) = self.sessions.get_mut(self.active) {
            open.session.write(bytes);
        }
        self.scroll = 0;
        cx.notify();
    }

    fn key_down(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let m = e.keystroke.modifiers;
        let key = e.keystroke.key.as_str();
        // Ctrl+V pastes; the terminal never claims Ctrl+C (copy needs a selection).
        if m.control && key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.send(text.replace("\r\n", "\r").as_bytes(), cx);
            }
            return;
        }
        if m.control && (key == "pageup" || key == "pagedown") {
            self.scroll_by(if key == "pageup" { 10 } else { -10 }, cx);
            return;
        }
        if let Some(bytes) = keys::encode(key, e.keystroke.key_char.as_deref(), m) {
            self.send(&bytes, cx);
        }
    }

    fn scroll_by(&mut self, lines: isize, cx: &mut Context<Self>) {
        let total = self
            .active_emulator()
            .map(|e| e.lock().map(|e| e.grid().total_lines()).unwrap_or(0))
            .unwrap_or(0);
        let max = total.saturating_sub(self.visible_rows()) as isize;
        self.scroll = (self.scroll as isize + lines).clamp(0, max.max(0)) as usize;
        cx.notify();
    }

    fn wheel(&mut self, e: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let lines = match e.delta {
            ScrollDelta::Lines(p) => p.y,
            ScrollDelta::Pixels(p) => f32::from(p.y) / LINE_H,
        };
        if lines != 0.0 {
            self.scroll_by(lines.round() as isize, cx);
        }
    }

    fn visible_rows(&self) -> usize {
        let (_, rows) = self.measured();
        if rows == 0 {
            12
        } else {
            rows as usize
        }
    }

    fn focus_body(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.scroll = 0;
        self.shell_menu = false;
        window.focus(&self.focus);
        cx.notify();
    }
}

/// A header icon button: a text glyph, as the rail and the tab bar use.
fn head_button(id: &'static str, glyph: &'static str, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(22.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .cursor_pointer()
        .text_color(t.mute)
        .hover(|d| d.bg(t.hov).text_color(t.ink))
        .child(glyph)
}

/// The screen text and colors of one row.
fn row_element(cells: &[Cell], t: &Theme) -> AnyElement {
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

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        let size = px(13.);
        let font_id = window.text_system().resolve_font(&font(MONO));
        if let Ok(advance) = window.text_system().advance(font_id, size, 'm') {
            self.char_w = f32::from(advance.width);
        }
        let (cols, rows) = self.measured();
        if cols > 0 {
            for open in &mut self.sessions {
                open.session.resize(cols, rows);
            }
        }
        let entity = cx.entity();
        let (cells, cursor) = match self.active_emulator() {
            Some(emulator) => match emulator.lock() {
                Ok(emulator) => {
                    let grid = emulator.grid();
                    let window = grid.window(self.visible_rows(), self.scroll);
                    (window.rows, window.cursor)
                }
                Err(_) => (Vec::new(), None),
            },
            None => (Vec::new(), None),
        };
        let char_w = if self.char_w > 0.0 { self.char_w } else { 8.0 };
        let mut body = div()
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .px(px(PAD_X))
            .pt(px(PAD_Y))
            .font_family(MONO)
            .text_size(size)
            .text_color(theme.ink)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.focus_body(window, cx)),
            )
            .on_scroll_wheel(cx.listener(Self::wheel))
            .child(
                gpui::canvas(
                    move |bounds, _, cx| entity.update(cx, |this, _| this.body = bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );
        for row in &cells {
            body = body.child(row_element(row, &theme));
        }
        if let Some((row, col)) = cursor {
            body = body.child(
                div()
                    .absolute()
                    .left(px(PAD_X + col as f32 * char_w))
                    .top(px(PAD_Y + row as f32 * LINE_H + 3.0))
                    .w(px(char_w))
                    .h(px(CURSOR_H))
                    .bg(theme.ink),
            );
        }
        if let Some(error) = self.error.clone() {
            body = body.child(div().text_color(theme.del).child(error));
        }
        div()
            .key_context("Terminal")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .relative()
            .h(px(PANEL_H))
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.bg)
            .border_t_1()
            .border_color(theme.line)
            .child(self.header(&theme, cx))
            .child(body)
            .children(self.shell_menu(&theme, cx))
    }
}

impl TerminalView {
    /// The session tabs and the new / shell / hide buttons.
    fn header(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut header = div()
            .h(px(HEAD_H))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .px(px(10.))
            .bg(t.side)
            .border_b_1()
            .border_color(t.line);
        for i in 0..self.sessions.len() {
            let active = i == self.active;
            let label: SharedString = self.tab_label(i).into();
            header = header.child(
                div()
                    .id(("term-tab", i))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(24.))
                    .px(px(10.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .whitespace_nowrap()
                    .when(active, |d| d.bg(t.sel).text_color(t.ink))
                    .when(!active, |d| {
                        d.text_color(t.mute).hover(|d| d.text_color(t.ink))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.active = i;
                            this.scroll = 0;
                            this.shell_menu = false;
                            window.focus(&this.focus);
                            cx.notify();
                        }),
                    )
                    .child(label)
                    .child(
                        div()
                            .id(("term-close-tab", i))
                            .px(px(3.))
                            .text_color(t.mute)
                            .hover(|d| d.text_color(t.del))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.close_session(i, cx);
                                }),
                            )
                            .child("×"),
                    ),
            );
        }
        header
            .child(head_button("term-new", "+", t).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    let cwd = this.root.clone();
                    this.shell_menu = false;
                    this.open_session(Shell::PowerShell, cwd, window, cx);
                }),
            ))
            .child(head_button("term-shell", "⌄", t).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.shell_menu = !this.shell_menu;
                    cx.notify();
                }),
            ))
            .child(div().flex_1())
            .child(head_button("term-hide", "×", t).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close_panel(cx)),
            ))
    }

    /// The name on the tab: the shell, numbered when that shell is already open.
    fn tab_label(&self, i: usize) -> String {
        let shell = self.sessions[i].session.shell();
        let before = self.sessions[..i]
            .iter()
            .filter(|o| o.session.shell() == shell)
            .count();
        if before == 0 {
            shell.label().to_string()
        } else {
            format!("{} {}", shell.label(), before + 1)
        }
    }

    /// The shell picker, drawn under its header button.
    fn shell_menu(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.shell_menu {
            return None;
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        for shell in Shell::ALL {
            rows.push(
                menu::item(
                    ("term-shell-item", shell as usize),
                    shell.label(),
                    None,
                    false,
                    true,
                    t,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let cwd = this.root.clone();
                    this.shell_menu = false;
                    this.open_session(shell, cwd, window, cx);
                }))
                .into_any_element(),
            );
        }
        Some(
            div()
                .absolute()
                .left(px(52.))
                .top(px(HEAD_H - 2.))
                .w(px(menu::WIDTH))
                .p(px(5.))
                .bg(t.side)
                .border_1()
                .border_color(t.line)
                .rounded(px(8.))
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.shell_menu = false;
                    cx.notify();
                }))
                .children(rows)
                .into_any_element(),
        )
    }
}
