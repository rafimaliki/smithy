//! File tree model: the open folder flattened into visible rows.
//! Only expanded directories are read, so a big folder costs nothing until opened.
mod context;
pub mod icons;
pub mod ops;
pub mod rename;
pub mod view;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

pub struct FileTree {
    pub root: PathBuf,
    expanded: HashSet<PathBuf>,
    pub rows: Vec<Row>,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        let mut t = Self {
            root,
            expanded: HashSet::new(),
            rows: Vec::new(),
        };
        t.refresh();
        t
    }

    pub fn toggle(&mut self, path: &Path) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_path_buf());
        }
        self.refresh();
    }

    /// Open `path` so an entry created inside it becomes visible.
    pub fn expand(&mut self, path: &Path) {
        if self.expanded.insert(path.to_path_buf()) {
            self.refresh();
        }
    }

    pub fn refresh(&mut self) {
        let mut rows = Vec::new();
        collect(&self.root, 0, &self.expanded, &mut rows);
        self.rows = rows;
    }
}

fn collect(dir: &Path, depth: usize, expanded: &HashSet<PathBuf>, out: &mut Vec<Row>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<(String, PathBuf, bool)> = read
        .filter_map(|e| e.ok())
        .map(|e| {
            let path = e.path();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (e.file_name().to_string_lossy().into_owned(), path, is_dir)
        })
        // ponytail: only `.git` is hidden; upgrade: honor .gitignore and a setting.
        .filter(|(name, _, is_dir)| !(*is_dir && name == ".git"))
        .collect();
    entries.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    for (name, path, is_dir) in entries {
        let is_open = is_dir && expanded.contains(&path);
        out.push(Row {
            path: path.clone(),
            name,
            is_dir,
            depth,
            expanded: is_open,
        });
        if is_open {
            collect(&path, depth + 1, expanded, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirs_first_sorted_and_children_only_when_expanded() {
        let root = std::env::temp_dir().join(format!("smithy-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("b.txt"), "").unwrap();
        std::fs::write(root.join("A.txt"), "").unwrap();
        std::fs::write(root.join("src").join("main.rs"), "").unwrap();

        let mut t = FileTree::new(root.clone());
        let names: Vec<_> = t.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["src", "A.txt", "b.txt"]);

        t.toggle(&root.join("src"));
        let names: Vec<_> = t.rows.iter().map(|r| (r.name.as_str(), r.depth)).collect();
        assert_eq!(
            names,
            [("src", 0), ("main.rs", 1), ("A.txt", 0), ("b.txt", 0)]
        );

        t.toggle(&root.join("src"));
        assert_eq!(t.rows.len(), 3);
        let _ = std::fs::remove_dir_all(root);
    }
}
