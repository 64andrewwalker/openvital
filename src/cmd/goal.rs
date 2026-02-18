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
    let goal = openvital::core::goal::set_goal(&db, resolved, target_value, dir, tf)?;

    if human {
        println!(
            "Goal set: {} {} {} ({})",
            goal.metric_type, goal.direction, goal.target_value, goal.timeframe
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
                let progress = s.progress.as_deref().unwrap_or("no data");
                println!(
                    "[{}] {} {} {} ({}) — {}",
                    met, s.metric_type, s.direction, s.target_value, s.timeframe, progress
                );
            }
        }
    } else {
        let out = output::success("goal", json!({ "goals": statuses }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
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
