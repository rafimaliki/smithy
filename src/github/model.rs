//! GitHub REST payloads as plain data, and the mapping from JSON to it. Pure:
//! no HTTP and no gpui, so every mapping is unit-tested on sample payloads.
use crate::git::split_path;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// The `owner/name` a folder's `origin` remote points at on GitHub.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RepoSlug {
    pub owner: String,
    pub name: String,
}

impl RepoSlug {
    /// Parse an `origin` URL: https, http, `ssh://` or the `git@github.com:` form.
    pub fn parse(remote: &str) -> Option<Self> {
        let after_host = remote
            .trim()
            .split_once("github.com")
            .map(|(_, rest)| rest)?;
        let path = after_host.trim_start_matches(['/', ':']);
        let path = path.strip_suffix(".git").unwrap_or(path);
        let mut parts = path.split('/').filter(|p| !p.is_empty());
        let owner = parts.next()?.to_string();
        let name = parts.next()?.to_string();
        Some(Self { owner, name })
    }

    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// The combined state of a commit's checks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckState {
    Success,
    Pending,
    Failure,
    Neutral,
}

#[derive(Clone, Debug)]
pub struct Checks {
    pub total: u32,
    pub passed: u32,
    pub state: CheckState,
}

/// One changed file, from the pull request's file list.
#[derive(Clone, Debug)]
pub struct FileStat {
    pub name: String,
    pub dir: String,
    pub added: u64,
    pub removed: u64,
}

/// A pull request as the list and detail views need it. `checks` and `files`
/// are filled by later requests; a PR without them is still shown.
#[derive(Clone, Debug)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub author: String,
    /// "open" or "closed".
    pub state: String,
    pub draft: bool,
    pub merged: bool,
    /// Human age of the last update, e.g. "2h ago".
    pub age: String,
    pub body: String,
    pub head_ref: String,
    pub base_ref: String,
    pub head_sha: String,
    pub html_url: String,
    pub changed_files: u64,
    pub checks: Option<Checks>,
    pub files: Vec<FileStat>,
    pub files_loaded: bool,
}

impl PullRequest {
    /// The state chip's text.
    pub fn state_label(&self) -> &'static str {
        if self.merged {
            "Merged"
        } else if self.state == "closed" {
            "Closed"
        } else {
            "Open"
        }
    }
}

#[derive(Clone, Debug)]
pub struct Account {
    pub login: String,
}

fn string(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn nested(v: &Value, key: &str, inner: &str) -> String {
    v.get(key)
        .and_then(|o| o.get(inner))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn count(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// `GET /repos/{owner}/{repo}/pulls`.
pub fn parse_pulls(json: &str) -> Result<Vec<PullRequest>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let array = value.as_array().ok_or("Expected a list of pull requests")?;
    Ok(array.iter().filter_map(parse_pull).collect())
}

fn parse_pull(v: &Value) -> Option<PullRequest> {
    let number = v.get("number").and_then(Value::as_u64)?;
    let updated = string(v, "updated_at");
    let created = string(v, "created_at");
    Some(PullRequest {
        number,
        title: string(v, "title"),
        author: nested(v, "user", "login"),
        state: string(v, "state"),
        draft: v.get("draft").and_then(Value::as_bool).unwrap_or(false),
        merged: v.get("merged").and_then(Value::as_bool).unwrap_or(false),
        age: age_of(if updated.is_empty() {
            &created
        } else {
            &updated
        }),
        body: string(v, "body"),
        head_ref: nested(v, "head", "ref"),
        base_ref: nested(v, "base", "ref"),
        head_sha: nested(v, "head", "sha"),
        html_url: string(v, "html_url"),
        changed_files: count(v, "changed_files"),
        checks: None,
        files: Vec::new(),
        files_loaded: false,
    })
}

/// `GET /repos/{owner}/{repo}/commits/{sha}/check-runs`.
pub fn parse_checks(json: &str) -> Result<Checks, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let runs = value
        .get("check_runs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    Ok(checks_from_runs(runs))
}

/// Failure wins over still-running, as GitHub shows it; no runs is neutral.
pub fn checks_from_runs(runs: &[Value]) -> Checks {
    let mut passed = 0;
    let mut failed = false;
    let mut pending = false;
    for run in runs {
        let status = run.get("status").and_then(Value::as_str).unwrap_or("");
        let conclusion = run.get("conclusion").and_then(Value::as_str);
        match conclusion {
            Some("success" | "neutral" | "skipped") => passed += 1,
            Some(
                "failure" | "timed_out" | "cancelled" | "action_required" | "startup_failure"
                | "stale",
            ) => failed = true,
            _ => {}
        }
        if status != "completed" || conclusion.is_none() {
            pending = true;
        }
    }
    let state = if runs.is_empty() {
        CheckState::Neutral
    } else if failed {
        CheckState::Failure
    } else if pending {
        CheckState::Pending
    } else {
        CheckState::Success
    };
    Checks {
        total: runs.len() as u32,
        passed,
        state,
    }
}

/// `GET /repos/{owner}/{repo}/pulls/{number}/files`.
pub fn parse_files(json: &str) -> Result<Vec<FileStat>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let array = value.as_array().ok_or("Expected a list of files")?;
    Ok(array
        .iter()
        .map(|f| {
            let (name, dir) = split_path(&string(f, "filename"));
            FileStat {
                name,
                dir,
                added: count(f, "additions"),
                removed: count(f, "deletions"),
            }
        })
        .collect())
}

/// `GET /user`.
pub fn parse_account(json: &str) -> Result<Account, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let login = string(&value, "login");
    if login.is_empty() {
        return Err("GitHub did not return an account".into());
    }
    Ok(Account { login })
}

