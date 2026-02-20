use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;

use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;

pub fn run_add(
    name: &str,
    dose: Option<&str>,
    freq: &str,
    route: &str,
    note: Option<&str>,
    started: Option<NaiveDate>,
    human: bool,
) -> Result<()> {
    let config = Config::load()?;
    let resolved = config.resolve_alias(name);
    let db = Database::open(&Config::db_path())?;

    let params = openvital::core::med::AddMedicationParams {
        name: &resolved,
        dose,
        freq,
        route: Some(route),
        note,
        started,
    };
    let medication = openvital::core::med::add_medication(&db, &config, params)?;

    if human {
        let dose_str = medication.dose.as_deref().unwrap_or("(no dose)");
        let note_str = medication
            .note
            .as_ref()
            .map(|n| format!("  \"{}\"", n))
            .unwrap_or_default();
        println!(
            "Added {} {} {} {} since {}{}",
            medication.name,
            dose_str,
            medication.route,
            medication.frequency,
            medication.started_at.format("%b %d"),
            note_str,
        );
    } else {
        let out = output::success(
            "med_add",
            json!({
                "id": medication.id,
                "name": medication.name,
                "dose": medication.dose,
                "route": medication.route,
                "frequency": medication.frequency,
                "active": medication.active,
                "started_at": medication.started_at.to_rfc3339(),
            }),
        );
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_take(
    name: &str,
    dose: Option<&str>,
    note: Option<&str>,
    tags: Option<&str>,
    date: Option<NaiveDate>,
    human: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;

    let (metric, medication) =
        openvital::core::med::take_medication(&db, &config, name, dose, note, tags, date)?;

    let is_stopped = !medication.active;

    if human {
        if is_stopped {
            eprintln!(
                "Warning: Medication '{}' is stopped. Recording anyway.",
                medication.name
            );
        }
        let dose_str = dose
            .map(String::from)
            .or(medication.dose.clone())
            .unwrap_or_else(|| "1 dose".to_string());
        let ts = metric.timestamp.format("%Y-%m-%d %H:%M");
        println!(
            "{}",
            openvital::output::human::format_med_take(
                &medication.name,
                &dose_str,
                &medication.route.to_string(),
                &ts.to_string(),
            )
        );
    } else {
        let mut data = json!({
            "medication": medication.name,
            "dose": dose.map(String::from).or(medication.dose),
            "route": medication.route,
            "entry": {
                "id": metric.id,
                "timestamp": metric.timestamp.to_rfc3339(),
                "type": metric.metric_type,
                "value": metric.value,
                "unit": metric.unit,
                "note": metric.note,
            }
        });
        if is_stopped {
            data["warning"] = json!(format!(
                "Medication '{}' is stopped. Recording anyway.",
                medication.name
            ));
            eprintln!(
                "Warning: Medication '{}' is stopped. Recording anyway.",
                medication.name
            );
        }
        let out = output::success("med_take", data);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_list(all: bool, human: bool) -> Result<()> {
    let db = Database::open(&Config::db_path())?;

    let meds = openvital::core::med::list_medications(&db, all)?;

    if human {
        println!("{}", openvital::output::human::format_med_list(&meds, all));
    } else {
        let count = meds.len();
        let out = output::success(
            "med_list",
            json!({
                "medications": meds,
                "count": count,
            }),
        );
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_stop(
    name: &str,
    reason: Option<&str>,
    date: Option<NaiveDate>,
    human: bool,
) -> Result<()> {
    let config = Config::load()?;
    let resolved = config.resolve_alias(name);
    let db = Database::open(&Config::db_path())?;

    let stopped = openvital::core::med::stop_medication(&db, &resolved, reason, date)?;

    if !stopped {
        anyhow::bail!("Medication '{}' not found or already stopped.", resolved);
    }

    if human {
        println!(
            "{}",
            openvital::output::human::format_med_stop(&resolved, reason)
        );
    } else {
        let out = output::success(
            "med_stop",
            json!({
                "name": resolved,
                "stopped": true,
                "reason": reason,
            }),
        );
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_remove(name: &str, yes: bool, human: bool) -> Result<()> {
    let config = Config::load()?;
    let resolved = config.resolve_alias(name);
    let db = Database::open(&Config::db_path())?;

    if !yes {
        eprint!(
            "Permanently delete medication '{}'? Metric history will be preserved. [y/N] ",
            resolved
        );
        use std::io::{self, BufRead, Write};
        io::stderr().flush().ok();
        let mut buf = String::new();
        let bytes = io::stdin().lock().read_line(&mut buf)?;
        if bytes == 0 || !buf.trim().eq_ignore_ascii_case("y") {
            anyhow::bail!("Aborted.");
        }
    }

    let removed = openvital::core::med::remove_medication(&db, &resolved)?;

    if !removed {
        anyhow::bail!("Medication '{}' not found.", resolved);
    }

    if human {
        println!("Removed medication: {}", resolved);
    } else {
        let out = output::success(
            "med_remove",
            json!({
                "name": resolved,
                "removed": true,
            }),
        );
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_status(name: Option<&str>, last: u32, human: bool) -> Result<()> {
    let config = Config::load()?;
    let resolved = name.map(|n| config.resolve_alias(n));
    let db = Database::open(&Config::db_path())?;

    let statuses = openvital::core::med::adherence_status(&db, resolved.as_deref(), last)?;

    if human {
        let today = chrono::Utc::now().date_naive();
        println!(
            "{}",
            openvital::output::human::format_med_status(&statuses, today)
        );
    } else {
        let data = if name.is_some() && statuses.len() == 1 {
            // Single medication: output directly
            json!(statuses.into_iter().next().unwrap())
        } else {
            // All medications: wrap with date and overall adherence
            let today = chrono::Utc::now().date_naive();
            let adherence_values: Vec<f64> =
                statuses.iter().filter_map(|s| s.adherence_7d).collect();
            let overall = if adherence_values.is_empty() {
                None
            } else {
                Some(adherence_values.iter().sum::<f64>() / adherence_values.len() as f64)
            };
            json!({
                "date": today.format("%Y-%m-%d").to_string(),
                "medications": statuses,
                "overall_adherence_7d": overall,
            })
        };
        let out = output::success("med_status", data);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
