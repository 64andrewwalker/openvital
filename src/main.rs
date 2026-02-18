mod cmd;
mod db;
mod models;
mod output;

use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use std::process;

#[derive(Parser)]
#[command(name = "openvital", version, about = "Agent-native health management CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output as human-readable text instead of JSON
    #[arg(long = "human", short = 'H', global = true)]
    human: bool,

    /// Override date (YYYY-MM-DD)
    #[arg(long, global = true)]
    date: Option<NaiveDate>,
}

#[derive(Subcommand)]
enum Commands {
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
enum ConfigAction {
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

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { skip } => cmd::init::run(skip),
        Commands::Log {
            r#type,
            value,
            note,
            tags,
            source,
            batch,
        } => {
            if let Some(batch_json) = batch {
                cmd::log::run_batch(&batch_json)
            } else {
                let t = r#type.as_deref().expect("type is required");
                let v = value.expect("value is required");
                cmd::log::run(
                    t,
                    v,
                    note.as_deref(),
                    tags.as_deref(),
                    source.as_deref(),
                    cli.date,
                    cli.human,
                )
            }
        }
        Commands::Show {
            r#type,
            last,
            from: _,
            to: _,
        } => cmd::show::run(r#type.as_deref(), last, cli.date, cli.human),
        Commands::Status => cmd::status::run(cli.human),
        Commands::Config { action } => match action {
            ConfigAction::Show => cmd::config::run_show(cli.human),
            ConfigAction::Set { key, value } => cmd::config::run_set(&key, &value),
        },
    };

    if let Err(e) = result {
        let err = output::error("", "general_error", &e.to_string());
        eprintln!("{}", serde_json::to_string(&err).unwrap());
        process::exit(1);
    }
}
