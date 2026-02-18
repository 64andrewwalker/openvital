use anyhow::Result;
use chrono::{Local, NaiveDate};
use serde::Serialize;
use serde_json::Value;

use crate::db::Database;
use crate::models::config::Config;

#[derive(Serialize)]
pub struct StatusData {
    pub date: NaiveDate,
    pub profile: ProfileStatus,
    pub today: TodayStatus,
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
    })
}
