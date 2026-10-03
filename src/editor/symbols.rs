//! Symbol outline for one file, from a tree-sitter parse. Used by the Search
//! add-on; no language server, no state kept between calls.
use super::grammars;
use super::lang::Lang;
use std::path::Path;
use tree_sitter::{Node, Parser};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SymbolKind {
    Module,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Function,
    Method,
    Property,
    Field,
    Constant,
    Variable,
    Type,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// 0-based line and char column of the name.
    pub line: usize,
    pub col: usize,
}

/// Declarations in `text`, in file order. Empty for a language Smithy has no
/// outline rules for, or when the file does not parse.
pub fn symbols(path: &Path, text: &str) -> Vec<Symbol> {
    let Some(lang) = Lang::for_path(path) else {
        return Vec::new();
    };
    symbols_for(lang, text)
}

fn symbols_for(lang: Lang, text: &str) -> Vec<Symbol> {
    let mut parser = Parser::new();
    if parser.set_language(&grammars::ts_language(lang)).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(text.as_bytes(), None) else {
        return Vec::new();
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    walk(tree.root_node(), lang, text.as_bytes(), &lines, &mut out);
    out.sort_by_key(|s| (s.line, s.col));
    out.dedup_by(|a, b| a.line == b.line && a.name == b.name);
    out
}

fn walk<'t>(node: Node<'t>, lang: Lang, src: &[u8], lines: &[&str], out: &mut Vec<Symbol>) {
    if let Some(kind) = symbol_kind(lang, node.kind()) {
        if let Some(name) = name_node(node) {
            let text = name.utf8_text(src).unwrap_or_default().trim();
            if !text.is_empty() {
                let at = name.start_position();
                let line = lines.get(at.row).copied().unwrap_or_default();
                out.push(Symbol {
                    name: text.to_string(),
                    kind,
                    line: at.row,
                    col: char_col(line, at.column),
                });
            }
        }
    }
    // ponytail: recursion depth is bounded by the source's nesting; upgrade:
    // an explicit stack if a pathological file ever overflows.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, lang, src, lines, out);
    }
}

/// The node holding the declared name, following the field the grammars use.
fn name_node(node: Node) -> Option<Node> {
    if let Some(n) = node.child_by_field_name("name") {
        return Some(n);
    }
    if is_ident(node) {
        return Some(node);
    }
    if let Some(d) = node.child_by_field_name("declarator") {
        return name_node(d);
    }
    let mut cursor = node.walk();
    let mut found = None;
    for child in node.children(&mut cursor) {
        if is_ident(child) {
            found = Some(child);
            break;
        }
    }
    found
}

fn is_ident(node: Node) -> bool {
    matches!(
        node.kind(),
        "identifier"
            | "type_identifier"
            | "field_identifier"
            | "property_identifier"
            | "simple_identifier"
            | "constant_identifier"
            | "function_name"
            | "word"
    )
}

fn char_col(line: &str, byte_col: usize) -> usize {
    line.get(..byte_col)
        .map(|p| p.chars().count())
        .unwrap_or_else(|| line.chars().count())
}

