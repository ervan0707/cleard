//! Safe, guarded directory removal.
//!
//! Deletion is irreversible, so every removal is bounded to within the scan
//! root and refuses to touch the root itself. Symlinked artifact dirs that
//! resolve outside the root are rejected rather than followed.

use std::path::Path;

use anyhow::{bail, Context, Result};

/// Permanently remove `target`, after verifying it lies strictly inside `root`.
pub fn remove(root: &Path, target: &Path) -> Result<()> {
    let root_canon = root
        .canonicalize()
        .with_context(|| format!("resolving scan root {}", root.display()))?;
    let target_canon = target
        .canonicalize()
        .with_context(|| format!("resolving {}", target.display()))?;

    if target_canon == root_canon {
        bail!("refusing to delete the scan root itself");
    }
    if !target_canon.starts_with(&root_canon) {
        bail!(
            "refusing to delete a path outside the scan root: {}",
            target.display()
        );
    }

    // TOCTOU guard: re-check, without following links, right before removing.
    // `target_canon` is fully resolved, so it should still be a real directory;
    // if it was swapped for a symlink since the scan/canonicalize, bail instead
    // of letting remove_dir_all act on the changed path.
    let meta = std::fs::symlink_metadata(&target_canon)
        .with_context(|| format!("re-checking {}", target.display()))?;
    if !meta.file_type().is_dir() {
        bail!(
            "refusing to delete {}: it is no longer a directory (changed since scan)",
            target.display()
        );
    }

    std::fs::remove_dir_all(&target_canon)
        .with_context(|| format!("deleting {}", target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn deletes_inside_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let victim = root.join("proj/node_modules");
        fs::create_dir_all(victim.join("dep")).unwrap();
        fs::write(victim.join("dep/index.js"), "x").unwrap();

        remove(root, &victim).unwrap();
        assert!(!victim.exists());
    }

    #[test]
    fn refuses_root_itself() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(remove(tmp.path(), tmp.path()).is_err());
    }

    #[test]
    fn refuses_outside_root() {
        let inside = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let dir = outside.path().join("data");
        fs::create_dir_all(&dir).unwrap();
        assert!(remove(inside.path(), &dir).is_err());
        assert!(dir.exists());
    }
}
