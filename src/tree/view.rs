//! Sidebar file tree: a virtualized list over `FileTree` rows, with file icons,
//! the right-click menu, the inline rename field and the F2 / Del shortcuts.
use super::context::Menu;
use super::icons::{self, Mark};
use super::rename::Input;
use super::FileTree;
use crate::editor::view::MONO;
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, font, prelude::*, px, AnyElement, Context, Div, EventEmitter, FocusHandle, KeyDownEvent,
    MouseButton, MouseDownEvent, Render, SharedString, Stateful, UniformListScrollHandle, Window,
};
use std::ops::Range;
use std::path::PathBuf;

pub enum TreeEvent {
    /// A file row was clicked, or Enter was pressed on it.
    Open(PathBuf),
    /// The user asked to delete; the workspace confirms and moves it to the Bin.
    DeleteRequested(PathBuf),
    /// A path moved on disk (rename or cut then paste); open tabs must follow.
    Moved { from: PathBuf, to: PathBuf },
    /// A file operation failed; the workspace shows it.
    Error(String),
}

pub(super) struct Rename {
    pub path: PathBuf,
    pub input: Input,
}

pub struct TreeView {
    pub tree: FileTree,
    pub selected: Option<PathBuf>,
    scroll: UniformListScrollHandle,
    pub(super) menu: Option<Menu>,
    pub(super) rename: Option<Rename>,
    pub(super) focus: FocusHandle,
    pub(super) ren_focus: FocusHandle,
}

impl EventEmitter<TreeEvent> for TreeView {}

