//! Split panes add-on. Turning it on is the switch that lets a second editor
//! group exist: the core asks every running add-on [`AddonInstance::enables_split`]
//! before it offers "Split right" or creates a group, so with this off there is
//! exactly one group and the tab menu has no split row. Design: board frame
//! `split-panes`.
//!
//! The two groups themselves live in the core workspace (`workspace/split.rs`);
//! this instance holds no state and costs nothing until it is started.
use super::{Addon, AddonContext, AddonInfo, AddonInstance};
use gpui::App;

pub struct SplitPanes;

impl Addon for SplitPanes {
    fn info(&self) -> AddonInfo {
        AddonInfo {
            id: "split-panes",
            name: "Split panes",
            description: "Two editor groups side by side, each with its own tabs.",
            requires: &[],
        }
    }

    fn start(&self, _ctx: &AddonContext, _cx: &mut App) -> Box<dyn AddonInstance> {
        Box::new(Instance)
    }
}

struct Instance;

impl AddonInstance for Instance {
    fn enables_split(&self) -> bool {
        true
    }
}
