use crate::db::Database;
use crate::models::goal::{Direction, Goal, Timeframe};
use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};
use serde::Serialize;

/// Set (or replace) a goal for a metric type.
pub fn set_goal(
    db: &Database,
    metric_type: String,
    target_value: f64,
    direction: Direction,
    timeframe: Timeframe,
) -> Result<Goal> {
    // Deactivate existing goal for same type
    if let Some(existing) = db.get_goal_by_type(&metric_type)? {
        db.remove_goal(&existing.id)?;
    }
    let goal = Goal::new(metric_type, target_value, direction, timeframe);
    db.insert_goal(&goal)?;
    Ok(goal)
}

/// Remove a goal by ID.
pub fn remove_goal(db: &Database, goal_id: &str) -> Result<bool> {
    db.remove_goal(goal_id)
}

#[derive(Serialize)]
pub struct GoalStatus {
    pub id: String,
    pub metric_type: String,
    pub target_value: f64,
    pub direction: String,
    pub timeframe: String,
    pub current_value: Option<f64>,
    pub is_met: bool,
    pub progress: Option<String>,
}

/// Get status of all active goals, or a specific metric type.
pub fn goal_status(db: &Database, metric_type: Option<&str>) -> Result<Vec<GoalStatus>> {
    let goals = db.list_goals(true)?;
    let today = Local::now().date_naive();

    let mut results = Vec::new();
    for goal in &goals {
        if let Some(t) = metric_type
            && goal.metric_type != t
        {
            continue;
        }
        let current = compute_current(db, goal, today)?;
        let is_met = current.map(|v| goal.is_met(v)).unwrap_or(false);
        let progress = current.map(|v| format_progress(goal, v));

        results.push(GoalStatus {
            id: goal.id.clone(),
            metric_type: goal.metric_type.clone(),
            target_value: goal.target_value,
            direction: goal.direction.to_string(),
            timeframe: goal.timeframe.to_string(),
            current_value: current,
            is_met,
            progress,
        });
    }
    Ok(results)
}

/// Compute the current value for a goal based on its timeframe.
fn compute_current(db: &Database, goal: &Goal, today: NaiveDate) -> Result<Option<f64>> {
    use crate::models::metric::is_cumulative;
    let cumulative = is_cumulative(&goal.metric_type);

    match goal.timeframe {
        Timeframe::Daily => {
            let entries = db.query_by_date(today)?;
            let day_entries: Vec<_> = entries
                .iter()
                .filter(|m| m.metric_type == goal.metric_type)
                .collect();
            if day_entries.is_empty() {
                return Ok(None);
            }
            if cumulative {
                Ok(Some(day_entries.iter().map(|m| m.value).sum()))
            } else {
                Ok(Some(day_entries.last().unwrap().value))
            }
        }
        Timeframe::Weekly => {
            let weekday = today.weekday().num_days_from_monday();
            let week_start = today - chrono::Duration::days(weekday as i64);
            let mut values = Vec::new();
            for i in 0..7 {
                let date = week_start + chrono::Duration::days(i);
                if date > today {
                    break;
                }
                let entries = db.query_by_date(date)?;
                for m in &entries {
                    if m.metric_type == goal.metric_type {
                        values.push(m.value);
                    }
                }
            }
            if values.is_empty() {
                Ok(None)
            } else if cumulative {
                Ok(Some(values.iter().sum()))
            } else {
                Ok(Some(*values.last().unwrap()))
            }
        }
        Timeframe::Monthly => {
            // For monthly, use the latest value
            let entries = db.query_by_type(&goal.metric_type, Some(1))?;
            Ok(entries.first().map(|m| m.value))
        }
    }
}

fn format_progress(goal: &Goal, current: f64) -> String {
    match goal.direction {
        Direction::Below => {
            if current <= goal.target_value {
                format!("at target ({} <= {})", current, goal.target_value)
            } else {
                format!(
                    "{} to go ({} → {})",
                    current - goal.target_value,
                    current,
                    goal.target_value
                )
            }
        }
        Direction::Above => {
            if current >= goal.target_value {
                format!("target met ({} >= {})", current, goal.target_value)
            } else {
                format!(
                    "{} remaining ({}/{})",
                    goal.target_value - current,
                    current,
                    goal.target_value
                )
            }
        }
        Direction::Equal => {
            if (current - goal.target_value).abs() < f64::EPSILON {
                format!("at target ({})", current)
            } else {
                format!("current: {}, target: {}", current, goal.target_value)
            }
        }
    }
}
