use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;

use openvital::core::logging::LogEntry;
use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;
use openvital::output::human;

pub fn run(
    metric_type: &str,
    value_str: &str,
    note: Option<&str>,
    tags: Option<&str>,
    source: Option<&str>,
    date: Option<NaiveDate>,
    human_flag: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let resolved_type = config.resolve_alias(metric_type);

    // Check for blood pressure compound value (e.g., "120/80")
    if (resolved_type == "blood_pressure" || resolved_type == "bp") && value_str.contains('/') {
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

        let m1 = openvital::core::logging::log_metric(
            &db,
            &config,
            LogEntry {
                metric_type: "bp_systolic",
                value: systolic,
                note,
                tags,
                source,
                date,
            },
        )?;
        let m2 = openvital::core::logging::log_metric(
            &db,
            &config,
            LogEntry {
                metric_type: "bp_diastolic",
                value: diastolic,
                note,
                tags,
                source,
                date,
            },
        )?;

        if human_flag {
            println!("Logged: BP {}/{} {}", m1.value, m2.value, m1.unit);
        } else {
            let out = output::success(
                "log",
                json!({
                    "entries": [
                        {"id": m1.id, "type": m1.metric_type, "value": m1.value, "unit": m1.unit},
                        {"id": m2.id, "type": m2.metric_type, "value": m2.value, "unit": m2.unit}
                    ]
                }),
            );
            println!("{}", serde_json::to_string(&out)?);
        }
        return Ok(());
    }

    // Normal single-value log
    let value: f64 = value_str
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid value: {}", value_str))?;
    let m = openvital::core::logging::log_metric(
        &db,
        &config,
        LogEntry {
            metric_type,
            value,
            note,
            tags,
            source,
            date,
        },
    )?;

    if human_flag {
        println!("Logged: {}", human::format_metric(&m));
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

pub fn run_batch(batch_input: &str, human_flag: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;

    // Detect format: JSON array starts with '[', otherwise simple format
    let batch_json = if batch_input.trim_start().starts_with('[') {
        batch_input.to_string()
    } else {
        openvital::core::logging::parse_simple_batch(batch_input)?
    };

    let metrics = openvital::core::logging::log_batch(&db, &config, &batch_json)?;

    if human_flag {
        for m in &metrics {
            println!("Logged: {}", human::format_metric(m));
        }
    } else {
        let entries: Vec<_> = metrics
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "type": m.metric_type,
                    "value": m.value,
                    "unit": m.unit
                })
            })
            .collect();
        let out = output::success("log", json!({ "entries": entries }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
