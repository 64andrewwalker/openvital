use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};

use crate::db::Database;
use crate::models::config::Config;
use crate::models::metric::Metric;

/// Log a single metric. Returns the created Metric.
pub fn log_metric(
    db: &Database,
    config: &Config,
    metric_type: &str,
    value: f64,
    note: Option<&str>,
    tags: Option<&str>,
    source: Option<&str>,
    date: Option<NaiveDate>,
) -> Result<Metric> {
    let resolved = config.resolve_alias(metric_type);
    let mut m = Metric::new(resolved, value);
    if let Some(n) = note {
        m.note = Some(n.to_string());
    }
    if let Some(t) = tags {
        m.tags = t.split(',').map(|s| s.trim().to_string()).collect();
    }
    if let Some(s) = source {
        m.source = s.to_string();
    }
    if let Some(d) = date {
        if let Some(dt) = d.and_hms_opt(12, 0, 0) {
            m.timestamp = Utc.from_utc_datetime(&dt);
        }
    }
    db.insert_metric(&m)?;
    Ok(m)
}

/// Batch-log metrics from a JSON array string. Returns created Metrics.
pub fn log_batch(
    db: &Database,
    config: &Config,
    batch_json: &str,
) -> Result<Vec<Metric>> {
    let entries: Vec<serde_json::Value> = serde_json::from_str(batch_json)?;
    let mut results = Vec::new();

    for entry in &entries {
        let metric_type = entry["type"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'type' in batch entry"))?;
        let value = entry["value"]
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("missing 'value' in batch entry"))?;
        let resolved = config.resolve_alias(metric_type);
        let mut m = Metric::new(resolved, value);
        if let Some(n) = entry["note"].as_str() {
            m.note = Some(n.to_string());
        }
        if let Some(tags) = entry["tags"].as_array() {
            m.tags = tags
                .iter()
                .filter_map(|t| t.as_str().map(String::from))
                .collect();
        }
        db.insert_metric(&m)?;
        results.push(m);
    }

    Ok(results)
}
