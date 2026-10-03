//! Blocking GitHub REST client. Every call here blocks on the network, so
//! callers run it on the background executor, never the UI thread.
use super::model::{self, Account, Checks, FileStat, PullRequest};
use std::fmt;

const API: &str = "https://api.github.com";
const USER_AGENT: &str = "smithy";

#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Client {
    token: String,
}

impl Client {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }

    /// Open pull requests (or closed ones) of `owner/repo`, newest first.
    pub fn pulls(&self, slug: &str, state: &str) -> Result<Vec<PullRequest>, Error> {
        let url = format!(
            "{API}/repos/{slug}/pulls?state={state}&sort=updated&direction=desc&per_page=30"
        );
        model::parse_pulls(&self.get(&url)?).map_err(Error)
    }

    pub fn checks(&self, slug: &str, sha: &str) -> Result<Checks, Error> {
        let url = format!("{API}/repos/{slug}/commits/{sha}/check-runs?per_page=100");
        model::parse_checks(&self.get(&url)?).map_err(Error)
    }

    pub fn files(&self, slug: &str, number: u64) -> Result<Vec<FileStat>, Error> {
        let url = format!("{API}/repos/{slug}/pulls/{number}/files?per_page=100");
        model::parse_files(&self.get(&url)?).map_err(Error)
    }

    pub fn account(&self) -> Result<Account, Error> {
        model::parse_account(&self.get(&format!("{API}/user"))?).map_err(Error)
    }

    fn get(&self, url: &str) -> Result<String, Error> {
        let response = ureq::get(url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .set("User-Agent", USER_AGENT)
            .call();
        match response {
            Ok(r) => r
                .into_string()
                .map_err(|e| Error(format!("GitHub sent a bad response: {e}"))),
            Err(ureq::Error::Status(code, _)) => Err(Error(status_message(code))),
            Err(e) => Err(Error(format!("Could not reach GitHub: {e}"))),
        }
    }
}

fn status_message(code: u16) -> String {
    match code {
        401 => "GitHub rejected the token. Check it in Settings > GitHub.".into(),
        403 => "GitHub refused the request: rate limit or missing access.".into(),
        404 => "Not found on GitHub. The token may not have access to this repository.".into(),
        _ => format!("GitHub returned status {code}."),
    }
}
