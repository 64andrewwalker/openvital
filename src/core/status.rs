use anyhow::Result;
use chrono::{Duration, Local, NaiveDate};
use serde::Serialize;
use serde_json::Value;

use crate::db::Database;
use crate::models::config::{Alerts, Config};

#[derive(Serialize)]
pub struct MedicationStatus {
    pub active_count: usize,
    pub adherent_today: usize,
    pub non_adherent_today: usize,
    pub as_needed: usize,
    pub missed: Vec<String>,
    pub overall_adherence_7d: Option<f64>,
}

#[derive(Serialize)]
pub struct StatusData {
    pub date: NaiveDate,
    pub profile: ProfileStatus,
    pub today: TodayStatus,
    pub streaks: Streaks,
    pub consecutive_pain_alerts: Vec<ConsecutivePainAlert>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medications: Option<MedicationStatus>,
}

#[derive(Serialize)]
pub struct ProfileStatus {
    pub height_cm: Option<f64>,
    pub latest_weight_kg: Option<f64>,
    pub bmi: Option<f64>,
    pub bmi_category: Option<&'static str>,
}

#[derive(Serialize)]
pub struct TodayStatus {
    pub logged: Vec<String>,
    pub pain_alerts: Vec<Value>,
}

#[derive(Debug, Serialize)]
pub struct Streaks {
    pub logging_days: u32,
}

#[derive(Debug, Serialize)]
pub struct ConsecutivePainAlert {
    pub metric_type: String,
    pub consecutive_days: u32,
    pub latest_value: f64,
}

/// Compute the daily status overview.
pub fn compute(db: &Database, config: &Config) -> Result<StatusData> {
    let today = Local::now().date_naive();
    let entries = db.query_by_date(today)?;

    let logged: Vec<String> = entries.iter().map(|m| m.metric_type.clone()).collect();

    let latest_weight = db.query_by_type("weight", Some(1))?;
    let weight_val = latest_weight.first().map(|m| m.value);

    let bmi = match (config.profile.height_cm, weight_val) {
        (Some(h), Some(w)) => {
            let h_m = h / 100.0;
            Some((w / (h_m * h_m) * 10.0).round() / 10.0)
        }
        _ => None,
    };

    let bmi_category = bmi.map(|b| match b {
        b if b < 18.5 => "underweight",
        b if b < 25.0 => "normal",
        b if b < 30.0 => "overweight",
        _ => "obese",
    });

    let threshold = config.alerts.pain_threshold as f64;
    let pain_alerts: Vec<Value> = entries
        .iter()
        .filter(|m| {
            (m.metric_type == "pain" || m.metric_type == "soreness") && m.value >= threshold
        })
        .map(|m| {
            serde_json::json!({
                "type": m.metric_type,
                "value": m.value,
                "tags": m.tags,
            })
        })
        .collect();

    let streaks = compute_streaks(db, today)?;
    let consecutive_pain_alerts = check_consecutive_pain(db, today, &config.alerts)?;

    // Compute medication status
    let medications = match crate::core::med::adherence_status(db, None, 7) {
        Ok(med_statuses) if !med_statuses.is_empty() => {
            let active_count = med_statuses.len();
            let mut adherent = 0;
            let mut non_adherent = 0;
            let mut as_needed_count = 0;
            let mut missed = Vec::new();

            for s in &med_statuses {
                match s.adherent_today {
                    Some(true) => adherent += 1,
                    Some(false) => {
                        non_adherent += 1;
                        let taken = s.taken_today;
                        if let Some(req) = s.required_today {
                            missed.push(format!("{} ({}/{} taken)", s.name, taken, req));
                        } else {
                            // Weekly meds: show taken count without required
                            missed.push(format!("{} ({} taken this week)", s.name, taken));
                        }
                    }
                    None => as_needed_count += 1,
                }
            }

            let adherence_values: Vec<f64> =
                med_statuses.iter().filter_map(|s| s.adherence_7d).collect();
            let overall = if adherence_values.is_empty() {
                None
            } else {
                Some(adherence_values.iter().sum::<f64>() / adherence_values.len() as f64)
            };

            Some(MedicationStatus {
                active_count,
                adherent_today: adherent,
                non_adherent_today: non_adherent,
                as_needed: as_needed_count,
                missed,
                overall_adherence_7d: overall,
            })
        }
        _ => None,
    };

    Ok(StatusData {
        date: today,
        profile: ProfileStatus {
            height_cm: config.profile.height_cm,
            latest_weight_kg: weight_val,
            bmi,
            bmi_category,
        },
        today: TodayStatus {
            logged,
            pain_alerts,
        },
        streaks,
        consecutive_pain_alerts,
        medications,
    })
}

/// Compute streak of consecutive days with any logged entry, ending at `today`.
pub fn compute_streaks(db: &Database, today: NaiveDate) -> Result<Streaks> {
    // Look back up to 365 days for streak calculation
    let from = today - Duration::days(365);
    let dates = db.distinct_entry_dates(from, today)?;

    let mut streak = 0u32;
    let mut check_date = today;
    for date_str in &dates {
        if let Ok(d) = date_str.parse::<NaiveDate>() {
            if d == check_date {
                streak += 1;
                check_date -= Duration::days(1);
            } else {
                break;
            }
        }
    }

    Ok(Streaks {
        logging_days: streak,
    })
}

/// Check if pain/soreness has been above threshold for N consecutive days.
pub fn check_consecutive_pain(
    db: &Database,
    today: NaiveDate,
    alerts: &Alerts,
) -> Result<Vec<ConsecutivePainAlert>> {
    let threshold = alerts.pain_threshold as f64;
    let required_days = alerts.pain_consecutive_days as u32;
    let mut result = Vec::new();

    for pain_type in &["pain", "soreness"] {
        let mut consecutive = 0u32;
        let mut latest_value = 0.0f64;

        for i in 0..30 {
            // look back up to 30 days
            let date = today - Duration::days(i);
            let entries = db.query_by_date(date)?;
            let day_pain: Vec<f64> = entries
                .iter()
                .filter(|m| m.metric_type == *pain_type && m.value >= threshold)
                .map(|m| m.value)
                .collect();

            if day_pain.is_empty() {
                break;
            }

            consecutive += 1;
            if i == 0 {
                latest_value = day_pain.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            }
        }

        if consecutive >= required_days {
            result.push(ConsecutivePainAlert {
                metric_type: pain_type.to_string(),
                consecutive_days: consecutive,
                latest_value,
            });
        }
    }

    Ok(result)
}
