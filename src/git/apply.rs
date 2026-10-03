//! Reading file contents and writing whole-file or partial changes into the index
//! or the working directory. Hunk edits reuse `model::apply_hunk`, so the maths is
//! unit-tested without a repository.
use super::model::*;
use super::repo::Repo;
use git2::{IndexEntry, IndexTime, Oid};
use std::path::{Path, PathBuf};

impl Repo {
    pub fn abs(&self, rel: &str) -> PathBuf {
        abs_path(self.root(), rel)
    }

    pub fn is_tracked(&self, rel: &str) -> bool {
        self.inner()
            .index()
            .map(|i| i.get_path(Path::new(rel), 0).is_some())
            .unwrap_or(false)
    }

    /// Stage a file: add it, or record its deletion when it is gone from disk.
    pub fn stage_file(&self, rel: &str) -> Result<(), git2::Error> {
        let mut index = self.inner().index()?;
        let path = Path::new(rel);
        if self.abs(rel).exists() {
            index.add_path(path)?;
        } else {
            index.remove_path(path)?;
        }
        index.write()
    }

    /// Unstage a file: put the index entry back to what HEAD has (or drop it).
    pub fn unstage_file(&self, rel: &str) -> Result<(), git2::Error> {
        let mut index = self.inner().index()?;
        match self.head_content(rel)? {
            Some(content) => self.write_index(&mut index, rel, &content)?,
            None => index.remove_path(Path::new(rel))?,
        }
        index.write()
    }

    /// Discard unstaged changes: restore the file from the index, or delete it when
    /// it is untracked.
    pub fn discard_file(&self, rel: &str) -> Result<(), git2::Error> {
        if !self.is_tracked(rel) {
            std::fs::remove_file(self.abs(rel))
                .map_err(|e| git2::Error::from_str(&e.to_string()))?;
            return Ok(());
        }
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.force().path(rel);
        self.inner().checkout_index(None, Some(&mut cb))
    }

    pub fn stage_hunk(&self, rel: &str, hunk: usize) -> Result<(), git2::Error> {
        let file = self.file_diff(rel, Section::Unstaged)?;
        let Some(h) = file.hunks.get(hunk) else {
            return Ok(());
        };
        let base = self.index_content(rel)?.unwrap_or_default();
        let content = join_lines(&apply_hunk(&lines_of(&base), h, true));
        let mut index = self.inner().index()?;
        self.write_index(&mut index, rel, &content)?;
        index.write()
    }

    pub fn unstage_hunk(&self, rel: &str, hunk: usize) -> Result<(), git2::Error> {
        let file = self.file_diff(rel, Section::Staged)?;
        let Some(h) = file.hunks.get(hunk) else {
            return Ok(());
        };
        let base = self.index_content(rel)?.unwrap_or_default();
        let content = join_lines(&apply_hunk(&lines_of(&base), h, false));
        let mut index = self.inner().index()?;
        if content.is_empty() && self.head_content(rel)?.is_none() {
            index.remove_path(Path::new(rel))?;
        } else {
            self.write_index(&mut index, rel, &content)?;
        }
        index.write()
    }

    /// Revert one hunk in the working file; the index is untouched.
    pub fn discard_hunk(&self, rel: &str, hunk: usize) -> Result<(), git2::Error> {
        let file = self.file_diff(rel, Section::Unstaged)?;
        let Some(h) = file.hunks.get(hunk) else {
            return Ok(());
        };
        let base = self.workdir_content(rel)?.unwrap_or_default();
        let content = join_lines(&apply_hunk(&lines_of(&base), h, false));
        std::fs::write(self.abs(rel), content).map_err(|e| git2::Error::from_str(&e.to_string()))
    }

    pub fn workdir_content(&self, rel: &str) -> Result<Option<String>, git2::Error> {
        match std::fs::read_to_string(self.abs(rel)) {
            Ok(text) => Ok(Some(text)),
            Err(_) => Ok(None),
        }
    }

    pub fn index_content(&self, rel: &str) -> Result<Option<String>, git2::Error> {
        let index = self.inner().index()?;
        Ok(index
            .get_path(Path::new(rel), 0)
            .and_then(|e| self.inner().find_blob(e.id).ok())
            .map(|b| String::from_utf8_lossy(b.content()).into_owned()))
    }

