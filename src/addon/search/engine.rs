//! The search corpus: walk the open folder (honouring `.gitignore`), read text,
//! run the query and collect symbols. No gpui and no UI state, so this runs on a
//! worker thread and the view only decides what to show.
use super::fuzzy::{self, Hit};
use super::matcher::{self, Options};
use crate::editor::lang::Lang;
use crate::editor::symbols::{self, SymbolKind};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Files larger than this are skipped by the text and symbol scans.
const MAX_BYTES: u64 = 2 * 1024 * 1024;
/// The walk stops after this many files, so a runaway folder cannot hang it.
const MAX_CORPUS: usize = 20_000;
/// Cap per result list, so the UI stays a list and memory stays flat.
const MAX_FILES: usize = 300;
const MAX_SYMBOLS: usize = 500;
const MAX_SYMBOL_FILES: usize = 2_000;
const MAX_TEXT_FILES: usize = 200;
const MAX_TEXT_HITS: usize = 500;
/// A hit line longer than this is clipped, so one minified file cannot fill it.
const LINE_MAX: usize = 400;

/// Which of the tabs asked for the search.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    All,
    Files,
    Symbols,
    Text,
}

/// A flag the worker checks between files; the view sets it to drop stale work.
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// One file in the folder.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    /// Path relative to the folder, with `/` separators.
    pub rel: String,
    pub name: String,
    pub dir: String,
}

#[derive(Clone, Debug)]
pub struct FileMatch {
    pub file: FileInfo,
    /// Matched char indices inside the file name.
    pub positions: Vec<usize>,
    pub score: i32,
}

#[derive(Clone, Debug)]
pub struct LineHit {
    pub line: usize,
    pub text: String,
    pub ranges: Vec<(usize, usize)>,
}

#[derive(Clone, Debug)]
pub struct TextFile {
    pub file: FileInfo,
    pub hits: Vec<LineHit>,
}

#[derive(Clone, Debug)]
pub struct SymbolMatch {
    pub file: FileInfo,
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    pub positions: Vec<usize>,
    pub score: i32,
}

#[derive(Clone, Default, Debug)]
pub struct Output {
    pub files: Vec<FileMatch>,
    pub symbols: Vec<SymbolMatch>,
    pub text: Vec<TextFile>,
}

impl Output {
    pub fn text_hits(&self) -> usize {
        self.text.iter().map(|f| f.hits.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.symbols.is_empty() && self.text.is_empty()
    }
}

/// Run one search for `kind`. `Err` carries the message for a broken regular
/// expression; an empty query is `Ok` with nothing in it.
pub fn run(
    root: &Path,
    kind: Kind,
    query: &str,
    opts: Options,
    include: &str,
    cancel: &Cancel,
) -> Result<Output, String> {
    if query.is_empty() {
        return Ok(Output::default());
    }
    Ok(match kind {
        Kind::Text => Output {
            text: text(
                root,
                query,
                opts,
                include,
                MAX_TEXT_FILES,
                MAX_TEXT_HITS,
                cancel,
            )?,
            ..Output::default()
        },
        Kind::Files => Output {
            files: find_files(root, query, cancel),
            ..Output::default()
        },
        Kind::Symbols => Output {
            symbols: find_symbols(root, query, cancel),
            ..Output::default()
        },
        Kind::All => Output {
            files: take(find_files(root, query, cancel), 8),
            symbols: take(find_symbols(root, query, cancel), 8),
            text: text(root, query, opts, include, 6, 12, cancel)?,
        },
    })
}

/// Every file in `root` that is not gitignored, in relative-path order.
/// `include` is an optional gitignore-style glob that keeps matching files only.
pub fn list_files(root: &Path, include: Option<&str>, cancel: &Cancel) -> Vec<FileInfo> {
    let filter = include.and_then(|glob| override_for(root, glob));
    let mut out = Vec::new();
    // `require_git(false)`: a folder that is not a repository still honours its
    // `.gitignore`, which is what a search expects. Hidden files are searched;
    // only `.git` itself is skipped.
    let walk = WalkBuilder::new(root)
        .require_git(false)
        .hidden(false)
        .build();
    for entry in walk.flatten() {
        if cancel.is_cancelled() || out.len() >= MAX_CORPUS {
            break;
        }
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if entry.path().components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        if let Some(filter) = &filter {
            if !filter.matched(entry.path(), false).is_whitelist() {
                continue;
            }
        }
        out.push(file_info(root, entry.path()));
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

/// File-name search: fuzzy over each file's name, best score first.
pub fn find_files(root: &Path, query: &str, cancel: &Cancel) -> Vec<FileMatch> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let mut out: Vec<FileMatch> = list_files(root, None, cancel)
        .into_iter()
        .filter_map(|file| {
            let Hit { score, positions } = fuzzy::fuzzy(query.trim(), &file.name)?;
            Some(FileMatch {
                file,
                positions,
                score,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.file.rel.cmp(&b.file.rel))
    });
    out.truncate(MAX_FILES);
    out
}

/// Symbol search over every file Smithy has an outline for.
pub fn find_symbols(root: &Path, query: &str, cancel: &Cancel) -> Vec<SymbolMatch> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let mut parsed = 0;
    let mut out = Vec::new();
    for file in list_files(root, None, cancel) {
        if cancel.is_cancelled() || parsed >= MAX_SYMBOL_FILES {
            break;
        }
        if Lang::for_path(&file.path).is_none() || too_big(&file.path, MAX_BYTES) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file.path) else {
            continue;
        };
        parsed += 1;
        for symbol in symbols::symbols(&file.path, &text) {
            let Some(Hit { score, positions }) = fuzzy::fuzzy(query, &symbol.name) else {
                continue;
            };
            out.push(SymbolMatch {
                file: file.clone(),
                name: symbol.name,
                kind: symbol.kind,
                line: symbol.line,
                positions,
                score,
            });
        }
    }
    out.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.file.rel.cmp(&b.file.rel))
            .then_with(|| a.line.cmp(&b.line))
    });
    out.truncate(MAX_SYMBOLS);
    out
}

