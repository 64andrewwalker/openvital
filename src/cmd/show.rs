use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;

use openvital::core::query::{self, ShowResult};
use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;
use openvital::output::human;

pub fn run(
    metric_type: Option<&str>,
    last: Option<u32>,
    date: Option<NaiveDate>,
    human_flag: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let result = query::show(&db, &config, metric_type, last, date)?;

    match result {
        ShowResult::ByType {
            metric_type,
            entries,
        } => {
            if human_flag {
                if entries.is_empty() {
                    println!("No entries found for '{}'", metric_type);
                } else {
                    for m in &entries {
                        println!("{}", human::format_metric_with_units(m, &config.units));
                    }
                }
            } else {
                let out =
                    output::success("show", json!({ "type": metric_type, "entries": entries }));
                println!("{}", serde_json::to_string(&out)?);
            }
        }
        ShowResult::ByDate { date, entries } => {
            if human_flag {
                if entries.is_empty() {
                    println!("No entries for {}", date);
                } else {
                    println!("--- {} ---", date);
                    for m in &entries {
                        println!("{}", human::format_metric_with_units(m, &config.units));
                    }
                }
            } else {
                let out = output::success(
                    "show",
                    json!({ "date": date.to_string(), "entries": entries }),
                );
                println!("{}", serde_json::to_string(&out)?);
            }
        }
    }
    Ok(())
}
