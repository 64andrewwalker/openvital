use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::models::goal::Goal;

use super::Database;

impl Database {
    pub fn insert_goal(&self, g: &Goal) -> Result<()> {
        self.conn.execute(
            "INSERT INTO goals (id, metric_type, target_value, direction, timeframe, active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                g.id,
                g.metric_type,
                g.target_value,
                g.direction.to_string(),
                g.timeframe.to_string(),
                g.active,
                g.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_goals(&self, active_only: bool) -> Result<Vec<Goal>> {
        let sql = if active_only {
            "SELECT id, metric_type, target_value, direction, timeframe, active, created_at
             FROM goals WHERE active = 1 ORDER BY created_at"
        } else {
            "SELECT id, metric_type, target_value, direction, timeframe, active, created_at
             FROM goals ORDER BY created_at"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(GoalRow {
                id: row.get(0)?,
                metric_type: row.get(1)?,
                target_value: row.get(2)?,
                direction: row.get(3)?,
                timeframe: row.get(4)?,
                active: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        let mut goals = Vec::new();
        for row in rows {
            let r = row?;
            goals.push(row_to_goal(r)?);
        }
        Ok(goals)
    }

    pub fn get_goal(&self, id: &str) -> Result<Option<Goal>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, metric_type, target_value, direction, timeframe, active, created_at
             FROM goals WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(GoalRow {
                id: row.get(0)?,
                metric_type: row.get(1)?,
                target_value: row.get(2)?,
                direction: row.get(3)?,
                timeframe: row.get(4)?,
                active: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row_to_goal(row?)?)),
            None => Ok(None),
        }
    }

    pub fn get_goal_by_type(&self, metric_type: &str) -> Result<Option<Goal>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, metric_type, target_value, direction, timeframe, active, created_at
             FROM goals WHERE metric_type = ?1 AND active = 1 LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![metric_type], |row| {
            Ok(GoalRow {
                id: row.get(0)?,
                metric_type: row.get(1)?,
                target_value: row.get(2)?,
                direction: row.get(3)?,
                timeframe: row.get(4)?,
                active: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row_to_goal(row?)?)),
            None => Ok(None),
        }
    }

    pub fn remove_goal(&self, id: &str) -> Result<bool> {
        let count = self.conn.execute(
            "UPDATE goals SET active = 0 WHERE id = ?1 AND active = 1",
            params![id],
        )?;
        Ok(count > 0)
    }

    pub fn remove_goal_by_type(&self, metric_type: &str) -> Result<bool> {
        let count = self.conn.execute(
            "UPDATE goals SET active = 0 WHERE metric_type = ?1 AND active = 1",
            params![metric_type],
        )?;
        Ok(count > 0)
    }
}

struct GoalRow {
    id: String,
    metric_type: String,
    target_value: f64,
    direction: String,
    timeframe: String,
    active: bool,
    created_at: String,
}

fn row_to_goal(r: GoalRow) -> Result<Goal> {
    let direction = r.direction.parse()?;
    let timeframe: crate::models::goal::Timeframe = r.timeframe.parse()?;
    let created_at: DateTime<Utc> =
        DateTime::parse_from_rfc3339(&r.created_at)?.with_timezone(&Utc);
    Ok(Goal {
        id: r.id,
        metric_type: r.metric_type,
        target_value: r.target_value,
        direction,
        timeframe,
        active: r.active,
        created_at,
    })
}
