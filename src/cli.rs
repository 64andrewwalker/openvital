use chrono::NaiveDate;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(
    name = "openvital",
    version,
    about = "Agent-native health management CLI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output as human-readable text instead of JSON
    #[arg(long = "human", short = 'H', global = true)]
    pub human: bool,

    /// Override date (YYYY-MM-DD)
    #[arg(long, global = true)]
    pub date: Option<NaiveDate>,

    /// Minimal output (just confirmation or error)
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Custom config file path
    #[arg(long = "config", global = true)]
    pub config_path: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize profile and data directory
    Init {
        /// Skip interactive setup, use defaults
        #[arg(long)]
        skip: bool,
        /// Unit system: metric (default) or imperial
        #[arg(long)]
        units: Option<String>,
    },

    /// Log a metric entry
    Log {
        /// Metric type (e.g. weight, cardio, pain) or alias
        #[arg(required_unless_present = "batch")]
        r#type: Option<String>,

        /// Metric value
        #[arg(required_unless_present = "batch")]
        value: Option<String>,

        /// Free-text note
        #[arg(long)]
        note: Option<String>,

        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,

        /// Data source (default: manual)
        #[arg(long)]
        source: Option<String>,

        /// Batch entries: JSON array or simple "type:value,type:value" format
        #[arg(long, conflicts_with_all = ["type", "value"])]
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

    /// Analyze trends and projections
    Trend {
        /// Metric type (e.g. weight, cardio)
        #[arg(required_unless_present = "correlate")]
        r#type: Option<String>,

        /// Period: daily, weekly, or monthly
        #[arg(long)]
        period: Option<String>,

        /// Number of periods to show
        #[arg(long)]
        last: Option<u32>,

        /// Correlation analysis between two metrics (comma-separated)
        #[arg(long)]
        correlate: Option<String>,
    },

    /// Quick status overview
    Status,

    /// Manage goals
    Goal {
        #[command(subcommand)]
        action: GoalAction,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Generate a report for a time period
    Report {
        /// Period: week or month
        #[arg(long)]
        period: Option<String>,

        /// Month in YYYY-MM format (for --period month)
        #[arg(long)]
        month: Option<String>,

        /// Start date
        #[arg(long)]
        from: Option<NaiveDate>,

        /// End date
        #[arg(long)]
        to: Option<NaiveDate>,
    },

    /// Export data for backup or analysis
    Export {
        /// Output format: csv or json
        #[arg(long, default_value = "json")]
        format: String,

        /// Output file path (stdout if omitted)
        #[arg(long)]
        output: Option<String>,

        /// Filter by metric type
        #[arg(long)]
        r#type: Option<String>,

        /// Filter from date
        #[arg(long)]
        from: Option<NaiveDate>,

        /// Filter to date
        #[arg(long)]
        to: Option<NaiveDate>,

        /// Include medication records in export
        #[arg(long)]
        with_medications: bool,
    },

    /// Import data from external sources
    Import {
        /// Source format: csv, json
        #[arg(long)]
        source: String,

        /// Input file path
        #[arg(long)]
        file: String,
    },

    /// Manage medications
    Med {
        #[command(subcommand)]
        action: MedAction,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

#[derive(Subcommand)]
pub enum GoalAction {
    /// Set a goal for a metric type
    Set {
        /// Metric type (e.g. weight, cardio, water)
        r#type: String,
        /// Target value (positional)
        #[arg(value_name = "TARGET_POS")]
        target_pos: Option<f64>,
        /// Direction (positional): above, below, or equal
        #[arg(value_name = "DIRECTION_POS")]
        direction_pos: Option<String>,
        /// Timeframe (positional): daily, weekly, or monthly
        #[arg(value_name = "TIMEFRAME_POS")]
        timeframe_pos: Option<String>,
        /// Target value (named)
        #[arg(long)]
        target: Option<f64>,
        /// Direction: above, below, or equal (named)
        #[arg(long)]
        direction: Option<String>,
        /// Timeframe: daily, weekly, or monthly (named)
        #[arg(long)]
        timeframe: Option<String>,
    },
    /// Check goal status
    Status {
        /// Optional metric type to filter
        r#type: Option<String>,
    },
    /// Remove a goal
    Remove {
        /// Goal ID or metric type to remove
        goal_id: String,
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

#[derive(Subcommand)]
pub enum MedAction {
    /// Add a medication to the active list
    Add {
        /// Medication name (e.g., "ibuprofen")
        name: String,
        /// Dosage (e.g., "400mg", "5ml", "thin layer")
        #[arg(long)]
        dose: Option<String>,
        /// Frequency: daily, 2x_daily, 3x_daily, weekly, as_needed
        #[arg(long)]
        freq: String,
        /// Administration route (default: oral)
        #[arg(long, default_value = "oral")]
        route: String,
        /// Notes (e.g., "take with food")
        #[arg(long)]
        note: Option<String>,
        /// Start date (default: today)
        #[arg(long)]
        started: Option<NaiveDate>,
    },
    /// Record a dose taken
    Take {
        /// Medication name
        name: String,
        /// Override dose for this intake
        #[arg(long)]
        dose: Option<String>,
        /// Note for this intake
        #[arg(long)]
        note: Option<String>,
        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,
    },
    /// List medications (active by default)
    List {
        /// Include stopped medications
        #[arg(long)]
        all: bool,
    },
    /// Mark a medication as stopped
    Stop {
        /// Medication name
        name: String,
        /// Reason for stopping
        #[arg(long)]
        reason: Option<String>,
    },
    /// Delete a medication record
    Remove {
        /// Medication name
        name: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// View adherence status
    Status {
        /// Medication name (all if omitted)
        name: Option<String>,
        /// Show adherence for last N days (default: 7)
        #[arg(long, default_value = "7")]
        last: u32,
    },
}

/// Generate shell completions and print to stdout.
pub fn print_completions(shell: Shell) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "openvital", &mut std::io::stdout());
}
