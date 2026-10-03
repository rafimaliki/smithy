//! Default key bindings and the shortcut catalog, overridable from settings
//! (`Settings::shortcuts`: action name -> keystroke; an empty keystroke unbinds).
use crate::actions::*;
use gpui::KeyBinding;

/// One rebindable action, as Settings > Keyboard shortcuts lists it.
pub struct Shortcut {
    pub action: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    /// Keystroke the action has out of the box, e.g. `ctrl-b`.
    pub keys: &'static str,
    /// A second default keystroke, if the action ships with one.
    pub alt: Option<&'static str>,
    /// Key context the binding only applies in, shown as a chip.
    pub context: Option<&'static str>,
    make: fn(&str, Option<&str>) -> KeyBinding,
}

macro_rules! s {
    ($name:ident, $label:expr, $group:expr, $keys:expr, $ctx:expr) => {
        Shortcut {
            action: stringify!($name),
            label: $label,
            group: $group,
            keys: $keys,
            alt: None,
            context: $ctx,
            make: |k, c| KeyBinding::new(k, $name, c),
        }
    };
    ($name:ident, $label:expr, $group:expr, $keys:expr, $alt:expr, $ctx:expr) => {
        Shortcut {
            action: stringify!($name),
            label: $label,
            group: $group,
            keys: $keys,
            alt: Some($alt),
            context: $ctx,
            make: |k, c| KeyBinding::new(k, $name, c),
        }
    };
}

const EDITOR: Option<&str> = Some("Editor");

/// Every action the app binds, in the order the settings page shows them.
pub const CATALOG: &[Shortcut] = &[
    // File
    s!(OpenFolder, "Open folder", "File", "ctrl-o", None),
    s!(Save, "Save", "File", "ctrl-s", None),
    s!(CloseTab, "Close tab", "File", "ctrl-w", None),
    s!(ReopenTab, "Reopen closed tab", "File", "ctrl-shift-t", None),
    s!(NextTab, "Next tab", "File", "ctrl-tab", None),
    s!(PrevTab, "Previous tab", "File", "ctrl-shift-tab", None),
    s!(Quit, "Quit", "File", "alt-f4", None),
    // Edit
    s!(Undo, "Undo", "Edit", "ctrl-z", EDITOR),
    s!(Redo, "Redo", "Edit", "ctrl-y", "ctrl-shift-z", EDITOR),
    s!(Copy, "Copy", "Edit", "ctrl-c", EDITOR),
    s!(Cut, "Cut", "Edit", "ctrl-x", EDITOR),
    s!(Paste, "Paste", "Edit", "ctrl-v", EDITOR),
    s!(SelectAll, "Select all", "Edit", "ctrl-a", EDITOR),
    s!(Find, "Find in file", "Edit", "ctrl-f", EDITOR),
    // View
    s!(ToggleSidebar, "Toggle sidebar", "View", "ctrl-b", None),
    s!(
        SearchEverywhere,
        "Search everywhere",
        "View",
        "ctrl-shift-o",
        None
    ),
    s!(
        TogglePreview,
        "Toggle Markdown preview",
        "View",
        "ctrl-shift-v",
        None
    ),
    s!(ToggleTerminal, "Toggle terminal", "View", "ctrl-`", None),
    // Editor
    s!(MoveLeft, "Move left", "Editor", "left", EDITOR),
    s!(MoveRight, "Move right", "Editor", "right", EDITOR),
    s!(MoveUp, "Move up", "Editor", "up", EDITOR),
    s!(MoveDown, "Move down", "Editor", "down", EDITOR),
    s!(SelectLeft, "Select left", "Editor", "shift-left", EDITOR),
    s!(SelectRight, "Select right", "Editor", "shift-right", EDITOR),
    s!(SelectUp, "Select up", "Editor", "shift-up", EDITOR),
    s!(SelectDown, "Select down", "Editor", "shift-down", EDITOR),
    s!(Home, "Go to line start", "Editor", "home", EDITOR),
    s!(End, "Go to line end", "Editor", "end", EDITOR),
    s!(
        SelectHome,
        "Select to line start",
        "Editor",
        "shift-home",
        EDITOR
    ),
    s!(
        SelectEnd,
        "Select to line end",
        "Editor",
        "shift-end",
        EDITOR
    ),
    s!(Backspace, "Backspace", "Editor", "backspace", EDITOR),
    s!(Delete, "Delete", "Editor", "delete", EDITOR),
    s!(Enter, "New line", "Editor", "enter", EDITOR),
    s!(Tab, "Indent", "Editor", "tab", EDITOR),
    s!(ToggleBlame, "Toggle blame", "Editor", "ctrl-alt-b", EDITOR),
    s!(ToggleWrap, "Toggle word wrap", "Editor", "alt-z", EDITOR),
    // Answered by the Language servers add-on.
    s!(GoToDefinition, "Go to definition", "Editor", "f12", EDITOR),
    s!(
        FindReferences,
        "Find references",
        "Editor",
        "shift-f12",
        EDITOR
    ),
    s!(NavBack, "Go back", "Editor", "alt-left", EDITOR),
    s!(NavForward, "Go forward", "Editor", "alt-right", EDITOR),
];

