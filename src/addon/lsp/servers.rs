//! The servers Smithy knows, found on the user's PATH — never bundled — and the
//! per-language on/off the user sets in Settings > Languages. Table from the
//! board's `settings-languages` frame.
use crate::editor::lang::Lang;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub struct ServerDef {
    /// Stable id, also the settings key for this language's server.
    pub id: &'static str,
    pub lang: Lang,
    /// `languageId` sent to the server in `didOpen`.
    pub language_id: &'static str,
    /// Executable looked up on PATH.
    pub command: &'static str,
    /// Shown in Settings > Languages (the design's own name for the server).
    pub label: &'static str,
    pub args: &'static [&'static str],
}

pub const SERVERS: &[ServerDef] = &[
    def(
        "rust",
        Lang::Rust,
        "rust",
        "rust-analyzer",
        "rust-analyzer",
        &[],
    ),
    def(
        "typescript",
        Lang::TypeScript,
        "typescript",
        "typescript-language-server",
        "typescript-language-server",
        &["--stdio"],
    ),
    def(
        "typescriptreact",
        Lang::TypeScriptReact,
        "typescriptreact",
        "typescript-language-server",
        "typescript-language-server",
        &["--stdio"],
    ),
    def(
        "javascript",
        Lang::JavaScript,
        "javascript",
        "typescript-language-server",
        "typescript-language-server",
        &["--stdio"],
    ),
    def(
        "javascriptreact",
        Lang::JavaScriptReact,
        "javascriptreact",
        "typescript-language-server",
        "typescript-language-server",
        &["--stdio"],
    ),
    def(
        "kotlin",
        Lang::Kotlin,
        "kotlin",
        "kotlin-language-server",
        "kotlin-language-server",
        &[],
    ),
    def("java", Lang::Java, "java", "jdtls", "jdtls", &[]),
    def(
        "python",
        Lang::Python,
        "python",
        "pyright-langserver",
        "pyright",
        &["--stdio"],
    ),
    def("go", Lang::Go, "go", "gopls", "gopls", &[]),
    def("c", Lang::C, "c", "clangd", "clangd", &[]),
    def("cpp", Lang::Cpp, "cpp", "clangd", "clangd", &[]),
];

const fn def(
    id: &'static str,
    lang: Lang,
    language_id: &'static str,
    command: &'static str,
    label: &'static str,
    args: &'static [&'static str],
) -> ServerDef {
    ServerDef {
        id,
        lang,
        language_id,
        command,
        label,
        args,
    }
}

pub fn for_lang(lang: Lang) -> Option<&'static ServerDef> {
    SERVERS.iter().find(|s| s.lang == lang)
}

/// The first of `name`, `name.exe`, `name.cmd`, `name.bat` that exists in a
/// directory of `path_var`. Pure so the lookup rule can be tested.
pub fn find_on_path(
    name: &str,
    path_var: &str,
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    for dir in std::env::split_paths(path_var) {
        for candidate in [
            name.to_string(),
            format!("{name}.exe"),
            format!("{name}.cmd"),
            format!("{name}.bat"),
        ] {
            let path = dir.join(candidate);
            if is_file(&path) {
                return Some(path);
            }
        }
    }
    None
}

/// Is `command` installed?
pub fn installed(command: &str) -> bool {
    let path_var = std::env::var("PATH").unwrap_or_default();
    find_on_path(command, &path_var, |p| p.is_file()).is_some()
}

/// The per-language choice, in `%APPDATA%\smithy\lsp.json` (the add-on's own
/// file; core settings stay core). Everything not listed is off.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Config {
    enabled: Vec<String>,
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("smithy").join("lsp.json"))
}

pub fn load_enabled() -> Vec<String> {
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Config>(&s).ok())
        .map(|c| c.enabled)
        .unwrap_or_default()
}

pub fn save_enabled(enabled: &[String]) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let config = Config {
        enabled: enabled.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_tries_the_extensions_windows_uses() {
        let path_var = r"C:\nowhere;C:\tools";
        let found = find_on_path("gopls", path_var, |p| p.ends_with(r"tools\gopls.exe"));
        assert_eq!(found, Some(PathBuf::from(r"C:\tools\gopls.exe")));
        assert_eq!(find_on_path("gopls", path_var, |_| false), None);
    }

    #[test]
    fn every_server_row_maps_to_a_language() {
        // A language with no server is not in the table at all.
        assert!(for_lang(Lang::Markdown).is_none());
        assert_eq!(for_lang(Lang::Rust).map(|s| s.id), Some("rust"));
        assert_eq!(
            for_lang(Lang::TypeScriptReact).map(|s| s.command),
            Some("typescript-language-server")
        );
        // One row per language, so a shared command cannot hide a language.
        let mut ids: Vec<&str> = SERVERS.iter().map(|s| s.id).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before);
    }
}
