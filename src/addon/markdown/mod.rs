//! Markdown preview add-on: a `.md` tab gets Code / Side by side / Preview.
//!
//! The core keeps the tab's editor (saving, dirty state and the unsaved prompt are
//! unchanged); this add-on puts the mode bar and the rendered pane around it through
//! [`AddonInstance::document_view`]. The board frame is `flows/files/markdown-preview`.
mod parse;
mod render;

use super::{Addon, AddonContext, AddonInfo, AddonInstance};
use crate::actions::TogglePreview;
use crate::editor::view::EditorView;
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyView, App, Context, Entity, FocusHandle, FontWeight, Render,
    SharedString, WeakEntity, Window,
};
use parse::Block;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const EXTENSIONS: [&str; 2] = ["md", "markdown"];

pub struct Markdown;

impl Addon for Markdown {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "markdown-preview",
            name: "Markdown preview",
            description: "Open Markdown files as code, side by side, or a rendered preview.",
            requires: &[],
        }
    }

    fn start(&self, _ctx: &AddonContext, _cx: &mut App) -> Box<dyn AddonInstance> {
        Box::new(Instance::default())
    }
}

#[derive(Default)]
struct Instance {
    /// One view per Markdown path, so its mode and scroll position survive a re-render.
    views: RefCell<HashMap<PathBuf, Entity<MarkdownView>>>,
}

impl AddonInstance for Instance {
    fn document_view(
        &self,
        path: &Path,
        editor: Entity<EditorView>,
        cx: &mut App,
    ) -> Option<AnyView> {
        if !handles(path) {
            return None;
        }
        let mut views = self.views.borrow_mut();
        let view = match views.get(path) {
            Some(view) => {
                view.update(cx, |view, _| view.editor = editor.downgrade());
                view.clone()
            }
            None => {
                let view =
                    cx.new(|cx| MarkdownView::new(path.to_path_buf(), editor.downgrade(), cx));
                views.insert(path.to_path_buf(), view.clone());
                view
            }
        };
        Some(view.into())
    }
}

fn handles(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Code,
    Side,
    Preview,
}

impl Mode {
    fn index(self) -> usize {
        match self {
            Mode::Code => 0,
            Mode::Side => 1,
            Mode::Preview => 2,
        }
    }
}

struct MarkdownView {
    path: PathBuf,
    /// The core owns the editor; this view only draws it.
    editor: WeakEntity<EditorView>,
    mode: Mode,
    focus: FocusHandle,
    blocks: Vec<Block>,
    /// Buffer revision the blocks were parsed from, so a frame does not re-parse.
    parsed: Option<u64>,
}

impl MarkdownView {
    fn new(path: PathBuf, editor: WeakEntity<EditorView>, cx: &mut Context<Self>) -> Self {
        Self {
            path,
            editor,
            mode: Mode::Code,
            focus: cx.focus_handle(),
            blocks: Vec::new(),
            parsed: None,
        }
    }

    fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = mode;
        match mode {
            Mode::Preview => window.focus(&self.focus),
            _ => {
                if let Some(editor) = self.editor.upgrade() {
                    editor.read(cx).focus(window);
                }
            }
        }
        cx.notify();
    }

    fn toggle_preview(&mut self, _: &TogglePreview, window: &mut Window, cx: &mut Context<Self>) {
        let next = match self.mode {
            Mode::Preview => Mode::Code,
            _ => Mode::Preview,
        };
        self.set_mode(next, window, cx);
    }

    /// Parse again only when the buffer changed since the last parse.
    fn reparse(&mut self, editor: Option<&Entity<EditorView>>, cx: &App) {
        let Some(editor) = editor else { return };
        let buffer = &editor.read(cx).state.buffer;
        if self.parsed == Some(buffer.revision) {
            return;
        }
        self.blocks = parse::parse(&buffer.text());
        self.parsed = Some(buffer.revision);
    }

    /// One mode button: `settings_view`'s chip shape, in the bar's muted ink.
    fn mode_button(
        &self,
        label: &'static str,
        mode: Mode,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let on = self.mode == mode;
        div()
            .id(("markdown-mode", mode.index()))
            .flex_none()
            .px(px(12.))
            .py(px(4.))
            .rounded(px(6.))
            .cursor_pointer()
            .border_1()
            .border_color(if on { t.sel } else { t.line })
            .when(on, |d| d.bg(t.sel).text_color(t.ink))
            .when(!on, |d| {
                d.text_color(t.mute)
                    .hover(|d| d.bg(t.hov).text_color(t.ink))
            })
            .on_click(cx.listener(move |this, _, window, cx| this.set_mode(mode, window, cx)))
            .child(label)
    }

    fn bar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        div()
            .h(px(40.))
            .flex_none()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(6.))
            .px(px(14.))
            .border_b_1()
            .border_color(t.line)
            .text_color(t.mute)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .mr(px(8.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.ink)
                    .child(SharedString::from(name)),
            )
            .child(self.mode_button("Code", Mode::Code, t, cx))
            .child(self.mode_button("Side by side", Mode::Side, t, cx))
            .child(self.mode_button("Preview", Mode::Preview, t, cx))
    }
}

impl Render for MarkdownView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::by_name(&cx.global::<Settings>().theme);
        let editor = self.editor.upgrade();
        if self.mode != Mode::Code {
            self.reparse(editor.as_ref(), cx);
        }
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::toggle_preview))
            .child(self.bar(&t, cx));
        match self.mode {
            Mode::Code => {
                root = root.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .when_some(editor, |d, editor| d.child(editor)),
                );
            }
            Mode::Preview => {
                root = root.child(div().flex_1().min_h_0().child(render::document(
                    &self.path,
                    &self.blocks,
                    &t,
                )));
            }
            Mode::Side => {
                root = root.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .when_some(editor, |d, editor| d.child(editor)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .border_l_1()
                                .border_color(t.line)
                                .child(render::document(&self.path, &self.blocks, &t)),
                        ),
                );
            }
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_markdown_case_insensitively_and_nothing_else() {
        assert!(handles(Path::new("a/README.MD")));
        assert!(handles(Path::new("x.markdown")));
        assert!(!handles(Path::new("x.mdx")));
        assert!(!handles(Path::new("noext")));
    }
}
