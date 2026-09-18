use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "gsu",
    version,
    about = "Read-only PyTorch performance static checks"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}
#[derive(Subcommand)]
pub enum Command {
    /// Check Python source files without executing them
    Check(CheckArgs),
    /// Explain a rule, including its limitations and manual suggestions
    Rule { rule_id: String },
}
#[derive(Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    #[default]
    #[value(alias = "console")]
    Full,
    Concise,
    Json,
}
#[derive(Clone, Copy, Default, ValueEnum)]
pub enum Color {
    #[default]
    Auto,
    Always,
    Never,
}
#[derive(clap::Args)]
pub struct CheckArgs {
    /// Files or directories (default: current directory; stdin is unsupported)
    pub paths: Vec<PathBuf>,
    /// Replace selected rules: exact codes or T,S,D,P,M,A; empty selects none
    #[arg(long)]
    pub select: Option<String>,
    /// Replace ignored rules; empty string clears the ignore list
    #[arg(long)]
    pub ignore: Option<String>,
    /// Replace excluded paths/globs; repeat for multiple patterns
    #[arg(long)]
    pub exclude: Vec<String>,
    /// Use this TOML file, which must contain [tool.gsu]
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Report format
    #[arg(long, value_enum, default_value = "full")]
    pub output_format: OutputFormat,
    /// Enable experimental rules within the selected categories
    #[arg(long, conflicts_with = "no_preview")]
    pub preview: bool,
    /// Disable experimental rules, overriding configuration
    #[arg(long, conflicts_with = "preview")]
    pub no_preview: bool,
    /// Color terminal reports (auto respects NO_COLOR)
    #[arg(long, value_enum, default_value = "auto")]
    pub color: Color,
}
