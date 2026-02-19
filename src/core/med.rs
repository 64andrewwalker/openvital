use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::db::Database;
use crate::models::config::Config;
use crate::models::med::{Frequency, Medication, Route, parse_dose};
use crate::models::metric::{Category, Metric};

// ---------------------------------------------------------------------------
// Adherence structs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct MedStatus {
    pub name: String,
    pub dose: Option<String>,
    pub route: String,
    pub frequency: String,
    pub required_today: Option<u32>,
    pub taken_today: u32,
    pub adherent_today: Option<bool>,
    pub streak_days: Option<u32>,
    pub adherence_7d: Option<f64>,
    pub adherence_30d: Option<f64>,
    pub adherence_history: Option<Vec<DayAdherence>>,
}

#[derive(Debug, Serialize)]
pub struct DayAdherence {
    pub date: NaiveDate,
    pub required: u32,
    pub taken: u32,
    pub adherent: bool,
}

// ---------------------------------------------------------------------------
// AddMedicationParams
// ---------------------------------------------------------------------------

/// Parameters for adding a new medication.
pub struct AddMedicationParams<'a> {
    pub name: &'a str,
    pub dose: Option<&'a str>,
    pub freq: &'a str,
    pub route: Option<&'a str>,
    pub note: Option<&'a str>,
    pub started: Option<NaiveDate>,
}

// ---------------------------------------------------------------------------
// add_medication
// ---------------------------------------------------------------------------

