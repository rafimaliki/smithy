//! Tree-sitter highlighting. One parse per edit (incremental after the first),
//! then the highlight query is run once over the tree to collect colored spans.
//! Rendering only reads the spans of the visible lines.
use super::grammars;
use super::lang::Lang;
use crate::theme::Theme;
use gpui::Rgba;
use std::ops::Range;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator, Tree};

/// The five colors the editor knows. Everything else draws in the text color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Keyword,
    Str,
    Func,
    Ty,
    Comment,
}

pub fn color(kind: Kind, theme: &Theme) -> Rgba {
    match kind {
        Kind::Keyword => theme.kw,
        Kind::Str => theme.str,
        Kind::Func => theme.func,
        Kind::Ty => theme.ty,
        Kind::Comment => theme.comment,
    }
}

/// A colored run on one line, in char columns. Spans for a line are ordered
/// outer first, so a later span overrides an earlier one where they overlap.
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub line: u32,
    pub start: u32,
    pub end: u32,
    pub kind: Kind,
}

/// A syntax error on one line, in char columns: tree-sitter's ERROR node, or a MISSING
/// token (drawn one column wide).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ErrorSpan {
    pub line: u32,
    pub start: u32,
    pub end: u32,
}

/// More than this and the file is mostly broken; the rest are not collected.
const MAX_ERRORS: usize = 500;

pub struct Highlighter {
    parser: Parser,
    query: Query,
    /// Capture index -> color, or `None` for captures that stay plain.
    capture_kinds: Vec<Option<Kind>>,
    cursor: QueryCursor,
    tree: Option<Tree>,
    spans: Vec<Span>,
    /// Filled only while the Error highlighting add-on is on.
    collect_errors: bool,
    errors: Vec<ErrorSpan>,
}

impl Highlighter {
    /// `None` when the grammar has no query that compiles, which means plain text.
    pub fn new(lang: Lang) -> Option<Self> {
        let language = grammars::ts_language(lang);
        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() {
            return None;
        }
        let query = Query::new(&language, &grammars::highlight_query(lang)).ok()?;
        let capture_kinds = query
            .capture_names()
            .iter()
            .map(|name| kind_of(name))
            .collect();
        Some(Self {
            parser,
            query,
            capture_kinds,
            cursor: QueryCursor::new(),
            tree: None,
            spans: Vec::new(),
            collect_errors: false,
            errors: Vec::new(),
        })
    }

    /// Turn error collection on or off; the next `update` fills or clears the list.
    pub fn set_collect_errors(&mut self, on: bool) {
        self.collect_errors = on;
        if !on {
            self.errors.clear();
        }
    }

    pub fn errors(&self) -> &[ErrorSpan] {
        &self.errors
    }

    /// The errors that start on `line`.
    pub fn errors_on_line(&self, line: usize) -> &[ErrorSpan] {
        let line = line as u32;
        let lo = self.errors.partition_point(|e| e.line < line);
        let hi = self.errors.partition_point(|e| e.line <= line);
        &self.errors[lo..hi]
    }

    /// Re-parse and re-collect the spans.
    // ponytail: a full reparse per edit. Reusing the old tree is only valid after
    // `Tree::edit` tells it what changed, which the buffer does not report; without
    // that it reuses stale nodes (wrong colors, phantom errors). Upgrade: have
    // `Buffer::replace` return an `InputEdit` and pass the edited old tree.
    pub fn update(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let Some(tree) = self.parser.parse(bytes, None) else {
            self.spans.clear();
            return;
        };
        self.spans.clear();
        let lines: Vec<&str> = text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .collect();
        let mut matches = self.cursor.matches(&self.query, tree.root_node(), bytes);
        while let Some(m) = matches.next() {
            for capture in m.captures() {
                let Some(Some(kind)) = self.capture_kinds.get(capture.index as usize) else {
                    continue;
                };
                let kind = *kind;
                let start = capture.node.start_position();
                let end = capture.node.end_position();
                for row in start.row..=end.row {
                    let Some(line) = lines.get(row) else { break };
                    let len = line.chars().count();
                    let from = if row == start.row {
                        char_col(line, start.column)
                    } else {
                        0
                    };
                    let to = if row == end.row {
                        char_col(line, end.column)
                    } else {
                        len
                    };
                    if to > from {
                        self.spans.push(Span {
                            line: row as u32,
                            start: from as u32,
                            end: to.min(len) as u32,
                            kind,
                        });
                    }
                }
            }
        }
        self.spans
            .sort_unstable_by_key(|s| (s.line, s.start, std::cmp::Reverse(s.end)));
        self.errors = if self.collect_errors {
            collect_errors(tree.root_node(), &lines)
        } else {
            Vec::new()
        };
        self.tree = Some(tree);
    }

