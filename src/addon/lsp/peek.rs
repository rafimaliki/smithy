//! The Language servers view: what a navigation gesture does, the trail through
//! definitions, and the peek's keyboard and focus. The round trip lives in
//! `query.rs`, the peek's rows in `refs.rs`, the drawing in `peek_view.rs`.
use super::manager::Manager;
use super::nav::{History, Loc};
use super::query::{self, Position};
use super::refs::{clamp_selection, row_index, Peek, Reference};
use super::uri;
use crate::addon::Navigate;
use crate::editor::lang::Lang;
use crate::workspace::Workspace;
use gpui::{
    Context, FocusHandle, KeyDownEvent, ScrollHandle, SharedString, Task, WeakEntity, Window,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct LspView {
    manager: Arc<Mutex<Manager>>,
    workspace: Option<WeakEntity<Workspace>>,
    pub(super) focus: FocusHandle,
    pub(crate) peek: Option<Peek>,
    /// One line of feedback drawn near the caret: a starting server, a miss.
    pub(crate) tip: Option<SharedString>,
    /// File lines read for the peek, by path.
    pub(crate) sources: HashMap<PathBuf, Vec<String>>,
    scroll: ScrollHandle,
    history: History,
    _task: Option<Task<()>>,
}

impl LspView {
    pub fn new(
        manager: Arc<Mutex<Manager>>,
        workspace: Option<WeakEntity<Workspace>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            manager,
            workspace,
            focus: cx.focus_handle(),
            peek: None,
            tip: None,
            sources: HashMap::new(),
            scroll: ScrollHandle::new(),
            history: History::default(),
            _task: None,
        }
    }

    pub(crate) fn scroll(&self) -> &ScrollHandle {
        &self.scroll
    }

    pub(crate) fn sources(&self) -> &HashMap<PathBuf, Vec<String>> {
        &self.sources
    }

    /// Escape from the peek, or a click on the dimmed backdrop.
    pub(crate) fn close(&mut self, cx: &mut Context<Self>) {
        self.peek = None;
        self.changed(cx);
    }

    /// The overlay is drawn by the workspace, so it has to repaint too.
    fn changed(&self, cx: &mut Context<Self>) {
        cx.notify();
        if let Some(workspace) = self.workspace.clone() {
            workspace.update(cx, |_, cx| cx.notify()).ok();
        }
    }

    /// True while the peek or a tip should be drawn over the window.
    pub fn overlay_open(&self) -> bool {
        self.peek.is_some() || self.tip.is_some()
    }

    /// A navigation gesture the editor saw. Returns true when this view took it.
    pub fn navigate(
        &mut self,
        what: Navigate,
        path: &Path,
        line: u32,
        character: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.tip = None;
        let origin = Loc::new(path.to_path_buf(), line, character);
        match what {
            Navigate::Definition => self.ask(&origin, false, window, cx),
            Navigate::References => self.ask(&origin, true, window, cx),
            Navigate::Back => self.walk(false, window, cx),
            Navigate::Forward => self.walk(true, window, cx),
        }
        self.changed(cx);
        true
    }

    fn walk(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let step = if forward {
            self.history.forward()
        } else {
            self.history.back()
        };
        if let Some(loc) = step {
            self.go(&loc, window, cx);
        }
    }

    /// Ask for a definition, or for the references of the symbol at `origin`.
    fn ask(&mut self, origin: &Loc, references: bool, window: &Window, cx: &mut Context<Self>) {
        let Some(lang) = Lang::for_path(&origin.path) else {
            self.tip = Some("No language server is known for this file.".into());
            return;
        };
        // Fail fast so the reason is on screen before any process starts.
        if let Err(why) = self.manager.lock().unwrap().def_and_state(lang) {
            self.tip = Some(why.into());
            return;
        }
        self.tip = Some(if references {
            "Finding references…".into()
        } else {
            "Finding definition…".into()
        });
        let manager = self.manager.clone();
        let origin = origin.clone();
        let path = origin.path.clone();
        let position = Position {
            line: origin.line,
            character: origin.character,
        };
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let work = cx
                .background_executor()
                .spawn(async move { query::load(manager, lang, path, position, references).await });
            let outcome = work.await;
            this.update_in(cx, |this, window, cx| {
                this.finish(outcome, origin, references, window, cx)
            })
            .ok();
        }));
    }

    fn finish(
        &mut self,
        outcome: Result<query::Outcome, String>,
        origin: Loc,
        references: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(why) => {
                self.tip = Some(why.into());
                self.changed(cx);
                return;
            }
        };
        self.tip = None;
        if references {
            self.show_peek(outcome, origin, window);
        } else {
            match outcome.locations.first().cloned() {
                None => {
                    self.tip =
                        Some(format!("No definition found for {}.", named(&outcome.symbol)).into());
                }
                // Ctrl+Click on a definition asks for its references, per the board.
                Some((path, line, _))
                    if uri::same_file(&path, &origin.path) && line == origin.line =>
                {
                    let references = Loc::new(path, line, origin.character);
                    self.ask(&references, true, window, cx);
                }
                Some((path, line, character)) => {
                    let target = Loc::new(path, line, character);
                    self.history.visit(origin);
                    self.history.visit(target.clone());
                    self.go(&target, window, cx);
                }
            }
        }
        self.changed(cx);
    }

    fn show_peek(&mut self, outcome: query::Outcome, origin: Loc, window: &mut Window) {
        if outcome.locations.is_empty() {
            self.tip = Some(format!("No references to {}.", named(&outcome.symbol)).into());
            return;
        }
        let definitions = outcome.definitions;
        let mut refs: Vec<Reference> = outcome
            .locations
            .iter()
            .map(|(path, line, character)| Reference {
                text: outcome
                    .sources
                    .get(path)
                    .and_then(|lines| lines.get(*line as usize))
                    .cloned()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                is_def: definitions.iter().any(|(def_path, def_line, _)| {
                    uri::same_file(def_path, path) && def_line == line
                }),
                loc: Loc::new(path.clone(), *line, *character),
            })
            .collect();
        refs.sort_by(|a, b| {
            a.loc
                .path
                .to_string_lossy()
                .to_lowercase()
                .cmp(&b.loc.path.to_string_lossy().to_lowercase())
                .then(a.loc.line.cmp(&b.loc.line))
        });
        // Land on the first use, not the declaration: that is what was asked for.
        let selected = refs.iter().position(|r| !r.is_def).unwrap_or(0);
        self.sources = outcome.sources;
        self.peek = Some(Peek {
            symbol: outcome.symbol,
            refs,
            selected,
            origin,
        });
        window.focus(&self.focus);
    }

    /// Open a place in the editor.
    fn go(&self, loc: &Loc, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let (path, line) = (loc.path.clone(), loc.line as usize);
        workspace
            .update(cx, |workspace, cx| {
                workspace.open_file_at(&path, line, window, cx)
            })
            .ok();
    }

    pub fn key_down(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.peek.is_none() {
            self.tip = None;
            self.changed(cx);
            return;
        }
        match e.keystroke.key.as_str() {
            "escape" => self.peek = None,
            "up" => self.move_selection(-1),
            "down" => self.move_selection(1),
            "enter" => self.open_selected(window, cx),
            _ => {}
        }
        self.changed(cx);
    }

    fn move_selection(&mut self, delta: isize) {
        let index = match self.peek.as_mut() {
            Some(peek) => {
                peek.selected = clamp_selection(peek.refs.len(), peek.selected, delta);
                row_index(peek, peek.selected)
            }
            None => return,
        };
        self.scroll.scroll_to_item(index);
    }

    pub(crate) fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        let index = match self.peek.as_mut() {
            Some(peek) => {
                peek.selected = index.min(peek.refs.len().saturating_sub(1));
                row_index(peek, peek.selected)
            }
            None => return,
        };
        self.scroll.scroll_to_item(index);
        self.changed(cx);
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(peek) = self.peek.take() else {
            return;
        };
        let Some(reference) = peek.refs.get(peek.selected) else {
            return;
        };
        let target = reference.loc.clone();
        self.history.visit(peek.origin);
        self.history.visit(target.clone());
        self.go(&target, window, cx);
    }
}

/// `“Button”`, or `the symbol` when the name is not known.
fn named(symbol: &str) -> String {
    if symbol.is_empty() {
        "the symbol".to_string()
    } else {
        format!("“{symbol}”")
    }
}
