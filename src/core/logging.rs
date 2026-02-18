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

/// Log a blood pressure compound value (e.g., "120/80").
/// Parses the value, converts units, and creates two metric entries (systolic + diastolic).
pub fn log_blood_pressure(
    db: &Database,
    config: &Config,
    value_str: &str,
    note: Option<&str>,
    tags: Option<&str>,
    source: Option<&str>,
    date: Option<NaiveDate>,
) -> Result<(Metric, Metric)> {
    let parts: Vec<&str> = value_str.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!("blood pressure format must be SYSTOLIC/DIASTOLIC (e.g., 120/80)");
    }
    let systolic: f64 = parts[0]
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid systolic value"))?;
    let diastolic: f64 = parts[1]
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid diastolic value"))?;

    let sys_metric = crate::core::units::from_input(systolic, "bp_systolic", &config.units);
    let dia_metric = crate::core::units::from_input(diastolic, "bp_diastolic", &config.units);

    let m1 = log_metric(
        db,
        config,
        LogEntry {
            metric_type: "bp_systolic",
            value: sys_metric,
            note,
            tags,
            source,
            date,
        },
    )?;
    let m2 = log_metric(
        db,
        config,
        LogEntry {
            metric_type: "bp_diastolic",
            value: dia_metric,
            note,
            tags,
            source,
            date,
        },
    )?;
    Ok((m1, m2))
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
        let value = crate::core::units::from_input(value, &resolved, &config.units);
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
