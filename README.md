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

## Install

`cleard` ships one binary through whichever ecosystem you already use — npm,
PyPI, and the `curl` script all deliver the same prebuilt binary; Cargo and Nix
build from source.

```sh
cargo install cleard                       # crates.io (compiles from source)
npm install -g cleard                      # prebuilt binary via npm
pip install cleard                         # prebuilt binary in a Python wheel
curl -fsSL https://raw.githubusercontent.com/ervan0707/cleard/main/install.sh | bash
nix profile install github:ervan0707/cleard
```

## Run

With Nix flakes, run it straight from GitHub (nothing to clone, no Rust needed):

```sh
# run once against a directory
nix run github:ervan0707/cleard -- ~/code

# run the current directory
nix run github:ervan0707/cleard

# pin a tag/branch/commit
nix run github:ervan0707/cleard/v0.1.0 -- ~/code
```

Install it into your profile:

```sh
nix profile install github:ervan0707/cleard
cleard ~/code
```

### Binary cache (skip the build)

CI publishes prebuilt outputs to [Cachix](https://www.cachix.org), so you can
download the binary instead of compiling Rust. The flake advertises the cache
via `nixConfig`, which Nix uses automatically if you're a trusted user.
Otherwise, opt in once:

```sh
cachix use skinnyvans
```

Or add it to your Nix config by hand:

```
substituters = https://skinnyvans.cachix.org
trusted-public-keys = skinnyvans.cachix.org-1:sgaZPgRhzsU4YScjc2U5Imc+4E3y9Ov/G/q8p/csX+o=
```

Or add it to your own flake:

```nix
{
  inputs.cleard.url = "github:ervan0707/cleard";
  # then use cleard.packages.${system}.default in your outputs
}
```

Don't have Nix? Build from source with Cargo:

```sh
git clone https://github.com/ervan0707/cleard
cd cleard
cargo build --release   # binary at ./target/release/cleard
```

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

Optional, at `~/.config/cleard/config.toml` (or pass `--config <path>`). Use it to
add your own detection rules, or to replace the built-in catalog entirely. If the
file doesn't exist, the built-in rules are used.

Each rule has:

- `ecosystem` — the label shown in the list.
- `dir_names` — directory names to match.
- `markers` — sibling files that must exist for a match. With markers, the dir is
  only flagged when one sits next to it (so a hand-written `zig-out/` with no
  `build.zig` is left alone). Omit `markers` to match the name anywhere ("safe by
  name") — only do that for unambiguous names.

`dir_names` and `markers` accept a single `*` glob (e.g. `*.csproj`, `*.egg-info`).
Your rules are checked before the built-ins, so they win on overlapping names.

Add a few ecosystems on top of the built-ins:

```toml
# use_default_rules = true   # default; keep the built-in catalog

[[rules]]
ecosystem = "Zig"
dir_names = ["zig-cache", "zig-out"]
markers = ["build.zig"]

[[rules]]
ecosystem = "Bazel"
dir_names = ["bazel-bin", "bazel-out", "bazel-testlogs"]
markers = ["WORKSPACE", "WORKSPACE.bazel", "MODULE.bazel"]

[[rules]]
ecosystem = "CMake"
dir_names = ["CMakeFiles"]   # unambiguous name, no marker needed
```

Or replace the built-ins entirely and clean only what you list:

```toml
use_default_rules = false

[[rules]]
ecosystem = "Node"
dir_names = ["node_modules"]

[[rules]]
ecosystem = "Rust"
dir_names = ["target"]
markers = ["Cargo.toml"]
```

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