    /// The spans of one line, outer first. Empty when the line has none.
    pub fn line_spans(&self, line: usize) -> &[Span] {
        let line = line as u32;
        let lo = self.spans.partition_point(|s| s.line < line);
        let hi = self.spans.partition_point(|s| s.line <= line);
        &self.spans[lo..hi]
    }
}

/// ERROR and MISSING nodes of a tree, outermost first, in line order.
fn collect_errors(root: Node, lines: &[&str]) -> Vec<ErrorSpan> {
    let mut out = Vec::new();
    if !root.has_error() {
        return out;
    }
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if out.len() >= MAX_ERRORS {
            break;
        }
        if node.is_error() || node.is_missing() {
            let (start, end) = (node.start_position(), node.end_position());
            let Some(line) = lines.get(start.row) else {
                continue;
            };
            let len = line.chars().count();
            let from = char_col(line, start.column).min(len);
            let to = if end.row == start.row {
                char_col(line, end.column).min(len)
            } else {
                len
            };
            // A missing token is zero wide: underline the column it belongs at.
            let to = to.max(from + 1).min(len.max(from + 1));
            out.push(ErrorSpan {
                line: start.row as u32,
                start: from as u32,
                end: to as u32,
            });
            continue;
        }
        if node.has_error() {
            let mut cursor = node.walk();
            let children: Vec<Node> = node.children(&mut cursor).collect();
            stack.extend(children.into_iter().rev());
        }
    }
    out.sort_unstable_by_key(|e| (e.line, e.start));
    out
}

/// Byte column (tree-sitter's counting) to char column, clamped to the line.
fn char_col(line: &str, byte_col: usize) -> usize {
    line.get(..byte_col)
        .map(|prefix| prefix.chars().count())
        .unwrap_or_else(|| line.chars().count())
}

/// Which theme color a capture name means. Names are the conventional ones from
/// the grammars' `highlights.scm`; anything unknown stays plain text.
fn kind_of(name: &str) -> Option<Kind> {
    match name {
        // Names that do not follow the head convention.
        "text.title" | "markup.heading" => return Some(Kind::Keyword),
        "text.literal" | "text.uri" | "text.escape" => return Some(Kind::Str),
        "text.reference" | "markup.link" => return Some(Kind::Func),
        "none" | "spell" => return None,
        _ => {}
    }
    match name.split('.').next().unwrap_or_default() {
        "keyword" | "conditional" | "repeat" | "exception" | "include" | "storageclass"
        | "modifier" | "tag" => Some(Kind::Keyword),
        "string" | "character" | "char" | "escape" => Some(Kind::Str),
        "function" | "method" | "constructor" => Some(Kind::Func),
        "type" | "namespace" => Some(Kind::Ty),
        "comment" => Some(Kind::Comment),
        _ => None,
    }
}

