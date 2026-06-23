//! Parallel, marker-aware directory scanner built on `jwalk`.
//!
//! `jwalk`'s `process_read_dir` callback hands us the full child list of each
//! directory at once. That is exactly what marker-aware detection needs (we can
//! check a candidate's *siblings* for a marker file) and it lets us prune: once
//! a directory is recognised as an artifact we set `read_children_path = None`
//! so the walk never descends into it (no nested `node_modules` rescans, and we
//! don't waste time walking a tree we're about to size wholesale).
//!
//! Discovery and sizing are decoupled: the walk streams `Found` messages to the
//! UI immediately, while a small pool of sizer threads computes directory sizes
//! in the background and streams `Sized` updates as they complete.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use crossbeam_channel::Sender;
use jwalk::{Parallelism, WalkDirGeneric};

use crate::model::Candidate;
use crate::rules::RuleSet;

/// Messages streamed from the scan/sizing threads to the UI thread.
pub enum ScanMsg {
    Found(Candidate),
    Sized {
        id: usize,
        bytes: u64,
        mtime: Option<SystemTime>,
    },
    /// The directory walk has finished (no more `Found` will arrive).
    ScanDone,
    /// All sizing has finished (no more `Sized` will arrive).
    SizingDone,
}

pub struct ScanOptions {
    pub root: PathBuf,
    pub rules: RuleSet,
    /// Directory names to skip entirely (never reported, never descended).
    pub excludes: Vec<String>,
    pub follow_links: bool,
}

/// Spawn the scanner on a background thread; results arrive on `ui_tx`.
pub fn spawn(opts: ScanOptions, ui_tx: Sender<ScanMsg>) {
    std::thread::spawn(move || run(opts, ui_tx));
}

fn run(opts: ScanOptions, ui_tx: Sender<ScanMsg>) {
    let counter = Arc::new(AtomicUsize::new(0));
    let rules = Arc::new(opts.rules);
    let excludes = Arc::new(opts.excludes);

    // Background sizing pool consuming (id, path) work items.
    let (size_tx, size_rx) = crossbeam_channel::unbounded::<(usize, PathBuf)>();
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 8);
    let mut sizers = Vec::with_capacity(workers);
    for _ in 0..workers {
        let rx = size_rx.clone();
        let tx = ui_tx.clone();
        sizers.push(std::thread::spawn(move || {
            while let Ok((id, path)) = rx.recv() {
                let (bytes, mtime) = dir_size_and_mtime(&path);
                let _ = tx.send(ScanMsg::Sized { id, bytes, mtime });
            }
        }));
    }
    drop(size_rx); // only the sizer clones should hold the receiver.

    // The directory walk. The closure runs on rayon worker threads, so every
    // captured value is shared via Arc / channel handles (all Send + Sync).
    {
        let counter = counter.clone();
        let rules = rules.clone();
        let excludes = excludes.clone();
        let found_tx = ui_tx.clone();
        let size_tx = size_tx.clone();

        let walk = WalkDirGeneric::<((), ())>::new(&opts.root)
            .skip_hidden(false)
            .follow_links(opts.follow_links)
            .process_read_dir(move |_depth, _dir_path, _state, children| {
                // Names of every sibling in this directory — used for markers.
                let sibling_names: Vec<String> = children
                    .iter()
                    .filter_map(|r| r.as_ref().ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();

                for child in children.iter_mut() {
                    let Ok(entry) = child else { continue };
                    if !entry.file_type().is_dir() {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().into_owned();

                    if excludes.iter().any(|x| *x == name) {
                        entry.read_children_path = None; // skip excluded subtree
                        continue;
                    }

                    if let Some(eco) = rules.match_dir(&name, &sibling_names) {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        let path = entry.path();
                        let _ = found_tx.send(ScanMsg::Found(Candidate::new(
                            id,
                            path.clone(),
                            eco.to_string(),
                        )));
                        let _ = size_tx.send((id, path));
                        entry.read_children_path = None; // prune: don't descend
                    }
                }
            });

        for _ in walk {} // drive the walk; the closure does all the work.
    } // `walk` (and its captured `size_tx` clone) drops here.

    drop(size_tx); // close the work queue so sizers can finish.
    let _ = ui_tx.send(ScanMsg::ScanDone);
    for s in sizers {
        let _ = s.join();
    }
    let _ = ui_tx.send(ScanMsg::SizingDone);
}

/// Sum the byte size of all files under `path` and read the directory's own
/// last-modified time. Walks serially because the caller already runs several
/// of these concurrently — one rayon pool per call would oversubscribe cores.
fn dir_size_and_mtime(path: &Path) -> (u64, Option<SystemTime>) {
    let mtime = std::fs::symlink_metadata(path)
        .ok()
        .and_then(|m| m.modified().ok());

    let mut total: u64 = 0;
    for entry in WalkDirGeneric::<((), ())>::new(path)
        .skip_hidden(false)
        .follow_links(false)
        .parallelism(Parallelism::Serial)
    {
        if let Ok(e) = entry {
            if e.file_type().is_file() {
                if let Ok(md) = e.metadata() {
                    total += md.len();
                }
            }
        }
    }
    (total, mtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::default_rules;
    use std::fs;

    /// Collect every candidate path the scanner reports for `root`.
    fn scan_paths(root: &Path) -> Vec<(PathBuf, String)> {
        let (tx, rx) = crossbeam_channel::unbounded();
        run(
            ScanOptions {
                root: root.to_path_buf(),
                rules: default_rules(),
                excludes: Vec::new(),
                follow_links: false,
            },
            tx,
        );
        let mut out = Vec::new();
        while let Ok(msg) = rx.recv() {
            if let ScanMsg::Found(c) = msg {
                out.push((c.path, c.ecosystem));
            }
        }
        out
    }

    #[test]
    fn marker_aware_detection_and_pruning() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Rust project: target/ next to Cargo.toml -> should be flagged.
        let rust = root.join("proj-rust");
        fs::create_dir_all(rust.join("target/debug")).unwrap();
        fs::write(rust.join("Cargo.toml"), "[package]").unwrap();
        fs::write(rust.join("target/debug/app"), b"binary").unwrap();

        // Node project with nested node_modules -> outer flagged, inner pruned.
        let node = root.join("proj-node");
        fs::create_dir_all(node.join("node_modules/dep/node_modules")).unwrap();
        fs::write(node.join("package.json"), "{}").unwrap();

        // Decoy: a hand-written build/ with NO build-system marker -> ignored.
        let decoy = root.join("decoy");
        fs::create_dir_all(decoy.join("build")).unwrap();
        fs::write(decoy.join("index.html"), "<html>").unwrap();

        let found = scan_paths(root);
        let paths: Vec<&PathBuf> = found.iter().map(|(p, _)| p).collect();

        assert!(paths.iter().any(|p| p.ends_with("proj-rust/target")));
        assert!(paths.iter().any(|p| p.ends_with("proj-node/node_modules")));
        // The decoy build/ must NOT be flagged (marker-aware safety).
        assert!(!paths.iter().any(|p| p.ends_with("decoy/build")));
        // Pruning: the nested node_modules is never reported as a second hit.
        let nm_hits = paths
            .iter()
            .filter(|p| p.to_string_lossy().contains("node_modules"))
            .count();
        assert_eq!(nm_hits, 1, "nested node_modules should be pruned");
    }
}
