//! File icons: brand by file type, or one outlined mark, per `Settings::file_icons`.
//! The brand list is the app-defined one on the board's `icons` page: a file name
//! match first (`package.json`), then an extension.
//!
//! Brand colors are the marks' own colors, not theme tokens. Three marks are black
//! in Simple Icons, so they use a lighter tint to show on a dark theme (Markdown's
//! tint is the one the board draws).
use gpui::{rgb, Rgba};
use std::path::Path;

pub const FILE: &str = "icons/file.svg";
pub const FOLDER: &str = "icons/folder.svg";
pub const FOLDER_FILLED: &str = "icons/folder-filled.svg";

pub enum Mark {
    /// A brand mark and its own tint.
    Brand { asset: &'static str, color: Rgba },
    /// A folder: a filled accent folder in brand mode, an outline in simple mode.
    Folder,
    /// The generic file outline, drawn in the theme's mute color.
    File,
}

/// A file name matched exactly (lowercase).
const NAMES: &[(&str, &str, u32)] = &[
    ("package.json", "icons/npm.svg", 0xCB3837),
    ("tsconfig.json", "icons/typescript.svg", 0x3178C6),
    (".gitignore", "icons/git.svg", 0xF03C2E),
];

/// An extension matched case-insensitively (no dot).
const EXTENSIONS: &[(&str, &str, u32)] = &[
    ("tsx", "icons/react.svg", 0x61DAFB),
    ("ts", "icons/typescript.svg", 0x3178C6),
    ("kt", "icons/kotlin.svg", 0x7F52FF),
    // Rust, JSON and Markdown are black marks on the board; lighter tints here.
    ("rs", "icons/rust.svg", 0xDEA584),
    ("json", "icons/json.svg", 0xCBCB41),
    ("md", "icons/markdown.svg", 0x519ABA),
    ("yml", "icons/yaml.svg", 0xCB171E),
    ("yaml", "icons/yaml.svg", 0xCB171E),
    ("toml", "icons/toml.svg", 0x9C4121),
];

pub fn mark(path: &Path, is_dir: bool) -> Mark {
    if is_dir {
        return Mark::Folder;
    }
    by_name(path).unwrap_or(Mark::File)
}

fn by_name(path: &Path) -> Option<Mark> {
    if is_actions_workflow(path) {
        return Some(brand("icons/github-actions.svg", 0x2088FF));
    }
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if let Some((_, asset, color)) = NAMES.iter().find(|(n, _, _)| *n == name) {
        return Some(brand(asset, *color));
    }
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    let (_, asset, color) = EXTENSIONS.iter().find(|(e, _, _)| *e == ext)?;
    Some(brand(asset, *color))
}

/// `ci.yml` inside `.github/workflows`, the one workflow the board names.
fn is_actions_workflow(path: &Path) -> bool {
    let named = |p: Option<&Path>, name: &str| {
        p.and_then(|p| p.file_name())
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
    };
    let dir = path.parent();
    named(Some(path), "ci.yml")
        && named(dir, "workflows")
        && named(dir.and_then(|d| d.parent()), ".github")
}

fn brand(asset: &'static str, color: u32) -> Mark {
    Mark::Brand {
        asset,
        color: rgb(color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset_of(p: &str) -> &'static str {
        match mark(Path::new(p), false) {
            Mark::Brand { asset, .. } => asset,
            Mark::Folder => FOLDER_FILLED,
            Mark::File => FILE,
        }
    }

    #[test]
    fn brand_by_extension_and_file_name() {
        assert_eq!(asset_of("a/Button.tsx"), "icons/react.svg");
        assert_eq!(asset_of("A/B.TS"), "icons/typescript.svg");
        assert_eq!(asset_of("x.kt"), "icons/kotlin.svg");
        assert_eq!(asset_of("x.rs"), "icons/rust.svg");
        assert_eq!(asset_of("x.toml"), "icons/toml.svg");
        assert_eq!(asset_of("README.md"), "icons/markdown.svg");
        // A file name wins over the extension: json alone is JSON, not npm.
        assert_eq!(asset_of("package.json"), "icons/npm.svg");
        assert_eq!(asset_of("tsconfig.json"), "icons/typescript.svg");
        assert_eq!(asset_of("app.json"), "icons/json.svg");
        assert_eq!(asset_of(".gitignore"), "icons/git.svg");
    }

    #[test]
    fn actions_workflow_needs_the_whole_path() {
        assert_eq!(
            asset_of(".github/workflows/ci.yml"),
            "icons/github-actions.svg"
        );
        assert_eq!(asset_of("other/ci.yml"), "icons/yaml.svg");
    }

    #[test]
    fn unknown_and_folders_are_generic() {
        assert_eq!(asset_of("notes.xyz"), FILE);
        assert_eq!(asset_of("noext"), FILE);
        assert!(matches!(mark(Path::new("src"), true), Mark::Folder));
        // The extension never wins over the fact that it is a folder.
        assert!(matches!(mark(Path::new("src.rs"), true), Mark::Folder));
    }
}
