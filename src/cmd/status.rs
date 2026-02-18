use anyhow::Result;
use chrono::Local;
use serde_json::json;

use crate::db::Database;
use crate::models::config::Config;
use crate::output;

pub fn run(human: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let today = Local::now().date_naive();
    let entries = db.query_by_date(today)?;

    let logged_types: Vec<&str> = entries.iter().map(|m| m.metric_type.as_str()).collect();

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

    let pain_alerts: Vec<_> = entries
        .iter()
        .filter(|m| (m.metric_type == "pain" || m.metric_type == "soreness") && m.value >= config.alerts.pain_threshold as f64)
        .map(|m| {
            json!({
                "type": m.metric_type,
                "value": m.value,
                "tags": m.tags,
            })
        })
        .collect();

    if human {
        println!("=== OpenVital Status — {} ===\n", today);
        if let (Some(w), Some(b)) = (weight_val, bmi) {
            println!(
                "Weight: {} kg | BMI: {} ({})",
                w,
                b,
                bmi_category.unwrap_or("?")
            );
        }
        if logged_types.is_empty() {
            println!("No entries logged today.");
        } else {
            println!("Logged today: {}", logged_types.join(", "));
        }
        if !pain_alerts.is_empty() {
            println!("Pain alerts: {} active", pain_alerts.len());
        }
    } else {
        let data = json!({
            "date": today.to_string(),
            "profile": {
                "height_cm": config.profile.height_cm,
                "latest_weight_kg": weight_val,
                "bmi": bmi,
                "bmi_category": bmi_category,
            },
            "today": {
                "logged": logged_types,
                "pain_alerts": pain_alerts,
            },
        });
        let out = output::success("status", data);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
