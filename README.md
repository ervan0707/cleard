# cleard

`npkill`, but for **everything** — an interactive terminal tool that finds
regenerable build / dependency / cache directories across many ecosystems and
lets you delete them to reclaim disk space.

It recursively scans a directory, streams matches into a live list sorted by
size, and deletes the ones you pick — with a running total of space reclaimed.

## Why it's safe

Unlike a plain name match, `cleard` is **marker-aware**: an ambiguous directory
is only flagged when its project marker is a sibling. `target/` is only a
candidate when a `Cargo.toml` (or `pom.xml`) sits next to it; a hand-written
`build/` source folder with no build-system marker is left untouched.
Unambiguous names (`node_modules`, `__pycache__`, `.terraform`, …) need no marker.

Deletion is bounded to the scan root and always asks for confirmation.

## Supported out of the box

Node (`node_modules`, `.next`, `.nuxt`, `.svelte-kit`, `.turbo`), Rust (`target`),
Java/Gradle/Maven (`build`, `.gradle`, `target`), Python (`.venv`, `venv`,
`__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, `*.egg-info`),
Go & PHP (`vendor`), .NET (`bin`, `obj`), CocoaPods (`Pods`), Terraform
(`.terraform`), Elixir (`_build`, `deps`), Elm (`elm-stuff`), Dart/Flutter
(`.dart_tool`). Extendable via config.

## Usage

```sh
cleard                 # scan the current directory
cleard ~/code          # scan a specific directory
cleard --dry-run       # show what would be reclaimed, delete nothing
cleard --min-size 100M # hide candidates smaller than 100 MiB
cleard -x .git -x dist # skip directories by name
```

### Keys

| Key | Action |
| --- | --- |
| `↑`/`k`, `↓`/`j` | move cursor |
| `g` / `G` | top / bottom |
| `space` | toggle selection |
| `a` / `c` | select all / clear selection |
| `d` / `Del` | delete selected (or focused) |
| `Enter` | delete focused |
| `s` | cycle sort (size / age / path) |
| `/` | filter by path or ecosystem |
| `?` | help |
| `q` | quit |

## Config

Optional, at `~/.config/cleard/config.toml`:

```toml
# use_default_rules = true   # set false to use only the rules below

[[rules]]
ecosystem = "Zig"
dir_names = ["zig-cache", "zig-out"]
markers = ["build.zig"]      # omit for a "safe by name" rule
```

`dir_names` and `markers` accept a single `*` glob (e.g. `*.csproj`).

## Develop (Nix)

```sh
nix develop          # dev shell with the pinned Rust toolchain + rust-analyzer
cargo run -- ./path
cargo test

nix build            # build the release binary -> ./result/bin/cleard
nix run . -- ~/code  # build and run
```

(With `direnv`, `direnv allow` loads the dev shell automatically.)

## License

MIT