fn age_of(stamp: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    rfc3339_epoch(stamp)
        .map(|then| crate::git::model::age(now, then))
        .unwrap_or_default()
}

/// Seconds since the Unix epoch for `YYYY-MM-DDThh:mm:ssZ` (the only form GitHub
/// sends). A trailing offset or fraction is ignored.
fn rfc3339_epoch(s: &str) -> Option<i64> {
    let part = |a: usize, b: usize| s.get(a..b)?.parse::<i64>().ok();
    if s.len() < 19 {
        return None;
    }
    let (y, mo, d) = (part(0, 4)?, part(5, 7)?, part(8, 10)?);
    let (h, mi, se) = (part(11, 13)?, part(14, 16)?, part(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3_600 + mi * 60 + se)
}

/// Days from 1970-01-01 to `y-m-d`, by Howard Hinnant's civil-date algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_origin_url_shape_and_rejects_other_hosts() {
        let want = RepoSlug {
            owner: "rafimaliki".into(),
            name: "smithy-web".into(),
        };
        assert_eq!(
            RepoSlug::parse("https://github.com/rafimaliki/smithy-web.git"),
            Some(want.clone())
        );
        assert_eq!(
            RepoSlug::parse("git@github.com:rafimaliki/smithy-web.git"),
            Some(want.clone())
        );
        assert_eq!(
            RepoSlug::parse("ssh://git@github.com/rafimaliki/smithy-web"),
            Some(want.clone())
        );
        assert_eq!(
            RepoSlug::parse("http://github.com/rafimaliki/smithy-web/"),
            Some(want)
        );
        assert_eq!(RepoSlug::parse("https://gitlab.com/a/b.git"), None);
        assert_eq!(RepoSlug::parse("C:/local/repo"), None);
    }

    #[test]
    fn maps_a_pull_list_and_reads_nested_branches() {
        let json = r#"[
          {"number":42,"title":"Add busy state","state":"open","draft":false,
           "html_url":"https://github.com/o/r/pull/42","changed_files":4,
           "created_at":"2026-10-03T10:00:00Z","updated_at":"2026-10-03T12:00:00Z",
           "body":"Adds a busy prop.","user":{"login":"rafimaliki"},
           "head":{"ref":"feat/button-busy","sha":"abc123"},"base":{"ref":"main"}},
          {"number":37,"title":"Draft: split","state":"open","draft":true,
           "user":{"login":"lena"},"head":{"ref":"x"},"base":{"ref":"main"}}
        ]"#;
        let pulls = parse_pulls(json).unwrap();
        assert_eq!(pulls.len(), 2);
        assert_eq!(pulls[0].number, 42);
        assert_eq!(pulls[0].author, "rafimaliki");
        assert_eq!(pulls[0].head_ref, "feat/button-busy");
        assert_eq!(pulls[0].base_ref, "main");
        assert_eq!(pulls[0].head_sha, "abc123");
        assert_eq!(pulls[0].changed_files, 4);
        assert!(pulls[1].draft);
        assert_eq!(pulls[1].state_label(), "Open");
    }

    #[test]
    fn check_state_prefers_failure_then_pending() {
        let runs = |v: &str| serde_json::from_str::<Value>(v).unwrap();
        let ok = checks_from_runs(&[runs(r#"{"status":"completed","conclusion":"success"}"#)]);
        assert_eq!((ok.total, ok.passed, ok.state), (1, 1, CheckState::Success));
        let fail = checks_from_runs(&[
            runs(r#"{"status":"completed","conclusion":"success"}"#),
            runs(r#"{"status":"completed","conclusion":"failure"}"#),
        ]);
        assert_eq!(
            (fail.total, fail.passed, fail.state),
            (2, 1, CheckState::Failure)
        );
        let running = checks_from_runs(&[runs(r#"{"status":"in_progress","conclusion":null}"#)]);
        assert_eq!(running.state, CheckState::Pending);
        assert_eq!(checks_from_runs(&[]).state, CheckState::Neutral);
    }

    #[test]
    fn parses_the_file_list_with_counts() {
        let json = r#"[{"filename":"src/components/Button.tsx","additions":3,"deletions":2},
                       {"filename":"README.md","additions":1,"deletions":0}]"#;
        let files = parse_files(json).unwrap();
        assert_eq!(files[0].name, "Button.tsx");
        assert_eq!(files[0].dir, "src/components");
        assert_eq!((files[0].added, files[0].removed), (3, 2));
        assert_eq!(files[1].dir, "");
    }

    #[test]
    fn rejects_malformed_json_and_missing_accounts() {
        assert!(parse_pulls("not json").is_err());
        assert!(parse_account(r#"{"message":"Bad credentials"}"#).is_err());
        assert_eq!(
            parse_account(r#"{"login":"rafimaliki"}"#).unwrap().login,
            "rafimaliki"
        );
    }

    #[test]
    fn reads_rfc3339_timestamps() {
        assert_eq!(rfc3339_epoch("2026-10-03T12:00:00Z"), Some(1_791_028_800));
        assert_eq!(rfc3339_epoch("2024-02-29T00:00:00Z"), Some(1_709_164_800));
        assert_eq!(
            rfc3339_epoch("2026-01-02T00:00:00Z").unwrap()
                - rfc3339_epoch("2026-01-01T00:00:00Z").unwrap(),
            86_400
        );
        assert_eq!(rfc3339_epoch("nope"), None);
    }
}
