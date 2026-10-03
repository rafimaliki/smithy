//! Rendering for the Search sidebar. State and behavior are in `sidebar.rs`.
use super::chrome::{self, Mark};
use super::engine::{FileMatch, Kind, SymbolMatch, TextFile};
use super::{SearchView, MODES};
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent, Render, SharedString,
    Window,
};

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

impl SearchView {
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
