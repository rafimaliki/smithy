//! Rendering for the Source control sidebar: the Changes panel, the empty and
//! not-a-repo states, and the discard confirmation dialog.
use super::changes::{ChangesView, Mode};
use super::compare;
use crate::git::{ChangeEntry, Section};
use crate::theme::{on_accent, Theme};
use gpui::{
    div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent, Render, Rgba,
    SharedString, Stateful, Window,
};

fn tint(c: Rgba, a: f32) -> Rgba {
    Rgba { a, ..c }
}

fn status_color(status: char, theme: &Theme) -> Rgba {
    match status {
        'A' | 'U' => theme.add,
        'D' => theme.del,
        _ => theme.acc,
    }
}

fn small_button(
    id: &'static str,
    label: &str,
    theme: &Theme,
    primary: bool,
) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(28.))
        .px(px(12.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .border_1()
        .border_color(theme.line)
        .cursor_pointer()
        .when(primary, |d| {
            d.bg(theme.acc)
                .border_color(theme.acc)
                .text_color(on_accent())
        })
        .when(!primary, |d| {
            d.text_color(theme.ink).hover(|d| d.bg(theme.hov))
        })
        .child(SharedString::from(label.to_string()))
}

fn section_header(theme: &Theme, label: &str, count: usize) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(26.))
        .px(px(10.))
        .flex_none()
        .text_size(px(12.))
        .child(
            div()
                .text_color(theme.ink)
                .child(SharedString::from(label.to_string())),
        )
        .child(
            div()
                .ml_auto()
                .px(px(7.))
                .rounded(px(9.))
                .bg(theme.sel)
                .text_color(theme.mute)
                .child(SharedString::from(count.to_string())),
        )
        .into_any_element()
}

fn entry_row(
    view: &ChangesView,
    theme: &Theme,
    cx: &mut Context<ChangesView>,
    index: usize,
    entry: &ChangeEntry,
) -> AnyElement {
    let active = view
        .selected
        .as_ref()
        .is_some_and(|(rel, section)| *rel == entry.rel && *section == entry.section);
    let group: SharedString = format!("row-{index}").into();
    let rel = entry.rel.clone();
    let row_rel = rel.clone();
    let section = entry.section;
    let mut row = div()
        .id(("entry", index))
        .group(group.clone())
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .pl(px(10.))
        .pr(px(10.))
        .flex_none()
        .cursor_pointer()
        .when(active, |d| d.bg(theme.sel))
        .hover(|d| d.bg(theme.hov))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, _, cx| this.select(&row_rel, section, cx)),
        )
        .child(
            div()
                .flex_none()
                .text_color(theme.ink)
                .child(SharedString::from(entry.name.clone())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_size(px(12.))
                .text_color(theme.mute)
                .child(SharedString::from(entry.dir.clone())),
        );
    let action = |label: &'static str, id: (&'static str, usize), run: Action| {
        let rel = rel.clone();
        div()
            .id(id)
            .w(px(18.))
            .h(px(18.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .invisible()
            .group_hover(group.clone(), |d| d.visible())
            .text_color(theme.mute)
            .hover(|d| d.bg(theme.hov).text_color(theme.ink))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    match run {
                        Action::Stage => this.stage_file(&rel, cx),
                        Action::Unstage => this.unstage_file(&rel, cx),
                        Action::Discard => this.request_discard_file(&rel, section, cx),
                    }
                }),
            )
            .child(label)
    };
    row = row.child(
        div()
            .flex()
            .flex_none()
            .gap(px(2.))
            .child(match entry.section {
                Section::Unstaged => action("+", ("stage", index), Action::Stage),
                Section::Staged => action("−", ("unstage", index), Action::Unstage),
            }),
    );
    if entry.section == Section::Unstaged {
        row = row.child(action("↺", ("discard", index), Action::Discard));
    }
    row.child(
        div()
            .w(px(16.))
            .flex_none()
            .text_center()
            .text_size(px(11.))
            .text_color(status_color(entry.status, theme))
            .child(SharedString::from(entry.status.to_string())),
    )
    .into_any_element()
}

#[derive(Clone, Copy)]
enum Action {
    Stage,
    Unstage,
    Discard,
}

