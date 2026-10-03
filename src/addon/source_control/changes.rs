//! Source control sidebar state and behavior: status lists, commit box, staging,
//! compare and the discard confirmation. Rendering lives in `sidebar.rs`.
use super::diff_view::DiffView;
use crate::addon::{BlameLine, EditorDecorations, LineMark, StatusInfo};
use crate::git::{
    diff_layout_from_setting, rel_path, ChangeEntry, Compare, DiffLayout, Repo, Section,
};
use crate::settings::Settings;
use crate::workspace::Workspace;
use gpui::{Context, FocusHandle, KeyDownEvent, SharedString, WeakEntity, Window};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Changes,
    Compare,
}

#[derive(Clone)]
pub enum ViewKind {
    File { rel: String, section: Section },
    Compare { rel: String },
}

pub(super) struct OpenView {
    pub(super) kind: ViewKind,
    pub(super) view: WeakEntity<DiffView>,
}

/// A discard that is waiting on the confirmation dialog.
#[derive(Clone)]
pub struct PendingDiscard {
    pub rel: String,
    pub hunk: Option<usize>,
    pub hunks: usize,
}

pub struct ChangesView {
    pub(crate) root: Option<PathBuf>,
    pub(crate) repo: Option<Repo>,
    pub(crate) workspace: Option<WeakEntity<Workspace>>,
    pub(crate) mode: Mode,
    pub(crate) branch: String,
    pub(crate) staged: Vec<ChangeEntry>,
    pub(crate) unstaged: Vec<ChangeEntry>,
    pub(crate) message: String,
    pub(crate) message_focus: FocusHandle,
    pub(crate) error: Option<SharedString>,
    pub(crate) selected: Option<(String, Section)>,
    pub(super) open: Vec<OpenView>,
    pub(crate) pending: Option<PendingDiscard>,
    pub(crate) base: Option<String>,
    pub(crate) branches: Vec<String>,
    pub(crate) picker: bool,
    pub(crate) filter: String,
    pub(crate) filter_focus: FocusHandle,
    pub(crate) compare: Option<Compare>,
    pub(crate) compare_error: Option<SharedString>,
    pub(crate) compare_selected: Option<String>,
    pub(super) layout: DiffLayout,
}

