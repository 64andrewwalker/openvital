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
    };

    if let Err(e) = result {
        let err = openvital::output::error("", "general_error", &e.to_string());
        eprintln!("{}", serde_json::to_string(&err).unwrap());
        process::exit(1);
    }
}
