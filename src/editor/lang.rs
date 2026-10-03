//! Language mode from the file extension (shown in the status bar).
use std::path::Path;

const TABLE: &[(&str, &str)] = &[
    ("rs", "Rust"),
    ("ts", "TypeScript"),
    ("tsx", "TypeScript React"),
    ("js", "JavaScript"),
    ("jsx", "JavaScript React"),
    ("mjs", "JavaScript"),
    ("json", "JSON"),
    ("toml", "TOML"),
    ("yml", "YAML"),
    ("yaml", "YAML"),
    ("md", "Markdown"),
    ("html", "HTML"),
    ("css", "CSS"),
    ("py", "Python"),
    ("go", "Go"),
    ("java", "Java"),
    ("kt", "Kotlin"),
    ("c", "C"),
    ("h", "C"),
    ("cpp", "C++"),
    ("hpp", "C++"),
    ("cs", "C#"),
    ("sh", "Shell"),
    ("ps1", "PowerShell"),
    ("sql", "SQL"),
    ("xml", "XML"),
    ("svg", "SVG"),
    ("dart", "Dart"),
];

pub fn language_for(path: &Path) -> &'static str {
    path.extension()
        .and_then(|e| e.to_str())
        .and_then(|e| TABLE.iter().find(|(x, _)| x.eq_ignore_ascii_case(e)))
        .map(|(_, name)| *name)
        .unwrap_or("Plain Text")
}

/// Built-in languages for Settings > Languages: the name and the extensions that
/// map to it, in the order `TABLE` declares them.
pub fn built_in() -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    for &(ext, name) in TABLE {
        match out.iter_mut().find(|(n, _)| *n == name) {
            Some((_, exts)) => exts.push_str(&format!(" .{ext}")),
            None => out.push((name, format!(".{ext}"))),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_extension_case_insensitive_else_plain_text() {
        assert_eq!(language_for(Path::new("A/B.TSX")), "TypeScript React");
        assert_eq!(language_for(Path::new("notes.xyz")), "Plain Text");
        assert_eq!(language_for(Path::new("Makefile")), "Plain Text");
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
