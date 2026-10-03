//! Settings > GitHub: connect, replace or remove the personal access token the
//! Pull requests add-on uses. The token goes to the Windows Credential Manager
//! only; Settings keeps the login. Board frames `gh-token` and `gh-connected`.
mod view;

use crate::github::{token, Client};
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{prelude::*, Context, FocusHandle, KeyDownEvent, Render, Task, Window};

pub struct GithubSettings {
    input: String,
    focus: FocusHandle,
    /// Last four characters of the stored token, `None` when not connected.
    tail: Option<String>,
    connecting: bool,
    replacing: bool,
    error: Option<gpui::SharedString>,
    task: Option<Task<()>>,
}

impl GithubSettings {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            input: String::new(),
            focus: cx.focus_handle(),
            tail: token::load_token().map(|t| token::tail(&t)),
            connecting: false,
            replacing: false,
            error: None,
            task: None,
        }
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        let value = self.input.trim().to_string();
        if value.is_empty() {
            self.error = Some("Paste a token first.".into());
            cx.notify();
            return;
        }
        self.connecting = true;
        self.error = None;
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let login = Client::new(value.clone())
                        .account()
                        .map_err(|e| e.to_string())?
                        .login;
                    token::save_token(&value)?;
                    Ok::<(String, String), String>((login, token::tail(&value)))
                })
                .await;
            this.update(cx, |this, cx| this.connected(result, cx)).ok();
        }));
        cx.notify();
    }

    fn connected(&mut self, result: Result<(String, String), String>, cx: &mut Context<Self>) {
        self.connecting = false;
        match result {
            Ok((login, tail)) => {
                self.tail = Some(tail);
                self.input.clear();
                self.replacing = false;
                let settings = cx.global_mut::<Settings>();
                settings.github_login = login;
                settings.save();
            }
            Err(e) => self.error = Some(e.into()),
        }
        cx.notify();
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        match token::clear_token() {
            Ok(()) => {
                self.tail = None;
                self.replacing = false;
                self.input.clear();
                let settings = cx.global_mut::<Settings>();
                settings.github_login.clear();
                settings.save();
            }
            Err(e) => self.error = Some(e.into()),
        }
        cx.notify();
    }

    /// Type, paste (Ctrl+V) or backspace in the token field; Enter connects.
    fn token_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let m = e.keystroke.modifiers;
        if m.platform {
            return;
        }
        if m.control {
            if e.keystroke.key == "v" {
                if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    self.input.push_str(text.trim());
                }
            }
            cx.notify();
            return;
        }
        match e.keystroke.key.as_str() {
            "backspace" => {
                self.input.pop();
            }
            "enter" => self.connect(cx),
            key => {
                let typed = e.keystroke.key_char.as_deref().or(match key {
                    "space" => Some(" "),
                    _ => None,
                });
                if let Some(ch) = typed {
                    if !ch.chars().any(char::is_control) {
                        self.input.push_str(ch);
                    }
                }
            }
        }
        cx.notify();
    }
}

impl Render for GithubSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::by_name(&cx.global::<Settings>().theme);
        let login = cx.global::<Settings>().github_login.clone();
        if self.tail.is_some() && !self.replacing {
            return view::connected(self, &t, &login, cx);
        }
        view::form(self, &t, cx)
    }
}
