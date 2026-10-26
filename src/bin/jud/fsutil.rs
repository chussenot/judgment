//! Which file a path names, for the rule that no subcommand writes over its
//! input (decision 0021), and writing a file no reader sees half of.

use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

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

/// Write `path` so that no reader ever sees half of it, and a failed write
/// leaves the file that was there: the text goes to a temporary file in the
/// same directory (so the rename stays on one file system), is flushed to
/// disk, and only then takes the name, replacing a symlink rather than
/// writing through it. The temporary name ends in `.tmp`, never `.jud` or
/// `.json`, so a [`judgment::Replay`] opened over the directory meanwhile
/// skips it, and carries the process id and a counter, so two writers never
/// share it. A failed write removes its temporary file.
pub(crate) fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = path.with_file_name(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let written = std::fs::File::create(&temporary).and_then(|mut file| {
        file.write_all(text.as_bytes())?;
        file.sync_all()
    });
    let renamed = written.and_then(|()| std::fs::rename(&temporary, path));
    if renamed.is_err() {
        // Best effort: the error being returned is the one that matters.
        let _ = std::fs::remove_file(&temporary);
    }
    renamed
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn a_write_that_fails_leaves_no_temporary_file_behind() {
        // A directory where the recording should go: the rename onto it
        // fails, after the text was written to the temporary file.
        let dir = std::env::temp_dir().join(format!("jud-fsutil-unit-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("taken.jud")).unwrap();
        assert!(write_atomically(&dir.join("taken.jud"), "text").is_err());
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            left,
            ["taken.jud"],
            "only the directory that was in the way"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_write_that_succeeds_leaves_only_the_recording() {
        let dir = std::env::temp_dir().join(format!("jud-fsutil-unit-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_atomically(&dir.join("fresh.jud"), "text").unwrap();
        write_atomically(&dir.join("fresh.jud"), "again").unwrap();
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["fresh.jud"]);
        assert_eq!(
            std::fs::read_to_string(dir.join("fresh.jud")).unwrap(),
            "again"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
