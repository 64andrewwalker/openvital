use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};

use crate::db::Database;
use crate::models::config::Config;
use crate::models::metric::Metric;

/// Parameters for logging a single metric.
pub struct LogEntry<'a> {
    pub metric_type: &'a str,
    pub value: f64,
    pub note: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub source: Option<&'a str>,
    pub date: Option<NaiveDate>,
}

/// Log a single metric. Returns the created Metric.
pub fn log_metric(db: &Database, config: &Config, entry: LogEntry<'_>) -> Result<Metric> {
    let resolved = config.resolve_alias(entry.metric_type);
    let mut m = Metric::new(resolved, entry.value);
    if let Some(n) = entry.note {
        m.note = Some(n.to_string());
    }
    if let Some(t) = entry.tags {
        m.tags = t.split(',').map(|s| s.trim().to_string()).collect();
    }
    if let Some(s) = entry.source {
        m.source = s.to_string();
    }
    if let Some(d) = entry.date
        && let Some(dt) = d.and_hms_opt(12, 0, 0)
    {
        m.timestamp = Utc.from_utc_datetime(&dt);
    }
    db.insert_metric(&m)?;
    Ok(m)
}

/// Batch-log metrics from a JSON array string. Returns created Metrics.
pub fn log_batch(db: &Database, config: &Config, batch_json: &str) -> Result<Vec<Metric>> {
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

/// Convert simple batch format ("weight:72.5,sleep:7.5") to JSON array string.
pub fn parse_simple_batch(input: &str) -> Result<String> {
    let entries: Vec<serde_json::Value> = input
        .split(',')
        .map(|pair| {
            let parts: Vec<&str> = pair.trim().splitn(2, ':').collect();
            if parts.len() != 2 {
                anyhow::bail!("invalid batch entry: '{}' (expected type:value)", pair);
            }
            let value: f64 = parts[1]
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid value in '{}'", pair))?;
            Ok(serde_json::json!({"type": parts[0].trim(), "value": value}))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(serde_json::to_string(&entries)?)
}
