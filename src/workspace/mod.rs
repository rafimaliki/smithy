//! The window's root view: open folder, tabs, sidebar, add-on registry.
//! Rendering lives in `render.rs` and `chrome.rs`; this file is state and behavior.
mod actions;
mod banner;
mod chrome;
mod dialog;
mod lang_menu;
mod launch;
pub(crate) mod menu;
mod render;
mod settings_github;
mod settings_languages;
mod settings_shortcuts;
mod settings_shortcuts_view;
mod settings_view;
mod tab_menu;
pub mod tab_set;
mod tabs;
mod tabs_view;
pub(crate) mod watch;

use crate::addon::{AddonContext, Registry};
use crate::editor::load::{self, Loaded};
use crate::editor::state::EditorState;
use crate::editor::view::EditorView;
use crate::settings::{Settings, SIDEBAR_MAX, SIDEBAR_MIN};
use crate::tree::view::{TreeEvent, TreeView};
use gpui::AppContext as _;
use gpui::{
    AnyView, Context, Entity, FocusHandle, Focusable, PathPromptOptions, SharedString,
    Subscription, Window,
};
use settings_github::GithubSettings;
use settings_shortcuts::Capture;
use settings_view::SettingsSection;
use std::path::{Path, PathBuf};
use tab_set::TabSet;

pub enum TabContent {
    Editor(Entity<EditorView>, #[allow(dead_code)] Subscription),
    Viewer(AnyView),
    /// An add-on's own view, e.g. the source-control diff.
    Addon {
        view: AnyView,
        title: SharedString,
    },
    /// A binary file, never loaded; the screen offers Reveal in File Explorer.
    Binary {
        path: PathBuf,
        size: u64,
    },
    /// Unreadable: nothing is loaded, this explains why.
    Notice(SharedString),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Sidebar {
    Files,
    Addon(&'static str),
}

pub struct Workspace {
    pub(crate) folder: Option<PathBuf>,
    pub(crate) registry: Registry,
    pub(crate) tree: Option<Entity<TreeView>>,
    tree_sub: Option<Subscription>,
    pub(crate) tabs: TabSet<TabContent>,
    pub(crate) sidebar: Sidebar,
    pub(crate) sidebar_visible: bool,
    /// Raw sidebar width while the divider is being dragged.
    pub(crate) drag: Option<f32>,
    pub(crate) show_settings: bool,
    /// The status-bar language picker is open.
    pub(crate) lang_menu: bool,
    /// Which settings section the pane shows.
    pub(crate) settings_section: SettingsSection,
    /// Settings > GitHub, built only while the Pull requests add-on is on.
    pub(crate) github_settings: Option<Entity<GithubSettings>>,
    /// What the settings page is taking keys for, if anything.
    pub(crate) capture: Option<Capture>,
    /// Text in the shortcuts search field.
    pub(crate) shortcut_filter: String,
    /// Tab waiting on the unsaved-changes prompt.
    pub(crate) pending_close: Option<usize>,
    /// Tabs a bulk close still has to close, highest index last.
    pending_closes: Vec<usize>,
    /// The tab right-click menu, while it is open.
    pub(crate) tab_menu: Option<tabs::TabMenu>,
    /// The `Closed <file>` toast, while it is showing.
    pub(crate) toast: Option<tabs::Toast>,
    /// Bumped per toast so an old timer cannot clear a newer one.
    pub(crate) toast_seq: u64,
    /// Path waiting on the delete-to-Recycle-Bin confirmation.
    pub(crate) pending_delete: Option<PathBuf>,
    pub(crate) error: Option<SharedString>,
    /// Filesystem watcher for the open folder; `None` until one is open.
    pub(crate) watch: Option<watch::Watch>,
    focus: FocusHandle,
    /// Keeps the keystroke interceptor alive for the window's life.
    _capture_sub: Subscription,
}

impl Focusable for Workspace {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Workspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.entity();
        let capture_sub = cx.intercept_keystrokes(move |e, _window, cx| {
            let active = handle.read_with(cx, |this, _| this.capture.is_some());
            if !active {
                return;
            }
            let consumed = handle.update(cx, |this, cx| this.on_captured_key(&e.keystroke, cx));
            if consumed {
                cx.stop_propagation();
            }
        });
        Self {
            folder: None,
            registry: Registry::new(),
            tree: None,
            tree_sub: None,
            tabs: TabSet::default(),
            sidebar: Sidebar::Files,
            sidebar_visible: true,
            drag: None,
            show_settings: false,
            lang_menu: false,
            settings_section: SettingsSection::Appearance,
            github_settings: None,
            capture: None,
            shortcut_filter: String::new(),
            pending_close: None,
            pending_closes: Vec::new(),
            tab_menu: None,
            toast: None,
            toast_seq: 0,
            pending_delete: None,
            error: None,
            watch: None,
            focus: cx.focus_handle(),
            _capture_sub: capture_sub,
        }
    }

    pub fn settings<'a>(&self, cx: &'a gpui::App) -> &'a Settings {
        cx.global::<Settings>()
    }

