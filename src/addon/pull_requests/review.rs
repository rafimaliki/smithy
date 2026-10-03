//! Reviewing a selected pull request: fetch the PR ref in the background, build
//! the local diff, and open it read-only in the Source control diff view.
use super::super::source_control::diff_view::{DiffOptions, DiffView};
use super::sidebar::{chip, small_button};
use super::state::{PrView, Review};
use crate::addon::StatusInfo;
use crate::git::{diff_layout_from_setting, Repo};
use crate::settings::Settings;
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};

impl PrView {
    /// Fetch the PR ref and diff it locally, then open the first file's diff.
    pub fn review(&mut self, cx: &mut Context<Self>) {
        let (Some(index), Some(root)) = (self.selected, self.root.clone()) else {
            return;
        };
        let Some(pr) = self.items.get(index).cloned() else {
            return;
        };
        let (number, base) = (pr.number, pr.base_ref.clone());
        self.review_loading = true;
        self.review_error = None;
        self.review_task = Some(cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_executor()
                .spawn(async move {
                    let repo =
                        Repo::discover(&root).ok_or_else(|| "No git repository".to_string())?;
                    repo.pull_request(number, &base)
                        .map_err(|e| format!("git: {e}"))
                })
                .await;
            this.update(cx, |this, cx| this.review_loaded(number, fetched, cx))
                .ok();
        }));
        cx.notify();
    }

    fn review_loaded(
        &mut self,
        number: u64,
        fetched: Result<crate::git::Compare, String>,
        cx: &mut Context<Self>,
    ) {
        self.review_loading = false;
        match fetched {
            Ok(compare) => {
                let pr = self.items.iter().find(|pr| pr.number == number);
                self.review = Some(Review {
                    number,
                    title: pr.map(|p| p.title.clone()).unwrap_or_default(),
                    state: pr.map(|p| p.state_label().to_string()).unwrap_or_default(),
                    head_ref: pr.map(|p| p.head_ref.clone()).unwrap_or_default(),
                    base_ref: pr.map(|p| p.base_ref.clone()).unwrap_or_default(),
                    files: compare.files,
                    selected: None,
                });
                if let Some(rel) = self
                    .review
                    .as_ref()
                    .and_then(|r| r.files.first().map(|f| f.rel.clone()))
                {
                    self.open_review_file(&rel, cx);
                }
            }
            Err(e) => self.review_error = Some(e),
        }
        cx.notify();
    }

    /// Open (or focus) one file of the fetched review in the diff view, read-only.
    pub fn open_review_file(&mut self, rel: &str, cx: &mut Context<Self>) {
        let Some(review) = self.review.as_mut() else {
            return;
        };
        review.selected = Some(rel.to_string());
        let Some(file) = review.files.iter().find(|f| f.rel == rel).cloned() else {
            return;
        };
        let number = review.number;
        let layout = diff_layout_from_setting(&cx.global::<Settings>().diff_layout);
        let options = DiffOptions {
            chip: None,
            hunk_actions: None,
            open_file: false,
        };
        let view = cx.new(|_| DiffView::new(vec![file], layout, options, None));
        let name = crate::git::split_path(rel).0;
        self.open_tab(
            &format!("pr/{number}/{rel}"),
            format!("{name} · PR #{number}"),
            view.into(),
            cx,
        );
        cx.notify();
    }

    /// Back to the list from the review sidebar.
    pub fn back_to_list(&mut self, cx: &mut Context<Self>) {
        self.review = None;
        self.review_error = None;
        cx.notify();
    }

    /// "read-only · pr/42" in the status bar while a review diff is open.
    pub fn status_info(&self) -> Option<StatusInfo> {
        let review = self.review.as_ref()?;
        Some(StatusInfo {
            branch: "read-only".into(),
            detail: crate::git::pull::ref_label(review.number),
        })
    }

    /// The sidebar while a review is loaded: the PR, its refs and its files.
    pub(super) fn review_panel(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let Some(review) = self.review.as_ref() else {
            return div().into_any_element();
        };
        let state = review.state.clone();
        let mut rows = div()
            .id("pr-files")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for (i, file) in review.files.iter().enumerate() {
            let active = review.selected.as_deref() == Some(file.rel.as_str());
            rows = rows.child(file_row(t, cx, i, file, active));
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .px(px(14.))
                    .pt(px(4.))
                    .text_size(px(11.))
                    .text_color(t.mute)
                    .child(SharedString::from(format!("PR #{}", review.number))),
            )
            .child(
                div()
                    .px(px(14.))
                    .pt(px(4.))
                    .child(SharedString::from(review.title.clone())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(14.))
                    .pt(px(6.))
                    .text_size(px(12.))
                    .text_color(t.mute)
                    .child(chip(t, &state, state == "Open"))
                    .child("fetched just now"),
            )
            .child(
                div()
                    .px(px(14.))
                    .pb(px(10.))
                    .text_size(px(12.))
                    .text_color(t.mute)
                    .child(SharedString::from(format!(
                        "{} into {}",
                        review.head_ref, review.base_ref
                    ))),
            )
            .child(rows)
            .child(
                div().p(px(14.)).child(
                    small_button("back-prs", "All pull requests", t, false)
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| this.back_to_list(cx))),
                ),
            )
            .into_any_element()
    }
}

/// One changed file of the review, with its line counts.
fn file_row(
    t: &Theme,
    cx: &mut Context<PrView>,
    index: usize,
    file: &crate::git::FileDiff,
    active: bool,
) -> AnyElement {
    let rel = file.rel.clone();
    div()
        .id(("pr-file", index))
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .pl(px(14.))
        .pr(px(10.))
        .flex_none()
        .cursor_pointer()
        .when(active, |d| d.bg(t.sel))
        .hover(|d| d.bg(t.hov))
        .on_click(cx.listener(move |this, _, _, cx| this.open_review_file(&rel, cx)))
        .child(
            div()
                .flex_none()
                .text_color(t.ink)
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
                        .child(SharedString::from(format!("+{}", file.added()))),
                )
                .child(
                    div()
                        .text_color(t.del)
                        .child(SharedString::from(format!("-{}", file.removed()))),
                ),
        )
        .into_any_element()
}
