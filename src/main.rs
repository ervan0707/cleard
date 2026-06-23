//! cleard — an interactive, multi-ecosystem build-artifact disk reclaimer.

mod cli;
mod config;
mod delete;
mod model;
mod rules;
mod scanner;
mod tui;

use anyhow::{Context, Result};
use clap::Parser;
use humansize::{format_size, BINARY};

use crate::cli::Cli;
use crate::model::AppState;
use crate::scanner::ScanOptions;

fn main() -> Result<()> {
    let args = Cli::parse();

    let root = args
        .path
        .canonicalize()
        .with_context(|| format!("cannot access path {}", args.path.display()))?;
    if !root.is_dir() {
        anyhow::bail!("{} is not a directory", root.display());
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
        tx,
    );

    let app = AppState::new(root, args.dry_run, args.min_size);
    let reclaimed = tui::run(app, rx)?;

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
