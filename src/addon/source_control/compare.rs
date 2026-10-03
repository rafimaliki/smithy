//! Branch compare: merge-base diff of a local base against the current branch,
//! plus the base picker. Rendering is a free function over `ChangesView`.
use super::changes::ChangesView;
use crate::git::split_path;
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, Context, KeyDownEvent, SharedString, Window};

impl ChangesView {
    pub(super) fn reload_compare(&mut self) {
        let Some(repo) = self.repo.as_ref() else {
            self.compare = None;
            return;
        };
        let Some(base) = self.base.clone() else {
            self.compare = None;
            return;
        };
        match repo.compare(&base) {
            Ok(compare) => {
                self.compare = Some(compare);
                self.compare_error = None;
            }
            Err(e) => {
                self.compare = None;
                self.compare_error = Some(format!("git: {e}").into());
            }
        }
    }

    pub fn set_base(&mut self, base: &str, cx: &mut Context<Self>) {
        self.base = Some(base.to_string());
        self.picker = false;
        self.filter.clear();
        self.compare_selected = None;
        self.reload_compare();
        self.refresh_open(cx);
        cx.notify();
    }

    /// Typing in the branch filter.
    pub fn filter_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let m = e.keystroke.modifiers;
        if m.control || m.alt || m.platform {
            return;
        }
        match e.keystroke.key.as_str() {
            "backspace" => {
                self.filter.pop();
            }
            "escape" => {
                self.picker = false;
            }
            key => {
                let typed = e.keystroke.key_char.as_deref().or(match key {
                    "space" => Some(" "),
                    _ => None,
                });
                if let Some(ch) = typed {
                    if !ch.chars().any(char::is_control) {
                        self.filter.push_str(ch);
                    }
                }
            }
        }
        cx.notify();
    }
}

fn selector(
    id: &'static str,
    label: &str,
    value: &str,
    theme: &Theme,
    on_click: impl Fn(&mut ChangesView, &mut Context<ChangesView>) + 'static,
    cx: &mut Context<ChangesView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(32.))
        .mx(px(12.))
        .mb(px(8.))
        .px(px(10.))
        .flex_none()
        .border_1()
        .border_color(theme.line)
        .rounded(px(6.))
        .bg(theme.bg)
        .cursor_pointer()
        .hover(|d| d.border_color(theme.acc))
        .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
        .child(
            div()
                .mr(px(8.))
                .text_size(px(11.))
                .text_color(theme.mute)
                .child(SharedString::from(label.to_string())),
        )
        .child(
            div()
                .text_color(theme.ink)
                .child(SharedString::from(value.to_string())),
        )
        .child(div().ml_auto().text_color(theme.mute).child("▾"))
        .into_any_element()
}

fn file_counts(view: &ChangesView) -> String {
    match &view.compare {
        Some(c) => format!(
            "{} commit{} · {} file{} · +{} -{}",
            c.commits,
            if c.commits == 1 { "" } else { "s" },
            c.files.len(),
            if c.files.len() == 1 { "" } else { "s" },
            c.added(),
            c.removed()
        ),
        None => String::new(),
    }
}

