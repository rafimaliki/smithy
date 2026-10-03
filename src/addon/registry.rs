use super::{all, resolve_disable, resolve_enable, Addon, AddonContext, AddonInfo, AddonInstance};
use gpui::App;
use std::collections::HashMap;

/// Owns every add-on descriptor and the instances of those that are on.
pub struct Registry {
    addons: Vec<Box<dyn Addon>>,
    running: HashMap<&'static str, Box<dyn AddonInstance>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            addons: all(),
            running: HashMap::new(),
        }
    }

    pub fn infos(&self) -> Vec<AddonInfo> {
        self.addons.iter().map(|a| a.info()).collect()
    }

    /// Start every add-on listed as enabled in settings (dependencies first).
    pub fn start_enabled(&mut self, enabled: &[String], ctx: &AddonContext, cx: &mut App) {
        for info in self.infos() {
            for id in resolve_enable(info.id, &[], &self.infos()) {
                if enabled.iter().any(|e| e == id) {
                    self.start(id, ctx, cx);
                }
            }
        }
    }

    fn start(&mut self, id: &'static str, ctx: &AddonContext, cx: &mut App) {
        if self.running.contains_key(id) {
            return;
        }
        if let Some(addon) = self.addons.iter().find(|a| a.info().id == id) {
            self.running.insert(id, addon.start(ctx, cx));
        }
    }

    /// Turn `id` on together with what it needs. Returns the ids that were started.
    pub fn enable(
        &mut self,
        id: &str,
        enabled: &[String],
        ctx: &AddonContext,
        cx: &mut App,
    ) -> Vec<&'static str> {
        let ids = resolve_enable(id, enabled, &self.infos());
        for id in &ids {
            self.start(id, ctx, cx);
        }
        ids
    }

    /// Turn `id` off together with what needs it. Dropping the instance frees it.
    /// Returns the ids that were stopped.
    pub fn disable(&mut self, id: &str, enabled: &[String]) -> Vec<&'static str> {
        let ids = resolve_disable(id, enabled, &self.infos());
        for id in &ids {
            self.running.remove(id);
        }
        ids
    }

    /// Running instances in registration order.
    pub fn running(&self) -> impl Iterator<Item = (&'static str, &dyn AddonInstance)> + '_ {
        self.addons
            .iter()
            .map(|a| a.info().id)
            .filter_map(|id| self.running.get(id).map(|i| (id, i.as_ref())))
    }

    pub fn instance(&self, id: &str) -> Option<&dyn AddonInstance> {
        self.running.get(id).map(|i| i.as_ref())
    }
}
