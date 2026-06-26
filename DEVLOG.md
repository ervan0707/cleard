# DEVLOG

A running history of what we built in `cleard`, why, and how. Newest entries
go on top. Update this every time the project changes (see CLAUDE.md).

---

## 2026-06-26 — README badges, MIT LICENSE, author email

### What

- Added a row of shields.io badges to the top of the README (crates.io, npm,
  PyPI, GitHub release, CI, license), each linking to its page.
- Added a real `LICENSE` file (MIT) — the crate/package metadata declared MIT
  but no license file existed, which crates.io warns about. The license badge
  now links to it.
- Set the author email to `ervanroot@gmail.com` across `Cargo.toml`,
  `pyproject.toml`, and both npm manifests (`package.json` +
  `package.json.tmpl`).

### Why / how

`LICENSE` is intentionally not in `Cargo.toml`'s `exclude` list, so it ships in
the published crate. The version badges read live from each registry, so they
stay blank until the next release populates them.

---

## 2026-06-26 — Scope the npm package (1.0.0 release fallout)

### What

The first 1.0.0 run published crates.io, PyPI, and the six npm platform
sub-packages, but the **main npm package failed**: npm rejects the bare name
`cleard` as "too similar to existing package `clear`" (typosquat protection).
Renamed the main package to the scoped **`@ervan0707/cleard`** and added
`publishConfig.access = "public"`. Install is now `npm i -g @ervan0707/cleard`;
the binary it puts on PATH is still `cleard`.

Also changed `"bin"` from the string form to `{ "cleard": "lib/index.js" }`.

### Why

- Scoped names skip npm's similarity check, so `@ervan0707/cleard` is guaranteed
  to publish where `cleard` can't.
