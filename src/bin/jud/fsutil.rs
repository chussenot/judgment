//! Which file a path names, for the rule that no subcommand writes over its
//! input (decision 0021).

use std::path::Path;

/// Whether two paths name one file: the same spelling, the same file once
/// both exist and are resolved (`./x`, a symlink), or, on Unix, two names for
/// one inode (a hard link, which resolving a path does not see).
pub(crate) fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b) {
        return true;
    }
    same_inode(a, b)
}

#[cfg(unix)]
fn same_inode(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (a.metadata(), b.metadata()) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_inode(_a: &Path, _b: &Path) -> bool {
    false
}

/// The directory a file would be created in, resolved: the parent of
/// `path` (the current directory for a bare name), canonicalised when it
/// exists.
pub(crate) fn parent_dir(path: &Path) -> std::path::PathBuf {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf())
}
