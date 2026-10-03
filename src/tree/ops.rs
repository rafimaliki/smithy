//! File operations behind the tree menu: create, rename, delete, copy, cut, paste.
//! Everything here takes plain paths, so it is tested without a window.
use std::io;
use std::path::{Path, PathBuf};

/// The app's own file clipboard for Copy/Cut/Paste in the tree.
///
/// ponytail: not the Windows file clipboard (CF_HDROP), so pasting into Explorer,
/// or from it, does not work yet; upgrade: put CF_HDROP on the system clipboard.
#[derive(Clone, Default)]
pub struct Clipboard {
    pub path: Option<PathBuf>,
    pub cut: bool,
}

impl gpui::Global for Clipboard {}

#[derive(Debug)]
pub enum RenameError {
    /// The typed name is not usable at all.
    Invalid(&'static str),
    /// Another entry in the folder already has the name.
    Exists,
    Io(io::Error),
}

impl RenameError {
    pub fn message(&self) -> String {
        match self {
            RenameError::Invalid(why) => (*why).to_string(),
            RenameError::Exists => "That name is taken.".to_string(),
            RenameError::Io(e) => e.to_string(),
        }
    }
}

/// Reject a name the file system would refuse, before touching the disk. The
/// messages are short because they are shown inline, in the rename field.
pub fn validate_name(name: &str) -> Result<(), &'static str> {
    if name.trim().is_empty() {
        return Err("Enter a name.");
    }
    if name == "." || name == ".." {
        return Err("Reserved name.");
    }
    if name.chars().any(|c| {
        matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Err("Illegal character.");
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err("No trailing dot or space.");
    }
    Ok(())
}

/// `dir/name`, or `dir/name 2`, `dir/name 3`… when that is taken.
pub fn unique_child(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = split_name(name);
    let named = |n: &str| match ext {
        Some(e) => format!("{stem} {n}.{e}"),
        None => format!("{stem} {n}"),
    };
    for n in 2..10_000u32 {
        let candidate = dir.join(named(&n.to_string()));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(named(&std::process::id().to_string()))
}

/// A free name in `dir` that starts with `wanted`, with a file created empty.
pub fn new_file(dir: &Path, wanted: &str) -> io::Result<PathBuf> {
    validate_name(wanted).map_err(|why| io::Error::new(io::ErrorKind::InvalidInput, why))?;
    let path = unique_child(dir, wanted);
    std::fs::write(&path, "")?;
    Ok(path)
}

/// A free name in `dir` that starts with `wanted`, with a folder created.
pub fn new_dir(dir: &Path, wanted: &str) -> io::Result<PathBuf> {
    validate_name(wanted).map_err(|why| io::Error::new(io::ErrorKind::InvalidInput, why))?;
    let path = unique_child(dir, wanted);
    std::fs::create_dir(&path)?;
    Ok(path)
}

/// Rename `path` to `name` inside the same folder.
pub fn rename(path: &Path, name: &str) -> Result<PathBuf, RenameError> {
    validate_name(name).map_err(RenameError::Invalid)?;
    let target = path.with_file_name(name);
    if target == path {
        return Ok(target);
    }
    // A rename that only changes case is fine even though the target "exists".
    let case_only = target
        .to_string_lossy()
        .to_lowercase()
        .eq(&path.to_string_lossy().to_lowercase());
    if !case_only && target.exists() {
        return Err(RenameError::Exists);
    }
    std::fs::rename(path, &target).map_err(RenameError::Io)?;
    Ok(target)
}

/// Move to the Recycle Bin. Never permanent.
pub fn delete(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| e.to_string())
}

/// Copy a file, or a folder and everything in it.
fn copy_entry(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

fn move_entry(from: &Path, to: &Path) -> io::Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        // Across volumes: copy, then drop the original.
        Err(_) => {
            copy_entry(from, to)?;
            if from.is_dir() {
                std::fs::remove_dir_all(from)
            } else {
                std::fs::remove_file(from)
            }
        }
    }
}

/// Paste `from` into the folder `into`, under a free name. Returns the new path.
pub fn paste(from: &Path, into: &Path, cut: bool) -> io::Result<PathBuf> {
    let name = from
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "nothing to paste"))?;
    let target = unique_child(into, &name.to_string_lossy());
    if cut {
        move_entry(from, &target)?;
    } else {
        copy_entry(from, &target)?;
    }
    Ok(target)
}

/// `path` below `root`, with forward slashes (what Copy relative path copies).
pub fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// `name` as (stem, extension without the dot). A leading dot is part of the stem,
/// so `.gitignore` has no extension.
fn split_name(name: &str) -> (&str, Option<&str>) {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, Some(ext)),
        _ => (name, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smithy-ops-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_are_validated_before_the_disk_is_touched() {
        assert!(validate_name("main.rs").is_ok());
        assert!(validate_name(".gitignore").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
        assert!(validate_name("..").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("a\\b").is_err());
        assert!(validate_name("a:b").is_err());
        assert!(validate_name("trailing.").is_err());
        assert!(validate_name("trailing ").is_err());
    }

    #[test]
    fn unique_child_counts_up_around_the_extension() {
        let dir = tmp("unique");
        assert_eq!(unique_child(&dir, "a.txt"), dir.join("a.txt"));
        std::fs::write(dir.join("a.txt"), "").unwrap();
        assert_eq!(unique_child(&dir, "a.txt"), dir.join("a 2.txt"));
        std::fs::write(dir.join("a 2.txt"), "").unwrap();
        assert_eq!(unique_child(&dir, "a.txt"), dir.join("a 3.txt"));
        std::fs::write(dir.join(".gitignore"), "").unwrap();
        assert_eq!(unique_child(&dir, ".gitignore"), dir.join(".gitignore 2"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rename_refuses_a_taken_name_and_allows_its_own() {
        let dir = tmp("rename");
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        std::fs::write(dir.join("b.txt"), "b").unwrap();
        assert!(matches!(
            rename(&dir.join("a.txt"), "b.txt"),
            Err(RenameError::Exists)
        ));
        assert!(matches!(
            rename(&dir.join("a.txt"), "a/b"),
            Err(RenameError::Invalid(_))
        ));
        // The same name is a no-op, not a conflict.
        assert_eq!(
            rename(&dir.join("a.txt"), "a.txt").unwrap(),
            dir.join("a.txt")
        );
        assert_eq!(
            rename(&dir.join("a.txt"), "c.txt").unwrap(),
            dir.join("c.txt")
        );
        assert!(dir.join("c.txt").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn paste_copies_folders_recursively_and_cuts_moves_them() {
        let dir = tmp("paste");
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("deep")).unwrap();
        std::fs::write(src.join("deep").join("f.txt"), "hi").unwrap();

        // Copy into the same folder: a fresh name, the original stays.
        let copy = paste(&src, &dir, false).unwrap();
        assert_eq!(copy, dir.join("src 2"));
        assert!(copy.join("deep").join("f.txt").exists());
        assert!(src.exists());

        let dest = tmp("paste-dest");
        let moved = paste(&src, &dest, true).unwrap();
        assert_eq!(moved, dest.join("src"));
        assert!(moved.join("deep").join("f.txt").exists());
        assert!(!src.exists());
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(dest);
    }

    #[test]
    fn relative_paths_use_forward_slashes() {
        let root = Path::new("C:/w/proj");
        assert_eq!(
            relative(&root.join("src").join("main.rs"), root),
            "src/main.rs"
        );
        assert_eq!(relative(Path::new("D:/other/x"), root), "D:/other/x");
    }
}
