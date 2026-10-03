//! Search everywhere (Ctrl+Shift+O): one modal from anywhere that shows files,
//! symbols and text together, with a type per tab. Same engine as the sidebar.
use super::chrome::{self, Mark};
use super::engine::{self, Cancel, FileMatch, Kind, LineHit, Output, SymbolMatch, TextFile};
use super::matcher::Options;
use crate::keymap;
use crate::settings::Settings;
use crate::theme::Theme;
use crate::workspace::Workspace;
use gpui::{
    div, prelude::*, px, AnyElement, Context, FocusHandle, KeyDownEvent, MouseButton,
    MouseDownEvent, Render, ScrollHandle, SharedString, Task, WeakEntity, Window,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

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
    pub fn show(&mut self, cx: &mut Context<Self>) {
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

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.task = None;
        self.open = false;
        self.running = false;
        cx.notify();
    }

    pub fn input_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "escape" => self.close(cx),
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

impl Render for EverywhereView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        let chip = keymap::effective("SearchEverywhere", &cx.global::<Settings>().shortcuts)
            .first()
            .map(|keys| keymap::display(keys));

        // Tabs.
        let mut tabs = div()
            .flex()
            .gap(px(4.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(theme.line);
        for (kind, id, label) in TABS {
            tabs = tabs.child(
                div()
                    .id(id)
                    .px(px(12.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(self.tab == kind, |d| d.bg(theme.sel).text_color(theme.ink))
                    .when(self.tab != kind, |d| {
                        d.text_color(theme.mute).hover(|d| d.text_color(theme.ink))
                    })
                    .child(SharedString::from(label))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_tab(kind, cx))),
            );
        }

        // Results.
        let mut results = div()
            .id("se-results")
            .flex()
            .flex_col()
            .max_h(px(520.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll);
        let mut flat = 0;
        for (label, kind) in self.groups() {
            results = results.child(group_label(label, &theme));
            for item in self.items(kind) {
                let active = flat == self.selected;
                let row = match item {
                    Item::File(file) => {
                        let path = file.file.path.clone();
                        chrome::row(("se", flat), active, &theme)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                    this.open(&path, 0, window, cx)
                                }),
                            )
                            .child(chrome::file_icon(&file.file.name, &theme))
                            .child(chrome::marked_positions(
                                &file.file.name,
                                &file.positions,
                                Mark::Accent,
                                &theme,
                            ))
                            .child(location(&file.file.dir, false, &theme))
                    }
                    Item::Symbol(symbol) => {
                        let path = symbol.file.path.clone();
                        let line = symbol.line;
                        chrome::row(("se", flat), active, &theme)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                    this.open(&path, line, window, cx)
                                }),
                            )
                            .child(chrome::kind_badge(symbol.kind, &theme))
                            .child(chrome::marked_positions(
                                &symbol.name,
                                &symbol.positions,
                                Mark::Accent,
                                &theme,
                            ))
                            .child(location(
                                &format!("{}:{}", symbol.file.name, symbol.line + 1),
                                false,
                                &theme,
                            ))
                    }
                    Item::Hit(file, hit) => {
                        let path = file.file.path.clone();
                        let line = hit.line;
                        chrome::row(("se", flat), active, &theme)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                    this.open(&path, line, window, cx)
                                }),
                            )
                            .child(chrome::marked(
                                &hit.text,
                                &hit.ranges,
                                Mark::Background,
                                &theme,
                            ))
                            .child(location(
                                &format!("{}:{}", file.file.name, hit.line + 1),
                                true,
                                &theme,
                            ))
                    }
                };
                results = results.child(row);
                flat += 1;
            }
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
            .child("Tab switch type")
            .child("Enter open")
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
                chrome::input_row("se-input", &self.focus, false, &theme, Self::input_key, cx)
                    .mx(px(0.))
                    .mb(px(0.))
                    .h(px(46.))
                    .px(px(16.))
                    .gap(px(10.))
                    .border_color(theme.line)
                    .child(div().text_color(theme.mute).child("⌕"))
                    .child(chrome::input_text(
                        &self.query,
                        "Search files, symbols and text",
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
            .child(tabs)
            .child(self.hint(&theme))
            .child(results)
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
                    .id("se-backdrop")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .bg(chrome::tint(theme.bg, 0.55))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, _, cx| this.close(cx)),
                    ),
            )
            .child(panel)
            .into_any_element()
    }
}

/// A group heading inside the modal.
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

/// The muted location at the right of a row.
fn location(location: &str, align_right: bool, theme: &Theme) -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .text_size(px(12.))
        .text_color(theme.mute)
        .when(align_right, |d| d.text_right())
        .child(SharedString::from(location.to_string()))
        .into_any_element()
}
