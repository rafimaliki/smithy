//! The Pull requests sidebar panels: the filtered list and the reviewed pull
//! request's file list. Split from `sidebar.rs` along the state seam.
use super::sidebar::{chip, dot_color, notice, small_button};
use super::state::{Load, PrView, StateFilter};
use crate::github::{PullRequest, RepoSlug};
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};

fn filter_row(view: &PrView, t: &Theme, cx: &mut Context<PrView>) -> AnyElement {
    let mut seg = div()
        .flex()
        .flex_none()
        .border_1()
        .border_color(t.line)
        .rounded(px(6.))
        .overflow_hidden();
    for (i, (label, filter)) in [("Open", StateFilter::Open), ("Closed", StateFilter::Closed)]
        .into_iter()
        .enumerate()
    {
        let on = view.state_filter == filter;
        seg = seg.child(
            div()
                .id(("pr-state", i))
                .px(px(12.))
                .py(px(4.))
                .cursor_pointer()
                .when(on, |d| d.bg(t.sel).text_color(t.ink))
                .when(!on, |d| d.text_color(t.mute))
                .on_click(cx.listener(move |this, _, _, cx| this.set_state_filter(filter, cx)))
                .child(SharedString::from(label)),
        );
    }
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .mx(px(12.))
        .mb(px(8.))
        .child(seg)
        .child(
            div()
                .id("pr-mine")
                .px(px(12.))
                .py(px(4.))
                .border_1()
                .border_color(if view.mine { t.acc } else { t.line })
                .rounded(px(6.))
                .cursor_pointer()
                .when(view.mine, |d| d.bg(t.sel).text_color(t.ink))
                .when(!view.mine, |d| d.text_color(t.mute).hover(|d| d.bg(t.hov)))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_mine(cx)))
                .child("Mine"),
        )
        .into_any_element()
}

fn repo_row(view: &PrView, t: &Theme, cx: &mut Context<PrView>) -> AnyElement {
    let slug = view
        .slug
        .as_ref()
        .map(RepoSlug::full_name)
        .unwrap_or_default();
    div()
        .flex()
        .items_center()
        .px(px(14.))
        .pb(px(8.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_size(px(12.))
                .text_color(t.mute)
                .child(SharedString::from(slug)),
        )
        .child(
            div()
                .id("pr-refresh")
                .w(px(20.))
                .h(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .cursor_pointer()
                .text_color(t.mute)
                .hover(|d| d.bg(t.hov).text_color(t.ink))
                .on_click(cx.listener(|this, _, _, cx| this.reload(cx)))
                .child("↻"),
        )
        .into_any_element()
}

fn pr_row(view: &PrView, t: &Theme, cx: &mut Context<PrView>, index: usize) -> AnyElement {
    let pr: &PullRequest = &view.items[index];
    let dot = dot_color(view.checks_dot(pr), t);
    let mut meta = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .pl(px(16.))
        .mt(px(3.))
        .text_size(px(12.))
        .text_color(t.mute)
        .child(SharedString::from(format!("#{}", pr.number)))
        .child(SharedString::from(pr.author.clone()))
        .child(SharedString::from(pr.age.clone()));
    if pr.draft {
        meta = meta.child(chip(t, "draft", false));
    }
    div()
        .id(("pr", index))
        .flex()
        .flex_col()
        .px(px(14.))
        .py(px(9.))
        .flex_none()
        .border_b_1()
        .border_color(t.line)
        .cursor_pointer()
        .when(view.selected == Some(index), |d| d.bg(t.sel))
        .hover(|d| d.bg(t.hov))
        .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().w(px(8.)).h(px(8.)).flex_none().rounded_full().bg(dot))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(SharedString::from(pr.title.clone())),
                ),
        )
        .child(meta)
        .into_any_element()
}

impl PrView {
    pub(super) fn list_panel(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div().flex().flex_col().flex_1().min_h_0();
        if let Some(err) = self.review_error.clone() {
            let retry = small_button("retry-review", "Retry", t, false)
                .on_click(cx.listener(|this, _, _, cx| this.review(cx)))
                .into_any_element();
            col = col.child(notice(
                t,
                "Could not fetch the pull request",
                &err,
                Some(retry),
            ));
        }
        if self.load == Load::Loading {
            col = col.child(
                div()
                    .px(px(14.))
                    .py(px(10.))
                    .text_size(px(12.))
                    .text_color(t.mute)
                    .child("Loading pull requests…"),
            );
        }
        col = col
            .child(filter_row(self, t, cx))
            .child(repo_row(self, t, cx));
        if self.load == Load::Ready && self.visible().is_empty() {
            col = col.child(
                div()
                    .px(px(14.))
                    .text_size(px(12.))
                    .text_color(t.mute)
                    .child("No pull requests to show."),
            );
        }
        let mut rows = div()
            .id("pr-rows")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        for index in self.visible() {
            rows = rows.child(pr_row(self, t, cx, index));
        }
        col.child(rows).into_any_element()
    }
}