fn shortcut(action: &str) -> Option<&'static Shortcut> {
    CATALOG.iter().find(|s| s.action == action)
}

fn override_of<'a>(action: &str, overrides: &'a [(String, String)]) -> Option<&'a str> {
    overrides
        .iter()
        .find(|(name, _)| name == action)
        .map(|(_, keys)| keys.as_str())
}

/// Keystrokes the action answers to right now (empty when it is unbound).
pub fn effective(action: &str, overrides: &[(String, String)]) -> Vec<String> {
    let Some(s) = shortcut(action) else {
        return Vec::new();
    };
    match override_of(action, overrides) {
        Some("") => Vec::new(),
        Some(keys) => vec![keys.to_string()],
        None => {
            let mut keys = vec![s.keys.to_string()];
            keys.extend(s.alt.map(str::to_string));
            keys
        }
    }
}

/// The catalog entry (other than `except`) that currently uses `key`.
pub fn owner_of(
    key: &str,
    except: &str,
    overrides: &[(String, String)],
) -> Option<&'static Shortcut> {
    CATALOG
        .iter()
        .find(|s| s.action != except && effective(s.action, overrides).iter().any(|k| k == key))
}

/// `action`'s binding once `key` is taken away from it: its other keystroke, or
/// empty when that was its only one.
pub fn without(action: &str, key: &str, overrides: &[(String, String)]) -> String {
    effective(action, overrides)
        .into_iter()
        .find(|k| k != key)
        .unwrap_or_default()
}

/// Record `keys` as `action`'s binding (an empty `keys` unbinds it).
pub fn set_override(overrides: &mut Vec<(String, String)>, action: &str, keys: &str) {
    match overrides.iter_mut().find(|(name, _)| name == action) {
        Some(entry) => entry.1 = keys.to_string(),
        None => overrides.push((action.to_string(), keys.to_string())),
    }
}

/// The default bindings with `overrides` applied. An override replaces every
/// default binding of that action; an empty override removes it.
pub fn bindings(overrides: &[(String, String)]) -> Vec<KeyBinding> {
    let mut out = Vec::new();
    for s in CATALOG {
        match override_of(s.action, overrides) {
            Some("") => {}
            Some(keys) => out.push((s.make)(keys, s.context)),
            None => {
                out.push((s.make)(s.keys, s.context));
                if let Some(alt) = s.alt {
                    out.push((s.make)(alt, s.context));
                }
            }
        }
    }
    out
}

/// A stored keystroke (`ctrl-shift-t`) as the page prints it (`Ctrl+Shift+T`).
pub fn display(key: &str) -> String {
    if key.is_empty() {
        return "Unassigned".into();
    }
    key.split('-')
        .map(|part| match part {
            "ctrl" => "Ctrl".to_string(),
            "alt" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "win" | "super" | "cmd" => "Win".to_string(),
            "fn" => "Fn".to_string(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_replaces_defaults_and_empty_unbinds() {
        let base = bindings(&[]).len();
        // Redo has two defaults; one override leaves a single binding.
        assert_eq!(
            bindings(&[("Redo".into(), "ctrl-r".into())]).len(),
            base - 1
        );
        // An empty override drops the binding.
        assert_eq!(bindings(&[("Redo".into(), String::new())]).len(), base - 2);
    }

    #[test]
    fn effective_reads_default_override_and_clear() {
        assert_eq!(effective("ToggleSidebar", &[]), vec!["ctrl-b"]);
        assert_eq!(effective("Redo", &[]), vec!["ctrl-y", "ctrl-shift-z"]);
        assert!(effective("ToggleSidebar", &[("ToggleSidebar".into(), "".into())]).is_empty());
        assert_eq!(
            effective(
                "ToggleSidebar",
                &[("ToggleSidebar".into(), "ctrl-1".into())]
            ),
            vec!["ctrl-1"]
        );
    }

    #[test]
    fn owner_is_the_other_action_using_the_key() {
        // The owner is found when someone else asks for its key.
        assert_eq!(
            owner_of("ctrl-b", "Save", &[]).unwrap().action,
            "ToggleSidebar"
        );
        // The action that holds the key does not own it against itself.
        assert!(owner_of("ctrl-b", "ToggleSidebar", &[]).is_none());
        // An override moves ownership.
        let overrides = vec![("Save".to_string(), "ctrl-b".to_string())];
        assert_eq!(
            owner_of("ctrl-b", "ToggleSidebar", &overrides)
                .unwrap()
                .action,
            "Save"
        );
    }

    #[test]
    fn without_keeps_the_actions_other_binding() {
        assert_eq!(without("Redo", "ctrl-shift-z", &[]), "ctrl-y");
        assert_eq!(without("Redo", "ctrl-y", &[]), "ctrl-shift-z");
        assert_eq!(without("Save", "ctrl-s", &[]), "");
    }

    #[test]
    fn display_prints_modifiers_and_keys() {
        assert_eq!(display("ctrl-shift-t"), "Ctrl+Shift+T");
        assert_eq!(display("alt-f4"), "Alt+F4");
        assert_eq!(display("left"), "Left");
        assert_eq!(display("ctrl-/"), "Ctrl+/");
        assert_eq!(display(""), "Unassigned");
    }
}
