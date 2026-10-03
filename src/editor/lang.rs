//! Language modes: the 24 built-in languages, chosen by file extension.
//! The status bar shows one of these; the picker overrides it for a single file.
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    C,
    CSharp,
    Cpp,
    Css,
    Dart,
    Go,
    Html,
    Java,
    JavaScript,
    JavaScriptReact,
    Json,
    Kotlin,
    Markdown,
    PowerShell,
    Python,
    Rust,
    Shell,
    Sql,
    Svg,
    Toml,
    TypeScript,
    TypeScriptReact,
    Xml,
    Yaml,
}

impl Lang {
    /// Every language the picker offers, in display order.
    pub const ALL: [Lang; 24] = [
        Lang::C,
        Lang::CSharp,
        Lang::Cpp,
        Lang::Css,
        Lang::Dart,
        Lang::Go,
        Lang::Html,
        Lang::Java,
        Lang::JavaScript,
        Lang::JavaScriptReact,
        Lang::Json,
        Lang::Kotlin,
        Lang::Markdown,
        Lang::PowerShell,
        Lang::Python,
        Lang::Rust,
        Lang::Shell,
        Lang::Sql,
        Lang::Svg,
        Lang::Toml,
        Lang::TypeScript,
        Lang::TypeScriptReact,
        Lang::Xml,
        Lang::Yaml,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Lang::C => "C",
            Lang::CSharp => "C#",
            Lang::Cpp => "C++",
            Lang::Css => "CSS",
            Lang::Dart => "Dart",
            Lang::Go => "Go",
            Lang::Html => "HTML",
            Lang::Java => "Java",
            Lang::JavaScript => "JavaScript",
            Lang::JavaScriptReact => "JavaScript React",
            Lang::Json => "JSON",
            Lang::Kotlin => "Kotlin",
            Lang::Markdown => "Markdown",
            Lang::PowerShell => "PowerShell",
            Lang::Python => "Python",
            Lang::Rust => "Rust",
            Lang::Shell => "Shell",
            Lang::Sql => "SQL",
            Lang::Svg => "SVG",
            Lang::Toml => "TOML",
            Lang::TypeScript => "TypeScript",
            Lang::TypeScriptReact => "TypeScript React",
            Lang::Xml => "XML",
            Lang::Yaml => "YAML",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Lang> {
        let e = ext.to_ascii_lowercase();
        Some(match e.as_str() {
            "c" | "h" => Lang::C,
            "cs" => Lang::CSharp,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" => Lang::Cpp,
            "css" => Lang::Css,
            "dart" => Lang::Dart,
            "go" => Lang::Go,
            "html" | "htm" => Lang::Html,
            "java" => Lang::Java,
            "js" | "mjs" | "cjs" => Lang::JavaScript,
            "jsx" => Lang::JavaScriptReact,
            "json" => Lang::Json,
            "kt" | "kts" => Lang::Kotlin,
            "md" | "markdown" => Lang::Markdown,
            "ps1" => Lang::PowerShell,
            "py" => Lang::Python,
            "rs" => Lang::Rust,
            "sh" | "bash" => Lang::Shell,
            "sql" => Lang::Sql,
            "svg" => Lang::Svg,
            "toml" => Lang::Toml,
            "ts" => Lang::TypeScript,
            "tsx" => Lang::TypeScriptReact,
            "xml" => Lang::Xml,
            "yml" | "yaml" => Lang::Yaml,
            _ => return None,
        })
    }

    pub fn for_path(path: &Path) -> Option<Lang> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Lang::from_extension)
    }

    /// Shown in the status bar and the picker's header.
    pub fn display(lang: Option<Lang>) -> &'static str {
        lang.map(Lang::name).unwrap_or("Plain Text")
    }
}

/// Every extension `Lang::from_extension` knows, for Settings > Languages.
const EXTENSIONS: &[&str] = &[
    "c", "h", "cs", "cpp", "cc", "cxx", "hpp", "hh", "css", "dart", "go", "html", "htm", "java",
    "js", "mjs", "cjs", "jsx", "json", "kt", "kts", "md", "markdown", "ps1", "py", "rs", "sh",
    "bash", "sql", "svg", "toml", "ts", "tsx", "xml", "yml", "yaml",
];

/// Built-in languages for Settings > Languages: the name and the extensions that
/// map to it, in picker order.
pub fn built_in() -> Vec<(&'static str, String)> {
    Lang::ALL
        .iter()
        .map(|&lang| {
            let exts: Vec<String> = EXTENSIONS
                .iter()
                .filter(|e| Lang::from_extension(e) == Some(lang))
                .map(|e| format!(".{e}"))
                .collect();
            (lang.name(), exts.join(" "))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_extension_case_insensitive_else_none() {
        assert_eq!(
            Lang::for_path(Path::new("A/B.TSX")),
            Some(Lang::TypeScriptReact)
        );
        assert_eq!(Lang::for_path(Path::new("x.rs")), Some(Lang::Rust));
        assert_eq!(Lang::for_path(Path::new("main.kt")), Some(Lang::Kotlin));
        assert_eq!(Lang::for_path(Path::new("notes.xyz")), None);
        assert_eq!(Lang::for_path(Path::new("Makefile")), None);
    }

    #[test]
    fn display_falls_back_to_plain_text() {
        assert_eq!(Lang::display(None), "Plain Text");
        assert_eq!(Lang::display(Some(Lang::Cpp)), "C++");
    }

    #[test]
    fn all_languages_are_listed_once_with_a_name() {
        assert_eq!(Lang::ALL.len(), 24);
        for (i, a) in Lang::ALL.iter().enumerate() {
            for b in &Lang::ALL[i + 1..] {
                assert_ne!(a, b, "duplicate in ALL");
                assert_ne!(a.name(), b.name(), "duplicate name");
            }
        }
    }

    #[test]
    fn built_in_groups_extensions_under_one_language() {
        let langs = built_in();
        let (_, exts) = langs.iter().find(|(n, _)| *n == "TypeScript").unwrap();
        assert_eq!(exts, ".ts");
        let (_, exts) = langs.iter().find(|(n, _)| *n == "YAML").unwrap();
        assert_eq!(exts, ".yml .yaml");
        // Every language maps at least one extension, and none appears twice.
        let mut names: Vec<&str> = langs.iter().map(|(n, _)| *n).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
        assert!(langs.iter().all(|(_, e)| !e.is_empty()));
    }
}
