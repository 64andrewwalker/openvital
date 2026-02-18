use anyhow::Result;
use chrono::{Local, NaiveDate};
use serde_json::json;

use crate::db::Database;
use crate::models::config::Config;
use crate::output;

pub fn run(
    metric_type: Option<&str>,
    last: Option<u32>,
    date: Option<NaiveDate>,
    human: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;

    // `openvital show today` or `openvital show --date ...`
    if metric_type == Some("today") || (metric_type.is_none() && date.is_none()) {
        let d = date.unwrap_or_else(|| Local::now().date_naive());
        return show_date(&db, d, human);
    }

    if let Some(d) = date {
        return show_date(&db, d, human);
    }

    let metric_type = metric_type.unwrap();
    let resolved = config.resolve_alias(metric_type);
    let entries = db.query_by_type(&resolved, Some(last.unwrap_or(1)))?;

    if human {
        if entries.is_empty() {
            println!("No entries found for '{}'", resolved);
        } else {
            for m in &entries {
                println!("{}", output::human_metric(m));
            }
        }
    } else {
        let out = output::success(
            "show",
            json!({
                "type": resolved,
                "entries": entries,
            }),
        );
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

fn show_date(db: &Database, date: NaiveDate, human: bool) -> Result<()> {
    let entries = db.query_by_date(date)?;

    if human {
        if entries.is_empty() {
            println!("No entries for {}", date);
        } else {
            println!("--- {} ---", date);
            for m in &entries {
                println!("{}", output::human_metric(m));
            }
        }
    } else {
        let out = output::success(
            "show",
            json!({
                "date": date.to_string(),
                "entries": entries,
            }),
        );
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
