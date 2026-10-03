//! The GitHub personal access token, stored in the Windows Credential Manager
//! (the `keyring` crate). Nothing else here: the token is never written to
//! settings, logs, files or git, and is held in memory only while it is used.

const SERVICE: &str = "smithy-github";
const USER: &str = "token";

/// The stored token, or `None` when none was saved.
pub fn load_token() -> Option<String> {
    let entry = keyring::Entry::new(SERVICE, USER).ok()?;
    entry.get_password().ok().filter(|t| !t.is_empty())
}

pub fn save_token(token: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, USER).map_err(|e| e.to_string())?;
    entry.set_password(token).map_err(|e| e.to_string())
}

/// Removing a token that is not there is not an error.
pub fn clear_token() -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, USER).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// The last four characters, for the "Token ending a3f9" line.
pub fn tail(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    chars[chars.len().saturating_sub(4)..].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::tail;

    #[test]
    fn tail_keeps_the_last_four_characters() {
        assert_eq!(tail("ghp_1234a3f9"), "a3f9");
        assert_eq!(tail("ab"), "ab");
        assert_eq!(tail(""), "");
    }
}
