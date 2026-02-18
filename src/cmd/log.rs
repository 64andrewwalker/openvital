use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};
use serde_json::json;

use crate::db::Database;
use crate::models::config::Config;
use crate::models::metric::Metric;
use crate::output;

pub fn run(
    metric_type: &str,
    value: f64,
    note: Option<&str>,
    tags: Option<&str>,
    source: Option<&str>,
    date: Option<NaiveDate>,
    human: bool,
) -> Result<()> {
    let config = Config::load()?;
    let resolved = config.resolve_alias(metric_type);
    let db = Database::open(&Config::db_path())?;

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

    if human {
        println!("Logged: {}", output::human_metric(&m));
    } else {
        let out = output::success(
            "log",
            json!({
                "entry": {
                    "id": m.id,
                    "timestamp": m.timestamp.to_rfc3339(),
                    "type": m.metric_type,
                    "value": m.value,
                    "unit": m.unit
                }
            }),
        );
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_batch(batch_json: &str) -> Result<()> {
    let db = Database::open(&Config::db_path())?;
    let config = Config::load()?;
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
        results.push(json!({
            "id": m.id,
            "type": m.metric_type,
            "value": m.value,
            "unit": m.unit
        }));
    }

    let out = output::success("log", json!({ "entries": results }));
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}
