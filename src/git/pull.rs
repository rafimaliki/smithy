//! Reviewing a pull request: fetch its head ref and diff it against its base
//! locally, so the review uses the same diff model as branch compare.
use super::repo::{diff_options, to_file_diff, Compare, Repo};
use git2::{BranchType, Oid};

impl Repo {
    /// Fetch `refs/pull/{number}/head` from `origin` and diff it against `base`,
    /// the way GitHub shows the pull request. Read-only beyond that one ref.
    pub fn pull_request(&self, number: u64, base: &str) -> Result<Compare, git2::Error> {
        let refname = format!("refs/pull/{number}/head");
        let refspec = format!("+{refname}:{refname}");
        self.inner()
            .find_remote("origin")?
            .fetch(&[refspec.as_str()], None, None)?;
        let head = self.inner().refname_to_id(&refname)?;
        let base_oid = self.base_commit(base)?;
        let merge_base = self.inner().merge_base(base_oid, head)?;
        let base_tree = self.inner().find_commit(merge_base)?.tree()?;
        let head_tree = self.inner().find_commit(head)?.tree()?;
        let commits = self
            .inner()
            .graph_ahead_behind(head, base_oid)
            .map(|(ahead, _)| ahead)
            .unwrap_or(0);
        let mut opts = diff_options(None);
        let diff =
            self.inner()
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

    /// A local branch, or the remote-tracking branch when the base only exists
    /// on `origin` (the usual case for a fork or a fresh clone).
    fn base_commit(&self, base: &str) -> Result<Oid, git2::Error> {
        if let Ok(branch) = self.inner().find_branch(base, BranchType::Local) {
            return branch.into_reference().peel_to_commit().map(|c| c.id());
        }
        let remote = format!("origin/{base}");
        self.inner()
            .find_branch(&remote, BranchType::Remote)?
            .into_reference()
            .peel_to_commit()
            .map(|c| c.id())
    }
}

/// A pull request's ref label for the status bar, e.g. "pr/42".
pub fn ref_label(number: u64) -> String {
    format!("pr/{number}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn scratch(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("smithy-pr-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Commit `text` as `file` on top of `parent`, without moving any branch.
    fn commit(
        repo: &Repository,
        file: &str,
        text: &str,
        parent: Option<&git2::Commit>,
    ) -> git2::Oid {
        fs::write(repo.workdir().unwrap().join(file), text).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(file)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let parents: Vec<&git2::Commit> = parent.into_iter().collect();
        repo.commit(None, &sig, &sig, "msg", &tree, &parents)
            .unwrap()
    }

    /// The whole review path: fetch `refs/pull/1/head` from origin and diff it.
    #[test]
    fn fetches_the_pr_ref_and_diffs_it_against_the_base() {
        let root = scratch("review");
        let upstream = root.join("upstream");
        let work = root.join("work");
        let base_oid = {
            let repo = Repository::init(&upstream).unwrap();
            let base = commit(&repo, "a.txt", "one\n", None);
            repo.branch("main", &repo.find_commit(base).unwrap(), true)
                .unwrap();
            repo.set_head("refs/heads/main").unwrap();
            let head = commit(
                &repo,
                "a.txt",
                "one\ntwo\n",
                Some(&repo.find_commit(base).unwrap()),
            );
            repo.reference("refs/pull/1/head", head, true, "test")
                .unwrap();
            base
        };
        assert!(!base_oid.is_zero());

        Repository::clone(upstream.to_str().unwrap(), &work).unwrap();
        let compare = {
            let repo = Repo::discover(&work).unwrap();
            repo.pull_request(1, "main").unwrap()
        };
        assert_eq!(compare.commits, 1);
        assert_eq!(compare.files.len(), 1);
        let file = &compare.files[0];
        assert_eq!(file.rel, "a.txt");
        assert_eq!((file.added(), file.removed()), (1, 0));

        drop(upstream);
        let _ = fs::remove_dir_all(&root);
    }
}