    pub fn head_content(&self, rel: &str) -> Result<Option<String>, git2::Error> {
        let Some(tree) = super::repo::head_tree(self.inner())? else {
            return Ok(None);
        };
        Ok(tree
            .get_path(Path::new(rel))
            .ok()
            .and_then(|e| self.inner().find_blob(e.id()).ok())
            .map(|b| String::from_utf8_lossy(b.content()).into_owned()))
    }

    /// Replace or create an index entry from a blob in memory.
    fn write_index(
        &self,
        index: &mut git2::Index,
        rel: &str,
        content: &str,
    ) -> Result<(), git2::Error> {
        let path = Path::new(rel);
        let mode = index.get_path(path, 0).map(|e| e.mode).unwrap_or(0o100644);
        let entry = IndexEntry {
            ctime: IndexTime::new(0, 0),
            mtime: IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: Oid::ZERO_SHA1,
            flags: 0,
            flags_extended: 0,
            path: rel.as_bytes().to_vec(),
        };
        index.add_frombuffer(&entry, content.as_bytes())
    }
}

fn lines_of(text: &str) -> Vec<String> {
    text.split('\n').map(str::to_string).collect()
}

fn join_lines(lines: &[String]) -> String {
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::Repo;

    fn repo_with(name: &str, original: &str, changed: &str) -> (PathBuf, Repo) {
        let dir = std::env::temp_dir().join(format!("smithy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let inner = git2::Repository::init(&dir).unwrap();
        std::fs::write(dir.join("f.txt"), original).unwrap();
        let mut index = inner.index().unwrap();
        index.add_path(Path::new("f.txt")).unwrap();
        index.write().unwrap();
        let tree = inner.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("test", "test@example.com").unwrap();
        inner
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        drop(tree);
        drop(index);
        std::fs::write(dir.join("f.txt"), changed).unwrap();
        let repo = Repo::discover(&dir).unwrap();
        (dir, repo)
    }

    fn lines(n: u32) -> String {
        (1..=n).map(|i| format!("line {i}\n")).collect()
    }

    #[test]
    fn stage_hunk_moves_only_that_hunk_into_the_index() {
        let original = lines(12);
        let changed = original.replace("line 6\n", "line 6 changed\n");
        let (dir, repo) = repo_with("stage-hunk", &original, &changed);
        let before = repo.file_diff("f.txt", Section::Unstaged).unwrap();
        assert_eq!(before.hunks.len(), 1);
        repo.stage_hunk("f.txt", 0).unwrap();
        assert!(repo
            .file_diff("f.txt", Section::Unstaged)
            .unwrap()
            .hunks
            .is_empty());
        assert_eq!(repo.index_content("f.txt").unwrap().unwrap(), changed);
        assert_eq!(repo.workdir_content("f.txt").unwrap().unwrap(), changed);
        assert_eq!(repo.file_diff("f.txt", Section::Staged).unwrap().added(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn discard_hunk_restores_the_working_file_only() {
        let original = lines(12);
        let changed = original.replace("line 4\n", "line 4 changed\n");
        let (dir, repo) = repo_with("discard-hunk", &original, &changed);
        assert_eq!(
            repo.file_diff("f.txt", Section::Unstaged)
                .unwrap()
                .hunks
                .len(),
            1
        );
        repo.discard_hunk("f.txt", 0).unwrap();
        assert_eq!(repo.workdir_content("f.txt").unwrap().unwrap(), original);
        assert_eq!(repo.index_content("f.txt").unwrap().unwrap(), original);
        assert!(repo
            .file_diff("f.txt", Section::Unstaged)
            .unwrap()
            .hunks
            .is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unstage_file_puts_the_index_back_to_head() {
        let original = lines(3);
        let changed = original.replace("line 2\n", "line 2 changed\n");
        let (dir, repo) = repo_with("unstage-file", &original, &changed);
        repo.stage_file("f.txt").unwrap();
        assert_eq!(repo.file_diff("f.txt", Section::Staged).unwrap().added(), 1);
        repo.unstage_file("f.txt").unwrap();
        assert!(repo
            .file_diff("f.txt", Section::Staged)
            .unwrap()
            .hunks
            .is_empty());
        assert_eq!(repo.index_content("f.txt").unwrap().unwrap(), original);
        let _ = std::fs::remove_dir_all(dir);
    }
}
