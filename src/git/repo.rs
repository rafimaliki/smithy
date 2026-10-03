//! git2 wrapper: repository status, diffs, staging, commit, compare and blame.
//! Everything here is plain data out; the UI never touches git2 types.

use super::model::*;
use git2::{BranchType, Diff, DiffOptions, Patch, Repository, Status, StatusOptions};
use std::path::{Path, PathBuf};

pub struct Repo {
    inner: Repository,
    root: PathBuf,
}

/// The read-only result of comparing a local base branch with the current one.
pub struct Compare {
    pub base: String,
    pub commits: usize,
    pub files: Vec<FileDiff>,
}

impl Compare {
    pub fn added(&self) -> usize {
        self.files.iter().map(|f| f.added()).sum()
    }

    pub fn removed(&self) -> usize {
        self.files.iter().map(|f| f.removed()).sum()
    }
}

impl Repo {
    pub fn discover(path: &Path) -> Option<Repo> {
        let inner = Repository::discover(path).ok()?;
        if inner.is_bare() {
            return None;
        }
        let root = inner.workdir()?.to_path_buf();
        Some(Repo { inner, root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Current branch shorthand, or "HEAD" when detached.
    pub fn branch(&self) -> String {
        match self.inner.head() {
            Ok(head) if head.is_branch() => head.shorthand().unwrap_or("HEAD").to_string(),
            _ => "HEAD".into(),
        }
    }

    pub fn local_branches(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(branches) = self.inner.branches(Some(BranchType::Local)) {
            for branch in branches.flatten() {
                if let Some(name) = branch.0.name().ok().flatten() {
                    out.push(name.to_string());
                }
            }
        }
        out.sort();
        out
    }

    /// Staged and unstaged changes, staged first then by path.
    pub fn changes(&self) -> Result<Vec<ChangeEntry>, git2::Error> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true);
        let statuses = self.inner.statuses(Some(&mut opts))?;
        let mut out = Vec::new();
        for entry in statuses.iter() {
            let Some(rel) = entry.path().ok().map(str::to_string) else {
                continue;
            };
            let s = entry.status();
            if let Some(c) = staged_char(s) {
                push(&mut out, &rel, c, Section::Staged);
            }
            if let Some(c) = unstaged_char(s) {
                push(&mut out, &rel, c, Section::Unstaged);
            }
        }
        Ok(out)
    }

    /// The diff for one file, against the index (unstaged) or against HEAD (staged).
    pub fn file_diff(&self, rel: &str, section: Section) -> Result<FileDiff, git2::Error> {
        let diff = self.diff_for(Some(rel), section)?;
        Ok(diff
            .deltas()
            .next()
            .map(|delta| to_file_diff(&diff, 0, delta))
            .unwrap_or_else(|| empty_diff(rel)))
    }

    /// Commit everything in the index. Fails with a clear message when git has no
    /// `user.name` / `user.email`.
    pub fn commit(&self, message: &str) -> Result<(), git2::Error> {
        let mut index = self.inner.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.inner.find_tree(tree_id)?;
        let sig = self.inner.signature()?;
        let parents = match self.inner.head() {
            Ok(head) => vec![head.peel_to_commit()?],
            Err(_) => Vec::new(),
        };
        let refs: Vec<&git2::Commit<'_>> = parents.iter().collect();
        self.inner
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &refs)?;
        Ok(())
    }

    /// Merge-base diff of `base` against the current branch, like a PR's diff.
    pub fn compare(&self, base: &str) -> Result<Compare, git2::Error> {
        let head = self.inner.head()?.peel_to_commit()?.id();
        let base_oid = self
            .inner
            .find_branch(base, BranchType::Local)?
            .into_reference()
            .peel_to_commit()?
            .id();
        let merge_base = self.inner.merge_base(base_oid, head)?;
        let base_tree = self.inner.find_commit(merge_base)?.tree()?;
        let head_tree = self.inner.find_commit(head)?.tree()?;
        let commits = self
            .inner
            .graph_ahead_behind(base_oid, head)
            .map(|(ahead, _)| ahead)
            .unwrap_or(0);
        let mut opts = diff_options(None);
        let diff =
            self.inner
                .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut opts))?;
        let files = (0..diff.deltas().len())
            .map(|i| to_file_diff(&diff, i, diff.get_delta(i).unwrap()))
            .collect();
        Ok(Compare {
            base: base.to_string(),
            commits,
            files,
        })
    }

    pub(crate) fn inner(&self) -> &Repository {
        &self.inner
    }

    /// A diff limited to `rel` (all files when `None`).
    pub(crate) fn diff_for(
        &self,
        rel: Option<&str>,
        section: Section,
    ) -> Result<Diff<'_>, git2::Error> {
        let mut opts = diff_options(rel);
        match section {
            Section::Unstaged => self.inner.diff_index_to_workdir(None, Some(&mut opts)),
            Section::Staged => {
                let head = head_tree(&self.inner)?;
                self.inner
                    .diff_tree_to_index(head.as_ref(), None, Some(&mut opts))
            }
        }
    }
}

