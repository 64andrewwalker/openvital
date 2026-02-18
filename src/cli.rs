use chrono::NaiveDate;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "openvital", version, about = "Agent-native health management CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output as human-readable text instead of JSON
    #[arg(long = "human", short = 'H', global = true)]
    pub human: bool,

    /// Override date (YYYY-MM-DD)
    #[arg(long, global = true)]
    pub date: Option<NaiveDate>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize profile and data directory
    Init {
        /// Skip interactive setup, use defaults
        #[arg(long)]
        skip: bool,
    },

    /// Log a metric entry
    Log {
        /// Metric type (e.g. weight, cardio, pain) or alias
        #[arg(required_unless_present = "batch")]
        r#type: Option<String>,

        /// Metric value
        #[arg(required_unless_present = "batch")]
        value: Option<f64>,

        /// Free-text note
        #[arg(long)]
        note: Option<String>,

        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,

        /// Data source (default: manual)
        #[arg(long)]
        source: Option<String>,

        /// Batch JSON array of entries
        #[arg(long)]
        batch: Option<String>,
    },

    /// Show metric history
    Show {
        /// Metric type, alias, or "today"
        r#type: Option<String>,

        /// Number of recent entries to show
        #[arg(long)]
        last: Option<u32>,

        /// Show entries from this date
        #[arg(long)]
        from: Option<NaiveDate>,

        /// Show entries to this date
        #[arg(long)]
        to: Option<NaiveDate>,
    },

    /// Quick status overview
    Status,

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a config value
    Set {
        /// Config key (e.g. height, birth_year, alias.w)
        key: String,
        /// Config value
        value: String,
    },
}
