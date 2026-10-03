//! Settings > Keyboard shortcuts: the list, click-to-record, conflicts, reset (view).
use super::settings_shortcuts::Capture;
use super::Workspace;
use crate::keymap::{self, Shortcut};
use crate::theme::{on_accent, Theme};
use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Stateful};

fn button(
    id: impl Into<gpui::ElementId>,
    label: &str,
    primary: bool,
    t: &Theme,
) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(12.))
        .py(px(4.))
        .rounded(px(5.))
        .cursor_pointer()
        .border_1()
        .border_color(t.line)
        .when(primary, |d| {
            d.bg(t.acc).text_color(on_accent()).border_color(t.acc)
        })
        .when(!primary, |d| d.hover(|d| d.bg(t.hov)))
        .child(SharedString::from(label.to_string()))
}

fn key_cell(keys: &str, t: &Theme) -> impl IntoElement {
    let unassigned = keys.is_empty();
    div()
        .px(px(7.))
        .py(px(1.))
        .rounded(px(4.))
        .border_1()
        .border_color(t.line)
        .bg(t.bg)
        .font_family("Cascadia Mono")
        .text_size(px(12.))
        .text_color(if unassigned { t.mute } else { t.ink })
        .child(SharedString::from(keymap::display(keys)))
}

impl Workspace {
    pub(super) fn shortcuts_section(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let overrides = &self.settings(cx).shortcuts;
        let filter = self.shortcut_filter.to_lowercase();
        let matches = |s: &Shortcut| {
            filter.is_empty()
                || s.label.to_lowercase().contains(&filter)
                || keymap::effective(s.action, overrides)
                    .iter()
                    .any(|k| keymap::display(k).to_lowercase().contains(&filter))
        };

        let mut groups: Vec<(&'static str, Vec<&'static Shortcut>)> = Vec::new();
        for s in keymap::CATALOG {
            if !matches(s) {
                continue;
            }
            match groups.iter_mut().find(|(g, _)| *g == s.group) {
                Some((_, rows)) => rows.push(s),
                None => groups.push((s.group, vec![s])),
            }
        }

        let total: usize = groups.iter().map(|(_, r)| r.len()).sum();
        let mut split = groups.len();
        let mut seen = 0;
        let mut best = usize::MAX;
        for (i, (_, rows)) in groups.iter().enumerate() {
            seen += rows.len();
            // Put the boundary where the two columns come out closest in length.
            let imbalance = (2 * seen).abs_diff(total);
            if imbalance < best {
                best = imbalance;
                split = i + 1;
            }
        }
        let right = groups.split_off(split);

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(div().text_size(px(20.)).child("Keyboard shortcuts"))
            .when(!overrides.is_empty(), |d| {
                d.child(
                    button("sc-reset", "Reset all", false, t)
                        .on_click(cx.listener(|this, _, _, cx| this.reset_shortcuts(cx))),
                )
            });

        let field = div()
            .id("sc-search")
            .w(px(360.))
            .h(px(34.))
            .mt(px(16.))
            .px(px(12.))
            .flex()
            .items_center()
            .rounded(px(6.))
            .border_1()
            .border_color(if matches!(self.capture, Some(Capture::Filter)) {
                t.acc
            } else {
                t.line
            })
            .bg(t.bg)
            .cursor_text()
            .font_family("Segoe UI")
            .text_color(if self.shortcut_filter.is_empty() {
                t.mute
            } else {
                t.ink
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.capture = Some(Capture::Filter);
                cx.notify();
            }))
            .child(if self.shortcut_filter.is_empty() {
                SharedString::from("Search shortcuts")
            } else {
                SharedString::from(self.shortcut_filter.clone())
            });

        let mut columns = div().flex().gap(px(44.)).mt(px(6.));
        if total == 0 {
            columns = columns.child(
                div()
                    .px(px(12.))
                    .pt(px(20.))
                    .text_color(t.mute)
                    .child("No shortcuts match."),
            );
        }
        for groups in [groups, right] {
            let mut col = div().flex_1().min_w_0().flex().flex_col().pb(px(24.));
            for (name, rows) in groups {
                col = col.child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.mute)
                        .px(px(12.))
                        .pt(px(14.))
                        .pb(px(4.))
                        .child(name.to_uppercase()),
                );
                for s in rows {
                    col = col.child(self.shortcut_row(s, t, cx));
                }
            }
            columns = columns.child(col);
        }

        div()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(t.mute)
                    .mt(px(4.))
                    .child("Defaults follow common editor habits. Stored on this machine."),
            )
            .child(field)
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(t.mute)
                    .mt(px(6.))
                    .child("Click a shortcut to change it. Esc cancels, Backspace clears."),
            )
            .child(columns)
            .into_any_element()
    }

    fn shortcut_row(&self, s: &'static Shortcut, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let recording = matches!(self.capture, Some(Capture::Record(a)) if a == s.action);
        let conflict = match &self.capture {
            Some(Capture::Conflict {
                action,
                keys,
                owner,
            }) if *action == s.action => Some((keys.clone(), *owner)),
            _ => None,
        };
        let keys = keymap::effective(s.action, &self.settings(cx).shortcuts);
        let current = keys.first().cloned().unwrap_or_default();
        let editing = recording || conflict.is_some();

        let mut key_area = div().flex().items_center().gap(px(6.));
        if let Some(ctx) = s.context {
            key_area = key_area.child(
                div()
                    .text_size(px(11.))
                    .text_color(t.mute)
                    .child(SharedString::from(ctx.to_string())),
            );
        }
        key_area = key_area.child(if recording {
            div()
                .px(px(10.))
                .py(px(2.))
                .rounded(px(4.))
                .border_1()
                .border_color(t.acc)
                .bg(t.bg)
                .text_color(t.mute)
                .child("Press the new shortcut…")
                .into_any_element()
        } else if let Some((keys, _)) = &conflict {
            key_cell(keys, t).into_any_element()
        } else {
            key_cell(&current, t).into_any_element()
        });

        let row = div()
            .id(s.action)
            .flex()
            .items_center()
            .h(px(27.))
            .px(px(12.))
            .rounded(px(6.))
            .when(editing, |d| d.bg(t.sel).border_1().border_color(t.acc))
            .when(!editing, |d| {
                d.cursor_pointer()
                    .hover(|d| d.bg(t.hov))
                    .on_click(cx.listener({
                        let action = s.action;
                        move |this, _, _, cx| {
                            this.capture = Some(Capture::Record(action));
                            cx.notify();
                        }
                    }))
            })
            .child(div().flex_1().min_w_0().child(SharedString::from(s.label)))
            .child(key_area);

        let Some((keys, owner)) = conflict else {
            return row.into_any_element();
        };
        let owner_label = keymap::CATALOG
            .iter()
            .find(|c| c.action == owner)
            .map(|c| c.label)
            .unwrap_or(owner);
        let message = format!(
            "{} is used by {owner_label}. Assigning it here removes it there.",
            keymap::display(&keys)
        );
        div()
            .flex()
            .flex_col()
            .child(row)
            .child(
                div()
                    .mx(px(12.))
                    .mt(px(4.))
                    .p(px(10.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(t.line)
                    .bg(t.bg)
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_color(t.ink).child(SharedString::from(message)))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(button("sc-cancel", "Cancel", false, t).on_click(
                                cx.listener(|this, _, _, cx| this.resolve_conflict(false, cx)),
                            ))
                            .child(button("sc-assign", "Assign", true, t).on_click(
                                cx.listener(|this, _, _, cx| this.resolve_conflict(true, cx)),
                            )),
                    ),
            )
            .into_any_element()
    }
}