impl ChangesView {
    pub fn new(
        root: Option<PathBuf>,
        workspace: Option<WeakEntity<Workspace>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            root,
            repo: None,
            workspace,
            mode: Mode::Changes,
            branch: String::new(),
            staged: Vec::new(),
            unstaged: Vec::new(),
            message: String::new(),
            message_focus: cx.focus_handle(),
            error: None,
            selected: None,
            open: Vec::new(),
            pending: None,
            base: None,
            branches: Vec::new(),
            picker: false,
            filter: String::new(),
            filter_focus: cx.focus_handle(),
            compare: None,
            compare_error: None,
            compare_selected: None,
            layout: diff_layout_from_setting(&cx.global::<Settings>().diff_layout),
        };
        this.reload_repo();
        this.reload_status();
        this
    }

    fn reload_repo(&mut self) {
        self.repo = self.root.as_deref().and_then(Repo::discover);
        self.branch = self.repo.as_ref().map(Repo::branch).unwrap_or_default();
        if let Some(repo) = &self.repo {
            self.branches = repo.local_branches();
            if self.base.is_none() {
                self.base = self
                    .branches
                    .iter()
                    .find(|b| **b != self.branch)
                    .cloned()
                    .or_else(|| self.branches.first().cloned());
            }
        }
    }

    fn reload_status(&mut self) {
        match self.repo.as_ref().map(|r| r.changes()) {
            Some(Ok(changes)) => {
                self.error = None;
                self.staged = changes
                    .iter()
                    .filter(|c| c.section == Section::Staged)
                    .cloned()
                    .collect();
                self.unstaged = changes
                    .iter()
                    .filter(|c| c.section == Section::Unstaged)
                    .cloned()
                    .collect();
            }
            Some(Err(e)) => self.error = Some(format!("git: {e}").into()),
            None => {
                self.staged.clear();
                self.unstaged.clear();
            }
        }
    }

    /// Re-read status and compare, refresh every open diff tab and ask the chrome
    /// to repaint.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.reload_status();
        if self.mode == Mode::Compare {
            self.reload_compare();
        }
        self.refresh_open(cx);
        self.notify_workspace(cx);
        cx.notify();
    }

    fn notify_workspace(&self, cx: &mut Context<Self>) {
        if let Some(ws) = self.workspace.clone() {
            ws.update(cx, |_, cx| cx.notify()).ok();
        }
    }

    pub fn status_info(&self) -> StatusInfo {
        if self.repo.is_none() {
            return StatusInfo {
                branch: "No repository".into(),
                detail: String::new(),
            };
        }
        StatusInfo {
            branch: self.branch.clone(),
            detail: match self.change_count() {
                0 => "No changes".into(),
                1 => "1 change".into(),
                n => format!("{n} changes"),
            },
        }
    }

    pub fn change_count(&self) -> usize {
        self.staged.len() + self.unstaged.len()
    }

    pub fn rail_badge(&self) -> Option<String> {
        (self.change_count() > 0).then(|| self.change_count().to_string())
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.picker = false;
        if mode == Mode::Compare {
            self.reload_compare();
        }
        self.refresh_open(cx);
        cx.notify();
    }

    pub fn toggle_picker(&mut self, cx: &mut Context<Self>) {
        self.picker = !self.picker;
        self.filter.clear();
        cx.notify();
    }

    // ---- commit -------------------------------------------------------------

    pub fn can_commit(&self) -> bool {
        !self.staged.is_empty() && !self.message.trim().is_empty()
    }

    pub fn commit(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.as_ref() else {
            return;
        };
        match repo.commit(self.message.trim()) {
            Ok(()) => {
                self.message.clear();
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Commit failed: {e}").into()),
        }
        self.refresh(cx);
    }

    /// Typing in the commit box; Enter commits.
    pub fn message_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let m = e.keystroke.modifiers;
        if m.control || m.alt || m.platform {
            return;
        }
        match e.keystroke.key.as_str() {
            "backspace" => {
                self.message.pop();
            }
            "enter" => {
                if self.can_commit() {
                    self.commit(cx);
                }
            }
            key => {
                let typed = e.keystroke.key_char.as_deref().or(match key {
                    "space" => Some(" "),
                    _ => None,
                });
                if let Some(ch) = typed {
                    if !ch.chars().any(char::is_control) {
                        self.message.push_str(ch);
                    }
                }
            }
        }
        cx.notify();
    }

    // ---- staging ------------------------------------------------------------

    pub fn stage_file(&mut self, rel: &str, cx: &mut Context<Self>) {
        self.apply(|r| r.stage_file(rel), cx);
    }

    pub fn unstage_file(&mut self, rel: &str, cx: &mut Context<Self>) {
        self.apply(|r| r.unstage_file(rel), cx);
    }

    pub fn stage_hunk(&mut self, rel: &str, hunk: usize, cx: &mut Context<Self>) {
        self.apply(|r| r.stage_hunk(rel, hunk), cx);
    }

    pub fn unstage_hunk(&mut self, rel: &str, hunk: usize, cx: &mut Context<Self>) {
        self.apply(|r| r.unstage_hunk(rel, hunk), cx);
    }

    fn apply(&mut self, op: impl FnOnce(&Repo) -> Result<(), git2::Error>, cx: &mut Context<Self>) {
        if let Some(repo) = self.repo.as_ref() {
            if let Err(e) = op(repo) {
                self.error = Some(format!("git: {e}").into());
            }
        }
        self.refresh(cx);
    }

    /// Discard the whole file, after confirmation.
    pub fn request_discard_file(&mut self, rel: &str, section: Section, cx: &mut Context<Self>) {
        let hunks = self
            .repo
            .as_ref()
            .and_then(|r| r.file_diff(rel, section).ok())
            .map(|f| f.hunk_count())
            .unwrap_or(0);
        self.open_file(rel, section, cx);
        self.pending = Some(PendingDiscard {
            rel: rel.to_string(),
            hunk: None,
            hunks,
        });
        self.refresh_open(cx);
        cx.notify();
    }

    /// Discard one hunk, after confirmation.
    pub fn request_discard_hunk(&mut self, rel: &str, hunk: usize, cx: &mut Context<Self>) {
        self.pending = Some(PendingDiscard {
            rel: rel.to_string(),
            hunk: Some(hunk),
            hunks: 1,
        });
        self.refresh_open(cx);
        cx.notify();
    }

    pub fn cancel_discard(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        self.refresh_open(cx);
        cx.notify();
    }

    pub fn confirm_discard(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.pending.take() else { return };
        if let Some(repo) = self.repo.as_ref() {
            let result = match p.hunk {
                Some(h) => repo.discard_hunk(&p.rel, h),
                None => repo.discard_file(&p.rel),
            };
            if let Err(e) = result {
                self.error = Some(format!("git: {e}").into());
            }
        }
        self.refresh(cx);
    }
}

/// Gutter marks and blame for a file the editor has open, from HEAD to disk.
pub fn decorations(root: &std::path::Path, path: &std::path::Path) -> Option<EditorDecorations> {
    let repo = Repo::discover(root)?;
    let rel = rel_path(root, path)?;
    let marks = repo.line_marks(&rel).ok()?;
    let blame = repo.blame(&rel).unwrap_or_default();
    Some(EditorDecorations {
        marks: marks
            .into_iter()
            .map(|m| {
                m.map(|m| match m {
                    crate::git::LineMark::Added => LineMark::Added,
                    crate::git::LineMark::Modified => LineMark::Modified,
                })
            })
            .collect(),
        blame: blame
            .into_iter()
            .map(|b| {
                b.map(|b| BlameLine {
                    author: b.author,
                    age: b.age,
                    subject: b.subject,
                })
            })
            .collect(),
    })
}
