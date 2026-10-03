//! File reading for the editor's gutter and blame: per-line marks since the last
//! commit, and per-line blame for the working file.
use super::model::*;
use super::repo::Repo;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Above these the work is skipped: blame on a huge file is not worth a freeze.
const MAX_BLAME_LINES: usize = 40_000;

impl Repo {
    /// Lines added or changed since the last commit, indexed from 0.
    pub fn line_marks(&self, rel: &str) -> Result<Vec<Option<LineMark>>, git2::Error> {
        let count = self
            .workdir_content(rel)?
            .map(|c| c.split('\n').count())
            .unwrap_or(0);
        let mut marks = vec![None; count];
        let Some(head) = super::repo::head_tree(self.inner())? else {
            return Ok(marks);
        };
        let mut opts = git2::DiffOptions::new();
        opts.context_lines(0).interhunk_lines(0).pathspec(rel);
        let diff = self
            .inner()
            .diff_tree_to_workdir_with_index(Some(&head), Some(&mut opts))?;
        for i in 0..diff.deltas().len() {
            let Ok(Some(patch)) = git2::Patch::from_diff(&diff, i) else {
                continue;
            };
            for h in 0..patch.num_hunks() {
                let Ok((hunk, lines)) = patch.hunk(h) else {
                    continue;
                };
                let _ = hunk;
                let changed = changed_lines(&patch, h, lines);
                let mark = if changed.removed {
                    LineMark::Modified
                } else {
                    LineMark::Added
                };
                for line in changed.new {
                    if line >= 1 && line as usize <= marks.len() {
                        marks[line as usize - 1] = Some(mark);
                    }
                }
            }
        }
        Ok(marks)
    }

    /// Blame for every line of the working file; `None` for lines git cannot
    /// attribute (uncommitted edits).
    pub fn blame(&self, rel: &str) -> Result<Vec<Option<BlameLine>>, git2::Error> {
        let count = self
            .workdir_content(rel)?
            .map(|c| c.split('\n').count())
            .unwrap_or(0);
        if count == 0 || count > MAX_BLAME_LINES {
            return Ok(vec![None; count]);
        }
        let blame = self.inner().blame_file(Path::new(rel), None)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let mut out = vec![None; count];
        for hunk in blame.iter() {
            let oid = hunk.final_commit_id();
            if oid.is_zero() {
                continue;
            }
            let commit = self.inner().find_commit(oid).ok();
            let author = commit
                .as_ref()
                .and_then(|c| c.author().name().ok().map(str::to_string))
                .unwrap_or_default();
            let when = commit.as_ref().map(|c| c.time().seconds()).unwrap_or(now);
            let subject = hunk.summary().ok().flatten().unwrap_or("").to_string();
            let entry = BlameLine {
                author,
                age: age(now, when),
                hash: oid.to_string().chars().take(7).collect(),
                subject,
            };
            let start = hunk.final_start_line();
            for line in start..start + hunk.lines_in_hunk() {
                if line >= 1 && line <= count {
                    out[line - 1] = Some(entry.clone());
                }
            }
        }
        Ok(out)
    }
}

struct Changed {
    new: Vec<u32>,
    removed: bool,
}

fn changed_lines(patch: &git2::Patch<'_>, h: usize, count: usize) -> Changed {
    let mut changed = Changed {
        new: Vec::new(),
        removed: false,
    };
    for l in 0..count {
        let Ok(line) = patch.line_in_hunk(h, l) else {
            continue;
        };
        match line.origin() {
            '+' => {
                if let Some(n) = line.new_lineno() {
                    changed.new.push(n);
                }
            }
            '-' => changed.removed = true,
            _ => {}
        }
    }
    changed
}