/// Text search: one entry per file with at least one matching line.
fn text(
    root: &Path,
    pattern: &str,
    opts: Options,
    include: &str,
    max_files: usize,
    max_hits: usize,
    cancel: &Cancel,
) -> Result<Vec<TextFile>, String> {
    let Some(re) = matcher::compile(pattern, opts)? else {
        return Ok(Vec::new());
    };
    let include = (!include.trim().is_empty()).then_some(include.trim());
    let mut out = Vec::new();
    let mut hits = 0;
    for file in list_files(root, include, cancel) {
        if cancel.is_cancelled() || hits >= max_hits || out.len() >= max_files {
            break;
        }
        if too_big(&file.path, MAX_BYTES) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file.path) else {
            continue;
        };
        let mut lines = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let ranges = matcher::ranges(&re, line);
            if ranges.is_empty() {
                continue;
            }
            let (text, ranges) = clip(line, &ranges);
            lines.push(LineHit {
                line: i,
                text,
                ranges,
            });
            hits += 1;
            if hits >= max_hits {
                break;
            }
        }
        if !lines.is_empty() {
            out.push(TextFile { file, hits: lines });
        }
    }
    Ok(out)
}

fn file_info(root: &Path, path: &Path) -> FileInfo {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = rel
        .rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .unwrap_or_default();
    FileInfo {
        path: path.to_path_buf(),
        rel,
        name,
        dir,
    }
}

/// The include glob as a matcher, or `None` when it is empty or malformed.
fn override_for(root: &Path, glob: &str) -> Option<ignore::overrides::Override> {
    let mut builder = OverrideBuilder::new(root);
    builder.add(glob.trim()).ok()?;
    builder.build().ok()
}

fn too_big(path: &Path, limit: u64) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() > limit)
        .unwrap_or(true)
}

/// Keep a hit line to `LINE_MAX` chars, clamping its ranges to match.
fn clip(line: &str, ranges: &[(usize, usize)]) -> (String, Vec<(usize, usize)>) {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= LINE_MAX {
        return (line.to_string(), ranges.to_vec());
    }
    let mut text: String = chars[..LINE_MAX - 1].iter().collect();
    text.push('…');
    let ranges = ranges
        .iter()
        .map(|&(a, b)| (a.min(LINE_MAX - 1), b.min(LINE_MAX - 1)))
        .filter(|(a, b)| b > a)
        .collect();
    (text, ranges)
}

fn take<T>(mut v: Vec<T>, n: usize) -> Vec<T> {
    v.truncate(n);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_info_splits_name_and_dir() {
        let root = Path::new("/p");
        let info = file_info(root, Path::new("/p/src/a.tsx"));
        assert_eq!(info.rel, "src/a.tsx");
        assert_eq!(info.name, "a.tsx");
        assert_eq!(info.dir, "src");
        let top = file_info(root, Path::new("/p/a.tsx"));
        assert_eq!(top.dir, "");
    }

    #[test]
    fn clip_keeps_short_lines_and_clamps_long_ones() {
        let (text, ranges) = clip("let busy = 1", &[(4, 8)]);
        assert_eq!(text, "let busy = 1");
        assert_eq!(ranges, vec![(4, 8)]);

        let long = "x".repeat(LINE_MAX + 10);
        let (text, ranges) = clip(&long, &[(0, 5), (LINE_MAX + 5, LINE_MAX + 9)]);
        assert_eq!(text.chars().count(), LINE_MAX);
        assert_eq!(ranges, vec![(0, 5)]);
    }

    /// The one place the walk and `.gitignore` are exercised: a file listed in
    /// `.gitignore` must not produce results, an included glob must filter.
    #[test]
    fn text_search_honours_gitignore_and_include() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("smithy-search-{stamp}"));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join(".gitignore"), "ignored.rs\n").unwrap();
        std::fs::write(root.join("src/keep.rs"), "let busy = 1;\n").unwrap();
        std::fs::write(root.join("src/other.txt"), "busy here\n").unwrap();
        std::fs::write(root.join("ignored.rs"), "let busy = 2;\n").unwrap();

        let cancel = Cancel::new();
        let all = run(&root, Kind::Text, "busy", Options::default(), "", &cancel).unwrap();
        let names: Vec<&str> = all.text.iter().map(|f| f.file.rel.as_str()).collect();
        assert!(names.contains(&"src/keep.rs"));
        assert!(names.contains(&"src/other.txt"));
        assert!(!names.iter().any(|n| n.contains("ignored")));

        let filtered = run(
            &root,
            Kind::Text,
            "busy",
            Options::default(),
            "src/**/*.rs",
            &cancel,
        )
        .unwrap();
        let names: Vec<&str> = filtered.text.iter().map(|f| f.file.rel.as_str()).collect();
        assert_eq!(names, vec!["src/keep.rs"]);

        std::fs::remove_dir_all(&root).ok();
    }
}
