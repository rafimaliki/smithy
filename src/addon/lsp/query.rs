//! One question to the right server: find the server or start it, tell it about
//! the document, ask, and hand back the answer. Runs off the UI thread.
use super::client::Client;
use super::manager::Manager;
use super::servers::ServerDef;
use super::uri;
use crate::editor::lang::Lang;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A file bigger than this is not read for preview text.
const MAX_SOURCE: u64 = 2 * 1024 * 1024;
/// At most this many files are read while collecting references.
const MAX_FILES: usize = 40;

#[derive(Clone, Copy)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

pub enum Query {
    Definition(Position),
    References(Position),
}

pub async fn run(
    manager: Arc<Mutex<Manager>>,
    lang: Lang,
    path: PathBuf,
    query: Query,
) -> Result<Value, String> {
    let def = manager.lock().unwrap().def_and_state(lang)?;
    let existing = manager.lock().unwrap().live(def.id);
    let client = match existing {
        Some(client) => client,
        None => start(&manager, def)?,
    };
    client.ensure_ready().await?;
    if !client.is_open(&path) {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        client.did_open(&path, def.language_id, &text)?;
    }
    let uri = uri::path_to_uri(&path);
    let (method, position) = match query {
        Query::Definition(p) => ("textDocument/definition", p),
        Query::References(p) => ("textDocument/references", p),
    };
    let position = json!({"line": position.line, "character": position.character});
    let params = match method {
        "textDocument/references" => json!({
            "textDocument": {"uri": uri},
            "position": position,
            "context": {"includeDeclaration": true},
        }),
        _ => json!({"textDocument": {"uri": uri}, "position": position}),
    };
    let answer = client.request(method, params)?;
    answer
        .recv()
        .await
        .map_err(|_| "the server closed".to_string())?
}

/// Start the server for `def` and keep it in the manager.
fn start(manager: &Arc<Mutex<Manager>>, def: &'static ServerDef) -> Result<Arc<Client>, String> {
    let root = manager.lock().unwrap().root().map(Path::to_path_buf);
    let client = Client::create(def, root.as_deref())?;
    manager.lock().unwrap().store(def.id, client.clone());
    Ok(client)
}

/// The locations inside a `definition` or `references` answer, as `(path, line,
/// character)`; an answer shape we cannot read is no locations.
pub fn locations(value: &Value) -> Vec<(PathBuf, u32, u32)> {
    use lsp_types::{GotoDefinitionResponse, Location};
    let parsed: Vec<Location> = if let Ok(Some(response)) =
        serde_json::from_value::<Option<GotoDefinitionResponse>>(value.clone())
    {
        match response {
            GotoDefinitionResponse::Scalar(location) => vec![location],
            GotoDefinitionResponse::Array(locations) => locations,
            GotoDefinitionResponse::Link(links) => links
                .into_iter()
                .map(|link| Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                })
                .collect(),
        }
    } else {
        serde_json::from_value::<Vec<Location>>(value.clone()).unwrap_or_default()
    };
    parsed
        .into_iter()
        .filter_map(|location| {
            let path = uri::path_from_uri(location.uri.as_str())?;
            Some((
                path,
                location.range.start.line,
                location.range.start.character,
            ))
        })
        .collect()
}

/// The server work behind one gesture: ask, then read the files the answer names.
pub async fn load(
    manager: Arc<Mutex<Manager>>,
    lang: Lang,
    path: PathBuf,
    position: Position,
    references: bool,
) -> Result<Outcome, String> {
    let query = if references {
        Query::References(position)
    } else {
        Query::Definition(position)
    };
    let answer = run(manager.clone(), lang, path.clone(), query).await?;
    let hits = locations(&answer);
    let definitions = if references {
        let answer = run(manager, lang, path.clone(), Query::Definition(position)).await?;
        locations(&answer)
    } else {
        Vec::new()
    };

    let mut sources: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let symbol = read_lines(&path)
        .and_then(|lines| lines.get(position.line as usize).cloned())
        .map(|line| word_at(&line, position.character))
        .unwrap_or_default();
    for (file, _, _) in hits.iter().chain(definitions.iter()) {
        if sources.contains_key(file) {
            continue;
        }
        if sources.len() >= MAX_FILES {
            break;
        }
        sources.insert(file.clone(), read_lines(file).unwrap_or_default());
    }
    sources.retain(|_, lines| !lines.is_empty());
    Ok(Outcome {
        symbol,
        locations: hits,
        definitions,
        sources,
    })
}

/// What one gesture's round trip brought back.
pub struct Outcome {
    /// The identifier under the position, for the peek's title.
    pub symbol: String,
    /// Where the symbol is, in file order.
    pub locations: Vec<(PathBuf, u32, u32)>,
    /// The declaration, when it was asked for separately.
    pub definitions: Vec<(PathBuf, u32, u32)>,
    /// The lines of every file the answer named, for the list and the preview.
    pub sources: HashMap<PathBuf, Vec<String>>,
}

/// A file's lines, or nothing when it is missing or too large to read for a peek.
fn read_lines(path: &Path) -> Option<Vec<String>> {
    if std::fs::metadata(path).ok()?.len() > MAX_SOURCE {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    Some(text.lines().map(|l| l.trim_end().to_string()).collect())
}

/// The identifier at `character` (UTF-16 units, as a server counts them) on
/// `line`: used to name the symbol in the peek's title.
pub fn word_at(line: &str, character: u32) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut units = 0u32;
    let mut at = chars.len();
    for (i, c) in chars.iter().enumerate() {
        if units >= character {
            at = i;
            break;
        }
        units += c.len_utf16() as u32;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut start = at;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = at;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    chars[start..end].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_at_finds_the_identifier_under_the_position() {
        let line = "    let Button = forward_ref(Button, 1);";
        assert_eq!(word_at(line, 8), "Button");
        assert_eq!(word_at(line, 29), "Button");
        assert_eq!(word_at(line, 4), "let");
        assert_eq!(word_at(line, 100), "");
    }

    #[test]
    fn word_at_counts_utf16_not_chars() {
        // "π" is one char but two UTF-16 units.
        assert_eq!(word_at("π + alpha", 4), "alpha");
    }

    #[test]
    fn locations_read_all_three_definition_shapes() {
        let scalar = json!({"uri": "file:///C:/a.rs", "range": {"start": {"line": 3, "character": 7}, "end": {"line": 3, "character": 9}}});
        let array = json!([scalar.clone(), scalar.clone()]);
        assert_eq!(locations(&scalar), vec![(PathBuf::from("C:/a.rs"), 3, 7)]);
        assert_eq!(locations(&array).len(), 2);
        assert_eq!(locations(&Value::Null), vec![]);
    }

    #[test]
    fn a_references_answer_is_a_plain_list() {
        let answer = json!([
            {"uri": "file:///C:/a.rs", "range": {"start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 4}}}
        ]);
        assert_eq!(locations(&answer), vec![(PathBuf::from("C:/a.rs"), 1, 0)]);
    }

    #[test]
    fn a_server_that_answers_with_a_link_is_understood() {
        let link = json!([{
            "targetUri": "file:///C:/b.rs",
            "targetRange": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}},
            "targetSelectionRange": {"start": {"line": 2, "character": 4}, "end": {"line": 2, "character": 8}}
        }]);
        assert_eq!(locations(&link), vec![(PathBuf::from("C:/b.rs"), 2, 4)]);
    }
}
