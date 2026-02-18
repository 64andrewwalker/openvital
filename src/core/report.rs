use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

use crate::db::Database;

#[derive(Debug, Serialize)]
pub struct ReportResult {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub days_with_entries: u32,
    pub total_entries: u32,
    pub metrics: Vec<MetricSummary>,
}

#[derive(Debug, Serialize)]
pub struct MetricSummary {
    #[serde(rename = "type")]
    pub metric_type: String,
    pub count: u32,
    pub avg: f64,
    pub min: f64,
    pub max: f64,
    pub unit: String,
}

/// Generate a comprehensive report for the given date range.
pub fn generate(db: &Database, from: NaiveDate, to: NaiveDate) -> Result<ReportResult> {
    let entries = db.query_by_date_range(from, to)?;

    if entries.is_empty() {
        return Ok(ReportResult {
            from,
            to,
            days_with_entries: 0,
            total_entries: 0,
            metrics: Vec::new(),
        });
    }

    // Count distinct days
    let distinct_days: HashSet<NaiveDate> =
        entries.iter().map(|e| e.timestamp.date_naive()).collect();

    // Group by metric type
    let mut grouped: BTreeMap<String, Vec<(f64, String)>> = BTreeMap::new();
    for entry in &entries {
        grouped
            .entry(entry.metric_type.clone())
            .or_default()
            .push((entry.value, entry.unit.clone()));
    }

    let metrics: Vec<MetricSummary> = grouped
        .into_iter()
        .map(|(metric_type, values)| {
            let count = values.len() as u32;
            let vals: Vec<f64> = values.iter().map(|(v, _)| *v).collect();
            let sum: f64 = vals.iter().sum();
            let avg = sum / vals.len() as f64;
            let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let unit = values.first().map(|(_, u)| u.clone()).unwrap_or_default();
            MetricSummary {
                metric_type,
                count,
                avg,
                min,
                max,
                unit,
            }
        })
        .collect();

    Ok(ReportResult {
        from,
        to,
        days_with_entries: distinct_days.len() as u32,
        total_entries: entries.len() as u32,
        metrics,
    })
}
