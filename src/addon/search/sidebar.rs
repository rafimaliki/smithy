//! The Search sidebar: Text, Files and Symbols over one query. Behavior and
//! state live here; the shared row and field styling is in `chrome.rs`.
//!
//! The search runs on a background task. Each keystroke replaces the task, which
//! cancels the one before it, and the worker also checks its `Cancel` flag, so a
//! slow folder never blocks typing. A short delay keeps a fast typist from
//! starting a search per character.
use super::chrome::{self, Mark};
use super::engine::{self, Cancel, FileMatch, Kind, Output, SymbolMatch, TextFile};
use super::matcher::Options;
use crate::settings::Settings;
use crate::theme::Theme;
use crate::workspace::Workspace;
use gpui::{
    div, prelude::*, px, AnyElement, Context, FocusHandle, KeyDownEvent, MouseButton,
    MouseDownEvent, Render, SharedString, Task, WeakEntity, Window,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

    /// The line above the results: the count, or why there is nothing.
    fn status(&self, theme: &Theme) -> AnyElement {
        if let Some(error) = &self.error {
            return chrome::error_line(&format!("Invalid regular expression: {error}"), theme);
        }
        if self.running {
            return chrome::count_line("Searching…", theme);
        }
        if self.query.is_empty() {
            return chrome::count_line(
                match self.mode {
                    Kind::Files => "Type to search file names in this folder",
                    Kind::Symbols => "Type to search symbols in this folder",
                    _ => "Type to search text in this folder",
                },
                theme,
            );
        }
        if self.output.is_empty() {
            return chrome::count_line(&format!("No results for “{}”", self.query.trim()), theme);
        }
        chrome::count_line(&self.summary(), theme)
    }

    fn summary(&self) -> String {
        match self.mode {
            Kind::Files => count(self.output.files.len(), "file"),
            Kind::Symbols => count(self.output.symbols.len(), "symbol"),
            _ => {
                let files = self.output.text.len();
                let hits = self.output.text_hits();
                format!("{} in {}", count(hits, "result"), count(files, "file"))
            }
        }
    }

    fn text_list(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div()
            .id("search-text-results")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for (i, file) in self.output.text.iter().enumerate() {
            col = col.child(self.text_group(i, file, theme, cx));
            if self.collapsed.contains(&file.file.rel) {
                continue;
            }
            for (j, hit) in file.hits.iter().enumerate() {
                let path = file.file.path.clone();
                let line = hit.line;
                col = col.child(
                    chrome::hit_row(("hit", i * 1000 + j), theme)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                this.open(&path, Some(line), window, cx)
                            }),
                        )
                        .child(chrome::marked(
                            &hit.text,
                            &hit.ranges,
                            Mark::Background,
                            theme,
                        )),
                );
            }
        }
        col.into_any_element()
    }

    /// One file group header: the file, its folder, and how many hits.
    fn text_group(
        &self,
        index: usize,
        file: &TextFile,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rel = file.file.rel.clone();
        let collapsed = self.collapsed.contains(&rel);
        div()
            .id(("group", index))
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(26.))
            .px(px(10.))
            .flex_none()
            .cursor_pointer()
            .text_size(px(12.))
            .hover(|d| d.bg(theme.hov))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    this.toggle_group(rel.clone(), cx)
                }),
            )
            .child(div().w(px(10.)).text_color(theme.mute).child(if collapsed {
                "▸"
            } else {
                "▾"
            }))
            .child(chrome::file_icon(&file.file.name, theme))
            .child(
                div()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(SharedString::from(file.file.name.clone())),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_color(theme.mute)
                    .child(SharedString::from(file.file.dir.clone())),
            )
            .child(
                div()
                    .px(px(7.))
                    .rounded(px(9.))
                    .bg(theme.sel)
                    .text_color(theme.mute)
                    .child(SharedString::from(file.hits.len().to_string())),
            )
            .into_any_element()
    }

    fn file_list(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div()
            .id("search-file-results")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for (i, file) in self.output.files.iter().enumerate() {
            col = col.child(file_row(i, file, theme, cx));
        }
        col.into_any_element()
    }

    fn symbol_list(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div()
            .id("search-symbol-results")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for (i, symbol) in self.output.symbols.iter().enumerate() {
            col = col.child(symbol_row(i, symbol, theme, cx));
        }
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(col)
            .child(chrome::footnote(
                "Symbols come from a built-in outline of your code (functions, classes, \
                 properties). No language server needed.",
                theme,
            ))
            .into_any_element()
    }
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn file_row(
    index: usize,
    file: &FileMatch,
    theme: &Theme,
    cx: &mut Context<SearchView>,
) -> AnyElement {
    let path = file.file.path.clone();
    chrome::row(("file", index), false, theme)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                this.open(&path, None, window, cx)
            }),
        )
        .child(chrome::file_icon(&file.file.name, theme))
        .child(chrome::marked_positions(
            &file.file.name,
            &file.positions,
            Mark::Accent,
            theme,
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_size(px(12.))
                .text_color(theme.mute)
                .child(SharedString::from(file.file.dir.clone())),
        )
        .into_any_element()
}

