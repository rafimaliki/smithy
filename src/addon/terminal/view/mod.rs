//! The bottom panel: session tabs, the shell picker, and the active session's grid.
//! Design: framery frame `flows/files/terminal`.
//!
//! ponytail: the panel is a fixed height and the cursor is a plain block (no blink),
//! and rows are drawn as styled spans, not shaped runs; upgrade: a draggable divider,
//! per-cell font shaping with a grid damage model.
//!
//! This file owns the view state and the panel frame; `input` handles keys and
//! scrolling, `header` the session tabs and shell picker, and `paint` draws the grid
//! cells as colored spans.

mod header;
mod input;
mod paint;

use super::pty::{Session, Shell};
use crate::editor::view::MONO;
use crate::settings::Settings;
use crate::theme::Theme;
use crate::workspace::Workspace;
use async_channel::Sender;
use gpui::{
    div, font, prelude::*, px, App, Bounds, Context, FocusHandle, Focusable, MouseButton,
    SharedString, WeakEntity, Window,
};
use paint::row_element;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const PANEL_H: f32 = 280.0;
const HEAD_H: f32 = 34.0;
const LINE_H: f32 = 20.0;
const PAD_X: f32 = 16.0;
const PAD_Y: f32 = 10.0;
const CURSOR_H: f32 = 15.0;

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

    pub(super) fn measured(&self) -> (u16, u16) {
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