impl TreeView {
    pub fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        Self {
            tree: FileTree::new(root),
            selected: None,
            scroll: UniformListScrollHandle::new(),
            menu: None,
            rename: None,
            focus: cx.focus_handle(),
            ren_focus: cx.focus_handle(),
        }
    }

    /// Re-read the folder and drop a selection that no longer exists.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.tree.refresh();
        if self.selected.as_ref().is_some_and(|p| !p.exists()) {
            self.selected = None;
        }
        cx.notify();
    }

    /// Start the inline rename field on `path`. `created` marks an entry made by
    /// New file / New folder, which Esc removes again.
    pub fn begin_rename(
        &mut self,
        path: PathBuf,
        created: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.selected = Some(path.clone());
        self.menu = None;
        self.rename = Some(Rename {
            path,
            input: Input::for_name(&name, created),
        });
        cx.notify();
        window.focus(&self.ren_focus);
    }

    /// The rename field drawn over the row's name.
    fn rename_field(&self, t: &Theme, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(rename) = self.rename.as_ref() else {
            return div().into_any_element();
        };
        let input = &rename.input;
        // ponytail: the field measures one monospace advance instead of shaping the
        // text, like the editor view does; upgrade: shape_line + x_for_index.
        let f = font(MONO);
        let id = window.text_system().resolve_font(&f);
        let char_w = window
            .text_system()
            .advance(id, px(13.), 'm')
            .map(|a| f32::from(a.width))
            .unwrap_or(8.0);
        let (a, b) = input.selection();
        let mut highlight: gpui::Hsla = t.acc.into();
        highlight.a = 0.45;
        let mut bar = div().relative().flex_1().min_w_0().h_full();
        if b > a {
            bar = bar.child(
                div()
                    .absolute()
                    .top_0()
                    .h_full()
                    .left(px(a as f32 * char_w))
                    .w(px((b - a) as f32 * char_w))
                    .bg(highlight),
            );
        } else {
            bar = bar.child(
                div()
                    .absolute()
                    .top(px(2.))
                    .h(px(14.))
                    .w(px(2.))
                    .left(px(input.cursor() as f32 * char_w))
                    .bg(t.acc),
            );
        }
        let mut field = div()
            .id("rename-field")
            .track_focus(&self.ren_focus)
            .key_context("Rename")
            .on_key_down(cx.listener(Self::rename_key))
            .flex()
            .items_center()
            .h(px(20.))
            .px(px(4.))
            .mr(px(8.))
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .rounded(px(3.))
            .bg(t.bg)
            .border_1()
            .border_color(if input.error.is_some() { t.del } else { t.acc })
            .font_family(MONO)
            .text_size(px(13.))
            .text_color(t.ink)
            .child(
                bar.child(
                    div()
                        .whitespace_nowrap()
                        .child(SharedString::from(input.text())),
                ),
            );
        if let Some(error) = input.error.clone() {
            field = field.child(
                div()
                    .ml(px(6.))
                    .text_size(px(11.))
                    .text_color(t.del)
                    .child(SharedString::from(error)),
            );
        }
        field.into_any_element()
    }

    fn rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Stateful<Div>> {
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        let simple = cx.global::<Settings>().file_icons != "brand";
        range
            .filter_map(|i| self.tree.rows.get(i).cloned().map(|r| (i, r)))
            .map(|(i, row)| {
                let active = self.selected.as_ref() == Some(&row.path);
                let (left_path, right_path) = (row.path.clone(), row.path.clone());
                let is_dir = row.is_dir;
                let chevron = match (row.is_dir, row.expanded) {
                    (true, true) => "▾",
                    (true, false) => "▸",
                    _ => " ",
                };
                let name: AnyElement = if self.rename.as_ref().is_some_and(|r| r.path == row.path) {
                    self.rename_field(&theme, window, cx)
                } else {
                    div()
                        .child(SharedString::from(row.name.clone()))
                        .into_any_element()
                };
                div()
                    .id(("row", i))
                    .flex()
                    .w_full()
                    .items_center()
                    .gap(px(7.))
                    .h(px(26.))
                    .pl(px(10. + 14. * row.depth as f32))
                    .whitespace_nowrap()
                    .text_color(theme.ink)
                    .when(active, |d| d.bg(theme.sel))
                    .hover(|d| d.bg(theme.hov))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            window.focus(&this.focus);
                            if is_dir {
                                this.selected = Some(left_path.clone());
                                this.tree.toggle(&left_path);
                            } else {
                                this.selected = Some(left_path.clone());
                                cx.emit(TreeEvent::Open(left_path.clone()));
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                            window.focus(&this.focus);
                            this.selected = Some(right_path.clone());
                            this.menu = Some(Menu {
                                path: right_path.clone(),
                                is_dir,
                                at: e.position,
                            });
                            cx.notify();
                        }),
                    )
                    .child(div().w(px(10.)).text_color(theme.mute).child(chevron))
                    .child(icon(&theme, simple, &row.path, row.is_dir))
                    .child(name)
            })
            .collect()
    }

    fn tree_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.rename.is_some() {
            return; // the field owns the keyboard
        }
        let had_menu = self.menu.take().is_some();
        let m = e.keystroke.modifiers;
        let Some(path) = self.selected.clone() else {
            if had_menu {
                cx.notify();
            }
            return;
        };
        match e.keystroke.key.as_str() {
            "f2" => self.begin_rename(path, false, window, cx),
            "enter" => {
                if path.is_dir() {
                    self.tree.toggle(&path);
                    cx.notify();
                } else {
                    cx.emit(TreeEvent::Open(path))
                }
            }
            "delete" => cx.emit(TreeEvent::DeleteRequested(path)),
            "c" if m.control && m.shift && m.alt => self.copy_path(path, false, cx),
            "c" if m.control => self.hold(path, false, cx),
            "x" if m.control => self.hold(path, true, cx),
            "v" if m.control => self.paste_near(path, cx),
            _ => {
                if had_menu {
                    cx.notify();
                }
            }
        }
    }

    fn rename_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = e.keystroke.key.clone();
        cx.stop_propagation();
        if key == "enter" {
            self.commit_rename(window, cx);
            return;
        }
        if key == "escape" {
            self.cancel_rename(cx);
            return;
        }
        let ctrl = e.keystroke.modifiers.control;
        let extend = e.keystroke.modifiers.shift;
        let Some(rename) = self.rename.as_mut() else {
            return;
        };
        let input = &mut rename.input;
        match key.as_str() {
            "backspace" => input.backspace(),
            "delete" => input.delete(),
            "left" => input.move_left(extend),
            "right" => input.move_right(extend),
            "home" => input.home(extend),
            "end" => input.end(extend),
            "a" if ctrl => input.select_all(),
            _ => {
                // Windows reports the space bar as key "space" with no key_char.
                if let Some(ch) = typed_char(e, &key) {
                    if !ctrl && !e.keystroke.modifiers.platform {
                        input.insert(&ch);
                    }
                }
            }
        }
        input.error = None;
        cx.notify();
    }
}

fn typed_char(e: &KeyDownEvent, key: &str) -> Option<String> {
    match (e.keystroke.key_char.as_deref(), key) {
        (Some(ch), _) if !ch.chars().any(|c| c.is_control()) => Some(ch.to_string()),
        (None, "space") => Some(" ".to_string()),
        _ => None,
    }
}

fn icon(t: &Theme, simple: bool, path: &std::path::Path, is_dir: bool) -> AnyElement {
    let (asset, color, size) = if simple {
        (
            if is_dir { icons::FOLDER } else { icons::FILE },
            t.mute,
            if is_dir { 16. } else { 14. },
        )
    } else {
        match icons::mark(path, is_dir) {
            Mark::Brand { asset, color } => (asset, color, 15.),
            Mark::Folder => (icons::FOLDER_FILLED, t.acc, 16.),
            Mark::File => (icons::FILE, t.mute, 14.),
        }
    };
    gpui::svg()
        .path(asset)
        .w(px(size))
        .h(px(size))
        .flex_none()
        .text_color(color)
        .into_any_element()
}

impl Render for TreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        let count = self.tree.rows.len();
        div()
            .relative()
            .size_full()
            .key_context("Tree")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::tree_key))
            .child(
                gpui::uniform_list("tree", count, cx.processor(Self::rows))
                    .track_scroll(self.scroll.clone())
                    .size_full(),
            )
            .children(self.context_menu(&theme, cx))
    }
}
