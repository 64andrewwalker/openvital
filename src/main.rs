mod cli;
mod cmd;

use anyhow::anyhow;
use clap::Parser;
use cli::{Cli, Commands, ConfigAction, GoalAction, MedAction};
use std::process;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { skip, units } => cmd::init::run(skip, units.as_deref()),
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
                target_pos,
                direction_pos,
                timeframe_pos,
                target,
                direction,
                timeframe,
            } => match (
                target.or(target_pos),
                direction.or(direction_pos),
                timeframe.or(timeframe_pos),
            ) {
                (Some(t), Some(d), Some(tf)) => cmd::goal::run_set(&r#type, t, &d, &tf, cli.human),
                (None, _, _) => Err(anyhow!("target is required (use positional or --target)")),
                (_, None, _) => Err(anyhow!(
                    "direction is required (use positional or --direction)"
                )),
                (_, _, None) => Err(anyhow!(
                    "timeframe is required (use positional or --timeframe)"
                )),
            },
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
            with_medications,
        } => cmd::export::run_export(
            &format,
            output.as_deref(),
            r#type.as_deref(),
            from,
            to,
            with_medications,
            cli.human,
        ),
        Commands::Import { source, file } => cmd::export::run_import(&source, &file, cli.human),
        Commands::Med { action } => match action {
            MedAction::Add {
                name,
                dose,
                freq,
                route,
                note,
                started,
            } => cmd::med::run_add(
                &name,
                dose.as_deref(),
                &freq,
                &route,
                note.as_deref(),
                started,
                cli.human,
            ),
            MedAction::Take {
                name,
                dose,
                note,
                tags,
            } => cmd::med::run_take(
                &name,
                dose.as_deref(),
                note.as_deref(),
                tags.as_deref(),
                cli.date,
                cli.human,
            ),
            MedAction::List { all } => cmd::med::run_list(all, cli.human),
            MedAction::Stop { name, reason } => {
                cmd::med::run_stop(&name, reason.as_deref(), cli.date, cli.human)
            }
            MedAction::Remove { name, yes } => cmd::med::run_remove(&name, yes, cli.human),
            MedAction::Status { name, last } => {
                cmd::med::run_status(name.as_deref(), last, cli.human)
            }
        },
        Commands::Anomaly {
            r#type,
            days,
            threshold,
        } => cmd::anomaly::run(r#type.as_deref(), days, &threshold, cli.human),
        Commands::Context { days, types } => cmd::context::run(days, types.as_deref(), cli.human),
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
