//! Filesystem watcher for the open folder and the open files.
//!
//! `notify` reports on its own thread; a channel carries the events to the UI
//! thread, where a background task drains them. The watcher lives only while a
//! folder is open: dropping [`Watch`] closes the channel and stops the task.
use super::{TabContent, Workspace};
use async_channel::Sender;
use gpui::{Context, Window};
use notify::event::{EventKind, ModifyKind};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// What the watcher tells the workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsEvent {
    /// The file's contents may have changed.
    Changed(PathBuf),
    /// Entries appeared or vanished under the folder.
    Tree,
}

pub struct Watch {
    watcher: RecommendedWatcher,
    root: PathBuf,
    /// Parent folders watched for open files outside the root, to catch an
    /// atomic replace (write a temp file, then rename it over the target).
    outside: HashSet<PathBuf>,
}

impl Watch {
    /// Watch `root` recursively, feeding `tx`.
    pub fn start(root: &Path, tx: Sender<FsEvent>) -> notify::Result<Self> {
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                dispatch(&event, &tx);
            }
        })?;
        watcher.watch(root, RecursiveMode::Recursive)?;
        Ok(Self {
            watcher,
            root: root.to_path_buf(),
            outside: HashSet::new(),
        })
    }

    /// Keep the open files covered. Files under the root already are.
    pub fn set_files(&mut self, files: &[PathBuf]) {
        let want: HashSet<PathBuf> = files
            .iter()
            .filter(|p| p.is_absolute() && !p.starts_with(&self.root))
            .filter_map(|p| p.parent().map(Path::to_path_buf))
            .collect();
        for dir in self.outside.difference(&want) {
            let _ = self.watcher.unwatch(dir);
        }
        for dir in want.difference(&self.outside) {
            let _ = self.watcher.watch(dir, RecursiveMode::NonRecursive);
        }
        self.outside = want;
    }
}

/// Turn a raw event into what the workspace cares about.
fn dispatch(event: &Event, tx: &Sender<FsEvent>) {
    // `.git` churns on every git command and the tree never shows it.
    if event
        .paths
        .iter()
        .any(|p| p.components().any(|c| c.as_os_str() == ".git"))
    {
        return;
    }
    if matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    ) {
        let _ = tx.try_send(FsEvent::Tree);
    }
    if !matches!(event.kind, EventKind::Access(_)) {
        // A save may be a remove-then-create, so those count as content changes too.
        for path in &event.paths {
            let _ = tx.try_send(FsEvent::Changed(path.clone()));
        }
    }
}

/// Start watching the open folder, replacing any previous watcher.
pub fn start(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    ws.watch = None;
    let Some(root) = ws.folder.clone() else {
        return;
    };
    let (tx, rx) = async_channel::unbounded();
    match Watch::start(&root, tx) {
        Ok(watch) => ws.watch = Some(watch),
        Err(e) => {
            ws.error = Some(format!("Could not watch the folder: {e}").into());
            return;
        }
    }
    cx.spawn_in(window, async move |ws, cx| {
        while let Ok(event) = rx.recv().await {
            if ws
                .update_in(cx, |ws, _, cx| on_event(ws, event, cx))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
}

/// Point the watcher at the currently open files.
pub fn sync(ws: &mut Workspace) {
    let Some(watch) = ws.watch.as_mut() else {
        return;
    };
    let files: Vec<PathBuf> = ws.tabs.tabs.iter().map(|t| t.path.clone()).collect();
    watch.set_files(&files);
}

/// Apply one watcher event to the tree and the open editors.
fn on_event(ws: &mut Workspace, event: FsEvent, cx: &mut Context<Workspace>) {
    match event {
        FsEvent::Tree => {
            if let Some(tree) = &ws.tree {
                tree.update(cx, |tree, cx| tree.reload(cx));
            }
        }
        FsEvent::Changed(path) => {
            for tab in &ws.tabs.tabs {
                if tab.path == path {
                    if let TabContent::Editor(editor, _) = &tab.content {
                        editor.update(cx, |editor, cx| editor.external_change(cx));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, DataChange, RenameMode};

    fn drain(event: Event) -> Vec<FsEvent> {
        let (tx, rx) = async_channel::unbounded();
        dispatch(&event, &tx);
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            out.push(e);
        }
        out
    }

    fn path(name: &str) -> PathBuf {
        PathBuf::from(r"C:\repo").join(name)
    }

    #[test]
    fn create_and_rename_refresh_the_tree_content_changes_do_not() {
        let p = path("a.txt");
        let create = Event::new(EventKind::Create(CreateKind::File)).add_path(p.clone());
        assert_eq!(
            drain(create),
            vec![FsEvent::Tree, FsEvent::Changed(p.clone())]
        );

        let rename = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(path("old.txt"))
            .add_path(p.clone());
        let events = drain(rename);
        assert!(events.contains(&FsEvent::Tree));
        assert!(events.contains(&FsEvent::Changed(p.clone())));

        let data =
            Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Any))).add_path(p.clone());
        assert_eq!(drain(data), vec![FsEvent::Changed(p.clone())]);
    }

    #[test]
    fn access_and_git_events_are_ignored() {
        let access = Event::new(EventKind::Access(AccessKind::Any)).add_path(path("a.txt"));
        assert!(drain(access).is_empty());

        let git = Event::new(EventKind::Modify(ModifyKind::Any))
            .add_path(PathBuf::from(r"C:\repo\.git\index"));
        assert!(drain(git).is_empty());
    }
}
