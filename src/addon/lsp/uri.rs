//! `file:` URIs, the only form a language server speaks for document paths.
//! Windows-first, pure, and tested (the drive-letter and space cases are exactly
//! where a hand-rolled encoder goes wrong).
use std::path::{Path, PathBuf};

/// A path as a `file:` URI: `C:\a b\c.rs` -> `file:///C:/a%20b/c.rs`.
pub fn path_to_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    format!("file:///{}", encode(&text))
}

/// The path a `file:` URI names, `None` for another scheme or a malformed URI.
pub fn path_from_uri(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?.strip_prefix('/')?;
    Some(PathBuf::from(decode(rest)?))
}

/// Do two paths name the same file? Windows paths differ in case, and a server
/// may report either the drive letter it was given or another one.
pub fn same_file(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            let hex = std::str::from_utf8(hex).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_letter_and_spaces_round_trip() {
        let uri = path_to_uri(Path::new(r"C:\Users\Rafi\my code\a.rs"));
        assert_eq!(uri, "file:///C:/Users/Rafi/my%20code/a.rs");
        assert_eq!(
            path_from_uri(&uri),
            Some(PathBuf::from("C:/Users/Rafi/my code/a.rs"))
        );
    }

    #[test]
    fn a_servers_uri_is_understood() {
        assert_eq!(
            path_from_uri("file:///c%3A/src/main.rs"),
            Some(PathBuf::from("c:/src/main.rs"))
        );
        assert_eq!(path_from_uri("untitled:Untitled-1"), None);
    }

    #[test]
    fn file_identity_ignores_case() {
        assert!(same_file(Path::new("C:/a/B.rs"), Path::new("c:/a/b.rs")));
        assert!(!same_file(Path::new("C:/a/b.rs"), Path::new("C:/a/c.rs")));
    }
}