impl ChangesView {
    fn tabs(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut seg = div()
            .flex()
            .flex_none()
            .mx(px(12.))
            .mb(px(10.))
            .border_1()
            .border_color(theme.line)
            .rounded(px(6.))
            .overflow_hidden();
        for (i, (label, mode)) in [("Changes", Mode::Changes), ("Compare", Mode::Compare)]
            .into_iter()
            .enumerate()
        {
            seg = seg.child(
                div()
                    .id(("seg", i))
                    .flex_1()
                    .py(px(4.))
                    .text_center()
                    .cursor_pointer()
                    .when(self.mode == mode, |d| d.bg(theme.sel).text_color(theme.ink))
                    .when(self.mode != mode, |d| d.text_color(theme.mute))
                    .child(SharedString::from(label))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_mode(mode, cx))),
            );
        }
        seg.into_any_element()
    }

    fn no_repo(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div()
            .px(px(14.))
            .pt(px(16.))
            .flex()
            .flex_col()
            .text_color(theme.mute)
            .child(
                div()
                    .mb(px(6.))
                    .text_color(theme.ink)
                    .child("Not a git repository"),
            )
            .child("Search and the file tree work here. Source control and pull requests need a folder with a git repository.");
        if let Some(ws) = self.workspace.clone() {
            col = col.child(div().mt(px(16.)).child(
                small_button("open-other", "Open another folder", theme, false).on_click(
                    cx.listener(move |_, _, window, cx| {
                        ws.update(cx, |w, cx| w.prompt_open_folder(window, cx)).ok();
                    }),
                ),
            ));
        }
        col.into_any_element()
    }

    fn empty_state(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(40.))
            .px(px(20.))
            .text_center()
            .text_color(theme.mute)
            .child(
                div()
                    .mb(px(12.))
                    .w(px(34.))
                    .h(px(34.))
                    .rounded_full()
                    .bg(tint(theme.add, 0.18))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme.add)
                    .child("✓"),
            )
            .child(div().text_color(theme.ink).child("No changes"))
            .child(SharedString::from(format!(
                "Working tree is clean on {}.",
                self.branch
            )))
            .child(
                div().mt(px(14.)).child(
                    small_button("compare-now", "Compare with a branch", theme, false)
                        .on_click(cx.listener(|this, _, _, cx| this.set_mode(Mode::Compare, cx))),
                ),
            )
            .into_any_element()
    }

    fn changes_panel(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div().flex().flex_col().flex_1().min_h_0();
        col = col
            .child(
                div()
                    .id("commit-message")
                    .track_focus(&self.message_focus)
                    .on_key_down(cx.listener(Self::message_key))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, _cx| window.focus(&this.message_focus)),
                    )
                    .mx(px(12.))
                    .mb(px(8.))
                    .h(px(64.))
                    .p(px(8.))
                    .flex_none()
                    .border_1()
                    .border_color(theme.line)
                    .rounded(px(6.))
                    .bg(theme.bg)
                    .text_color(if self.message.is_empty() {
                        theme.mute
                    } else {
                        theme.ink
                    })
                    .child(SharedString::from(if self.message.is_empty() {
                        "Message".to_string()
                    } else {
                        self.message.clone()
                    })),
            )
            .child(
                div().mx(px(12.)).mb(px(8.)).child(
                    small_button(
                        "commit",
                        &match self.staged.len() {
                            0 => "Commit".to_string(),
                            n => format!("Commit {n} staged"),
                        },
                        theme,
                        self.can_commit(),
                    )
                    .w_full()
                    .when(self.can_commit(), |d| {
                        d.on_click(cx.listener(|this, _, _, cx| this.commit(cx)))
                    }),
                ),
            );
        if let Some(error) = self.error.clone() {
            col = col.child(
                div()
                    .px(px(12.))
                    .pb(px(8.))
                    .text_size(px(12.))
                    .text_color(theme.del)
                    .child(error),
            );
        }
        if self.change_count() == 0 {
            return col.child(self.empty_state(theme, cx)).into_any_element();
        }
        let mut rows = div()
            .id("change-rows")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        if !self.staged.is_empty() {
            rows = rows.child(section_header(
                theme,
                Section::Staged.label(),
                self.staged.len(),
            ));
            for (i, entry) in self.staged.iter().enumerate() {
                rows = rows.child(entry_row(self, theme, cx, i, entry));
            }
        }
        if !self.unstaged.is_empty() {
            let base = self.staged.len();
            rows = rows.child(section_header(
                theme,
                Section::Unstaged.label(),
                self.unstaged.len(),
            ));
            for (i, entry) in self.unstaged.iter().enumerate() {
                rows = rows.child(entry_row(self, theme, cx, base + i, entry));
            }
        }
        col.child(rows).into_any_element()
    }
}

impl Render for ChangesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::by_name(&cx.global::<crate::settings::Settings>().theme);
        let mut panel = div()
            .flex()
            .flex_col()
            .size_full()
            .pt(px(2.))
            .text_color(theme.ink)
            .text_size(px(13.))
            .font_family("Segoe UI")
            .child(self.tabs(&theme, cx));
        if self.repo.is_none() {
            panel = panel.child(self.no_repo(&theme, cx));
        } else {
            panel = match self.mode {
                Mode::Changes => panel.child(self.changes_panel(&theme, cx)),
                Mode::Compare => panel.child(compare::panel(self, &theme, cx)),
            };
        }
        panel
    }
}
