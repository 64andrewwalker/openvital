use anyhow::Result;
use serde_json::json;

use openvital::db::Database;
use openvital::models::config::Config;
use openvital::models::goal::{Direction, Timeframe};
use openvital::output;

pub fn run_set(
    metric_type: &str,
    target_value: f64,
    direction: &str,
    timeframe: &str,
    human: bool,
) -> Result<()> {
    let config = Config::load()?;
    let resolved = config.resolve_alias(metric_type);
    let db = Database::open(&Config::db_path())?;

    let dir: Direction = direction.parse()?;
    let tf: Timeframe = timeframe.parse()?;
    // Convert target from user units (e.g., imperial) to metric for storage
    let stored_target = openvital::core::units::from_input(target_value, &resolved, &config.units);
    let goal = openvital::core::goal::set_goal(&db, resolved, stored_target, dir, tf)?;

    if human {
        let (display_target, display_unit) =
            openvital::core::units::to_display(goal.target_value, &goal.metric_type, &config.units);
        println!(
            "Goal set: {} {} {:.1} {} ({})",
            goal.metric_type, goal.direction, display_target, display_unit, goal.timeframe
        );
    } else {
        let out = output::success("goal", json!({ "goal": goal }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_status(metric_type: Option<&str>, human: bool) -> Result<()> {
    let config = Config::load()?;
    let resolved = metric_type.map(|t| config.resolve_alias(t));
    let db = Database::open(&Config::db_path())?;

    let statuses = openvital::core::goal::goal_status(&db, resolved.as_deref())?;

    if human {
        if statuses.is_empty() {
            println!("No active goals.");
        } else {
            for s in &statuses {
                let met = if s.is_met { "MET" } else { "..." };
                let (display_target, display_unit) = openvital::core::units::to_display(
                    s.target_value,
                    &s.metric_type,
                    &config.units,
                );
                let progress = format_progress_human(s, &config.units);
                println!(
                    "[{}] {} {} {:.1} {} ({}) — {}",
                    met,
                    s.metric_type,
                    s.direction,
                    display_target,
                    display_unit,
                    s.timeframe,
                    progress
                );
            }
        }
    } else {
        let out = output::success("goal", json!({ "goals": statuses }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

fn format_progress_human(
    status: &openvital::core::goal::GoalStatus,
    units: &openvital::models::config::Units,
) -> String {
    let Some(current_raw) = status.current_value else {
        return "no data".to_string();
    };

    let (current, unit) =
        openvital::core::units::to_display(current_raw, &status.metric_type, units);
    let (target, _) =
        openvital::core::units::to_display(status.target_value, &status.metric_type, units);

    match status.direction.as_str() {
        "below" => {
            if current_raw <= status.target_value {
                format!("at target ({:.1} <= {:.1} {})", current, target, unit)
            } else {
                format!(
                    "{:.1} to go ({:.1} -> {:.1} {})",
                    current - target,
                    current,
                    target,
                    unit
                )
            }
        }
        "above" => {
            if current_raw >= status.target_value {
                format!("target met ({:.1} >= {:.1} {})", current, target, unit)
            } else {
                format!(
                    "{:.1} remaining ({:.1}/{:.1} {})",
                    target - current,
                    current,
                    target,
                    unit
                )
            }
        }
        "equal" => {
            if (current_raw - status.target_value).abs() < f64::EPSILON {
                format!("at target ({:.1} {})", current, unit)
            } else {
                format!(
                    "current: {:.1} {}, target: {:.1} {}",
                    current, unit, target, unit
                )
            }
        }
        _ => status
            .progress
            .clone()
            .unwrap_or_else(|| "no data".to_string()),
    }
}

pub fn run_remove(goal_id: &str, human: bool) -> Result<()> {
    let db = Database::open(&Config::db_path())?;
    let removed = openvital::core::goal::remove_goal(&db, goal_id)?;

    if !removed {
        anyhow::bail!("goal not found or already inactive: {}", goal_id);
    }

    if human {
        println!("Goal removed: {}", goal_id);
    } else {
        let out = output::success("goal", json!({ "removed": goal_id }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
