//! Reading a file for the editor: large and binary file guard.
use std::path::Path;

/// Above this a file opens read-only (and, later, without highlighting).
pub const LARGE_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Above this a file is not loaded at all.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

pub enum Loaded {
    Text { text: String, read_only: bool },
    Binary,
    TooLarge(u64),
    Error(String),
}

pub fn load(path: &Path) -> Loaded {
    let size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) => return Loaded::Error(e.to_string()),
    };
    if size > MAX_FILE_BYTES {
        return Loaded::TooLarge(size);
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return Loaded::Error(e.to_string()),
    };
    if is_binary(&bytes) {
        return Loaded::Binary;
    }
    match String::from_utf8(bytes) {
        Ok(text) => Loaded::Text {
            text,
            read_only: size > LARGE_FILE_BYTES,
        },
        // ponytail: non-UTF-8 text is treated as binary; upgrade: encoding detection.
        Err(_) => Loaded::Binary,
    }
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|&b| b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("smithy-test-{}-{name}", std::process::id()));
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn nul_byte_means_binary_and_text_loads() {
        let b = tmp("bin", &[b'a', 0, b'b']);
        let t = tmp("txt", b"hi\n");
        assert!(matches!(load(&b), Loaded::Binary));
        assert!(matches!(
            load(&t),
            Loaded::Text {
                read_only: false,
                ..
            }
        ));
        let _ = std::fs::remove_file(b);
        let _ = std::fs::remove_file(t);
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(matches!(
            load(Path::new("Z:/nope/none.txt")),
            Loaded::Error(_)
        ));
    }
}