/// Declarations worth listing, per language. Languages without an entry here
/// (CSS, HTML, JSON, Markdown, SQL, TOML, XML, YAML) have no outline yet.
fn symbol_kind(lang: Lang, kind: &str) -> Option<SymbolKind> {
    use Lang::*;
    use SymbolKind::*;
    Some(match (lang, kind) {
        (Rust, "function_item" | "macro_definition") => Function,
        (Rust, "struct_item") => Struct,
        (Rust, "enum_item") => Enum,
        (Rust, "trait_item") => Trait,
        (Rust, "mod_item") => Module,
        (Rust, "const_item" | "static_item") => Constant,
        (Rust, "type_item") => Type,
        (Rust, "field_declaration") => Field,

        (C, "function_definition") => Function,
        (C | Cpp, "struct_specifier" | "union_specifier") => Struct,
        (C | Cpp, "enum_specifier") => Enum,
        (C | Cpp, "type_definition") => Type,
        (C | Cpp, "enumerator") => Constant,
        (Cpp, "class_specifier") => Class,
        (Cpp, "namespace_definition") => Module,

        (CSharp, "class_declaration" | "record_declaration") => Class,
        (CSharp, "interface_declaration") => Interface,
        (CSharp, "struct_declaration") => Struct,
        (CSharp, "enum_declaration") => Enum,
        (CSharp, "namespace_declaration") => Module,
        (CSharp, "method_declaration" | "constructor_declaration") => Method,
        (CSharp, "property_declaration") => Property,
        (CSharp, "field_declaration") => Field,

        (Java, "class_declaration" | "record_declaration") => Class,
        (Java, "interface_declaration" | "annotation_type_declaration") => Interface,
        (Java, "enum_declaration") => Enum,
        (Java, "method_declaration" | "constructor_declaration") => Method,
        (Java, "field_declaration") => Field,

        (JavaScript | JavaScriptReact | TypeScript | TypeScriptReact, "function_declaration") => {
            Function
        }
        (
            JavaScript | JavaScriptReact | TypeScript | TypeScriptReact,
            "class_declaration" | "abstract_class_declaration",
        ) => Class,
        (JavaScript | JavaScriptReact | TypeScript | TypeScriptReact, "method_definition") => {
            Method
        }
        (TypeScript | TypeScriptReact, "interface_declaration") => Interface,
        (TypeScript | TypeScriptReact, "type_alias_declaration") => Type,
        (JavaScript | JavaScriptReact | TypeScript | TypeScriptReact, "enum_declaration") => Enum,
        (
            JavaScript | JavaScriptReact | TypeScript | TypeScriptReact,
            "public_field_definition",
        ) => Field,

        (Go, "function_declaration") => Function,
        (Go, "method_declaration") => Method,
        (Go, "type_spec") => Type,
        (Go, "const_spec") => Constant,
        (Go, "var_spec") => Variable,

        (Python, "function_definition") => Function,
        (Python, "class_definition") => Class,

        (Kotlin, "function_declaration") => Function,
        (Kotlin, "class_declaration" | "object_declaration") => Class,
        (Kotlin, "property_declaration") => Property,
        (Kotlin, "type_alias") => Type,

        (Dart, "function_declaration" | "function_signature") => Function,
        (Dart, "method_declaration" | "method_signature") => Method,
        (Dart, "class_declaration" | "mixin_declaration" | "extension_declaration") => Class,
        (Dart, "enum_declaration") => Enum,

        (Shell, "function_definition") => Function,
        (PowerShell, "function_statement") => Function,
        (PowerShell, "class_statement") => Class,

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[Symbol]) -> Vec<&str> {
        v.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn rust_outline_has_nested_and_top_level_declarations() {
        let src = "mod a {\n    pub fn inner() {}\n}\npub struct S;\nfn top() {}\n";
        let got = symbols(Path::new("x.rs"), src);
        assert_eq!(names(&got), vec!["a", "inner", "S", "top"]);
        assert_eq!(got[0].kind, SymbolKind::Module);
        assert_eq!(got[1].kind, SymbolKind::Function);
        assert_eq!(got[1].line, 1);
        assert_eq!(got[1].col, 11);
    }

    #[test]
    fn python_and_typescript_outlines() {
        let py = symbols(
            Path::new("m.py"),
            "class A:\n    def go(self):\n        pass\ndef top():\n    pass\n",
        );
        assert_eq!(names(&py), vec!["A", "go", "top"]);
        assert_eq!(py[0].kind, SymbolKind::Class);
        assert_eq!(py[1].kind, SymbolKind::Function);

        let ts = symbols(
            Path::new("m.ts"),
            "interface I {}\ntype T = number;\nexport function f() {}\nclass C {}\n",
        );
        assert_eq!(names(&ts), vec!["I", "T", "f", "C"]);
        assert_eq!(ts[0].kind, SymbolKind::Interface);
        assert_eq!(ts[1].kind, SymbolKind::Type);
        assert_eq!(ts[3].kind, SymbolKind::Class);
    }

    #[test]
    fn c_follows_the_declarator_to_the_name() {
        let got = symbols(
            Path::new("m.c"),
            "struct P { int x; };\nint add(int a) { return a; }\n",
        );
        assert_eq!(names(&got), vec!["P", "add"]);
        assert_eq!(got[1].kind, SymbolKind::Function);
        assert_eq!(got[1].line, 1);
    }

    #[test]
    fn unknown_extension_and_unoutlined_language_are_empty() {
        assert!(symbols(Path::new("a.xyz"), "abc").is_empty());
        assert!(symbols(Path::new("a.json"), "{\"a\": 1}").is_empty());
    }
}