/// The colored runs of a line, in order and non-overlapping. `spans` must be
/// outer-first (as `Highlighter::line_spans` returns them); where they nest the
/// inner one wins.
///
// ponytail: one byte per char of the visible line, skipped past 10k chars
// (minified files), where coloring is worth less than the allocation; upgrade:
// interval merging over the sorted spans.
pub fn segments(line_len: usize, spans: &[Span]) -> Vec<(Range<usize>, Kind)> {
    if spans.is_empty() || line_len == 0 || line_len > 10_000 {
        return Vec::new();
    }
    let mut kinds = vec![None; line_len];
    for s in spans {
        let start = (s.start as usize).min(line_len);
        let end = (s.end as usize).min(line_len);
        for slot in &mut kinds[start..end] {
            *slot = Some(s.kind);
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < kinds.len() {
        let Some(kind) = kinds[i] else {
            i += 1;
            continue;
        };
        let start = i;
        while i < kinds.len() && kinds[i] == Some(kind) {
            i += 1;
        }
        out.push((start..i, kind));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(line: u32, start: u32, end: u32, kind: Kind) -> Span {
        Span {
            line,
            start,
            end,
            kind,
        }
    }

    #[test]
    fn segments_skip_plain_gaps_and_let_inner_win() {
        // "let x = \"s\";": keyword 0..3, string 8..11, type nested inside it 9..10.
        let spans = [
            span(0, 0, 3, Kind::Keyword),
            span(0, 8, 11, Kind::Str),
            span(0, 9, 10, Kind::Ty),
        ];
        assert_eq!(
            segments(12, &spans),
            vec![
                (0..3, Kind::Keyword),
                (8..9, Kind::Str),
                (9..10, Kind::Ty),
                (10..11, Kind::Str),
            ]
        );
    }

    #[test]
    fn segments_clamp_and_ignore_long_lines() {
        assert_eq!(
            segments(3, &[span(0, 1, 99, Kind::Str)]),
            vec![(1..3, Kind::Str)]
        );
        assert!(segments(20_000, &[span(0, 0, 5, Kind::Str)]).is_empty());
        assert!(segments(5, &[]).is_empty());
    }

    /// Every language's query must compile against its grammar, and a trivial
    /// snippet must highlight something. A bad node kind in a query fails here,
    /// at `Highlighter::new`, rather than silently going plain in the app.
    #[test]
    fn every_language_builds_and_highlights_its_hello_world() {
        let samples: [(&str, Lang); 24] = [
            ("int main() { return 0; } // c", Lang::C),
            ("class A { void B() {} } // c#", Lang::CSharp),
            ("int main() { return 0; } // c++", Lang::Cpp),
            ("/* c */\n.a { color: red; }", Lang::Css),
            ("void main() { var x = 1; } // dart", Lang::Dart),
            ("package main\nfunc main() {}\n", Lang::Go),
            ("<p class=\"a\">hi</p>", Lang::Html),
            ("class A { void b() {} }", Lang::Java),
            ("function f() { return \"s\"; }", Lang::JavaScript),
            ("const A = () => <p>hi</p>;", Lang::JavaScriptReact),
            ("{\"a\": 1}", Lang::Json),
            ("fun main() { val x = 1 }", Lang::Kotlin),
            ("# Title\n\ntext\n", Lang::Markdown),
            ("function Get-Thing { param($x) } # ps", Lang::PowerShell),
            ("def f():\n    return \"s\"\n", Lang::Python),
            ("fn main() { let x = 1; } // rust", Lang::Rust),
            ("echo \"hi\" # sh\n", Lang::Shell),
            ("SELECT a FROM t WHERE a = 1;", Lang::Sql),
            ("<svg><rect/></svg>", Lang::Svg),
            ("a = \"b\" # c\n", Lang::Toml),
            ("const a: number = 1;", Lang::TypeScript),
            ("const A = (): number => 1;", Lang::TypeScriptReact),
            ("<a b=\"c\"/>", Lang::Xml),
            ("a: 1 # c\n", Lang::Yaml),
        ];
        for (text, lang) in samples {
            let mut hl = Highlighter::new(lang)
                .unwrap_or_else(|| panic!("{}: query did not compile", lang.name()));
            hl.update(text);
            assert!(
                !hl.spans.is_empty(),
                "{}: nothing highlighted in {:?}",
                lang.name(),
                text
            );
        }
    }

    #[test]
    fn kotlin_keywords_comments_and_strings_are_colored() {
        let mut hl = Highlighter::new(Lang::Kotlin).unwrap();
        hl.update("fun main() {\n  // hi\n  val a = \"x\"\n}\n");
        let kinds: Vec<Kind> = hl.spans.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&Kind::Keyword), "no keyword: {kinds:?}");
        assert!(kinds.contains(&Kind::Comment), "no comment: {kinds:?}");
        assert!(kinds.contains(&Kind::Str), "no string: {kinds:?}");
        assert!(kinds.contains(&Kind::Func), "no function: {kinds:?}");
    }

    #[test]
    fn syntax_errors_are_collected_only_when_asked() {
        let broken = "fn main() {\n    let x = ;\n}\n";
        let mut h = Highlighter::new(Lang::Rust).unwrap();
        h.update(broken);
        assert!(h.errors().is_empty());
        h.set_collect_errors(true);
        h.update(broken);
        assert!(!h.errors().is_empty());
        assert!(h.errors().iter().all(|e| e.line == 1));
        h.update("fn main() {\n    let x = 1;\n}\n");
        assert!(h.errors().is_empty());
    }
}
