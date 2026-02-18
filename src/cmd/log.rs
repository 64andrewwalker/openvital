use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;

use crate::db::Database;
use crate::models::config::Config;
use crate::output;
use crate::output::human;

pub fn run(
    metric_type: &str,
    value: f64,
    note: Option<&str>,
    tags: Option<&str>,
    source: Option<&str>,
    date: Option<NaiveDate>,
    human_flag: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let m = crate::core::logging::log_metric(&db, &config, metric_type, value, note, tags, source, date)?;

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

pub fn run_batch(batch_json: &str) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let metrics = crate::core::logging::log_batch(&db, &config, batch_json)?;

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
    Ok(())
}
