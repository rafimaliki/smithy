//! Pull requests add-on state: the PR list and the selected detail. The review
//! diff lives in `review.rs`; rendering in `sidebar.rs`, `panels.rs`, `detail.rs`.
use crate::git::FileDiff;
use crate::github::{token, CheckState, Client, PullRequest, RepoSlug};
use crate::workspace::Workspace;
use gpui::{AppContext as _, Context, Task, WeakEntity};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub enum StateFilter {
    Open,
    Closed,
}

/// What the sidebar is showing.
#[derive(Clone, PartialEq)]
pub enum Load {
    Idle,
    Loading,
    Ready,
    /// GitHub was reached but refused or failed.
    Error(String),
    /// No git repository, or its origin is not on GitHub.
    Unavailable(String),
    NoToken,
}

/// A fetched pull request diff: the PR's files, ready for the read-only view.
pub struct Review {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub head_ref: String,
    pub base_ref: String,
    pub files: Vec<FileDiff>,
    pub selected: Option<String>,
}

pub struct PrView {
    pub(crate) root: Option<PathBuf>,
    pub(crate) slug: Option<RepoSlug>,
    /// False when the open folder has no git repository at all.
    pub(crate) has_repo: bool,
    pub(crate) workspace: Option<WeakEntity<Workspace>>,
    pub(crate) load: Load,
    pub(crate) state_filter: StateFilter,
    pub(crate) mine: bool,
    pub(crate) login: Option<String>,
    pub(crate) items: Vec<PullRequest>,
    pub(crate) selected: Option<usize>,
    pub(crate) review: Option<Review>,
    pub(crate) review_loading: bool,
    pub(crate) review_error: Option<String>,
    list_task: Option<Task<()>>,
    detail_task: Option<Task<()>>,
    pub(super) review_task: Option<Task<()>>,
}

impl PrView {
    pub fn new(
        root: Option<PathBuf>,
        slug: Option<RepoSlug>,
        has_repo: bool,
        workspace: Option<WeakEntity<Workspace>>,
        _cx: &mut Context<Self>,
    ) -> Self {
        let has_token = token::load_token().is_some();
        Self {
            root,
            slug,
            has_repo,
            workspace,
            load: if has_token { Load::Idle } else { Load::NoToken },
            state_filter: StateFilter::Open,
            mine: false,
            login: None,
            items: Vec::new(),
            selected: None,
            review: None,
            review_loading: false,
            review_error: None,
            list_task: None,
            detail_task: None,
            review_task: None,
        }
    }

    /// Called on every render. Starts the first load when a token appears, and
    /// picks up a token connected in Settings while the no-token panel is up.
    pub(crate) fn sync(&mut self, cx: &mut Context<Self>) {
        if self.load == Load::NoToken && token::load_token().is_some() {
            self.load = Load::Idle;
        }
        if self.load != Load::Idle {
            return;
        }
        match (&self.slug, self.has_repo) {
            (Some(_), _) => self.reload(cx),
            (None, false) => {
                self.load = Load::Unavailable(
                    "This folder has no git repository. Open one to list pull requests.".into(),
                )
            }
            (None, true) => {
                self.load =
                    Load::Unavailable("This folder's origin remote is not on GitHub.".into())
            }
        }
    }

    // ---- list ---------------------------------------------------------------

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(slug) = self.slug.clone() else {
            return;
        };
        self.load = Load::Loading;
        let state = match self.state_filter {
            StateFilter::Open => "open",
            StateFilter::Closed => "closed",
        };
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_executor()
                .spawn(async move { fetch_list(&slug, state) })
                .await;
            this.update(cx, |this, cx| this.list_loaded(fetched, cx))
                .ok();
        }));
        cx.notify();
    }

    fn list_loaded(
        &mut self,
        fetched: Result<(Vec<PullRequest>, String), String>,
        cx: &mut Context<Self>,
    ) {
        match fetched {
            Ok((items, login)) => {
                self.items = items;
                self.login = (!login.is_empty()).then_some(login);
                self.selected = self.selected.filter(|i| *i < self.items.len());
                self.load = Load::Ready;
            }
            Err(e) => self.load = Load::Error(e),
        }
        cx.notify();
    }

    pub fn set_state_filter(&mut self, filter: StateFilter, cx: &mut Context<Self>) {
        if self.state_filter == filter {
            return;
        }
        self.state_filter = filter;
        self.selected = None;
        self.items.clear();
        self.reload(cx);
    }

    pub fn toggle_mine(&mut self, cx: &mut Context<Self>) {
        self.mine = !self.mine;
        cx.notify();
    }

    /// The rows to draw, after the Mine filter.
    pub(crate) fn visible(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, pr)| {
                !self.mine
                    || self
                        .login
                        .as_deref()
                        .is_some_and(|login| pr.author == login)
            })
            .map(|(i, _)| i)
            .collect()
    }

    // ---- detail -------------------------------------------------------------

    pub fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(pr) = self.items.get(index).cloned() else {
            return;
        };
        self.selected = Some(index);
        self.load_files(pr.number, cx);
        let panel = cx.entity();
        let view = cx.new(|cx| super::detail::PrDetailView::new(panel, pr.number, cx));
        let title = format!("Pull request #{}", pr.number);
        self.open_tab(&format!("prs/{}", pr.number), title, view.into(), cx);
        cx.notify();
    }

    fn load_files(&mut self, number: u64, cx: &mut Context<Self>) {
        let Some(slug) = self.slug.clone() else {
            return;
        };
        if self
            .items
            .iter()
            .any(|pr| pr.number == number && pr.files_loaded)
        {
            return;
        }
        self.detail_task = Some(cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_executor()
                .spawn(async move {
                    let token = token::load_token().ok_or_else(|| "No GitHub token".to_string())?;
                    Client::new(token)
                        .files(&slug.full_name(), number)
                        .map_err(|e| e.to_string())
                })
                .await;
            this.update(cx, |this, cx| {
                if let Some(pr) = this.items.iter_mut().find(|pr| pr.number == number) {
                    pr.files_loaded = true;
                    if let Ok(files) = fetched {
                        pr.files = files;
                    }
                }
                cx.notify();
            })
            .ok();
        }));
    }

    // ---- review -------------------------------------------------------------

    /// The newest dot state for a PR, for the status bar and the review header.
    pub(crate) fn checks_dot(&self, pr: &PullRequest) -> Option<CheckState> {
        pr.checks.as_ref().map(|c| c.state)
    }

    pub(super) fn open_tab(
        &self,
        key: &str,
        title: String,
        view: gpui::AnyView,
        cx: &mut Context<Self>,
    ) {
        if let Some(ws) = self.workspace.clone() {
            ws.update(cx, |workspace, cx| {
                workspace.open_addon_tab(key, title, view, cx)
            })
            .ok();
        }
    }
}

/// One background round trip: the list, the account for the Mine filter, and
/// every row's checks. Runs off the UI thread.
fn fetch_list(slug: &RepoSlug, state: &str) -> Result<(Vec<PullRequest>, String), String> {
    let token = token::load_token().ok_or_else(|| "No GitHub token".to_string())?;
    let client = Client::new(token);
    let mut pulls = client
        .pulls(&slug.full_name(), state)
        .map_err(|e| e.to_string())?;
    let login = client.account().map(|a| a.login).unwrap_or_default();
    for pr in &mut pulls {
        if !pr.head_sha.is_empty() {
            pr.checks = client.checks(&slug.full_name(), &pr.head_sha).ok();
        }
    }
    Ok((pulls, login))
}
