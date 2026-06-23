//! Command-line interface.

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "cleard",
    version,
    about = "Interactive, multi-ecosystem build-artifact disk reclaimer (npkill, but for everything)"
)]
pub struct Cli {
    /// Directory to scan recursively.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Don't delete anything; only show what would be reclaimed.
    #[arg(long)]
    pub dry_run: bool,

    /// Hide candidates smaller than this size (e.g. 100M, 1.5G, 500K).
    #[arg(long, value_parser = parse_size, default_value = "0")]
    pub min_size: u64,

    /// Directory name to skip entirely (repeatable, e.g. -x .git -x dist).
    #[arg(short = 'x', long = "exclude")]
    pub exclude: Vec<String>,

    /// Path to a config file (default: ~/.config/cleard/config.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Ignore the built-in rule catalog; use only rules from config.
    #[arg(long)]
    pub no_default_rules: bool,

    /// Follow symbolic links while scanning.
    #[arg(long)]
    pub follow_links: bool,
}

/// Parse a human size like `100M`, `1.5G`, `512K`, `2T` (binary units) or a
/// plain byte count.
fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty size".into());
    }
    let (num, mult) = match s.chars().last().unwrap().to_ascii_uppercase() {
        'K' => (&s[..s.len() - 1], 1u64 << 10),
        'M' => (&s[..s.len() - 1], 1u64 << 20),
        'G' => (&s[..s.len() - 1], 1u64 << 30),
        'T' => (&s[..s.len() - 1], 1u64 << 40),
        'B' => (&s[..s.len() - 1], 1),
        c if c.is_ascii_digit() => (s, 1),
        other => return Err(format!("unknown size suffix '{other}'")),
    };
    let value: f64 = num
        .trim()
        .parse()
        .map_err(|_| format!("invalid size number '{num}'"))?;
    if value < 0.0 {
        return Err("size must be non-negative".into());
    }
    Ok((value * mult as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::parse_size;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("0").unwrap(), 0);
        assert_eq!(parse_size("1024").unwrap(), 1024);
        assert_eq!(parse_size("1K").unwrap(), 1024);
        assert_eq!(parse_size("1M").unwrap(), 1 << 20);
        assert_eq!(parse_size("1.5G").unwrap(), (1.5 * (1u64 << 30) as f64) as u64);
        assert!(parse_size("abc").is_err());
    }
}