fn symbol_row(
    index: usize,
    symbol: &SymbolMatch,
    theme: &Theme,
    cx: &mut Context<SearchView>,
) -> AnyElement {
    let path = symbol.file.path.clone();
    let line = symbol.line;
    chrome::row(("symbol", index), false, theme)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                this.open(&path, Some(line), window, cx)
            }),
        )
        .child(chrome::kind_badge(symbol.kind, theme))
        .child(chrome::marked_positions(
            &symbol.name,
            &symbol.positions,
            Mark::Accent,
            theme,
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_size(px(12.))
                .text_color(theme.mute)
                .child(SharedString::from(format!(
                    "{} · {}",
                    symbol.file.name, symbol.file.dir
                ))),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.mute)
                .child(SharedString::from((line + 1).to_string())),
        )
        .into_any_element()
}

impl Render for SearchView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        let items = MODES
            .iter()
            .map(|(kind, id, label)| {
                let kind = *kind;
                (
                    *id,
                    *label,
                    self.mode == kind,
                    Box::new(move |this: &mut Self, cx: &mut Context<Self>| this.set_mode(kind, cx))
                        as Box<dyn Fn(&mut Self, &mut Context<Self>)>,
                )
            })
            .collect();
        let query_focused = window.focused(cx).as_ref() == Some(&self.focus);
        let include_focused = window.focused(cx).as_ref() == Some(&self.include_focus);
        let placeholder = match self.mode {
            Kind::Files => "Search files by name",
            Kind::Symbols => "Search symbols",
            _ => "Search",
        };
        let mut input = chrome::input_row(
            "search-input",
            &self.focus,
            false,
            &theme,
            Self::input_key,
            cx,
        )
        .border_color(if query_focused { theme.acc } else { theme.line })
        .child(chrome::input_text(&self.query, placeholder, 13., &theme));
        if self.mode == Kind::Text {
            input = input
                .child(
                    chrome::toggle("toggle-case", "Aa", self.opts.case, &theme)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_case(cx))),
                )
                .child(
                    chrome::toggle("toggle-word", "ab", self.opts.word, &theme)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_word(cx))),
                )
                .child(
                    chrome::toggle("toggle-regex", ".*", self.opts.regex, &theme)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_regex(cx))),
                );
        }
        let mut col = div()
            .flex()
            .flex_col()
            .size_full()
            .pt(px(2.))
            .text_color(theme.ink)
            .text_size(px(13.))
            .font_family("Segoe UI")
            .child(chrome::segmented(items, &theme, cx))
            .child(input);
        if self.mode == Kind::Text {
            col = col.child(
                chrome::input_row(
                    "search-include",
                    &self.include_focus,
                    true,
                    &theme,
                    Self::include_key,
                    cx,
                )
                .border_color(if include_focused {
                    theme.acc
                } else {
                    theme.line
                })
                .child(chrome::input_text(
                    &self.include,
                    "files to include, e.g. src/**/*.tsx",
                    12.,
                    &theme,
                )),
            );
        }
        col = col.child(self.status(&theme));
        match self.mode {
            Kind::Files => col.child(self.file_list(&theme, cx)),
            Kind::Symbols => col.child(self.symbol_list(&theme, cx)),
            _ => col.child(self.text_list(&theme, cx)),
        }
    }
}
