//! User configuration loaded from `~/.config/cleard/config.toml`.
//!
//! The config can disable the built-in catalog and/or add extra rules. Example:
//!
//! ```toml
//! # use_default_rules = true   # default
//!
//! [[rules]]
//! ecosystem = "Zig"
//! dir_names = ["zig-cache", "zig-out"]
//! markers = ["build.zig"]
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::rules::{default_rules, Rule, RuleSet, RuleSpec};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// Whether to include the built-in catalog. Defaults to true.
    #[serde(default = "default_true")]
    pub use_default_rules: bool,
    /// Extra rules contributed by the user.
    #[serde(default)]
    pub rules: Vec<RuleSpec>,
}

fn default_true() -> bool {
    true
}

impl Config {
    /// Build the effective rule set: user rules first (so they take precedence
    /// on overlapping names), then the built-ins unless disabled.
    pub fn into_ruleset(self) -> RuleSet {
        let mut rules: Vec<Rule> = self.rules.into_iter().map(Rule::from).collect();
        if self.use_default_rules {
            rules.extend(default_rules().rules);
        }
        RuleSet { rules }
    }
}

/// Default config path: `$XDG_CONFIG_HOME/cleard/config.toml` (or platform equiv).
pub fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("cleard").join("config.toml"))
}

/// Load config from an explicit path (must exist) or the default path
/// (optional — a missing default file yields the default config).
pub fn load(explicit: Option<&PathBuf>) -> Result<Config> {
    let path = match explicit {
        Some(p) => Some(p.clone()),
        None => default_config_path().filter(|p| p.exists()),
    };
    match path {
        Some(p) => {
            let text = std::fs::read_to_string(&p)
                .with_context(|| format!("reading config {}", p.display()))?;
            let cfg: Config = toml::from_str(&text)
                .with_context(|| format!("parsing config {}", p.display()))?;
            Ok(cfg)
        }
        None => Ok(Config {
            use_default_rules: true,
            rules: Vec::new(),
        }),
    }
}
