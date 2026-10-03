//! Settings > Keyboard shortcuts: the full list, click-to-record, conflicts, reset.
//!
//! Recording and the search field take keys through the app keystroke interceptor
//! (registered in `Workspace::new`), so no widget library is needed.
use super::Workspace;
use crate::keymap;
use gpui::{Context, Keystroke};

#[derive(Clone)]
pub(crate) enum Capture {
    /// Waiting for the new keys for this action.
    Record(&'static str),
    /// The captured keys belong to another action; ask before stealing them.
    Conflict {
        action: &'static str,
        keys: String,
        owner: &'static str,
    },
    /// The search field is taking text.
    Filter,
}

fn is_modifier_key(key: &str) -> bool {
    matches!(
        key,
        "" | "control" | "alt" | "shift" | "platform" | "function" | "win" | "super" | "cmd"
    )
}

impl Workspace {
    // ---- behavior -----------------------------------------------------------

    /// Keys that reach the window while the settings page is recording or the
    /// search field is focused. Returns whether the page consumed the key.
    pub(crate) fn on_captured_key(&mut self, k: &Keystroke, cx: &mut Context<Self>) -> bool {
        let Some(capture) = self.capture.take() else {
            return false;
        };
        let modified = k.modifiers.control || k.modifiers.alt || k.modifiers.platform;
        match capture {
            Capture::Filter => {
                if k.key == "escape" {
                    // Leave the field.
                } else {
                    if !modified {
                        if k.key == "backspace" {
                            self.shortcut_filter.pop();
                        } else if let Some(ch) = k.key_char.as_deref() {
                            if !ch.chars().any(char::is_control) {
                                self.shortcut_filter.push_str(ch);
                            }
                        }
                    }
                    self.capture = Some(Capture::Filter);
                }
            }
            Capture::Record(action) => {
                if k.key == "escape" && !k.modifiers.modified() {
                    // Cancelled: leave the binding as it was.
                } else if k.key == "backspace" && !modified && !k.modifiers.shift {
                    self.set_shortcut(action, "", cx);
                } else if is_modifier_key(&k.key) {
                    self.capture = Some(Capture::Record(action));
                } else {
                    self.record(action, k.unparse(), cx);
                }
            }
            Capture::Conflict {
                action,
                keys,
                owner,
            } => {
                if k.key != "escape" {
                    self.capture = Some(Capture::Conflict {
                        action,
                        keys,
                        owner,
                    });
                }
            }
        }
        cx.notify();
        true
    }

    /// Apply the recorded keys, or ask first when another action owns them.
    fn record(&mut self, action: &'static str, keys: String, cx: &mut Context<Self>) {
        let overrides = self.settings(cx).shortcuts.clone();
        match keymap::owner_of(&keys, action, &overrides) {
            Some(owner) => {
                self.capture = Some(Capture::Conflict {
                    action,
                    keys,
                    owner: owner.action,
                })
            }
            None => self.set_shortcut(action, &keys, cx),
        }
    }

    pub(crate) fn set_shortcut(&mut self, action: &str, keys: &str, cx: &mut Context<Self>) {
        let (action, keys) = (action.to_string(), keys.to_string());
        self.update_settings(cx, |s| {
            keymap::set_override(&mut s.shortcuts, &action, &keys)
        });
        self.apply_shortcuts(cx);
    }

    /// Assign the captured key to the recording action, taking it from its owner.
    pub(crate) fn resolve_conflict(&mut self, assign: bool, cx: &mut Context<Self>) {
        let Some(Capture::Conflict {
            action,
            keys,
            owner,
        }) = self.capture.clone()
        else {
            return;
        };
        if assign {
            let remaining = keymap::without(owner, &keys, &self.settings(cx).shortcuts);
            self.update_settings(cx, |s| {
                keymap::set_override(&mut s.shortcuts, action, &keys);
                keymap::set_override(&mut s.shortcuts, owner, &remaining);
            });
            self.apply_shortcuts(cx);
        }
        self.capture = None;
        cx.notify();
    }

    pub(crate) fn reset_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.update_settings(cx, |s| s.shortcuts.clear());
        self.capture = None;
        self.apply_shortcuts(cx);
    }

    /// Register the current overrides with the app, live.
    pub(crate) fn apply_shortcuts(&mut self, cx: &mut Context<Self>) {
        let overrides = self.settings(cx).shortcuts.clone();
        cx.clear_key_bindings();
        cx.bind_keys(keymap::bindings(&overrides));
        cx.notify();
    }
}
