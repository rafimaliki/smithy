//! Sidebar file tree: a virtualized list over `FileTree` rows.
use super::FileTree;
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, Context, EventEmitter, MouseButton, MouseDownEvent, SharedString,
    UniformListScrollHandle, Window,
};
use std::ops::Range;
use std::path::PathBuf;

pub enum TreeEvent {
    /// A file row was clicked.
    Open(PathBuf),
}

pub struct TreeView {
    pub tree: FileTree,
    pub selected: Option<PathBuf>,
    scroll: UniformListScrollHandle,
}

impl EventEmitter<TreeEvent> for TreeView {}

impl TreeView {
    pub fn new(root: PathBuf) -> Self {
        Self {
            tree: FileTree::new(root),
            selected: None,
            scroll: UniformListScrollHandle::new(),
        }
    }

    fn rows(
        &mut self,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<impl IntoElement> {
        let theme = Theme::by_name(&cx.global::<Settings>().theme);
        range
            .filter_map(|i| self.tree.rows.get(i).cloned().map(|r| (i, r)))
            .map(|(i, row)| {
                let active = self.selected.as_ref() == Some(&row.path);
                let path = row.path.clone();
                let is_dir = row.is_dir;
                let chevron = match (row.is_dir, row.expanded) {
                    (true, true) => "▾",
                    (true, false) => "▸",
                    _ => " ",
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
                        cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                            if is_dir {
                                this.tree.toggle(&path);
                            } else {
                                this.selected = Some(path.clone());
                                cx.emit(TreeEvent::Open(path.clone()));
                            }
                            cx.notify();
                        }),
                    )
                    .child(div().w(px(10.)).text_color(theme.mute).child(chevron))
                    .child(SharedString::from(row.name))
            })
            .collect()
    }
}

impl Render for TreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.tree.rows.len();
        gpui::uniform_list("tree", count, cx.processor(Self::rows))
            .track_scroll(self.scroll.clone())
            .size_full()
    }
}
