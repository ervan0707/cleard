# CLAUDE.md

Guidance for Claude Code (and any agent) working in this repo.

## Always update the devlog

**Every time you make a change to this project, add an entry to `DEVLOG.md`.**

- Put the newest entry at the top, under a dated heading (`## YYYY-MM-DD — short title`).
- Cover three things: **what** changed, **why** (the intent or tradeoff, not just
  the diff), and **how** (the approach, anything non-obvious, any gotchas).
- Keep it short and plain. Write like you're telling a coworker what you did.
- Do this as part of the same change, before committing. A code change without a
  devlog entry is incomplete.

## Project shape

`cleard` is an interactive, multi-ecosystem build-artifact disk reclaimer
(npkill, but for every ecosystem). Rust + ratatui TUI, built and distributed
with a Nix flake.

Source layout:

- `src/rules.rs` — marker-aware rule catalog + matching. The core of safety.
- `src/config.rs` — optional user config (`~/.config/cleard/config.toml`).
- `src/scanner.rs` — `jwalk` parallel walk, marker detection, pruning, sizer pool.
- `src/model.rs` — app state (sort, filter, selection, reclaimed total).
- `src/delete.rs` — guarded removal (bounded to the scan root).
- `src/cli.rs`, `src/main.rs` — args and wiring.
- `src/tui/` — event loop, rendering, keybindings.

## Build, test, run

Use the Nix dev shell for everything (there is no system Rust):

```sh
nix develop -c cargo build
nix develop -c cargo test
nix develop -c cargo run -- <path>

nix build            # release binary at ./result/bin/cleard
nix run . -- <path>
```

The pinned toolchain lives in `rust-toolchain.toml`; the flake builds the
package with that same toolchain (via `makeRustPlatform`), so keep them in sync.

## Conventions

- Marker-aware detection is the whole point. When adding a rule for an ambiguous
  directory name (one that could also be hand-written source, like `build` or
  `dist`), require a sibling marker file. Only mark a rule "safe by name" when
  the name is unambiguous (`node_modules`, `__pycache__`, `.terraform`).
- Deletion must stay bounded to the scan root and stay behind a confirm prompt.
- Add or update tests when you touch `rules.rs`, `scanner.rs`, or `delete.rs`.

## Nix gotcha

Flakes only see git-tracked files. After adding new files, `git add -A` before
running `nix build`/`nix develop`, or Nix will report the file as missing.
