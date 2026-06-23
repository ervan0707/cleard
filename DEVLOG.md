# DEVLOG

A running history of what we built in `cleard`, why, and how. Newest entries
go on top. Update this every time the project changes (see CLAUDE.md).

---

## 2026-06-23 — Delete on a background thread (fix UI freeze)

### What

Deletion no longer blocks the UI. Pressing `d` then `y` hands the selected
directories to a background worker thread; the event loop keeps drawing and
reading input while rows strike out as each removal completes, and the footer
shows a `deleting…` indicator.

### Why

Deletion ran synchronously on the UI thread, so confirming a large batch (the
user hit it with ~11.87 GiB selected) called `remove_dir_all` on every target
before the loop could redraw or read a key. The screen looked frozen for many
seconds — it was just busy deleting, and only "came back" once the batch
finished. Reported by the user.

### How

Added `ScanMsg::{Deleted, DeleteFailed, DeleteBatchDone}` and reused the existing
scan channel. `main` keeps a `Sender` clone and passes it to `tui::run`. On
confirm, `spawn_deletion` captures stable candidate **ids** (not view indices,
which shift as rows are struck out) as `(id, path, size)`, sets
`app.deleting = true`, and spawns a thread that deletes each and reports back.
The drain loop applies `Deleted` (mark deleted + add to `reclaimed`),
`DeleteFailed` (surface the error, leave the row so it can be retried), and
clears `deleting` on `DeleteBatchDone`. Re-entrant delete requests are ignored
while a batch runs.

Gotcha: quitting (`q`) mid-batch detaches the worker — already-removed dirs stay
removed; a dir caught mid-removal may be left partially deleted. Acceptable for
now. Tests still 10/10.

---

## 2026-06-23 — Initial build: scaffold, scanner, TUI, Nix packaging

### What

First working version of `cleard`: an interactive, multi-ecosystem disk
reclaimer. Think `npkill`, but instead of only finding `node_modules` it finds
regenerable build/dependency/cache directories across many ecosystems, shows
them in a live TUI sorted by size, and deletes the ones you pick.

Shipped in this pass:

- Project scaffold: `Cargo.toml`, `rust-toolchain.toml`, `.envrc`, `.gitignore`,
  `README.md`.
- Nix flake with a dev shell (pinned Rust toolchain + rust-analyzer) and a
  package output, so `nix develop`, `nix build`, and `nix run` all work.
- `rules.rs`: marker-aware rule catalog + matching logic.
- `config.rs`: optional user config at `~/.config/cleard/config.toml`.
- `scanner.rs`: parallel `jwalk` walk with sibling-marker detection, pruning,
  and a background sizer pool.
- `model.rs`: app state (sort, filter, selection, cursor, reclaimed total).
- `delete.rs`: guarded directory removal.
- `cli.rs` + `main.rs`: argument parsing and wiring.
- `tui/`: ratatui event loop, rendering, and keybindings.
- Tests: 10 passing (rules, scanner pruning/safety, delete guards, size parse).

### Why these decisions

- **Rust.** A disk scanner lives or dies on fast parallel directory walking,
  and a single static binary is easy to hand out. Confirmed with the user.
- **Nix flake for dev + distribution.** User asked for it. Gives a reproducible
  toolchain and `nix run github:<user>/cleard` with no local Rust needed.
- **Marker-aware detection (not plain name matching).** The real danger in a
  multi-ecosystem cleaner is nuking a folder that looks like an artifact but is
  actually source (a hand-written `build/` or `dist/`). So an ambiguous dir is
  only flagged when its project marker sits next to it: `target/` needs a
  sibling `Cargo.toml`, `vendor/` needs `go.mod`/`composer.json`, `bin`/`obj`
  need a `*.csproj`/`*.sln`. Unambiguous names (`node_modules`, `__pycache__`,
  `.terraform`) are safe by name and need no marker.
- **Broad built-in catalog, user-overridable.** Node, Rust, Python, Go,
  Java/Gradle/Maven, .NET, PHP, CocoaPods, Terraform, Elixir, Elm, Dart out of
  the box; extra rules via config.
- **Interactive TUI as the main interface.** Matches the npkill experience the
  user wanted. Headless `--json`/`--yes` was left as a later option.

### How it fits together

```
scanner thread            sizer pool                 UI thread (ratatui)
 jwalk + rule match  -->   sum bytes in parallel  --> live list, sort, input,
 marker check, prune       (one job per dir)          delete on confirm
        \___________ crossbeam-channel (ScanMsg) ___________/
```

The walk uses `jwalk`'s `process_read_dir` callback, which hands us a
directory's full child list at once. That is exactly what the marker check
needs (look at a candidate's siblings) and it lets us prune by setting
`read_children_path = None` so we never descend into a matched artifact dir
(no nested `node_modules` rescans). Discovery streams `Found` messages to the UI
immediately; sizing happens in the background and streams `Sized` updates.
Deletion is bounded to the scan root and always confirmed.

### Build notes / gotchas

- **Toolchain bump 1.83 -> 1.90.** First build pinned Rust 1.83. The resolved
  deps needed newer Cargo: `clap_lex` wants the `edition2024` feature (Cargo
  1.85+) and `darling` wants 1.88. Bumped the pin to 1.90 and it built clean.
- **Flake hardened with `makeRustPlatform`.** `buildRustPackage` defaults to
  nixpkgs' own rustc (1.95 here), which works but can drift. Switched the
  package to build with the same pinned 1.90 toolchain as the dev shell so the
  package and dev environment never disagree.
- **Nix + a fresh git repo.** There was a `.git` at the project root with
  nothing tracked. Nix flakes only copy git-tracked files, so the flake source
  in the store was "missing" `flake.nix`. Fix: `git add -A` to stage the files
  (no commit needed) so Nix can see them.

### Verified

- `cargo test`: 10/10 pass.
- `nix build` + `nix run`: produce and run a ~1.9 MB binary.
- End-to-end against a fixture tree: `target` (Cargo.toml), nested
  `node_modules` (package.json, counted once), and `__pycache__` were detected
  and deleted; a decoy `build/` with no marker was left alone; all source files
  survived. Dry-run reported `Reclaimed 7.50 MiB` without touching disk.
