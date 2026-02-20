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

/// Remove a goal by ID or metric type.
pub fn remove_goal(db: &Database, id_or_type: &str) -> Result<bool> {
    if db.remove_goal(id_or_type)? {
        return Ok(true);
    }
    db.remove_goal_by_type(id_or_type)
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

/// Check if a metric type is exclusively a medication (no non-medication entries).
/// Returns false if non-medication entries exist for this type (name collision).
fn is_medication_type(db: &Database, metric_type: &str) -> Result<bool> {
    use crate::models::metric::Category;
    let entries = db.query_by_type(metric_type, Some(20))?;
    if entries.is_empty() {
        return Ok(false);
    }
    // If any non-medication entry exists, this is a regular metric type
    let has_non_med = entries.iter().any(|e| e.category != Category::Medication);
    Ok(!has_non_med)
}

/// Compute the current value for a goal based on its timeframe.
fn compute_current(db: &Database, goal: &Goal, today: NaiveDate) -> Result<Option<f64>> {
    use crate::models::metric::{Category, is_cumulative};
    let is_med = is_medication_type(db, &goal.metric_type)?;
    let cumulative = is_cumulative(&goal.metric_type) || is_med;

    let (start_date, end_date) = match goal.timeframe {
        Timeframe::Daily => (today, today),
        Timeframe::Weekly => {
            let weekday = today.weekday().num_days_from_monday();
            (today - chrono::Duration::days(weekday as i64), today)
        }
        Timeframe::Monthly => (today.with_day(1).unwrap(), today),
    };

    let entries = db.query_by_date_range(start_date, end_date)?;
    let filtered_entries: Vec<_> = entries
        .iter()
        .filter(|m| m.metric_type == goal.metric_type)
        .filter(|m| {
            if is_med {
                m.category == Category::Medication
            } else {
                m.category != Category::Medication
            }
        })
        .collect();

    if filtered_entries.is_empty() {
        return Ok(None);
    }

    if cumulative {
        Ok(Some(filtered_entries.iter().map(|m| m.value).sum()))
    } else {
        Ok(Some(filtered_entries.last().unwrap().value))
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
            if (current - goal.target_value).abs() < 0.01 {
                format!("at target ({})", current)
            } else {
                format!("current: {}, target: {}", current, goal.target_value)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::metric::Metric;
    use chrono::{NaiveTime, TimeZone, Utc};
    use tempfile::TempDir;

    fn setup_db() -> (TempDir, Database) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();
        (dir, db)
    }

    fn make_metric(metric_type: &str, value: f64, date: NaiveDate) -> Metric {
        let dt = date.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        let ts = Utc.from_utc_datetime(&dt);
        let mut m = Metric::new(metric_type.to_string(), value);
        m.timestamp = ts;
        m
    }

    #[test]
    fn test_compute_current_daily() -> Result<()> {
        let (_dir, db) = setup_db();
        let today = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let goal = Goal::new("water".into(), 2000.0, Direction::Above, Timeframe::Daily);

        db.insert_metric(&make_metric("water", 500.0, today))?;
        db.insert_metric(&make_metric("water", 1000.0, today))?;

        let val = compute_current(&db, &goal, today)?;
        assert_eq!(val, Some(1500.0)); // water is cumulative
        Ok(())
    }

    #[test]
    fn test_compute_current_weekly() -> Result<()> {
        let (_dir, db) = setup_db();
        // 2024-01-01 is Monday
        let monday = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let wednesday = NaiveDate::from_ymd_opt(2024, 1, 3).unwrap();
        let goal = Goal::new("water".into(), 10000.0, Direction::Above, Timeframe::Weekly);

        db.insert_metric(&make_metric("water", 1000.0, monday))?;
        db.insert_metric(&make_metric("water", 2000.0, wednesday))?;

        let val = compute_current(&db, &goal, wednesday)?;
        assert_eq!(val, Some(3000.0));
        Ok(())
    }

    #[test]
    fn test_compute_current_snapshot() -> Result<()> {
        let (_dir, db) = setup_db();
        let today = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let goal = Goal::new("weight".into(), 70.0, Direction::Below, Timeframe::Daily);

        db.insert_metric(&make_metric("weight", 75.0, today))?;
        let mut m2 = make_metric("weight", 74.0, today);
        m2.timestamp = m2.timestamp + chrono::Duration::hours(1);
        db.insert_metric(&m2)?;

        let val = compute_current(&db, &goal, today)?;
        assert_eq!(val, Some(74.0)); // weight is snapshot
        Ok(())
    }
}
