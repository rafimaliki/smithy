//! Which tree-sitter grammar and highlight query colors which language.
//! One place to change when a grammar is added or swapped.
use super::lang::Lang;
use tree_sitter::Language as TsLanguage;

/// The grammar for a language. Loading is lazy: this is only called when a file
/// of that type is opened.
pub fn ts_language(lang: Lang) -> TsLanguage {
    match lang {
        Lang::C => tree_sitter_c::LANGUAGE.into(),
        Lang::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        Lang::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        Lang::Css => tree_sitter_css::LANGUAGE.into(),
        Lang::Dart => tree_sitter_dart::LANGUAGE.into(),
        Lang::Go => tree_sitter_go::LANGUAGE.into(),
        Lang::Html => tree_sitter_html::LANGUAGE.into(),
        Lang::Java => tree_sitter_java::LANGUAGE.into(),
        Lang::JavaScript | Lang::JavaScriptReact => tree_sitter_javascript::LANGUAGE.into(),
        Lang::Json => tree_sitter_json::LANGUAGE.into(),
        Lang::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        Lang::Markdown => tree_sitter_md::LANGUAGE.into(),
        Lang::PowerShell => tree_sitter_powershell::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Shell => tree_sitter_bash::LANGUAGE.into(),
        Lang::Sql => tree_sitter_sequel::LANGUAGE.into(),
        Lang::Svg | Lang::Xml => tree_sitter_xml::LANGUAGE_XML.into(),
        Lang::Toml => tree_sitter_toml_ng::LANGUAGE.into(),
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Lang::TypeScriptReact => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Lang::Yaml => tree_sitter_yaml::LANGUAGE.into(),
    }
}

/// The highlight query for a language, as one source string.
///
/// TypeScript and TSX are supersets of JavaScript, but their own query only
/// covers the TypeScript-specific nodes, so the JavaScript query is prepended
/// (the same `inherits` arrangement the grammar repos use).
pub fn highlight_query(lang: Lang) -> String {
    let js = tree_sitter_javascript::HIGHLIGHT_QUERY;
    let jsx = tree_sitter_javascript::JSX_HIGHLIGHT_QUERY;
    let join = |parts: &[&str]| parts.join("\n");
    match lang {
        Lang::C => tree_sitter_c::HIGHLIGHT_QUERY.into(),
        Lang::CSharp => tree_sitter_c_sharp::HIGHLIGHTS_QUERY.into(),
        // C++ is a superset of C; its own query covers only the C++-specific
        // nodes, so the C query supplies comments, keywords, strings and types.
        Lang::Cpp => join(&[
            tree_sitter_c::HIGHLIGHT_QUERY,
            tree_sitter_cpp::HIGHLIGHT_QUERY,
        ]),
        Lang::Css => tree_sitter_css::HIGHLIGHTS_QUERY.into(),
        Lang::Dart => tree_sitter_dart::HIGHLIGHTS_QUERY.into(),
        Lang::Go => tree_sitter_go::HIGHLIGHTS_QUERY.into(),
        Lang::Html => tree_sitter_html::HIGHLIGHTS_QUERY.into(),
        Lang::Java => tree_sitter_java::HIGHLIGHTS_QUERY.into(),
        Lang::JavaScript => js.into(),
        Lang::JavaScriptReact => join(&[js, jsx]),
        Lang::Json => tree_sitter_json::HIGHLIGHTS_QUERY.into(),
        // The Kotlin grammar ships no query, so Smithy carries a small one.
        Lang::Kotlin => KOTLIN_QUERY.into(),
        // ponytail: the block grammar only, so inline emphasis is plain; upgrade:
        // a second parse with INLINE_LANGUAGE over the block grammar's inline ranges.
        Lang::Markdown => tree_sitter_md::HIGHLIGHT_QUERY_BLOCK.into(),
        Lang::PowerShell => tree_sitter_powershell::HIGHLIGHTS_QUERY.into(),
        Lang::Python => tree_sitter_python::HIGHLIGHTS_QUERY.into(),
        Lang::Rust => tree_sitter_rust::HIGHLIGHTS_QUERY.into(),
        Lang::Shell => tree_sitter_bash::HIGHLIGHT_QUERY.into(),
        Lang::Sql => tree_sitter_sequel::HIGHLIGHTS_QUERY.into(),
        Lang::Svg | Lang::Xml => tree_sitter_xml::XML_HIGHLIGHT_QUERY.into(),
        Lang::Toml => tree_sitter_toml_ng::HIGHLIGHTS_QUERY.into(),
        Lang::TypeScript => join(&[js, tree_sitter_typescript::HIGHLIGHTS_QUERY]),
        Lang::TypeScriptReact => join(&[js, jsx, tree_sitter_typescript::HIGHLIGHTS_QUERY]),
        Lang::Yaml => tree_sitter_yaml::HIGHLIGHTS_QUERY.into(),
    }
}

/// `tree-sitter-kotlin-ng` has no query in its crate, so this is Smithy's own.
const KOTLIN_QUERY: &str = r#"
(line_comment) @comment
(block_comment) @comment
(shebang) @comment
(string_literal) @string
(multiline_string_literal) @string
(character_literal) @string
(escape_sequence) @string.escape
(function_declaration (identifier) @function)
(class_declaration (identifier) @type)
(object_declaration (identifier) @type)
(type_alias (identifier) @type)
(user_type (identifier) @type)
[
  "abstract" "actual" "annotation" "as" "by" "catch" "class" "companion" "const"
  "constructor" "crossinline" "data" "delegate" "do" "dynamic" "else" "enum"
  "expect" "external" "field" "file" "final" "finally" "for" "fun" "get" "if"
  "import" "in" "infix" "init" "inline" "inner" "interface" "internal" "is"
  "lateinit" "noinline" "object" "open" "operator" "out" "override" "package"
  "param" "private" "property" "protected" "public" "receiver" "return" "sealed"
  "set" "setparam" "super" "suspend" "tailrec" "this" "throw" "try" "typealias"
  "val" "value" "var" "vararg" "when" "where" "while"
] @keyword
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_a_query() {
        for lang in Lang::ALL {
            assert!(
                !highlight_query(lang).trim().is_empty(),
                "{} has no highlight query",
                lang.name()
            );
        }
    }
}
