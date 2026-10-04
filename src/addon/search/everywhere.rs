//! Search everywhere (Ctrl+Shift+O): one modal from anywhere that shows files,
//! symbols and text together, with a type per tab. Same engine as the sidebar.
use super::chrome;
use super::engine::{self, Cancel, FileMatch, Kind, LineHit, Output, SymbolMatch, TextFile};
use super::matcher::Options;
use crate::theme::Theme;
use crate::workspace::Workspace;
use gpui::{
    div, prelude::*, px, AnyElement, Context, FocusHandle, KeyDownEvent, ScrollHandle,
    SharedString, Task, WeakEntity, Window,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod view;

const DEBOUNCE: Duration = Duration::from_millis(180);
const TABS: [(Kind, &str, &str); 4] = [
    (Kind::All, "se-tab-all", "All"),
    (Kind::Files, "se-tab-files", "Files"),
    (Kind::Symbols, "se-tab-symbols", "Symbols"),
    (Kind::Text, "se-tab-text", "Text"),
];

/// One result row of the modal.
enum Item<'a> {
    File(&'a FileMatch),
    Symbol(&'a SymbolMatch),
    Hit(&'a TextFile, &'a LineHit),
}

impl Item<'_> {
    fn target(&self) -> (PathBuf, usize) {
        match self {
            Item::File(f) => (f.file.path.clone(), 0),
            Item::Symbol(s) => (s.file.path.clone(), s.line),
            Item::Hit(f, h) => (f.file.path.clone(), h.line),
        }
    }
}

pub struct EverywhereView {
    root: Option<PathBuf>,
    workspace: Option<WeakEntity<Workspace>>,
    open: bool,
    tab: Kind,
    query: String,
    output: Output,
    selected: usize,
    running: bool,
    error: Option<SharedString>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    cancel: Cancel,
    task: Option<Task<()>>,
    /// What had the keyboard before the modal opened; Esc hands it back.
    previous: Option<FocusHandle>,
}

impl EverywhereView {
    pub fn new(
        root: Option<PathBuf>,
        workspace: Option<WeakEntity<Workspace>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            root,
            workspace,
            open: false,
            tab: Kind::All,
            query: String::new(),
            output: Output::default(),
            selected: 0,
            running: false,
            error: None,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            cancel: Cancel::new(),
            task: None,
            previous: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The query field's focus handle; the core focuses it when the modal opens.
    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    /// Open with a fresh query and no results.
    pub fn show(&mut self, previous: Option<FocusHandle>, cx: &mut Context<Self>) {
        self.previous = previous;
        self.cancel.cancel();
        self.task = None;
        self.open = true;
        self.tab = Kind::All;
        self.query.clear();
        self.output = Output::default();
        self.selected = 0;
        self.running = false;
        self.error = None;
        cx.notify();
    }

    /// Open on one tab, e.g. Ctrl+P on Files.
    pub fn show_tab(&mut self, tab: Kind, previous: Option<FocusHandle>, cx: &mut Context<Self>) {
        self.show(previous, cx);
        self.tab = tab;
    }

    /// Close and give the keyboard back, so shortcuts keep working.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close(cx);
        if let Some(previous) = self.previous.take() {
            window.focus(&previous);
        }
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.task = None;
        self.open = false;
        self.running = false;
        cx.notify();
    }

    pub fn input_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "escape" => self.dismiss(window, cx),
            "enter" => self.open_selected(window, cx),
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            "tab" => {
                let delta = if e.keystroke.modifiers.shift { -1 } else { 1 };
                self.cycle_tab(delta, cx);
            }
            _ => {
                if chrome::typed(&mut self.query, e) {
                    self.selected = 0;
                    self.restart(cx);
                }
                cx.notify();
            }
        }
    }

    fn set_tab(&mut self, tab: Kind, cx: &mut Context<Self>) {
        if self.tab != tab {
            self.tab = tab;
            self.selected = 0;
            self.restart(cx);
        }
    }

    fn cycle_tab(&mut self, delta: isize, cx: &mut Context<Self>) {
        let at = TABS
            .iter()
            .position(|(k, _, _)| *k == self.tab)
            .unwrap_or(0) as isize;
        let next = (at + delta).rem_euclid(TABS.len() as isize) as usize;
        self.set_tab(TABS[next].0, cx);
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let rows = self.flat();
        if rows.is_empty() {
            return;
        }
        let last = rows.len() as isize - 1;
        self.selected = (self.selected.min(last as usize) as isize + delta).clamp(0, last) as usize;
        self.scroll.scroll_to_item(rows[self.selected].0);
        cx.notify();
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.flat();
        let Some((_, path, line)) = rows.get(self.selected) else {
            return;
        };
        let (path, line) = (path.clone(), *line);
        self.close(cx);
        if let Some(workspace) = self.workspace.clone() {
            workspace
                .update(cx, |workspace, cx| {
                    workspace.open_file_at(&path, line, window, cx)
                })
                .ok();
        }
    }

    fn open(&self, path: &Path, line: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let path = path.to_path_buf();
        self.cancel.cancel();
        workspace
            .update(cx, |workspace, cx| {
                workspace.open_file_at(&path, line, window, cx)
            })
            .ok();
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
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
        let kind = self.tab;
        let cancel = self.cancel.clone();
        self.running = true;
        self.error = None;
        self.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            if cancel.is_cancelled() {
                return;
            }
            let (work_cancel, work_root, work_query) = (cancel.clone(), root, query);
            let result = cx
                .background_executor()
                .spawn(async move {
                    engine::run(
                        &work_root,
                        kind,
                        &work_query,
                        Options::default(),
                        "",
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

    /// The groups shown, in display order, with their label.
    fn groups(&self) -> Vec<(&'static str, Kind)> {
        match self.tab {
            Kind::Files => vec![("Files in this project", Kind::Files)],
            Kind::Symbols => vec![("Symbols in this project", Kind::Symbols)],
            Kind::Text => vec![("Text in this project", Kind::Text)],
            Kind::All => [
                ("Files", Kind::Files),
                ("Symbols", Kind::Symbols),
                ("Text", Kind::Text),
            ]
            .into_iter()
            .filter(|(_, kind)| !self.items(*kind).is_empty())
            .collect(),
        }
    }

    fn items(&self, kind: Kind) -> Vec<Item<'_>> {
        match kind {
            Kind::Files => self.output.files.iter().map(Item::File).collect(),
            Kind::Symbols => self.output.symbols.iter().map(Item::Symbol).collect(),
            Kind::Text => self
                .output
                .text
                .iter()
                .flat_map(|file| file.hits.iter().map(move |hit| Item::Hit(file, hit)))
                .collect(),
            Kind::All => Vec::new(),
        }
    }

    /// Every selectable row: its child index in the scroll container, the file
    /// to open and the line to land on.
    fn flat(&self) -> Vec<(usize, PathBuf, usize)> {
        let mut child = 0;
        let mut out = Vec::new();
        for (_, kind) in self.groups() {
            child += 1; // the group label
            for item in self.items(kind) {
                let (path, line) = item.target();
                out.push((child, path, line));
                child += 1;
            }
        }
        out
    }

    fn hint(&self, theme: &Theme) -> AnyElement {
        if let Some(error) = &self.error {
            return chrome::count_line(&format!("Invalid regular expression: {error}"), theme);
        }
        if self.running {
            return chrome::count_line("Searching…", theme);
        }
        if !self.query.is_empty() && self.output.is_empty() {
            return chrome::count_line(&format!("No results for “{}”", self.query.trim()), theme);
        }
        div().h(px(6.)).into_any_element()
    }
}
