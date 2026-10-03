//! Opening and refreshing the diff tabs the Changes view owns.
use super::changes::{ChangesView, OpenView, ViewKind};
use super::diff_view::{ActionHandler, DiffAction, DiffOptions, DiffView, HunkActions};
use crate::git::{abs_path, split_path, FileDiff, Section};
use gpui::{AppContext as _, Context, SharedString, WeakEntity};

impl ChangesView {
    // ---- diff tabs ----------------------------------------------------------

    fn data_for(&self, kind: &ViewKind) -> Vec<FileDiff> {
        match kind {
            ViewKind::File { rel, section } => self
                .repo
                .as_ref()
                .and_then(|r| r.file_diff(rel, *section).ok())
                .filter(|f| f.binary || !f.hunks.is_empty())
                .map(|f| vec![f])
                .unwrap_or_default(),
            ViewKind::Compare { rel } => self
                .compare
                .as_ref()
                .and_then(|c| c.files.iter().find(|f| &f.rel == rel).cloned())
                .map(|f| vec![f])
                .unwrap_or_default(),
        }
    }

    pub fn select(&mut self, rel: &str, section: Section, cx: &mut Context<Self>) {
        self.selected = Some((rel.to_string(), section));
        self.open_file(rel, section, cx);
        cx.notify();
    }

    pub fn open_compare_file(&mut self, rel: &str, cx: &mut Context<Self>) {
        self.compare_selected = Some(rel.to_string());
        let kind = ViewKind::Compare {
            rel: rel.to_string(),
        };
        let files = self.data_for(&kind);
        let (name, _) = split_path(rel);
        let base = self
            .compare
            .as_ref()
            .map(|c| c.base.clone())
            .or_else(|| self.base.clone())
            .unwrap_or_default();
        let chip: SharedString = format!("{}...{}", base, self.branch).into();
        let options = DiffOptions {
            chip: Some(chip),
            hunk_actions: None,
            open_file: false,
        };
        let key = format!("compare/{}/{}", base, rel);
        self.push_view(key, name, kind, files, options, cx);
        cx.notify();
    }

    pub(super) fn open_file(&mut self, rel: &str, section: Section, cx: &mut Context<Self>) {
        let kind = ViewKind::File {
            rel: rel.to_string(),
            section,
        };
        let files = self.data_for(&kind);
        let (name, _) = split_path(rel);
        let key = format!(
            "changes/{}/{}",
            match section {
                Section::Staged => "staged",
                Section::Unstaged => "unstaged",
            },
            rel
        );
        let options = DiffOptions {
            chip: Some(match section {
                Section::Staged => "staged".into(),
                Section::Unstaged => "changes".into(),
            }),
            hunk_actions: Some(match section {
                Section::Staged => HunkActions::Staged,
                Section::Unstaged => HunkActions::Unstaged,
            }),
            open_file: true,
        };
        self.push_view(key, name, kind, files, options, cx);
    }

    fn push_view(
        &mut self,
        key: String,
        name: String,
        kind: ViewKind,
        files: Vec<FileDiff>,
        options: DiffOptions,
        cx: &mut Context<Self>,
    ) {
        let title = match &options.chip {
            Some(chip) => format!("{name} · {chip}"),
            None => name,
        };
        let changes = cx.entity();
        let workspace = self.workspace.clone();
        let root = self.root.clone();
        let handler: ActionHandler = Box::new(move |request, window, cx| match request {
            DiffAction::StageHunk { rel, hunk } => {
                changes.update(cx, |c, cx| c.stage_hunk(&rel, hunk, cx));
            }
            DiffAction::UnstageHunk { rel, hunk } => {
                changes.update(cx, |c, cx| c.unstage_hunk(&rel, hunk, cx));
            }
            DiffAction::DiscardHunk { rel, hunk } => {
                changes.update(cx, |c, cx| c.request_discard_hunk(&rel, hunk, cx));
            }
            DiffAction::OpenFile { rel } => {
                if let (Some(ws), Some(root)) = (workspace.clone(), root.clone()) {
                    let path = abs_path(&root, &rel);
                    ws.update(cx, |w, cx| w.open_file(&path, window, cx)).ok();
                }
            }
        });
        let layout = self.layout;
        let view = cx.new(|_| DiffView::new(files, layout, options, Some(handler)));
        self.open.push(OpenView {
            kind,
            view: view.downgrade(),
        });
        if let Some(ws) = self.workspace.clone() {
            ws.update(cx, |w, cx| w.open_addon_tab(&key, title, view.into(), cx))
                .ok();
        }
    }

    /// Recompute the data of every live diff tab and push the discard prompt.
    /// Deferred: this runs while a diff tab may itself be mid-update, and a
    /// re-entrant entity update would panic.
    pub(super) fn refresh_open(&mut self, cx: &mut Context<Self>) {
        self.open.retain(|o| o.view.upgrade().is_some());
        let overlay = self.discard_overlay(cx);
        let payload: Vec<(WeakEntity<DiffView>, Vec<FileDiff>)> = self
            .open
            .iter()
            .map(|o| (o.view.clone(), self.data_for(&o.kind)))
            .collect();
        cx.defer(move |cx| {
            for (view, files) in payload {
                let _ = view.update(cx, |v, cx| {
                    v.set_files(files);
                    v.set_overlay(overlay.clone());
                    cx.notify();
                });
            }
        });
    }

    /// The confirmation dialog, drawn by whichever diff tab is open.
    fn discard_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyView> {
        let pending = self.pending.clone()?;
        let (name, _) = split_path(&pending.rel);
        let detail = match pending.hunk {
            Some(_) => {
                "This hunk will go back to the last commit. This cannot be undone.".to_string()
            }
            None => format!(
                "{} hunk{} will go back to the last commit. This cannot be undone.",
                pending.hunks,
                if pending.hunks == 1 { "" } else { "s" }
            ),
        };
        let changes = cx.entity();
        Some(
            cx.new(|_| super::dialog::DiscardDialog {
                changes,
                name,
                detail,
            })
            .into(),
        )
    }
}
