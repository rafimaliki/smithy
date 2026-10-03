//! The reusable file-diff view: inline or side by side, word-level marks, and
//! optional per-hunk actions. `a-pr` uses the same view read-only.
use super::diff_lines::{body, file_header};
use crate::git::{DiffLayout, FileDiff};
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyView, Context, Render, SharedString, Window};

/// What a hunk button asks the owner to do.
#[derive(Clone)]
pub enum DiffAction {
    StageHunk { rel: String, hunk: usize },
    UnstageHunk { rel: String, hunk: usize },
    DiscardHunk { rel: String, hunk: usize },
    OpenFile { rel: String },
}

pub type ActionHandler = Box<dyn Fn(DiffAction, &mut Window, &mut Context<DiffView>)>;

#[derive(Clone, Copy, PartialEq)]
pub enum HunkActions {
    Staged,
    Unstaged,
}

#[derive(Clone, Default)]
pub struct DiffOptions {
    /// Shown as a chip in the toolbar, e.g. "changes" or "main...feat/x".
    pub chip: Option<SharedString>,
    pub hunk_actions: Option<HunkActions>,
    pub open_file: bool,
}

pub struct DiffView {
    files: Vec<FileDiff>,
    pub(super) layout: DiffLayout,
    options: DiffOptions,
    on_action: Option<ActionHandler>,
    /// A view the owner wants drawn over the diff, e.g. the discard dialog.
    overlay: Option<AnyView>,
}

impl DiffView {
    pub fn new(
        files: Vec<FileDiff>,
        layout: DiffLayout,
        options: DiffOptions,
        on_action: Option<ActionHandler>,
    ) -> Self {
        Self {
            files,
            layout,
            options,
            on_action,
            overlay: None,
        }
    }

    pub fn set_files(&mut self, files: Vec<FileDiff>) {
        self.files = files;
    }

    pub fn set_overlay(&mut self, overlay: Option<AnyView>) {
        self.overlay = overlay;
    }
}

pub(super) fn action(
    view: &mut DiffView,
    request: DiffAction,
    window: &mut Window,
    cx: &mut Context<DiffView>,
) {
    if let Some(handler) = view.on_action.as_ref() {
        handler(request, window, cx);
    }
}

impl Render for DiffView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::by_name(&cx.global::<crate::settings::Settings>().theme);
        let layout = self.layout;
        let options = self.options.clone();
        let mut column = div().flex().flex_col().p(px(6.));
        if self.files.is_empty() {
            column = column.child(
                div()
                    .p(px(24.))
                    .text_color(theme.mute)
                    .child("No changes to show."),
            );
        }
        for (i, file) in self.files.iter().enumerate() {
            let header = file_header(&theme, file, i, layout, &options, cx);
            let hunks = body(&theme, file, i, layout, &options, cx);
            column = column.child(header).child(hunks);
        }
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.bg)
            .text_color(theme.ink)
            .font_family("Segoe UI")
            .text_size(px(13.))
            .child(
                div()
                    .id("diff-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(column),
            )
            .children(self.overlay.clone())
    }
}