    pub fn update_settings(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Settings)) {
        let s = cx.global_mut::<Settings>();
        f(s);
        s.save();
        cx.notify();
    }

    // ---- folder -------------------------------------------------------------

    pub fn prompt_open_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(path) = paths.into_iter().next() {
                    this.update_in(cx, |this, window, cx| this.open_folder(path, window, cx))
                        .ok();
                }
            }
        })
        .detach();
    }

    pub fn open_folder(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if !path.is_dir() {
            self.error = Some(format!("{} is not a folder", path.display()).into());
            cx.notify();
            return;
        }
        self.error = None;
        self.tabs = TabSet::default();
        self.pending_close = None;
        self.pending_closes.clear();
        self.tab_menu = None;
        self.toast = None;
        self.pending_delete = None;
        self.sidebar = Sidebar::Files;
        self.sidebar_visible = true;
        self.show_settings = false;
        self.lang_menu = false;
        let tree = cx.new(|cx| TreeView::new(path.clone(), cx));
        self.tree_sub = Some(cx.subscribe_in(
            &tree,
            window,
            |this, _, e: &TreeEvent, window, cx| match e {
                TreeEvent::Open(p) => this.open_file(p, window, cx),
                TreeEvent::DeleteRequested(p) => {
                    this.pending_delete = Some(p.clone());
                    cx.notify();
                }
                TreeEvent::Moved { from, to } => this.remap_paths(from, to, cx),
                TreeEvent::Error(message) => {
                    this.error = Some(message.clone().into());
                    cx.notify();
                }
            },
        ));
        self.tree = Some(tree);
        self.folder = Some(path.clone());
        self.update_settings(cx, |s| s.push_recent(path.clone()));
        self.restart_addons(cx);
        watch::start(self, window, cx);
    }

    /// Drop every running add-on and start the enabled ones again for the current folder.
    fn restart_addons(&mut self, cx: &mut Context<Self>) {
        self.registry = Registry::new();
        let ctx = AddonContext {
            root: self.folder.clone(),
            workspace: Some(cx.entity().downgrade()),
        };
        let enabled = cx.global::<Settings>().addons.clone();
        self.registry.start_enabled(&enabled, &ctx, cx);
        if let Sidebar::Addon(id) = self.sidebar {
            if self.registry.instance(id).is_none() {
                self.sidebar = Sidebar::Files;
            }
        }
        self.sync_github_settings(cx);
    }

    /// Keep Settings > GitHub alive only while the Pull requests add-on is on.
    fn sync_github_settings(&mut self, cx: &mut Context<Self>) {
        if self.registry.instance("pull-requests").is_some() {
            if self.github_settings.is_none() {
                self.github_settings = Some(cx.new(GithubSettings::new));
            }
        } else {
            self.github_settings = None;
            if self.settings_section == SettingsSection::Github {
                self.settings_section = SettingsSection::Appearance;
            }
        }
    }

    /// The Pull requests add-on's no-token state opens the settings page here.
    pub(crate) fn open_github_settings(&mut self, cx: &mut Context<Self>) {
        self.show_settings = true;
        self.settings_section = SettingsSection::Github;
        self.capture = None;
        cx.notify();
    }

    pub fn set_addon(&mut self, id: &str, on: bool, cx: &mut Context<Self>) {
        let enabled = cx.global::<Settings>().addons.clone();
        let ctx = AddonContext {
            root: self.folder.clone(),
            workspace: Some(cx.entity().downgrade()),
        };
        let changed: Vec<&'static str> = if on {
            self.registry.enable(id, &enabled, &ctx, cx)
        } else {
            self.registry.disable(id, &enabled)
        };
        self.update_settings(cx, |s| {
            for c in &changed {
                s.addons.retain(|a| a != c);
                if on {
                    s.addons.push((*c).to_string());
                }
            }
        });
        if let Sidebar::Addon(sid) = self.sidebar {
            if self.registry.instance(sid).is_none() {
                self.sidebar = Sidebar::Files;
            }
        }
        self.sync_github_settings(cx);
    }

    // ---- tabs ---------------------------------------------------------------

    pub fn open_file(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if path.starts_with("<addon>") {
            return;
        }
        self.show_settings = false;
        match self.tabs.position(path) {
            Some(i) => self.tabs.active = i,
            None => {
                let content = self.make_content(path, window, cx);
                self.tabs.open(path, || content);
            }
        }
        self.focus_tab(window, cx);
        watch::sync(self);
        cx.notify();
    }

    /// Open `path` and put the caret on `line` (0-based), scrolling it into view.
    /// Used by the Search add-on to jump to a result.
    pub fn open_file_at(
        &mut self,
        path: &Path,
        line: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_file(path, window, cx);
        if let Some(TabContent::Editor(editor, _)) = self.tabs.active_tab().map(|t| &t.content) {
            editor.update(cx, |editor, cx| editor.goto_line(line, cx));
        }
        self.focus_tab(window, cx);
    }

    fn make_content(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TabContent {
        for (_, inst) in self.registry.running() {
            if let Some(view) = inst.viewer_for(path, window, cx) {
                return TabContent::Viewer(view);
            }
        }
        match load::load(path) {
            Loaded::Text { text, read_only } => {
                let state = EditorState::new(&text, Some(path.to_path_buf()), read_only);
                let decorations = self.decorations_for(path);
                let editor = cx.new(|cx| {
                    let mut view = EditorView::new(state, cx);
                    view.set_decorations(decorations);
                    view
                });
                let sub = cx.observe(&editor, |_, _, cx| cx.notify());
                TabContent::Editor(editor, sub)
            }
            Loaded::Binary => {
                let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                TabContent::Binary {
                    path: path.to_path_buf(),
                    size,
                }
            }
            Loaded::TooLarge(n) => TabContent::Notice(
                format!("This file is {} MB, too large to open.", n / (1024 * 1024)).into(),
            ),
            Loaded::Error(e) => TabContent::Notice(format!("Could not read the file: {e}").into()),
        }
    }

    pub(crate) fn focus_tab(&self, window: &mut Window, cx: &mut Context<Self>) {
        match self.tabs.active_tab().map(|t| &t.content) {
            Some(TabContent::Editor(e, _)) => e.read(cx).focus(window),
            _ => window.focus(&self.focus),
        }
    }

    pub(crate) fn is_dirty(&self, i: usize, cx: &gpui::App) -> bool {
        match self.tabs.tabs.get(i).map(|t| &t.content) {
            Some(TabContent::Editor(e, _)) => e.read(cx).state.is_dirty(),
            _ => false,
        }
    }

    /// Gutter marks and blame for `path`, from whichever add-on has them.
    fn decorations_for(&self, path: &Path) -> crate::addon::EditorDecorations {
        let mut out = crate::addon::EditorDecorations::default();
        for (_, inst) in self.registry.running() {
            if let Some(d) = inst.editor_decorations(path) {
                out = d;
            }
        }
        out
    }

    /// Open (or focus) an add-on view in the editor area, keyed by `key`.
    pub fn open_addon_tab(
        &mut self,
        key: &str,
        title: String,
        view: AnyView,
        cx: &mut Context<Self>,
    ) {
        self.show_settings = false;
        let path = PathBuf::from(format!("<addon>/{key}"));
        let title: SharedString = title.into();
        match self.tabs.position(&path) {
            Some(i) => {
                self.tabs.active = i;
                if let Some(tab) = self.tabs.tabs.get_mut(i) {
                    tab.content = TabContent::Addon { view, title };
                }
            }
            None => {
                self.tabs.open(&path, || TabContent::Addon { view, title });
            }
        }
        cx.notify();
    }

    /// A path moved on disk (renamed or cut and pasted): open tabs and their
    /// editors follow it, so saving writes to the new place.
    fn remap_paths(&mut self, from: &Path, to: &Path, cx: &mut Context<Self>) {
        for tab in &self.tabs.tabs {
            if let Ok(rest) = tab.path.strip_prefix(from) {
                let new = to.join(rest);
                if let TabContent::Editor(e, _) = &tab.content {
                    e.update(cx, |e, _| e.state.path = Some(new));
                }
            }
        }
        self.tabs.remap_paths(from, to);
        watch::sync(self);
        cx.notify();
    }

    // ---- sidebar ------------------------------------------------------------

    pub(crate) fn clamp_width(w: f32) -> f32 {
        w.clamp(SIDEBAR_MIN, SIDEBAR_MAX)
    }

    /// Rail click: the active view collapses the sidebar, another view shows it.
    pub(crate) fn select_sidebar(&mut self, s: Sidebar, cx: &mut Context<Self>) {
        let leaving_settings = std::mem::take(&mut self.show_settings);
        if leaving_settings {
            self.capture = None;
        }
        if self.sidebar == s && self.sidebar_visible && !leaving_settings {
            self.sidebar_visible = false;
        } else {
            self.sidebar = s;
            self.sidebar_visible = true;
        }
        cx.notify();
    }
}