pub fn add_medication(
    db: &Database,
    _config: &Config,
    params: AddMedicationParams<'_>,
) -> Result<Medication> {
    let frequency: Frequency = params.freq.parse()?;
    let route_parsed: Route = params
        .route
        .unwrap_or("oral")
        .parse()
        .unwrap_or(Route::Oral);
    let parsed = parse_dose(params.dose);

    let mut med = Medication::new(params.name, frequency);
    med.route = route_parsed;

    if let Some(d) = params.dose {
        med.dose = Some(d.to_string());
    }
    med.dose_value = parsed.value;
    med.dose_unit = Some(parsed.unit);

    if let Some(n) = params.note {
        med.note = Some(n.to_string());
    }

    if let Some(d) = params.started
        && let Some(dt) = d.and_hms_opt(12, 0, 0)
    {
        med.started_at = Utc.from_utc_datetime(&dt);
    }

    match db.insert_medication(&med) {
        Ok(()) => Ok(med),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE") || msg.contains("unique") || msg.contains("constraint") {
                bail!(
                    "Medication '{}' is already active. Stop it first before re-adding.",
                    params.name
                );
            }
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// take_medication
// ---------------------------------------------------------------------------

pub fn take_medication(
    db: &Database,
    config: &Config,
    name: &str,
    dose_override: Option<&str>,
    note: Option<&str>,
    tags: Option<&str>,
    date: Option<NaiveDate>,
) -> Result<(Metric, Medication)> {
    let resolved = config.resolve_alias(name);

    // Look up medication: active first, then any
    let medication = match db.get_medication_by_name(&resolved)? {
        Some(m) => m,
        None => match db.get_medication_by_name_any(&resolved)? {
            Some(m) => m,
            None => bail!("Medication '{}' not found. Use `med add` first.", resolved),
        },
    };

    let is_stopped = !medication.active;

    // Build note
    let dose_note = if let Some(ov) = dose_override {
        Some(format!("{ov} (override)"))
    } else {
        medication.dose.clone()
    };

    let final_note = match (dose_note, is_stopped, note) {
        (Some(dn), true, Some(n)) => Some(format!("{dn} (stopped); {n}")),
        (Some(dn), true, None) => Some(format!("{dn} (stopped)")),
        (Some(dn), false, Some(n)) => Some(format!("{dn}; {n}")),
        (Some(dn), false, None) => Some(dn),
        (None, true, Some(n)) => Some(format!("(stopped); {n}")),
        (None, true, None) => Some("(stopped)".to_string()),
        (None, false, Some(n)) => Some(n.to_string()),
        (None, false, None) => None,
    };

    // Build timestamp
    let timestamp = if let Some(d) = date
        && let Some(dt) = d.and_hms_opt(12, 0, 0)
    {
        Utc.from_utc_datetime(&dt)
    } else {
        Utc::now()
    };

    // Build tags
    let parsed_tags: Vec<String> = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Build metric struct directly (NOT Metric::new) to set category correctly
    let metric = Metric {
        id: Uuid::new_v4().to_string(),
        timestamp,
        category: Category::Medication,
        metric_type: resolved,
        value: 1.0,
        unit: "dose".to_string(),
        note: final_note,
        tags: parsed_tags,
        source: "med_take".to_string(),
    };

    db.insert_metric(&metric)?;

    Ok((metric, medication))
}

// ---------------------------------------------------------------------------
// stop_medication
// ---------------------------------------------------------------------------

pub fn stop_medication(
    db: &Database,
    name: &str,
    reason: Option<&str>,
    date: Option<NaiveDate>,
) -> Result<bool> {
    let stopped_at = if let Some(d) = date
        && let Some(dt) = d.and_hms_opt(12, 0, 0)
    {
        Utc.from_utc_datetime(&dt)
    } else {
        Utc::now()
    };

    db.stop_medication(name, stopped_at, reason)
}

// ---------------------------------------------------------------------------
// remove_medication
// ---------------------------------------------------------------------------

pub fn remove_medication(db: &Database, name: &str) -> Result<bool> {
    db.remove_medication(name)
}

// ---------------------------------------------------------------------------
// list_medications
// ---------------------------------------------------------------------------

pub fn list_medications(db: &Database, include_stopped: bool) -> Result<Vec<Medication>> {
    db.list_medications(include_stopped)
}

// ---------------------------------------------------------------------------
// adherence_status
// ---------------------------------------------------------------------------

pub fn adherence_status(
    db: &Database,
    name: Option<&str>,
    last_days: u32,
) -> Result<Vec<MedStatus>> {
    let meds = if let Some(n) = name {
        match db.get_medication_by_name(n)? {
            Some(m) => vec![m],
            None => match db.get_medication_by_name_any(n)? {
                Some(m) => vec![m],
                None => bail!("Medication '{}' not found.", n),
            },
        }
    } else {
        db.list_medications(false)?
    };

    let single_med = name.is_some();
    let today = Utc::now().date_naive();

    let mut results = Vec::new();
    for med in &meds {
        let required_per_day = med.frequency.required_per_day();
        let is_as_needed = med.frequency == Frequency::AsNeeded;
        let is_weekly = med.frequency == Frequency::Weekly;

        // Count today's intakes
        let today_entries = db.query_by_date(today)?;
        let taken_today = today_entries
            .iter()
            .filter(|m| m.metric_type == med.name && m.source == "med_take")
            .count() as u32;

        // required_today
        let required_today = if is_weekly || is_as_needed {
            None
        } else {
            required_per_day
        };

        // adherent_today
        let adherent_today = if is_as_needed {
            None
        } else if is_weekly {
            let weekday = today.weekday().num_days_from_monday();
            let week_start = today - chrono::Duration::days(weekday as i64);
            let week_entries = db.query_by_date_range(week_start, today)?;
            let taken_this_week = week_entries
                .iter()
                .filter(|m| m.metric_type == med.name && m.source == "med_take")
                .count();
            Some(taken_this_week >= 1)
        } else {
            Some(taken_today >= required_per_day.unwrap_or(0))
        };

        // Compute streak and adherence over last N days
        let (streak_days, adherence_7d, adherence_30d, adherence_history) = if is_as_needed {
            (None, None, None, None)
        } else {
            let started_date = med.started_at.date_naive();
            let stopped_date = med.stopped_at.map(|t| t.date_naive());

            // Streak: count backward from today
            let mut streak = 0u32;
            if is_weekly {
                // For weekly: iterate week by week
                let weekday = today.weekday().num_days_from_monday();
                let mut week_start = today - chrono::Duration::days(weekday as i64);
                loop {
                    if week_start < started_date - chrono::Duration::days(6) {
                        break;
                    }
                    if let Some(sd) = stopped_date
                        && week_start > sd
                    {
                        break;
                    }
                    let week_end = week_start + chrono::Duration::days(6);
                    let entries = db.query_by_date_range(week_start, week_end)?;
                    let taken = entries
                        .iter()
                        .filter(|m| m.metric_type == med.name && m.source == "med_take")
                        .count();
                    if taken >= 1 {
                        streak += 1;
                    } else {
                        break;
                    }
                    week_start -= chrono::Duration::days(7);
                }
            } else {
                for i in 0.. {
                    let day = today - chrono::Duration::days(i);
                    if day < started_date {
                        break;
                    }
                    if let Some(sd) = stopped_date
                        && day > sd
                    {
                        break;
                    }
                    let day_adherent = check_day_adherent(db, &med.name, day, &med.frequency)?;
                    if day_adherent {
                        streak += 1;
                    } else {
                        break;
                    }
                }
            }

            // 7-day adherence
            let adh_7d = compute_adherence_window(
                db,
                &med.name,
                &med.frequency,
                today,
                7,
                started_date,
                stopped_date,
            )?;

            // 30-day adherence (only for single med)
            let adh_30d = if single_med {
                compute_adherence_window(
                    db,
                    &med.name,
                    &med.frequency,
                    today,
                    30,
                    started_date,
                    stopped_date,
                )?
            } else {
                None
            };

            // History (only for single med)
            let history = if single_med {
                let mut days = Vec::new();
                for i in 0..last_days {
                    let day = today - chrono::Duration::days(i as i64);
                    if day < started_date {
                        break;
                    }
                    if let Some(sd) = stopped_date
                        && day > sd
                    {
                        continue;
                    }
                    let required = day_required(&med.frequency, db, &med.name, day)?;
                    let day_entries = db.query_by_date(day)?;
                    let taken = day_entries
                        .iter()
                        .filter(|m| m.metric_type == med.name && m.source == "med_take")
                        .count() as u32;
                    let adherent = taken >= required;
                    days.push(DayAdherence {
                        date: day,
                        required,
                        taken,
                        adherent,
                    });
                }
                Some(days)
            } else {
                None
            };

            (Some(streak), adh_7d, adh_30d, history)
        };

        results.push(MedStatus {
            name: med.name.clone(),
            dose: med.dose.clone(),
            route: med.route.to_string(),
            frequency: med.frequency.to_string(),
            required_today,
            taken_today,
            adherent_today,
            streak_days,
            adherence_7d,
            adherence_30d,
            adherence_history,
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if a specific day is adherent for a given medication.
fn check_day_adherent(
    db: &Database,
    med_name: &str,
    day: NaiveDate,
    frequency: &Frequency,
) -> Result<bool> {
    if *frequency == Frequency::Weekly {
        let weekday = day.weekday().num_days_from_monday();
        let week_start = day - chrono::Duration::days(weekday as i64);
        let week_end = week_start + chrono::Duration::days(6);
        let entries = db.query_by_date_range(week_start, week_end)?;
        let taken = entries
            .iter()
            .filter(|m| m.metric_type == med_name && m.source == "med_take")
            .count();
        return Ok(taken >= 1);
    }

    let required = frequency.required_per_day().unwrap_or(1);
    let entries = db.query_by_date(day)?;
    let taken = entries
        .iter()
        .filter(|m| m.metric_type == med_name && m.source == "med_take")
        .count() as u32;
    Ok(taken >= required)
}

/// Compute required doses for a day depending on frequency.
fn day_required(
    frequency: &Frequency,
    db: &Database,
    med_name: &str,
    day: NaiveDate,
) -> Result<u32> {
    if *frequency == Frequency::Weekly {
        let weekday = day.weekday().num_days_from_monday();
        let week_start = day - chrono::Duration::days(weekday as i64);
        let entries = db.query_by_date_range(week_start, day)?;
        let taken = entries
            .iter()
            .filter(|m| m.metric_type == med_name && m.source == "med_take")
            .count() as u32;
        if taken >= 1 || weekday == 6 {
            return Ok(1);
        }
        return Ok(0);
    }
    Ok(frequency.required_per_day().unwrap_or(1))
}

/// Compute adherence percentage over a window of days.
fn compute_adherence_window(
    db: &Database,
    med_name: &str,
    frequency: &Frequency,
    today: NaiveDate,
    window: u32,
    started_date: NaiveDate,
    stopped_date: Option<NaiveDate>,
) -> Result<Option<f64>> {
    let mut eligible = 0u32;
    let mut adherent_count = 0u32;

    for i in 0..window {
        let day = today - chrono::Duration::days(i as i64);
        if day < started_date {
            continue;
        }
        if let Some(sd) = stopped_date
            && day > sd
        {
            continue;
        }
        eligible += 1;
        if check_day_adherent(db, med_name, day, frequency)? {
            adherent_count += 1;
        }
    }

    if eligible == 0 {
        Ok(None)
    } else {
        Ok(Some(f64::from(adherent_count) / f64::from(eligible)))
    }
}
