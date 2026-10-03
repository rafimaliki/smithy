//! Default key bindings, overridable from settings (`Settings::shortcuts`).
use crate::actions::*;
use gpui::KeyBinding;

struct Binding {
    action: &'static str,
    keys: &'static str,
    context: Option<&'static str>,
    make: fn(&str, Option<&str>) -> KeyBinding,
}

macro_rules! b {
    ($name:ident, $keys:expr, $ctx:expr) => {
        Binding {
            action: stringify!($name),
            keys: $keys,
            context: $ctx,
            make: |k, c| KeyBinding::new(k, $name, c),
        }
    };
}

const EDITOR: Option<&str> = Some("Editor");

fn defaults() -> Vec<Binding> {
    vec![
        b!(MoveLeft, "left", EDITOR),
        b!(MoveRight, "right", EDITOR),
        b!(MoveUp, "up", EDITOR),
        b!(MoveDown, "down", EDITOR),
        b!(SelectLeft, "shift-left", EDITOR),
        b!(SelectRight, "shift-right", EDITOR),
        b!(SelectUp, "shift-up", EDITOR),
        b!(SelectDown, "shift-down", EDITOR),
        b!(Home, "home", EDITOR),
        b!(End, "end", EDITOR),
        b!(SelectHome, "shift-home", EDITOR),
        b!(SelectEnd, "shift-end", EDITOR),
        b!(Backspace, "backspace", EDITOR),
        b!(Delete, "delete", EDITOR),
        b!(Enter, "enter", EDITOR),
        b!(Tab, "tab", EDITOR),
        b!(Undo, "ctrl-z", EDITOR),
        b!(Redo, "ctrl-y", EDITOR),
        b!(Redo, "ctrl-shift-z", EDITOR),
        b!(Copy, "ctrl-c", EDITOR),
        b!(Cut, "ctrl-x", EDITOR),
        b!(Paste, "ctrl-v", EDITOR),
        b!(SelectAll, "ctrl-a", EDITOR),
        b!(Save, "ctrl-s", None),
        b!(ToggleSidebar, "ctrl-b", None),
        b!(OpenFolder, "ctrl-o", None),
        b!(CloseTab, "ctrl-w", None),
        b!(NextTab, "ctrl-tab", None),
        b!(PrevTab, "ctrl-shift-tab", None),
        b!(ReopenTab, "ctrl-shift-t", None),
        b!(Quit, "alt-f4", None),
    ]
}

/// The default bindings with `overrides` (action name, keystrokes) applied.
/// An override replaces every default binding of that action with one binding.
pub fn bindings(overrides: &[(String, String)]) -> Vec<KeyBinding> {
    let mut out = Vec::new();
    let mut overridden: Vec<&str> = Vec::new();
    for b in defaults() {
        match overrides.iter().find(|(name, _)| name == b.action) {
            Some((name, keys)) => {
                if !overridden.contains(&name.as_str()) {
                    overridden.push(name);
                    out.push((b.make)(keys, b.context));
                }
            }
            None => out.push((b.make)(b.keys, b.context)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_replaces_all_defaults_of_an_action() {
        let base = bindings(&[]).len();
        let one = bindings(&[("Redo".into(), "ctrl-r".into())]).len();
        // Redo had two defaults; the override leaves one.
        assert_eq!(one, base - 1);
    }
}
