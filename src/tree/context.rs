//! The file tree's right-click menu and the file actions behind it.
//!
//! The items the board draws that belong to add-ons are not here yet: Find in
//! folder… (Search) and Open in Terminal (Terminal) have no add-on to run them,
//! so they arrive with those add-ons.
use super::ops;
use super::view::{TreeEvent, TreeView};
use crate::theme::Theme;
use crate::workspace::menu;
use gpui::{prelude::*, AnyElement, ClipboardItem, Context, MouseDownEvent, Pixels, Point, Window};
use std::path::PathBuf;

/// An open right-click menu: what was clicked and where it was drawn.
pub(super) struct Menu {
    pub path: PathBuf,
    pub is_dir: bool,
    pub at: Point<Pixels>,
}

impl TreeView {
    pub(crate) fn context_menu(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open_menu = self.menu.as_ref()?;
        let path = open_menu.path.clone();
        let is_dir = open_menu.is_dir;
        let at = open_menu.at;
        let can_paste = cx.global::<ops::Clipboard>().path.is_some();
        // The opened folder itself (right-click on empty space): it can hold new
        // entries and be pasted into, but not be copied, cut, renamed or deleted.
        let is_root = path == self.tree.root;

        let mut rows: Vec<AnyElement> = Vec::new();
        if is_dir {
            let (new_file, new_folder) = (path.clone(), path.clone());
            rows.push(
                menu::item("m-new-file", "New file…", None, false, true, t)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.menu = None;
                        this.new_entry(new_file.clone(), false, window, cx);
                    }))
                    .into_any_element(),
            );
            rows.push(
                menu::item("m-new-folder", "New folder…", None, false, true, t)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.menu = None;
                        this.new_entry(new_folder.clone(), true, window, cx);
                    }))
                    .into_any_element(),
            );
            rows.push(menu::separator(t).into_any_element());
        } else {
            let open = path.clone();
            rows.push(
                menu::item("m-open", "Open", Some("Enter"), false, true, t)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu = None;
                        this.selected = Some(open.clone());
                        cx.emit(TreeEvent::Open(open.clone()));
                        cx.notify();
                    }))
                    .into_any_element(),
            );
            rows.push(menu::separator(t).into_any_element());
        }

        if !is_root {
            let (copy, cut) = (path.clone(), path.clone());
            rows.push(
                menu::item("m-copy", "Copy", Some("Ctrl+C"), false, true, t)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu = None;
                        this.hold(copy.clone(), false, cx);
                    }))
                    .into_any_element(),
            );
            rows.push(
                menu::item("m-cut", "Cut", Some("Ctrl+X"), false, true, t)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu = None;
                        this.hold(cut.clone(), true, cx);
                    }))
                    .into_any_element(),
            );
        }
        if is_dir {
            let paste = path.clone();
            rows.push(
                menu::item("m-paste", "Paste", Some("Ctrl+V"), false, can_paste, t)
                    .when(can_paste, |d| {
                        d.on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = None;
                            this.paste_into(paste.clone(), cx);
                        }))
                    })
                    .into_any_element(),
            );
        }
        rows.push(menu::separator(t).into_any_element());

        let (copy_abs, copy_rel) = (path.clone(), path.clone());
        rows.push(
            menu::item(
                "m-copy-path",
                "Copy path",
                Some("Shift+Alt+C"),
                false,
                true,
                t,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.menu = None;
                this.copy_path(copy_abs.clone(), false, cx);
            }))
            .into_any_element(),
        );
        // The root has no path relative to itself.
        if !is_root {
            rows.push(
                menu::item("m-copy-rel", "Copy relative path", None, false, true, t)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu = None;
                        this.copy_path(copy_rel.clone(), true, cx);
                    }))
                    .into_any_element(),
            );
        }
        rows.push(menu::separator(t).into_any_element());

        if !is_root {
            let (rename, delete) = (path.clone(), path.clone());
            rows.push(
                menu::item("m-rename", "Rename…", Some("F2"), false, true, t)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.menu = None;
                        this.begin_rename(rename.clone(), false, window, cx);
                    }))
                    .into_any_element(),
            );
            rows.push(
                menu::item("m-delete", "Delete", Some("Del"), true, true, t)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request_delete(delete.clone(), cx);
                    }))
                    .into_any_element(),
            );
            rows.push(menu::separator(t).into_any_element());
        }
        // Only when the Terminal add-on is on; its menus are hidden while it is off.
        if is_dir
            && cx
                .global::<crate::settings::Settings>()
                .addon_enabled("terminal")
        {
            let dir = path.clone();
            rows.push(
                menu::item("m-terminal", "Open in Terminal", None, false, true, t)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu = None;
                        cx.emit(TreeEvent::OpenTerminal(dir.clone()));
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        let reveal = path.clone();
        rows.push(
            menu::item("m-reveal", "Reveal in File Explorer", None, false, true, t)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.menu = None;
                    this.reveal(reveal.clone(), cx);
                }))
                .into_any_element(),
        );

        let entity = cx.entity();
        Some(menu::popup(
            t,
            at,
            move |_: &MouseDownEvent, _: &mut Window, cx| {
                entity.update(cx, |this, cx| {
                    this.menu = None;
                    cx.notify();
                });
            },
            rows,
        ))
    }

    /// Remember `path` for Paste. Smithy's own clipboard, see `ops::Clipboard`.
    pub(crate) fn hold(&mut self, path: PathBuf, cut: bool, cx: &mut Context<Self>) {
        let clip = cx.global_mut::<ops::Clipboard>();
        clip.path = Some(path);
        clip.cut = cut;
        cx.notify();
    }

    pub(crate) fn copy_path(&self, path: PathBuf, relative: bool, cx: &mut Context<Self>) {
        let text = if relative {
            ops::relative(&path, &self.tree.root)
        } else {
            path.display().to_string()
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    pub(crate) fn reveal(&self, path: PathBuf, cx: &mut Context<Self>) {
        cx.reveal_path(&path);
    }

    pub(crate) fn paste_into(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let clip = cx.global::<ops::Clipboard>().clone();
        let Some(from) = clip.path else {
            return;
        };
        match ops::paste(&from, &dir, clip.cut) {
            Ok(to) => {
                if clip.cut {
                    cx.global_mut::<ops::Clipboard>().path = None;
                    cx.emit(TreeEvent::Moved {
                        from,
                        to: to.clone(),
                    });
                }
                self.selected = Some(to);
                self.reload(cx);
            }
            Err(e) => cx.emit(TreeEvent::Error(format!("Could not paste: {e}"))),
        }
    }

    /// Paste from the keyboard: into `path` when it is a folder, else beside it.
    pub(crate) fn paste_near(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let dir = if path.is_dir() {
            path
        } else {
            match path.parent() {
                Some(p) => p.to_path_buf(),
                None => return,
            }
        };
        self.paste_into(dir, cx);
    }

    /// Create an entry in `dir` and immediately edit its name inline.
    pub(crate) fn new_entry(
        &mut self,
        dir: PathBuf,
        folder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let made = if folder {
            ops::new_dir(&dir, "New folder")
        } else {
            ops::new_file(&dir, "untitled.txt")
        };
        match made {
            Ok(path) => {
                self.tree.expand(&dir);
                self.reload(cx);
                self.begin_rename(path, true, window, cx);
            }
            Err(e) => cx.emit(TreeEvent::Error(format!("Could not create: {e}"))),
        }
    }

    /// Ask the workspace to confirm; it owns the dialog and the open tabs.
    pub(crate) fn request_delete(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.menu = None;
        self.selected = Some(path.clone());
        cx.emit(TreeEvent::DeleteRequested(path));
    }

    /// Enter in the rename field: rename on disk, or keep the field open with the
    /// reason shown when the name is refused.
    pub(crate) fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.rename.as_ref() else {
            return;
        };
        let from = rename.path.clone();
        let created = rename.input.created;
        if self
            .rename
            .as_mut()
            .is_some_and(|r| r.input.check().is_err())
        {
            cx.notify();
            return;
        }
        let name = self
            .rename
            .as_ref()
            .map(|r| r.input.text())
            .unwrap_or_default();
        match ops::rename(&from, &name) {
            Ok(to) => {
                self.rename = None;
                if !created && from != to {
                    cx.emit(TreeEvent::Moved {
                        from,
                        to: to.clone(),
                    });
                }
                self.selected = Some(to);
                self.reload(cx);
                window.focus(&self.focus);
            }
            Err(e) => {
                if let Some(rename) = self.rename.as_mut() {
                    rename.input.error = Some(e.message());
                }
                cx.notify();
            }
        }
    }

    /// Esc: keep the old name, and remove an entry that New file / New folder made.
    pub(crate) fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        let Some(rename) = self.rename.take() else {
            return;
        };
        if rename.input.created {
            let _ = if rename.path.is_dir() {
                std::fs::remove_dir_all(&rename.path)
            } else {
                std::fs::remove_file(&rename.path)
            };
        }
        self.reload(cx);
    }
}
