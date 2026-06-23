//! Marker-aware rules describing which directories are safe to reclaim.
//!
//! A rule matches a directory by *name* (e.g. `target`). For ambiguous names
//! (`target`, `build`, `vendor`, `bin`, `obj`, …) the rule also requires a
//! sibling *marker* file (e.g. `Cargo.toml`) to exist in the same parent
//! directory before the match is accepted — this prevents deleting a
//! hand-written source folder that merely happens to be named `build`.
//!
//! Names and markers support a single trailing/leading `*` glob, which is all
//! the real-world cases need (`*.csproj`, `*.egg-info`).

use serde::Deserialize;

/// One detection rule.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Human label shown in the UI, e.g. "Rust" or "Node".
    pub ecosystem: String,
    /// Directory names this rule matches (glob with a single `*` allowed).
    pub dir_names: Vec<String>,
    /// Sibling files that confirm the match. Empty when `requires_marker`
    /// is false. Glob with a single `*` allowed (e.g. `*.csproj`).
    pub markers: Vec<String>,
    /// When true, at least one `markers` entry must exist as a sibling.
    /// When false the directory name alone is unambiguous and safe.
    pub requires_marker: bool,
}

impl Rule {
    fn new(eco: &str, dir_names: &[&str], markers: &[&str]) -> Self {
        Rule {
            ecosystem: eco.to_string(),
            dir_names: dir_names.iter().map(|s| s.to_string()).collect(),
            markers: markers.iter().map(|s| s.to_string()).collect(),
            requires_marker: !markers.is_empty(),
        }
    }

    fn safe(eco: &str, dir_names: &[&str]) -> Self {
        Rule {
            ecosystem: eco.to_string(),
            dir_names: dir_names.iter().map(|s| s.to_string()).collect(),
            markers: Vec::new(),
            requires_marker: false,
        }
    }
}

/// Match a concrete name against a pattern allowing a single `*` glob at the
/// start (suffix match) or end (prefix match). No `*` means an exact match.
fn name_matches(pattern: &str, name: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix('*') {
        name.ends_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        pattern == name
    }
}

/// A collection of rules with the matching entry point used by the scanner.
#[derive(Debug, Clone)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

impl RuleSet {
    /// Decide whether `dir_name` (a child of some directory) is a reclaimable
    /// artifact, given `sibling_names` — the set of file/dir names present in
    /// that same parent directory. Returns the ecosystem label on a match.
    pub fn match_dir<S: AsRef<str>>(&self, dir_name: &str, sibling_names: &[S]) -> Option<&str> {
        for rule in &self.rules {
            if !rule.dir_names.iter().any(|p| name_matches(p, dir_name)) {
                continue;
            }
            if !rule.requires_marker
                || rule.markers.iter().any(|m| {
                    sibling_names.iter().any(|s| name_matches(m, s.as_ref()))
                })
            {
                return Some(&rule.ecosystem);
            }
        }
        None
    }
}

/// The broad built-in catalog. Ambiguous directory names always carry markers.
pub fn default_rules() -> RuleSet {
    let rules = vec![
        // Node / JS ecosystem — names are unambiguous.
        Rule::safe("Node", &["node_modules"]),
        Rule::safe("Node", &[".next", ".nuxt", ".svelte-kit", ".turbo", ".parcel-cache"]),
        // Rust
        Rule::new("Rust", &["target"], &["Cargo.toml"]),
        // Java / Gradle / Maven — `build`/`target` are ambiguous.
        Rule::new("Maven", &["target"], &["pom.xml"]),
        Rule::new("Gradle", &["build", ".gradle"], &["build.gradle", "build.gradle.kts", "settings.gradle"]),
        // Python — caches are unambiguous; virtualenvs are nearly so.
        Rule::safe("Python", &["__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache", ".tox"]),
        Rule::safe("Python", &[".venv", "venv", "*.egg-info"]),
        // Go (vendored deps)
        Rule::new("Go", &["vendor"], &["go.mod"]),
        // PHP (Composer)
        Rule::new("PHP", &["vendor"], &["composer.json"]),
        // .NET — bin/obj are very ambiguous, require a project/solution file.
        Rule::new(".NET", &["bin", "obj"], &["*.csproj", "*.fsproj", "*.vbproj", "*.sln"]),
        // iOS / CocoaPods
        Rule::new("CocoaPods", &["Pods"], &["Podfile"]),
        // Terraform
        Rule::safe("Terraform", &[".terraform"]),
        // Elixir
        Rule::new("Elixir", &["_build", "deps"], &["mix.exs"]),
        // Elm
        Rule::new("Elm", &["elm-stuff"], &["elm.json"]),
        // Dart / Flutter
        Rule::new("Dart", &[".dart_tool"], &["pubspec.yaml"]),
    ];
    RuleSet { rules }
}

/// A rule as expressed in the user config file (`config.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct RuleSpec {
    pub ecosystem: String,
    pub dir_names: Vec<String>,
    #[serde(default)]
    pub markers: Vec<String>,
}

impl From<RuleSpec> for Rule {
    fn from(spec: RuleSpec) -> Self {
        Rule {
            ecosystem: spec.ecosystem,
            dir_names: spec.dir_names,
            requires_marker: !spec.markers.is_empty(),
            markers: spec.markers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_required_dir_needs_sibling() {
        let rs = default_rules();
        // `target` with Cargo.toml present -> Rust.
        assert_eq!(rs.match_dir("target", &["Cargo.toml", "src"]), Some("Rust"));
        // `target` with no marker -> not matched.
        assert_eq!(rs.match_dir("target", &["src", "README.md"]), None);
    }

    #[test]
    fn safe_by_name_needs_no_marker() {
        let rs = default_rules();
        assert_eq!(rs.match_dir("node_modules", &[] as &[&str]), Some("Node"));
        assert_eq!(rs.match_dir("__pycache__", &["app.py"]), Some("Python"));
    }

    #[test]
    fn decoy_build_without_marker_is_ignored() {
        let rs = default_rules();
        // A hand-written `build/` folder with no build system marker.
        assert_eq!(rs.match_dir("build", &["index.html", "style.css"]), None);
        // With a Gradle marker it becomes a real artifact.
        assert_eq!(rs.match_dir("build", &["build.gradle"]), Some("Gradle"));
    }

    #[test]
    fn glob_markers_and_names() {
        let rs = default_rules();
        assert_eq!(rs.match_dir("obj", &["App.csproj"]), Some(".NET"));
        assert_eq!(rs.match_dir("mypkg.egg-info", &["setup.py"]), Some("Python"));
    }

    #[test]
    fn name_matches_globs() {
        assert!(name_matches("*.csproj", "App.csproj"));
        assert!(!name_matches("*.csproj", "App.fsproj"));
        assert!(name_matches("*.egg-info", "mypkg.egg-info"));
        assert!(name_matches("node_modules", "node_modules"));
        assert!(!name_matches("node_modules", "node_modules_old"));
    }
}