- For a scoped package the string `bin` form would name the command after the
  package (`@ervan0707/cleard`), which isn't a valid command. The object form
  pins the command name to `cleard` (this also clears the "bin was converted to
  an object" npm warning).

### How / fallout to know about

semantic-release creates and pushes the git tag *before* the publish plugins
run, so even though the npm step failed, `v1.0.0` + the `chore(release): 1.0.0`
commit landed on `main`, and crates.io/PyPI/sub-packages are at 1.0.0. But the
`@semantic-release/github` step runs *after* npm, so **no GitHub Release (and no
binaries) was created for 1.0.0**, which also leaves the `curl | bash` installer
broken (it reads the latest release).

Fix path: this rename ships as a `fix:` commit, so the next push cuts a clean
**1.0.1** across every channel and creates the GitHub Release + binaries that
1.0.0 never got. The 1.0.0 artifacts on crates.io/PyPI are immutable but
harmless; users just land on 1.0.1. The orphan `v1.0.0` tag (no release
attached) can be left as-is or deleted.

---

## 2026-06-26 — Multi-registry release pipeline (npm, PyPI, crates.io, GitHub)

### What

Implemented the release strategy in `RELEASE-STRATEGY.md`: one push of a
Conventional-Commit `feat:`/`fix:` to `main` now versions and publishes `cleard`
to **four** targets at once — crates.io, npm, PyPI, and GitHub Releases — on top
of the existing Nix/Cachix channel.

New files:

- `.releaserc.json` — semantic-release config; commit-analyzer is the single
  version authority, then exec/npm/github/git plugins stamp + publish + tag.
- `.github/workflows/release.yml` — the pipeline: a dry-run `prepare` gate, a
  6-target build matrix (maturin builds both the binary and the Python wheel),
  parallel best-effort publishers, then the real semantic-release run.
- `pyproject.toml` — maturin with `bindings = "bin"` so `pip install` ships the
  binary, not a Python module; `dynamic = ["version"]` reads it from Cargo.toml.
- `packages/npm/` — the `optionalDependencies` + `os`/`cpu` pattern: a main
  `cleard` package with a JS shim (`lib/index.js`) that execs the right
  per-platform sub-package, plus `package.json.tmpl` for the 6 sub-packages.
- `install.sh` — `curl | bash` installer pulling the matching GitHub Release
  asset.

Changed `Cargo.toml`: added `authors`/`homepage`/`documentation` and an
`exclude` list so the published crate drops all the CI/packaging cruft. Updated
the README with the five install commands.

### Why

Meet users in whatever ecosystem they already live in, while compiling from
source only on crates.io — npm/PyPI/curl are thin wrappers over the same
prebuilt binary. Versioning is fully automated from commit messages so all
channels always publish the identical version with no hand-bumping.

### How / gotchas

- The version is computed **once** by the dry-run gate and threaded to every job
  as artifacts (stamped `Cargo.toml` + `package.json`), never recomputed — no
  drift between registries. The real tag/commit happens last, so a failed build
  never leaves a dangling tag.
- Keep the **`x64` (npm) vs `x86_64` (release asset)** naming split intact — the
  workflow translates between them; the shim uses Node's `process.arch` (`x64`).
- The `.releaserc.json` first `exec` perl command is copied verbatim from the
  strategy (heavy quote escaping) — only `APP`→`cleard` was swapped.
- Nix is untouched and stays separate: `flake.nix`/`flake.lock` are in the
  crate's `exclude` list, and the existing `ci.yml` (Cachix) is a distinct
  workflow from `release.yml`.
- **Out-of-repo TODO before the first release** (can't be done from here):
  reserve the names on crates.io / npm (incl. all 6 `cleard-<os>-<arch>`
  sub-packages) / PyPI, and add the 4 GitHub secrets: `NPM_TOKEN`,
  `PYPI_API_TOKEN`, `CRATES_IO_TOKEN` (`GITHUB_TOKEN` is automatic).

---

## 2026-06-25 — Build + cache all four systems via a CI matrix

### What

Turned the single-runner CI job into a matrix so every system the flake defines
gets built and pushed to Cachix:

- `ubuntu-latest` -> `x86_64-linux`
- `ubuntu-24.04-arm` -> `aarch64-linux`
- `macos-13` -> `x86_64-darwin`
- `macos-14` -> `aarch64-darwin`

### Why

The previous job ran only on `ubuntu-latest`, so `nix build` only realised
`x86_64-linux` and that was the sole system landing in the cache. Mac and ARM
users still compiled from source. Each platform needs a real runner (no
cross-compile set up here), so a matrix is the way to cover all four.

### How

`nix build` builds the runner's native system, so each matrix leg covers one
system with no extra flags. `fail-fast: false` so one platform breaking doesn't
cancel the others. Test and audit run on every leg too (the filesystem walk is
worth testing per-platform; the audit is redundant across legs but cheap).

---

## 2026-06-25 — Publish builds to a Cachix binary cache

### What

Wired the flake up to the `skinnyvans` Cachix cache so users can download the
prebuilt `cleard` binary instead of compiling Rust from source.

- `flake.nix`: added `nixConfig.extra-substituters` /
  `extra-trusted-public-keys` pointing at `skinnyvans.cachix.org`.
- `.github/workflows/ci.yml`: added the `cachix/cachix-action` step (pulls +
  pushes, authed via the `CACHIX_AUTH_TOKEN` repo secret) and switched the build
  step from `nix develop -c cargo build` to `nix build`.
- `README.md`: added a "Binary cache" section documenting `cachix use skinnyvans`
  and the manual substituter/key config.

### Why

Compiling the Rust toolchain build from scratch is slow for anyone running
`nix run github:ervan0707/cleard`. A shared cache makes first run a download.
Also retired `magic-nix-cache-action` — Determinate Systems shut that service
down in Feb 2025, so it was effectively dead weight in CI.

### How

`cachix-action` uploads every store path realised during the job. The old CI
built inside the dev shell (`cargo build` → `./target`), which produces no Nix
store output, so nothing cacheable was created — `nix build` produces the actual
flake package derivation, which is what gets pushed. Tests/audit still run in the
dev shell. The `nixConfig` substituter only auto-applies for trusted users;
others run `cachix use skinnyvans` (documented in the README).

Out-of-repo setup (done on cachix.org / GitHub, not in this commit): the
`skinnyvans` cache exists and a `CACHIX_AUTH_TOKEN` secret must be added under
the repo's Actions secrets for pushes to succeed.

---

## 2026-06-25 — Shrink the installed closure from ~2.8 GB to ~46 MiB

### What

Added a `postInstall` step to the flake's package that runs
`remove-references-to -t ${rustToolchain}` over `$out/bin/cleard`. The runtime
closure dropped from 2.79 GB to 45.6 MiB (the remainder is `libiconv`, which
every Darwin binary links).

### Why

`nix profile install` was pulling ~2 GB into the store for a 1.6 MB binary.
The binary's only direct reference was the entire Rust toolchain
(`rust-default`), which transitively drags in LLVM and cctools/binutils-darwin
(~1.3 GB on its own). Nobody actually needs the toolchain at runtime — it was
being retained purely by accident.

### How

The reference wasn't an rpath or a linked dylib (`otool -l` showed none). It was
33 plain string constants in the binary: Rust's panic/backtrace source-location
paths for `std`/`core`/`alloc`, e.g.
`…-rust-default-…/lib/rustlib/src/rust/library/std/src/io/mod.rs`. rust-overlay
keeps the std source tree inside the toolchain's store path, the precompiled std
bakes those paths in, and Nix's scanner sees the store hash and treats the whole
toolchain as a dependency. The strings are diagnostic only — nothing reads those
files at runtime — so `remove-references-to` nulls out the hash in place and the
dependency disappears. Verified the binary still runs (`cleard --version`) after
scrubbing. Gotcha: this is `nativeBuildInputs = [ pkgs.removeReferencesTo ]`
(camelCase attr) providing the `remove-references-to` binary (hyphenated).

## 2026-06-23 — Show selected total, stop cursor auto-advancing on select

### What

Two UX fixes: (1) the footer now shows the selected count and their combined
size (e.g. `3 selected (4.20 GiB)`) whenever anything is selected; (2) pressing
space to select no longer moves the cursor down, it stays on the current row.

### Why

You couldn't see how much you'd picked before deleting, and the auto-advance on
select made multi-select feel jumpy. Both reported by the user.

### How

Added `AppState::selected_summary()` -> `(count, bytes)` over selected,
not-deleted candidates; `draw_footer` renders it as a span shown only when the
count is > 0. Removed the `cursor += 1` from the `ToggleSelect` handler. Tests
still 10/10.

---

## 2026-06-23 — Expand the README config section

### What

Grew the README "Config" section from a single Zig snippet into a proper
explainer: what the file is for, what `ecosystem`/`dir_names`/`markers` mean (and
the safe-by-name vs marker behavior), the glob and precedence rules, plus two
examples (adding rules on top of the built-ins, and replacing them entirely).

### Why

The old snippet showed the syntax but not the semantics, so it wasn't obvious
when to use markers vs a name-only rule, or how to disable the built-in catalog.
Docs only, no code change.

---

## 2026-06-23 — Point repo URLs at ervan0707/cleard, document running from Git

### What

Set the real repo (`https://github.com/ervan0707/cleard`) everywhere there was a
placeholder (`Cargo.toml` `repository`, the flake comment, a DEVLOG line) and
added an "Install / run" section to the README showing how to run/install
straight from GitHub via Nix flakes.

### Why / how

`nix run github:ervan0707/cleard` builds and runs the flake straight from the
repo with no clone and no local Rust, which is the main distribution story. The
README now covers `nix run` (with a pinned tag form), `nix profile install`,
adding it as a flake input, and a Cargo fallback for non-Nix users. Docs +
metadata only, no code change.

---

## 2026-06-23 — Security review hardening

### What

Acted on a security review of the codebase (a tool that does irreversible
deletes). No High/Critical issues were found; these are the hardening changes:

- **ratatui 0.29 -> 0.30.** Clears both `cargo audit` warnings (`paste`
  unmaintained RUSTSEC-2024-0436, `lru` unsound RUSTSEC-2026-0002), which both
  came in transitively through ratatui. Audit is now clean (0 vulns, 0 warnings).
- **Shallow-root guard** (`main.rs`). Before scanning, if the canonical root is
  `/`, `$HOME`, or a top-level dir (<= 2 path components), print a warning and
  require a y/N confirmation. EOF or an unreadable stdin fails safe (aborts).
- **TOCTOU guard** (`delete.rs`). Right before `remove_dir_all`, `symlink_metadata`
  the resolved target and bail if it is no longer a real directory (caught being
  swapped for a symlink since the scan).
- **Terminal-escape safety.** Startup error paths now print with `{:?}` so a dir
  name containing control/escape sequences can't be injected into the terminal.
- **cargo-audit wired in.** Added to the dev shell and a GitHub Actions CI
  workflow (build + test + audit on push/PR).

### Why

Deletion is irreversible, so the threat model is "delete something unintended."
The existing guards (canonicalize + component-wise `starts_with` containment,
`follow_links=false` default, mandatory confirm) were already solid. These add
defense in depth: a smaller blast radius on broad roots, a narrower TOCTOU
window, no terminal-injection surface on printed paths, and automated CVE
scanning so transitive advisories surface in CI.

### How / gotchas

- ratatui 0.30 made `Backend::Error` an associated type that is not Send/Sync,
  so `terminal.draw(...)?` / `show_cursor()?` no longer convert into
  `anyhow::Error` from a generic `B: Backend`. Fixed by making `event_loop` and
  `restore_terminal` concrete over `Terminal<CrosstermBackend<Stdout>>` (error is
  `io::Error`, which is Send + Sync).
- Dropped the direct `crossterm` dependency; the TUI now uses
  `ratatui::crossterm` so there's only one crossterm version in the tree.
- Dep count grew (131 -> 227 crates) because ratatui 0.30 is split into several
  sub-crates. Audit stays clean.
- Verified: build clean, 10/10 tests pass, `cargo audit` clean, normal fixture
  delete still works (TOCTOU re-check doesn't break the happy path), and the
  guard aborts safely on `/` with non-interactive stdin.

---

## 2026-06-23 — Don't flicker unsized rows when --min-size is set

### What

When a `--min-size` threshold is set, directories whose size hasn't been
computed yet are now hidden instead of shown. Without a threshold, behavior is
unchanged (freshly found dirs still appear instantly with a `…` size).

### Why

The scan is a streaming pipeline: the scanner emits `Found` with `size: None`
immediately, and the sizer pool fills in sizes later. The min-size filter used
`size.map(|s| s >= min).unwrap_or(true)`, so unsized rows passed the filter and
showed up, then dropped out once measured below the threshold. With a large
threshold (e.g. `--min-size 10000M`) every row flickered in during the scan and
then vanished, which looked like a bug. Reported by the user.

### How

`view_indices` (`src/model.rs`) now matches on `size`: `Some(s) => s >= min`,
`None => min == 0`. So pending rows are shown only when there's no threshold,
and once a threshold is active rows appear only after their size confirms they
qualify. One-line behavior change, tests still 10/10.

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
  toolchain and `nix run github:ervan0707/cleard` with no local Rust needed.
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
