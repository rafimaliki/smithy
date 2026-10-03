//! Which server runs for which language: the per-language on/off the user sets,
//! and the processes currently running, keyed by language. Whether a server is
//! installed is looked up once, when the add-on starts.
use super::client::Client;
use super::servers::{self, ServerDef};
use crate::addon::{LanguageServerRow, ServerState};
use crate::editor::lang::Lang;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Manager {
    root: Option<PathBuf>,
    enabled: Vec<String>,
    installed: HashMap<&'static str, bool>,
    clients: HashMap<&'static str, Arc<Client>>,
}

impl Manager {
    pub fn new(root: Option<PathBuf>) -> Self {
        let installed = servers::SERVERS
            .iter()
            .map(|s| (s.id, servers::installed(s.command)))
            .collect();
        Self {
            root,
            enabled: servers::load_enabled(),
            installed,
            clients: HashMap::new(),
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    fn is_enabled(&self, id: &str) -> bool {
        self.enabled.iter().any(|e| e == id)
    }

    /// Where this language's server stands right now.
    pub fn state(&self, def: &ServerDef) -> ServerState {
        if !self.is_enabled(def.id) {
            ServerState::Off
        } else if self.installed.get(def.id).copied().unwrap_or(false) {
            ServerState::On
        } else {
            ServerState::NotInstalled
        }
    }

    /// The rows Settings > Languages shows, one per language with a known server.
    pub fn rows(&self) -> Vec<LanguageServerRow> {
        servers::SERVERS
            .iter()
            .map(|s| LanguageServerRow {
                id: s.id,
                language: s.lang.name(),
                server: s.label,
                state: self.state(s),
            })
            .collect()
    }

    /// Flip one language's server on or off; turning it off stops it now.
    pub fn toggle(&mut self, id: &str) {
        if let Some(at) = self.enabled.iter().position(|e| e == id) {
            self.enabled.remove(at);
            if let Some(client) = self.clients.remove(id) {
                client.kill();
            }
        } else {
            self.enabled.push(id.to_string());
        }
        servers::save_enabled(&self.enabled);
    }

    /// The running server for `id`, if it is still alive.
    pub fn live(&mut self, id: &str) -> Option<Arc<Client>> {
        let alive = self.clients.get(id).is_some_and(|c| c.is_alive());
        if !alive {
            self.clients.remove(id);
            return None;
        }
        self.clients.get(id).cloned()
    }

    pub fn store(&mut self, id: &'static str, client: Arc<Client>) {
        self.clients.insert(id, client);
    }

    /// The server for `lang`, if it is on and installed.
    pub fn def_and_state(&self, lang: Lang) -> Result<&'static ServerDef, String> {
        let def = servers::for_lang(lang).ok_or("No language server is known for this file.")?;
        match self.state(def) {
            ServerState::On => Ok(def),
            ServerState::Off => Err(format!(
                "{} is off. Turn it on in Settings > Languages.",
                def.label
            )),
            ServerState::NotInstalled => Err(format!(
                "{} is not installed. Smithy uses servers already on your PATH.",
                def.label
            )),
        }
    }

    /// Stop every server: the add-on is being turned off, or the folder changed.
    pub fn shutdown_all(&mut self) {
        for (_, client) in self.clients.drain() {
            client.kill();
        }
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}
