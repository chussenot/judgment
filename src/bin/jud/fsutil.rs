//! Which file a path names, for the rule that no subcommand writes over its
//! input (decision 0021), and writing a file no reader sees half of.

use judgment::eval::write_atomically;
use std::io::Write as _;
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

/// Write a file the user named (`jud tune --out`) as a write to it would:
/// through a symlink to the file it names, and into a pipe or a device
/// (`/dev/stdout`, a FIFO) as a stream. A regular file, or none yet, is
/// replaced whole by [`write_atomically`], so a failed write leaves it as it
/// was; renaming over a pipe or a device would delete it and lose the text,
/// and a stream has no half-written file to leave behind.
pub(crate) fn write_out(path: &Path, text: &str) -> std::io::Result<()> {
    // A link to a file that is there resolves to it; a dangling link, or one
    // to a pipe (`/dev/stdout` is `/proc/self/fd/1`), does not.
    let target = match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::canonicalize(path).ok(),
        _ => Some(path.to_path_buf()),
    };
    match target {
        Some(file) if std::fs::metadata(&file).map_or(true, |meta| meta.is_file()) => {
            write_atomically(&file, |f| f.write_all(text.as_bytes()))
        }
        _ => std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?
            .write_all(text.as_bytes()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// A FIFO is written to, not renamed over: the reader gets the text and
    /// the FIFO is still one.
    #[cfg(unix)]
    #[test]
    fn write_out_streams_into_a_fifo_and_leaves_it_one() {
        use std::os::unix::fs::FileTypeExt;
        let dir = std::env::temp_dir().join(format!("jud-fsutil-fifo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fifo = dir.join("out.jud");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(made.success());
        let reading = fifo.clone();
        let reader = std::thread::spawn(move || std::fs::read_to_string(reading).unwrap());
        write_out(&fifo, "text").unwrap();
        // Checked before the join: a FIFO renamed over leaves the reader
        // waiting for a writer that never comes.
        let kind = std::fs::symlink_metadata(&fifo).unwrap().file_type();
        assert!(kind.is_fifo(), "the FIFO was replaced");
        assert_eq!(reader.join().unwrap(), "text");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Through a link, the file it names is replaced whole and the link is
    /// kept; a regular file or a new one is replaced whole.
    #[cfg(unix)]
    #[test]
    fn write_out_replaces_the_file_a_link_names_and_keeps_the_link() {
        let dir = std::env::temp_dir().join(format!("jud-fsutil-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("target.jud");
        std::fs::write(&target, "old").unwrap();
        let link = dir.join("link.jud");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        write_out(&link, "new").unwrap();
        write_out(&dir.join("fresh.jud"), "fresh").unwrap();
        let link_kept = std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink();
        let target_text = std::fs::read_to_string(&target).unwrap();
        let mut left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(link_kept);
        assert_eq!(target_text, "new");
        assert_eq!(left, ["fresh.jud", "link.jud", "target.jud"]);
    }

    #[test]
    fn a_write_that_fails_leaves_no_temporary_file_behind() {
        // A directory where the recording should go: the rename onto it
        // fails, after the text was written to the temporary file.
        let dir = std::env::temp_dir().join(format!("jud-fsutil-unit-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("taken.jud")).unwrap();
        assert!(write_atomically(&dir.join("taken.jud"), |f| f.write_all(b"text")).is_err());
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
        write_atomically(&dir.join("fresh.jud"), |f| f.write_all(b"text")).unwrap();
        write_atomically(&dir.join("fresh.jud"), |f| f.write_all(b"again")).unwrap();
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
