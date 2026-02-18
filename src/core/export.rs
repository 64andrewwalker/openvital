use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;

use crate::db::Database;
use crate::models::metric::{Category, Metric, default_unit};

/// Export metrics to CSV format.
pub fn to_csv(
    db: &Database,
    metric_type: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<String> {
    let entries = db.query_all(metric_type, from, to)?;
    let mut out = String::from("timestamp,type,value,unit,note,tags,source\n");
    for e in &entries {
        let note = e.note.as_deref().unwrap_or("");
        let tags = if e.tags.is_empty() {
            "[]".to_string()
        } else {
            serde_json::to_string(&e.tags)?
        };
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            e.timestamp.to_rfc3339(),
            e.metric_type,
            e.value,
            e.unit,
            note,
            tags,
            e.source,
        ));
    }
    Ok(out)
}

/// Export metrics to JSON format (array of metric objects).
pub fn to_json(
    db: &Database,
    metric_type: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<String> {
    let entries = db.query_all(metric_type, from, to)?;
    Ok(serde_json::to_string_pretty(&entries)?)
}

#[derive(Deserialize)]
struct ImportEntry {
    #[serde(rename = "type")]
    metric_type: String,
    value: f64,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    source: Option<String>,
}

/// Import metrics from JSON string (array of entries).
pub fn import_json(db: &Database, json_str: &str) -> Result<usize> {
    let entries: Vec<ImportEntry> = serde_json::from_str(json_str)?;
    let mut count = 0;
    for e in entries {
        let mut m = Metric::new(e.metric_type.clone(), e.value);
        if let Some(ts) = &e.timestamp {
            m.timestamp = ts.parse::<DateTime<Utc>>()?;
        }
        m.note = e.note;
        m.tags = e.tags.unwrap_or_default();
        m.source = e.source.unwrap_or_else(|| "import".to_string());
        db.insert_metric(&m)?;
        count += 1;
    }
    Ok(count)
}

/// Import metrics from CSV string.
pub fn import_csv(db: &Database, csv_str: &str) -> Result<usize> {
    let mut lines = csv_str.lines();
    let _header = lines.next(); // skip header
    let mut count = 0;
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.splitn(7, ',').collect();
        if fields.len() < 3 {
            continue;
        }
        let timestamp: DateTime<Utc> = fields[0].parse()?;
        let metric_type = fields[1].to_string();
        let value: f64 = fields[2].parse()?;
        let unit = if fields.len() > 3 && !fields[3].is_empty() {
            fields[3].to_string()
        } else {
            default_unit(&metric_type).to_string()
        };
        let note = if fields.len() > 4 && !fields[4].is_empty() {
            Some(fields[4].to_string())
        } else {
            None
        };
        let tags: Vec<String> = if fields.len() > 5 && !fields[5].is_empty() {
            serde_json::from_str(fields[5]).unwrap_or_default()
        } else {
            Vec::new()
        };
        let source = if fields.len() > 6 && !fields[6].is_empty() {
            fields[6].to_string()
        } else {
            "import".to_string()
        };

        let category = Category::from_type(&metric_type);
        let m = Metric {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp,
            category,
            metric_type,
            value,
            unit,
            note,
            tags,
            source,
        };
        db.insert_metric(&m)?;
        count += 1;
    }
    Ok(count)
}
