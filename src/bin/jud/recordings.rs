//! The recordings under a directory, read the way `Replay::open` reads them:
//! the regular `.json` and `.jud` files, never a link or a directory, in
//! name order. Two things need to know which file a recording came from,
//! which `Replay` does not say: the provenance of a tuning run (the server
//! that answered) and the warning that two files record one request.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use judgment::eval::Recording;

/// One recording and the file it was read from.
pub(crate) struct Found {
    pub path: PathBuf,
    pub recording: Recording,
}

/// The recordings under `dir`, sorted by path. A file `Replay::open`
/// already read cannot fail here; one that does is left out.
pub(crate) fn scan(dir: &Path) -> Vec<Found> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.symlink_metadata()
                .is_ok_and(|meta| meta.file_type().is_file())
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let recording = match path.extension().and_then(|e| e.to_str()) {
                Some("json") => serde_json::from_str::<Recording>(&text).ok()?,
                Some(judgment::jud::EXTENSION) => judgment::jud::parse_recording(&text).ok()?,
                _ => return None,
            };
            Some(Found { path, recording })
        })
        .collect()
}

/// Request fingerprints that more than one file records, with the files.
/// `Replay` keeps one recording per fingerprint, so which of the files
/// answers is a matter of reading order; the files should not both be there.
pub(crate) fn duplicates(found: &[Found]) -> Vec<(String, Vec<&Path>)> {
    let mut by_fingerprint: BTreeMap<&str, Vec<&Path>> = BTreeMap::new();
    for f in found {
        if let Some(fingerprint) = f.recording.fingerprint.as_deref() {
            by_fingerprint
                .entry(fingerprint)
                .or_default()
                .push(f.path.as_path());
        }
    }
    by_fingerprint
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .map(|(fingerprint, paths)| (fingerprint.to_owned(), paths))
        .collect()
}

/// Print one note per fingerprint that two files record, naming them.
pub(crate) fn warn_duplicates(dir: &Path) {
    for (fingerprint, paths) in duplicates(&scan(dir)) {
        let names: Vec<String> = paths
            .iter()
            .map(|p| {
                p.file_name().map_or_else(
                    || p.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                )
            })
            .collect();
        crate::out::note!(
            "jud: warning: {} files under {} record the same request ({fingerprint}): {}; a replay answers from one of them, delete the one that is stale",
            names.len(),
            dir.display(),
            names.join(", ")
        );
    }
}
