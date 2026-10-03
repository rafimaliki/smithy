//! The pull request detail tab: state, branches, checks, description and the
//! changed-file list, with Review changes and Open on GitHub.
use super::sidebar::{chip, small_button, tint};
use super::state::PrView;
use crate::github::{CheckState, Checks, FileStat};
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, AnyElement, Context, Entity, Render, SharedString, Subscription, Window,
};

pub struct PrDetailView {
    panel: Entity<PrView>,
    number: u64,
    _sub: Subscription,
}

impl PrDetailView {
    pub fn new(panel: Entity<PrView>, number: u64, cx: &mut Context<Self>) -> Self {
        let sub = cx.observe(&panel, |_, _, cx| cx.notify());
        Self {
            panel,
            number,
            _sub: sub,
        }
    }
}

fn checks_label(checks: &Checks) -> String {
    match checks.state {
        CheckState::Success => format!("{}/{} checks passing", checks.passed, checks.total),
        CheckState::Failure => format!(
            "{} of {} checks failed",
            checks.total - checks.passed,
            checks.total
        ),
        CheckState::Pending => "Checks running".to_string(),
        CheckState::Neutral => "No checks".to_string(),
    }
}

fn checks_chip(t: &Theme, checks: &Checks) -> AnyElement {
    let (bg, fg) = match checks.state {
        CheckState::Success => (tint(t.add, 0.22), t.add),
        CheckState::Failure => (tint(t.del, 0.22), t.del),
        _ => (t.sel, t.mute),
    };
    div()
        .px(px(8.))
        .h(px(20.))
        .flex()
        .items_center()
        .flex_none()
        .rounded(px(10.))
        .text_size(px(12.))
        .bg(bg)
        .text_color(fg)
        .child(SharedString::from(checks_label(checks)))
        .into_any_element()
}

/// The extension in tiny letters, the board's simple-icon style.
fn file_badge(t: &Theme, name: &str) -> AnyElement {
    let ext: String = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_uppercase())
        .unwrap_or_default()
        .chars()
        .take(3)
        .collect();
    div()
        .w(px(20.))
        .flex_none()
        .text_center()
        .font_family(crate::editor::view::MONO)
        .text_size(px(10.))
        .text_color(t.mute)
        .child(SharedString::from(ext))
        .into_any_element()
}

fn file_row(t: &Theme, index: usize, file: &FileStat) -> AnyElement {
    div()
        .id(("pr-detail-file", index))
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(14.))
        .flex_none()
        .border_b_1()
        .border_color(t.line)
        .child(file_badge(t, &file.name))
        .child(
            div()
                .flex_none()
                .child(SharedString::from(file.name.clone())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_size(px(12.))
                .text_color(t.mute)
                .child(SharedString::from(file.dir.clone())),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .gap(px(6.))
                .font_family(crate::editor::view::MONO)
                .text_size(px(12.))
                .child(
                    div()
                        .text_color(t.add)
                        .child(SharedString::from(format!("+{}", file.added))),
                )
                .child(
                    div()
                        .text_color(t.del)
                        .child(SharedString::from(format!("-{}", file.removed))),
                ),
        )
        .into_any_element()
}

impl Render for PrDetailView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::by_name(&cx.global::<Settings>().theme);
        let (pr, loading) = {
            let panel = self.panel.read(cx);
            (
                panel
                    .items
                    .iter()
                    .find(|pr| pr.number == self.number)
                    .cloned(),
                panel.review_loading,
            )
        };
        let root = div().size_full().flex().flex_col().bg(t.bg);
        let Some(pr) = pr else {
            return root.child(
                div()
                    .p(px(36.))
                    .text_color(t.mute)
                    .child("This pull request is not in the list any more."),
            );
        };

        let label = if loading {
            "Fetching…"
        } else {
            "Review changes"
        };
        let review_panel = self.panel.clone();
        let review_button = small_button("review-pr", label, &t, !loading).on_click(cx.listener(
            move |_, _, _, cx| {
                review_panel.update(cx, |panel, cx| {
                    if !panel.review_loading {
                        panel.review(cx);
                    }
                });
            },
        ));
        let url = pr.html_url.clone();
        let github_button = small_button("open-gh", "Open on GitHub", &t, false)
            .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url)));

        let mut chips = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .mb(px(20.))
            .child(chip(&t, pr.state_label(), pr.state_label() == "Open"))
            .child(chip(&t, &pr.head_ref, false))
            .child(div().text_size(px(12.)).text_color(t.mute).child("into"))
            .child(chip(&t, &pr.base_ref, false));
        if let Some(checks) = &pr.checks {
            chips = chips.child(div().ml_auto().child(checks_chip(&t, checks)));
        }

        let mut body = div().w(px(760.)).text_color(t.ink).mb(px(22.));
        if pr.body.trim().is_empty() {
            body = body.child(div().text_color(t.mute).child("No description."));
        } else {
            for line in pr.body.lines() {
                body = body.child(div().child(SharedString::from(line.to_string())));
            }
        }

        let mut files = div()
            .w(px(760.))
            .border_1()
            .border_color(t.line)
            .rounded(px(8.))
            .overflow_hidden();
        if pr.files.is_empty() {
            files = files.child(
                div()
                    .p(px(14.))
                    .text_color(t.mute)
                    .child(if pr.files_loaded {
                        "No files changed."
                    } else {
                        "Loading files…"
                    }),
            );
        } else {
            for (i, file) in pr.files.iter().enumerate() {
                files = files.child(file_row(&t, i, file));
            }
        }

        root.child(
            div()
                .id("pr-detail")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(36.))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(t.mute)
                        .child(SharedString::from(format!(
                            "#{} · updated {} by {}",
                            pr.number, pr.age, pr.author
                        ))),
                )
                .child(
                    div()
                        .text_size(px(20.))
                        .my(px(6.))
                        .child(SharedString::from(pr.title.clone())),
                )
                .child(chips)
                .child(body)
                .child(
                    div()
                        .flex()
                        .gap(px(10.))
                        .mb(px(26.))
                        .child(review_button)
                        .child(github_button),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .mb(px(8.))
                        .child("Files changed")
                        .child(
                            div()
                                .text_color(t.mute)
                                .child(SharedString::from(pr.changed_files.to_string())),
                        ),
                )
                .child(files),
        )
    }
}
