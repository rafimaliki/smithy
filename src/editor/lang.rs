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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_extension_case_insensitive_else_plain_text() {
        assert_eq!(language_for(Path::new("A/B.TSX")), "TypeScript React");
        assert_eq!(language_for(Path::new("notes.xyz")), "Plain Text");
        assert_eq!(language_for(Path::new("Makefile")), "Plain Text");
    }
}