/// HEAD's tree, or `None` in a repository with no commits yet.
pub(crate) fn head_tree(repo: &Repository) -> Result<Option<git2::Tree<'_>>, git2::Error> {
    match repo.head() {
        Ok(head) => Ok(Some(head.peel_to_tree()?)),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => Ok(None),
        Err(e) => Err(e),
    }
}

fn push(out: &mut Vec<ChangeEntry>, rel: &str, status: char, section: Section) {
    let (name, dir) = split_path(rel);
    out.push(ChangeEntry {
        rel: rel.to_string(),
        name,
        dir,
        status,
        section,
    });
}

fn staged_char(s: Status) -> Option<char> {
    if s.contains(Status::INDEX_NEW) {
        Some('A')
    } else if s.contains(Status::INDEX_MODIFIED) {
        Some('M')
    } else if s.contains(Status::INDEX_DELETED) {
        Some('D')
    } else if s.contains(Status::INDEX_RENAMED) {
        Some('R')
    } else if s.contains(Status::INDEX_TYPECHANGE) {
        Some('T')
    } else {
        None
    }
}

fn unstaged_char(s: Status) -> Option<char> {
    if s.contains(Status::WT_NEW) {
        Some('U')
    } else if s.contains(Status::WT_MODIFIED) {
        Some('M')
    } else if s.contains(Status::WT_DELETED) {
        Some('D')
    } else if s.contains(Status::WT_RENAMED) {
        Some('R')
    } else if s.contains(Status::WT_TYPECHANGE) {
        Some('T')
    } else {
        None
    }
}

fn diff_options(rel: Option<&str>) -> DiffOptions {
    let mut opts = DiffOptions::new();
    opts.context_lines(3).interhunk_lines(0);
    opts.include_untracked(true)
        .show_untracked_content(true)
        .include_typechange(true);
    if let Some(rel) = rel {
        opts.pathspec(rel);
    }
    opts
}

fn to_file_diff(diff: &Diff<'_>, i: usize, delta: git2::DiffDelta<'_>) -> FileDiff {
    let rel = delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let (name, dir) = split_path(&rel);
    let mut file = FileDiff {
        rel: rel.clone(),
        name,
        dir,
        binary: delta.old_file().is_binary() || delta.new_file().is_binary(),
        hunks: Vec::new(),
    };
    let Ok(Some(patch)) = Patch::from_diff(diff, i) else {
        return file;
    };
    for h in 0..patch.num_hunks() {
        let Ok((hunk, count)) = patch.hunk(h) else {
            continue;
        };
        let mut lines = Vec::new();
        for l in 0..count {
            let Ok(line) = patch.line_in_hunk(h, l) else {
                continue;
            };
            let origin = match line.origin() {
                '+' => Origin::Add,
                '-' => Origin::Del,
                _ => Origin::Context,
            };
            let text = String::from_utf8_lossy(line.content())
                .trim_end_matches(['\n', '\r'])
                .to_string();
            lines.push(DiffLine {
                origin,
                old: line.old_lineno(),
                new: line.new_lineno(),
                text,
            });
        }
        file.hunks.push(Hunk {
            header: String::from_utf8_lossy(hunk.header())
                .trim_end()
                .to_string(),
            old_start: hunk.old_start(),
            old_lines: hunk.old_lines(),
            new_start: hunk.new_start(),
            new_lines: hunk.new_lines(),
            lines,
        });
    }
    file
}

fn empty_diff(rel: &str) -> FileDiff {
    let (name, dir) = split_path(rel);
    FileDiff {
        rel: rel.to_string(),
        name,
        dir,
        binary: false,
        hunks: Vec::new(),
    }
}