/// The Compare half of the sidebar.
pub(super) fn panel(
    view: &ChangesView,
    theme: &Theme,
    cx: &mut Context<ChangesView>,
) -> AnyElement {
    let current = view.branch.clone();
    let base = view.base.clone().unwrap_or_else(|| "pick a base".into());
    let mut col = div()
        .flex()
        .flex_col()
        .size_full()
        .child(selector(
            "base-branch",
            "base",
            &base,
            theme,
            |this, cx| this.toggle_picker(cx),
            cx,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .h(px(32.))
                .mx(px(12.))
                .mb(px(8.))
                .px(px(10.))
                .flex_none()
                .border_1()
                .border_color(theme.line)
                .rounded(px(6.))
                .bg(theme.bg)
                .child(
                    div()
                        .mr(px(8.))
                        .text_size(px(11.))
                        .text_color(theme.mute)
                        .child("compare"),
                )
                .child(
                    div()
                        .text_color(theme.ink)
                        .child(SharedString::from(current)),
                )
                .child(
                    div()
                        .ml_auto()
                        .px(px(8.))
                        .rounded(px(10.))
                        .bg(theme.sel)
                        .text_size(px(12.))
                        .child("current"),
                ),
        );
    if let Some(error) = view.compare_error.clone() {
        col = col.child(
            div()
                .px(px(14.))
                .pb(px(8.))
                .text_color(theme.del)
                .child(error),
        );
    } else {
        col = col.child(
            div()
                .px(px(14.))
                .pb(px(8.))
                .text_size(px(12.))
                .text_color(theme.mute)
                .child(SharedString::from(file_counts(view))),
        );
    }
    let mut rows = div()
        .id("compare-rows")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0();
    if let Some(compare) = &view.compare {
        rows = rows.overflow_y_scroll();
        for (i, file) in compare.files.iter().enumerate() {
            let rel = file.rel.clone();
            let (name, dir) = split_path(&rel);
            let active = view.compare_selected.as_deref() == Some(rel.as_str());
            let added = file.added();
            let removed = file.removed();
            rows = rows.child(
                div()
                    .id(("cf", i))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .pl(px(14.))
                    .pr(px(10.))
                    .flex_none()
                    .cursor_pointer()
                    .when(active, |d| d.bg(theme.sel))
                    .hover(|d| d.bg(theme.hov))
                    .on_click(cx.listener(move |this, _, _, cx| this.open_compare_file(&rel, cx)))
                    .child(
                        div()
                            .flex_none()
                            .text_color(theme.ink)
                            .child(SharedString::from(name)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_size(px(12.))
                            .text_color(theme.mute)
                            .child(SharedString::from(dir)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .gap(px(6.))
                            .font_family(crate::editor::view::MONO)
                            .text_size(px(12.))
                            .child(
                                div()
                                    .text_color(theme.add)
                                    .child(SharedString::from(format!("+{added}"))),
                            )
                            .child(
                                div()
                                    .text_color(theme.del)
                                    .child(SharedString::from(format!("-{removed}"))),
                            ),
                    ),
            );
        }
    }
    col.child(
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(rows)
            .when(view.picker, |d| d.child(picker(view, theme, cx))),
    )
    .into_any_element()
}

/// The filterable local-branch list.
fn picker(view: &ChangesView, theme: &Theme, cx: &mut Context<ChangesView>) -> AnyElement {
    let filter = view.filter.to_lowercase();
    let current = view.branch.clone();
    let mut list = div()
        .absolute()
        .top_0()
        .left(px(12.))
        .right(px(12.))
        .flex()
        .flex_col()
        .bg(theme.side)
        .border_1()
        .border_color(theme.line)
        .rounded(px(8.))
        .overflow_hidden();
    list = list.child(
        div()
            .id("branch-filter")
            .track_focus(&view.filter_focus)
            .on_key_down(cx.listener(ChangesView::filter_key))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, _cx| window.focus(&this.filter_focus)),
            )
            .h(px(32.))
            .px(px(10.))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(theme.line)
            .text_color(if view.filter.is_empty() {
                theme.mute
            } else {
                theme.ink
            })
            .child(SharedString::from(if view.filter.is_empty() {
                "Filter branches".to_string()
            } else {
                view.filter.clone()
            })),
    );
    for (i, branch) in view.branches.iter().enumerate() {
        if !filter.is_empty() && !branch.to_lowercase().contains(&filter) {
            continue;
        }
        let name = branch.clone();
        let is_current = *branch == current;
        list = list.child(
            div()
                .id(("branch", i))
                .flex()
                .items_center()
                .h(px(28.))
                .px(px(11.))
                .cursor_pointer()
                .hover(|d| d.bg(theme.hov))
                .on_click(cx.listener(move |this, _, _, cx| this.set_base(&name, cx)))
                .child(SharedString::from(branch.clone()))
                .child(
                    div()
                        .ml_auto()
                        .text_size(px(12.))
                        .text_color(theme.mute)
                        .child(SharedString::from(if is_current { "current" } else { "" })),
                ),
        );
    }
    list.into_any_element()
}
