use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::params;

use crate::models::metric::{Category, Metric};

use super::Database;

struct MetricRow {
    id: String,
    timestamp: String,
    category: String,
    metric_type: String,
    value: f64,
    unit: String,
    note: Option<String>,
    tags: Option<String>,
    source: String,
}

fn row_to_metric(r: MetricRow) -> Result<Metric> {
    let tags: Vec<String> = match r.tags {
        Some(ref t) => serde_json::from_str(t).unwrap_or_default(),
        None => Vec::new(),
    };
    let timestamp: DateTime<Utc> = DateTime::parse_from_rfc3339(&r.timestamp)?.with_timezone(&Utc);
    let category = match r.category.as_str() {
        "body" => Category::Body,
        "exercise" => Category::Exercise,
        "sleep" => Category::Sleep,
        "nutrition" => Category::Nutrition,
        "pain" => Category::Pain,
        "habit" => Category::Habit,
        _ => Category::Custom,
    };
    Ok(Metric {
        id: r.id,
        timestamp,
        category,
        metric_type: r.metric_type,
        value: r.value,
        unit: r.unit,
        note: r.note,
        tags,
        source: r.source,
    })
}

impl Database {
    pub fn insert_metric(&self, m: &Metric) -> Result<()> {
        let tags_json = if m.tags.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&m.tags)?)
        };
        self.conn.execute(
            "INSERT INTO metrics (id, timestamp, category, type, value, unit, note, tags, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                m.id,
                m.timestamp.to_rfc3339(),
                m.category.to_string(),
                m.metric_type,
                m.value,
                m.unit,
                m.note,
                tags_json,
                m.source,
            ],
        )?;
        Ok(())
    }

    pub fn query_by_type(&self, metric_type: &str, limit: Option<u32>) -> Result<Vec<Metric>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, category, type, value, unit, note, tags, source
             FROM metrics WHERE type = ?1 ORDER BY timestamp DESC LIMIT ?2",
        )?;
        let limit = limit.unwrap_or(1) as i64;
        let rows = stmt.query_map(params![metric_type, limit], |row| {
            Ok(MetricRow {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                category: row.get(2)?,
                metric_type: row.get(3)?,
                value: row.get(4)?,
                unit: row.get(5)?,
                note: row.get(6)?,
                tags: row.get(7)?,
                source: row.get(8)?,
            })
        })?;

        let mut metrics = Vec::new();
        for row in rows {
            let r = row?;
            metrics.push(row_to_metric(r)?);
        }
        Ok(metrics)
    }

    pub fn query_by_date(&self, date: NaiveDate) -> Result<Vec<Metric>> {
        let start = format!("{}T00:00:00", date);
        let end = format!("{}T23:59:59", date);
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, category, type, value, unit, note, tags, source
             FROM metrics WHERE timestamp >= ?1 AND timestamp <= ?2 ORDER BY timestamp",
        )?;
        let rows = stmt.query_map(params![start, end], |row| {
            Ok(MetricRow {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                category: row.get(2)?,
                metric_type: row.get(3)?,
                value: row.get(4)?,
                unit: row.get(5)?,
                note: row.get(6)?,
                tags: row.get(7)?,
                source: row.get(8)?,
            })
        })?;

        let mut metrics = Vec::new();
        for row in rows {
            let r = row?;
            metrics.push(row_to_metric(r)?);
        }
        Ok(metrics)
    }
}
