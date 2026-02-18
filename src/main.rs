mod cli;
mod cmd;

use clap::Parser;
use cli::{Cli, Commands, ConfigAction, GoalAction};
use std::process;

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
                cmd::log::run_batch(&batch_json, cli.human)
            } else {
                let t = r#type.as_deref().expect("type is required");
                let v = value.as_deref().expect("value is required");
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
        Commands::Trend {
            r#type,
            period,
            last,
            correlate,
        } => {
            if let Some(corr) = correlate {
                cmd::trend::run_correlate(&corr, last, cli.human)
            } else {
                let t = r#type.as_deref().expect("type is required");
                cmd::trend::run(t, period.as_deref(), last, cli.human)
            }
        }
        Commands::Status => cmd::status::run(cli.human),
        Commands::Goal { action } => match action {
            GoalAction::Set {
                r#type,
                target,
                direction,
                timeframe,
            } => cmd::goal::run_set(&r#type, target, &direction, &timeframe, cli.human),
            GoalAction::Status { r#type } => cmd::goal::run_status(r#type.as_deref(), cli.human),
            GoalAction::Remove { goal_id } => cmd::goal::run_remove(&goal_id, cli.human),
        },
        Commands::Config { action } => match action {
            ConfigAction::Show => cmd::config::run_show(cli.human),
            ConfigAction::Set { key, value } => cmd::config::run_set(&key, &value),
        },
        Commands::Report {
            period,
            month,
            from,
            to,
        } => cmd::report::run(period.as_deref(), month.as_deref(), from, to, cli.human),
        Commands::Export {
            format,
            output,
            r#type,
            from,
            to,
        } => cmd::export::run_export(
            &format,
            output.as_deref(),
            r#type.as_deref(),
            from,
            to,
            cli.human,
        ),
        Commands::Import { source, file } => cmd::export::run_import(&source, &file, cli.human),
        Commands::Completions { shell } => {
            cli::print_completions(shell);
            Ok(())
        }
    };

    if let Err(e) = result {
        let err = openvital::output::error("", "general_error", &e.to_string());
        eprintln!("{}", serde_json::to_string(&err).unwrap());
        process::exit(1);
    }
}
