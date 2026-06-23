//! cleard — an interactive, multi-ecosystem build-artifact disk reclaimer.

mod cli;
mod config;
mod delete;
mod model;
mod rules;
mod scanner;
mod tui;

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use clap::Parser;
use humansize::{format_size, BINARY};

use crate::cli::Cli;
use crate::model::AppState;
use crate::scanner::ScanOptions;

fn main() -> Result<()> {
    let args = Cli::parse();

    // Paths are printed with `{:?}` so a directory name containing terminal
    // control/escape sequences can't be injected into the terminal here.
    let root = args
        .path
        .canonicalize()
        .with_context(|| format!("cannot access path {:?}", args.path))?;
    if !root.is_dir() {
        anyhow::bail!("{:?} is not a directory", root);
    }

    // Guard against pointing cleard at a very broad root (/, $HOME, a top-level
    // dir). Containment only bounds deletion to the root, so a broad root means
    // a broad blast radius. Confirm before scanning anything that wide.
    if let Some(reason) = risky_root_reason(&root) {
        if !confirm_risky_root(&reason, &root)? {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    let mut cfg = config::load(args.config.as_ref())?;
    if args.no_default_rules {
        cfg.use_default_rules = false;
    }
    let rules = cfg.into_ruleset();

    // Start scanning immediately; results stream to the UI as they arrive.
    let (tx, rx) = crossbeam_channel::unbounded();
    scanner::spawn(
        ScanOptions {
            root: root.clone(),
            rules,
            excludes: args.exclude,
            follow_links: args.follow_links,
        },
        tx.clone(),
    );

    let app = AppState::new(root, args.dry_run, args.min_size);
    // `tx` is kept so the UI can spawn background deletion workers that report
    // progress back over the same channel.
    let reclaimed = tui::run(app, rx, tx)?;

    if reclaimed > 0 {
        let suffix = if args.dry_run {
            " (dry-run — nothing was deleted)"
        } else {
            ""
        };
        println!("Reclaimed {}{}", format_size(reclaimed, BINARY), suffix);
    }
    Ok(())
}

/// Returns a human reason when `root` is too broad to scan without a heads-up.
fn risky_root_reason(root: &Path) -> Option<String> {
    if root == Path::new("/") {
        return Some("the filesystem root".to_string());
    }
    if let Some(home) = dirs::home_dir() {
        if root == home {
            return Some("your home directory".to_string());
        }
    }
    // A very shallow path: "/" is 1 component, "/Users" / "/opt" are 2.
    if root.components().count() <= 2 {
        return Some("a top-level directory".to_string());
    }
    None
}

/// Ask the user to confirm scanning a broad root. A non-interactive stdin
/// (EOF) is treated as "no" so scripts fail safe.
fn confirm_risky_root(reason: &str, root: &Path) -> Result<bool> {
    eprintln!(
        "Warning: {:?} is {}. This will surface build/dependency dirs across everything beneath it.",
        root, reason
    );
    eprint!("Continue? [y/N] ");
    std::io::stderr().flush().ok();

    let mut line = String::new();
    // EOF (0) or an unreadable/non-interactive stdin both fail safe: abort.
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => return Ok(false),
        Ok(_) => {}
    }
    Ok(matches!(line.trim(), "y" | "Y" | "yes" | "Yes"))
}
