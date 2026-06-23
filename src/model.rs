//! Application state: the list of candidates and the view (sort/filter/cursor).

use std::path::PathBuf;
use std::time::SystemTime;

/// A reclaimable directory discovered by the scanner.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: usize,
    pub path: PathBuf,
    pub ecosystem: String,
    /// `None` until the sizer reports a value.
    pub size: Option<u64>,
    pub mtime: Option<SystemTime>,
    pub selected: bool,
    /// Set once the directory has been removed (kept in the list, struck out).
    pub deleted: bool,
}

impl Candidate {
    pub fn new(id: usize, path: PathBuf, ecosystem: String) -> Self {
        Candidate {
            id,
            path,
            ecosystem,
            size: None,
            mtime: None,
            selected: false,
            deleted: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Size,
    Age,
    Path,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            SortMode::Size => SortMode::Age,
            SortMode::Age => SortMode::Path,
            SortMode::Path => SortMode::Size,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Size => "size",
            SortMode::Age => "age",
            SortMode::Path => "path",
        }
    }
}

/// Whole-app state owned by the UI thread.
pub struct AppState {
    pub root: PathBuf,
    pub dry_run: bool,
    pub min_size: u64,
    candidates: Vec<Candidate>,
    pub sort: SortMode,
    pub filter: String,
    /// Cursor position within the current (sorted+filtered) view.
    pub cursor: usize,
    pub scanning: bool,
    pub sizing: bool,
    pub reclaimed: u64,
    /// Spinner animation tick.
    pub spinner: usize,
}

impl AppState {
    pub fn new(root: PathBuf, dry_run: bool, min_size: u64) -> Self {
        AppState {
            root,
            dry_run,
            min_size,
            candidates: Vec::new(),
            sort: SortMode::Size,
            filter: String::new(),
            cursor: 0,
            scanning: true,
            sizing: true,
            reclaimed: 0,
            spinner: 0,
        }
    }

    pub fn push(&mut self, c: Candidate) {
        self.candidates.push(c);
    }

    pub fn set_size(&mut self, id: usize, bytes: u64, mtime: Option<SystemTime>) {
        if let Some(c) = self.candidates.iter_mut().find(|c| c.id == id) {
            c.size = Some(bytes);
            c.mtime = mtime;
        }
    }

    /// Total bytes of all not-yet-deleted candidates that pass the size filter.
    pub fn reclaimable(&self) -> u64 {
        self.candidates
            .iter()
            .filter(|c| !c.deleted)
            .filter_map(|c| c.size)
            .filter(|&s| s >= self.min_size)
            .sum()
    }

    pub fn found_count(&self) -> usize {
        self.view_indices().len()
    }

    /// Indices into `candidates`, ordered and filtered for display.
    pub fn view_indices(&self) -> Vec<usize> {
        let f = self.filter.to_lowercase();
        let mut idx: Vec<usize> = self
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| c.size.map(|s| s >= self.min_size).unwrap_or(true))
            .filter(|(_, c)| {
                f.is_empty()
                    || c.path.to_string_lossy().to_lowercase().contains(&f)
                    || c.ecosystem.to_lowercase().contains(&f)
            })
            .map(|(i, _)| i)
            .collect();

        match self.sort {
            SortMode::Size => idx.sort_by(|&a, &b| {
                self.candidates[b]
                    .size
                    .unwrap_or(0)
                    .cmp(&self.candidates[a].size.unwrap_or(0))
            }),
            SortMode::Age => idx.sort_by(|&a, &b| {
                // Oldest first (largest age). None mtime sinks to the bottom.
                self.candidates[a]
                    .mtime
                    .cmp(&self.candidates[b].mtime)
            }),
            SortMode::Path => idx.sort_by(|&a, &b| {
                self.candidates[a].path.cmp(&self.candidates[b].path)
            }),
        }
        idx
    }

    pub fn get(&self, idx: usize) -> &Candidate {
        &self.candidates[idx]
    }

    pub fn candidate_mut(&mut self, idx: usize) -> &mut Candidate {
        &mut self.candidates[idx]
    }

    pub fn select_all_in_view(&mut self) {
        for i in self.view_indices() {
            if !self.candidates[i].deleted {
                self.candidates[i].selected = true;
            }
        }
    }

    pub fn clear_selection(&mut self) {
        for c in &mut self.candidates {
            c.selected = false;
        }
    }

    pub fn clamp_cursor(&mut self) {
        let n = self.found_count();
        if n == 0 {
            self.cursor = 0;
        } else if self.cursor >= n {
            self.cursor = n - 1;
        }
    }

    /// Resolve the candidate currently under the cursor, if any.
    pub fn focused_index(&self) -> Option<usize> {
        self.view_indices().get(self.cursor).copied()
    }

    /// Candidate indices selected for deletion (or the focused one if none
    /// are explicitly selected). Excludes already-deleted entries.
    pub fn deletion_targets(&self) -> Vec<usize> {
        let view = self.view_indices();
        let selected: Vec<usize> = view
            .iter()
            .copied()
            .filter(|&i| self.candidates[i].selected && !self.candidates[i].deleted)
            .collect();
        if !selected.is_empty() {
            return selected;
        }
        match self.focused_index() {
            Some(i) if !self.candidates[i].deleted => vec![i],
            _ => Vec::new(),
        }
    }
}
