use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};

use openvital::core::report;
use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;

pub fn run(
    period: Option<&str>,
    month: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    human: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;

    let (from_date, to_date) = resolve_range(period, month, from, to)?;
    let result = report::generate(&db, from_date, to_date)?;

    if human {
        println!(
            "=== OpenVital Report: {} to {} ===\n",
            result.from, result.to
        );
        println!(
            "  Days with entries: {} | Total entries: {}",
            result.days_with_entries, result.total_entries
        );
        if result.metrics.is_empty() {
            println!("\n  No data in this period.");
        } else {
            println!();
            for s in &result.metrics {
                let (avg, _) =
                    openvital::core::units::to_display(s.avg, &s.metric_type, &config.units);
                let (min, _) =
                    openvital::core::units::to_display(s.min, &s.metric_type, &config.units);
                let (max, unit) =
                    openvital::core::units::to_display(s.max, &s.metric_type, &config.units);
                println!(
                    "  {:16} | avg: {:8.1} min: {:8.1} max: {:8.1} (n={}) [{}]",
                    s.metric_type, avg, min, max, s.count, unit
                );
            }
        }
        println!();
    } else {
        let out = output::success("report", serde_json::to_value(&result)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

fn resolve_range(
    period: Option<&str>,
    month: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<(NaiveDate, NaiveDate)> {
    if let (Some(f), Some(t)) = (from, to) {
        return Ok((f, t));
    }

    let today = Local::now().date_naive();

    match period.unwrap_or("week") {
        "week" => {
            let from = today - chrono::Duration::days(6);
            Ok((from, today))
        }
        "month" => {
            if let Some(m) = month {
                // Parse "2026-01" format
                let parts: Vec<&str> = m.split('-').collect();
                if parts.len() == 2 {
                    let year: i32 = parts[0].parse()?;
                    let mon: u32 = parts[1].parse()?;
                    let first = NaiveDate::from_ymd_opt(year, mon, 1)
                        .ok_or_else(|| anyhow::anyhow!("invalid month: {}", m))?;
                    let last = if mon == 12 {
                        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap() - chrono::Duration::days(1)
                    } else {
                        NaiveDate::from_ymd_opt(year, mon + 1, 1).unwrap()
                            - chrono::Duration::days(1)
                    };
                    Ok((first, last))
                } else {
                    anyhow::bail!("invalid month format: {} (expected YYYY-MM)", m)
                }
            } else {
                let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
                Ok((first, today))
            }
        }
        other => anyhow::bail!("invalid period: {} (expected week/month)", other),
    }
}
