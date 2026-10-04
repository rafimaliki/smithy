//! Error highlighting add-on: syntax errors found by the editor's tree-sitter parse
//! are underlined in red, counted in the status bar, and F8 / Shift+F8 step through
//! them. The add-on holds no state: the editor reads whether it is on from Settings
//! and collects errors during the parse it already does for highlighting, so while it
//! is off nothing extra runs. Board frame `error-highlight`.
use super::{Addon, AddonContext, AddonInfo, AddonInstance};
use gpui::App;

/// Settings id; the editor checks `Settings::addon_enabled(ID)`.
pub const ID: &str = "problems";

pub struct Problems;

impl Addon for Problems {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: ID,
            name: "Error highlighting",
            description: "Underline syntax errors in the editor and count them in the status bar.",
            requires: &[],
        }
    }

    fn start(&self, _ctx: &AddonContext, _cx: &mut App) -> Box<dyn AddonInstance> {
        Box::new(Instance)
    }
}

struct Instance;

impl AddonInstance for Instance {}
