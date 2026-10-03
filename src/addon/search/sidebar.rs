//! The Search sidebar: Text, Files and Symbols over one query. Behavior and
//! state live here; the shared row and field styling is in `chrome.rs`.
//!
//! The search runs on a background task. Each keystroke replaces the task, which
//! cancels the one before it, and the worker also checks its `Cancel` flag, so a
//! slow folder never blocks typing. A short delay keeps a fast typist from
//! starting a search per character.
use super::chrome;
use super::engine::{self, Cancel, Kind, Output};
use super::matcher::Options;
use crate::workspace::Workspace;
use gpui::{Context, FocusHandle, KeyDownEvent, SharedString, Task, WeakEntity, Window};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod view;

const DEBOUNCE: Duration = Duration::from_millis(180);
const MODES: [(Kind, &str, &str); 3] = [
    (Kind::Text, "seg-text", "Text"),
    (Kind::Files, "seg-files", "Files"),
    (Kind::Symbols, "seg-symbols", "Symbols"),
];

pub struct SearchView {
    root: Option<PathBuf>,
    workspace: Option<WeakEntity<Workspace>>,
    mode: Kind,
    query: String,
    include: String,
    opts: Options,
    output: Output,
    running: bool,
    error: Option<SharedString>,
    /// File groups the user has collapsed, by relative path.
    collapsed: HashSet<String>,
    focus: FocusHandle,
    include_focus: FocusHandle,
    cancel: Cancel,
    task: Option<Task<()>>,
}

impl SearchView {
    pub fn new(
        root: Option<PathBuf>,
        workspace: Option<WeakEntity<Workspace>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            root,
            workspace,
            mode: Kind::Text,
            query: String::new(),
            include: String::new(),
            opts: Options::default(),
            output: Output::default(),
            running: false,
            error: None,
            collapsed: HashSet::new(),
            focus: cx.focus_handle(),
            include_focus: cx.focus_handle(),
            cancel: Cancel::new(),
            task: None,
        }
    }

    fn set_mode(&mut self, mode: Kind, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            self.restart(cx);
        }
    }

    fn toggle_case(&mut self, cx: &mut Context<Self>) {
        self.opts.case = !self.opts.case;
        self.restart(cx);
    }

    fn toggle_word(&mut self, cx: &mut Context<Self>) {
        self.opts.word = !self.opts.word;
        self.restart(cx);
    }

    fn toggle_regex(&mut self, cx: &mut Context<Self>) {
        self.opts.regex = !self.opts.regex;
        self.restart(cx);
    }

    fn toggle_group(&mut self, rel: String, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&rel) {
            self.collapsed.insert(rel);
        }
        cx.notify();
    }

    pub fn input_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if chrome::typed(&mut self.query, e) {
            self.restart(cx);
        }
        cx.notify();
    }

    pub fn include_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if chrome::typed(&mut self.include, e) {
            self.restart(cx);
        }
        cx.notify();
    }

    /// Replace the running search with one for the current query and mode.
    fn restart(&mut self, cx: &mut Context<Self>) {
        // Cancels whatever the previous task is doing, then supersedes its token.
        self.cancel.cancel();
        self.cancel = Cancel::new();
        let Some(root) = self.root.clone() else {
            return;
        };
        let query = self.query.clone();
        if query.is_empty() {
            self.output = Output::default();
            self.error = None;
            self.running = false;
            self.task = None;
            cx.notify();
            return;
        }
        let (include, opts, kind) = (self.include.clone(), self.opts, self.mode);
        let cancel = self.cancel.clone();
        self.running = true;
        self.error = None;
        self.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            if cancel.is_cancelled() {
                return;
            }
            let (work_cancel, work_root, work_query, work_include) =
                (cancel.clone(), root, query, include);
            let result = cx
                .background_executor()
                .spawn(async move {
                    engine::run(
                        &work_root,
                        kind,
                        &work_query,
                        opts,
                        &work_include,
                        &work_cancel,
                    )
                })
                .await;
            this.update(cx, |this, cx| {
                if cancel.is_cancelled() {
                    return;
                }
                this.running = false;
                match result {
                    Ok(output) => {
                        this.output = output;
                        this.error = None;
                    }
                    Err(message) => {
                        this.output = Output::default();
                        this.error = Some(message.into());
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn open(&self, path: &Path, line: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        workspace
            .update(cx, |workspace, cx| match line {
                Some(line) => workspace.open_file_at(path, line, window, cx),
                None => workspace.open_file(path, window, cx),
            })
            .ok();
    }
}
