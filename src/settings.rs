//! Local settings, stored as JSON in `%APPDATA%\smithy\settings.json`.
//! Never holds secrets (the GitHub token lives in the Windows Credential Manager).
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub font_size: f32,
    pub sidebar_width: f32,
    pub word_wrap: bool,
    /// "brand" or "simple".
    pub file_icons: String,
    /// Diff layout default: "inline" or "split".
    pub diff_layout: String,
    /// Ids of enabled add-ons. Everything not listed is off and never started.
    pub addons: Vec<String>,
    /// Most recent first, capped at `MAX_RECENT`.
    pub recent: Vec<PathBuf>,
    /// GitHub account login from the last successful Connect. The token itself
    /// is only ever in the Windows Credential Manager.
    pub github_login: String,
    /// Action name -> keystroke override, e.g. "ToggleSidebar" -> "ctrl-b".
    pub shortcuts: Vec<(String, String)>,
}

impl gpui::Global for Settings {}

pub const MAX_RECENT: usize = 8;
pub const SIDEBAR_MIN: f32 = 180.0;
pub const SIDEBAR_MAX: f32 = 600.0;
pub const SIDEBAR_DEFAULT: f32 = 300.0;

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "Nord Dark".into(),
            font_size: 13.0,
            sidebar_width: SIDEBAR_DEFAULT,
            word_wrap: false,
            file_icons: "brand".into(),
            diff_layout: "inline".into(),
            addons: Vec::new(),
            recent: Vec::new(),
            github_login: String::new(),
            shortcuts: Vec::new(),
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("smithy").join("settings.json"))
}

impl Settings {
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, json);
        }
    }

    pub fn addon_enabled(&self, id: &str) -> bool {
        self.addons.iter().any(|a| a == id)
    }

    pub fn push_recent(&mut self, folder: PathBuf) {
        self.recent.retain(|p| p != &folder);
        self.recent.insert(0, folder);
        self.recent.truncate(MAX_RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_is_deduped_capped_and_most_recent_first() {
        let mut s = Settings::default();
        for i in 0..12 {
            s.push_recent(PathBuf::from(format!("c:/p{i}")));
        }
        s.push_recent(PathBuf::from("c:/p5"));
        assert_eq!(s.recent.len(), MAX_RECENT);
        assert_eq!(s.recent[0], PathBuf::from("c:/p5"));
        assert_eq!(
            s.recent
                .iter()
                .filter(|p| p.as_path() == std::path::Path::new("c:/p5"))
                .count(),
            1
        );
    }

    #[test]
    fn unknown_or_partial_json_falls_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"theme":"Daylight"}"#).unwrap();
        assert_eq!(s.theme, "Daylight");
        assert_eq!(s.sidebar_width, SIDEBAR_DEFAULT);
    }
}
