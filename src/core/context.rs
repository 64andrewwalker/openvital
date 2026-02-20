use std::collections::HashMap;

use anyhow::Result;
use chrono::{Duration, Local};
use serde::Serialize;

use crate::core::anomaly;
use crate::core::status;
use crate::core::trend::TrendPeriod;
use crate::db::Database;
use crate::models::anomaly::{Anomaly, Threshold};
use crate::models::config::Config;

#[derive(Debug, Serialize)]
pub struct ContextResult {
    pub generated_at: String,
    pub period: ContextPeriod,
    pub summary: String,
    pub metrics: HashMap<String, MetricContext>,
    pub goals: Vec<GoalContext>,
    pub medications: Option<MedicationContext>,
    pub streaks: status::Streaks,
    pub alerts: Vec<AlertItem>,
    pub anomalies: Vec<Anomaly>,
}

#[derive(Debug, Serialize)]
pub struct ContextPeriod {
    pub start: String,
    pub end: String,
    pub days: u32,
}

#[derive(Debug, Serialize)]
pub struct MetricContext {
    pub latest: Option<LatestValue>,
    pub trend: Option<TrendInfo>,
    pub stats: MetricStats,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct LatestValue {
    pub value: f64,
    pub unit: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct TrendInfo {
    pub direction: String,
    pub rate: f64,
    pub rate_unit: String,
}

#[derive(Debug, Serialize)]
pub struct MetricStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub count: u32,
}

#[derive(Debug, Serialize)]
pub struct GoalContext {
    pub metric_type: String,
    pub target: f64,
    pub direction: String,
    pub timeframe: String,
    pub current: Option<f64>,
    pub is_met: bool,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct MedicationContext {
    pub active_count: usize,
    pub adherence_today: f64,
    pub adherence_7d: Option<f64>,
    pub medications: Vec<MedBrief>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct MedBrief {
    pub name: String,
    pub adherent_today: Option<bool>,
    pub adherence_7d: Option<f64>,
    pub streak: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct AlertItem {
    #[serde(rename = "type")]
    pub alert_type: String,
    pub message: String,
}

/// Compute the full health context briefing.
pub fn compute(
    db: &Database,
    config: &Config,
    days: u32,
    type_filter: Option<&[&str]>,
) -> Result<ContextResult> {
    let today = Local::now().date_naive();
    let start_date = today - Duration::days(days as i64);
    let now = chrono::Utc::now();

    // 1. Get all distinct metric types
    let all_types = db.distinct_metric_types()?;
    let types: Vec<&str> = if let Some(filter) = type_filter {
        all_types
            .iter()
            .filter(|t| filter.contains(&t.as_str()))
            .map(|t| t.as_str())
            .collect()
    } else {
        all_types.iter().map(|t| t.as_str()).collect()
    };

    // 2. Build per-metric context
    let mut metrics = HashMap::new();
    for metric_type in &types {
        let entries = db.query_all(Some(metric_type), Some(start_date), Some(today))?;
        if entries.is_empty() {
            continue;
        }

        let latest = entries.last().map(|e| LatestValue {
            value: e.value,
            unit: e.unit.clone(),
            timestamp: e.timestamp.to_rfc3339(),
        });

        let values: Vec<f64> = entries.iter().map(|e| e.value).collect();
        let count = values.len() as u32;
        let sum: f64 = values.iter().sum();
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let raw_avg = sum / values.len() as f64;
        let avg = (raw_avg * 10.0).round() / 10.0;

        let stats = MetricStats {
            min,
            max,
            avg,
            count,
        };

        // Compute trend if enough data
        let trend = if count >= 2 {
            match crate::core::trend::compute(db, metric_type, TrendPeriod::Daily, Some(days)) {
                Ok(t) => Some(TrendInfo {
                    direction: t.trend.direction.clone(),
                    rate: t.trend.rate,
                    rate_unit: t.trend.rate_unit.clone(),
                }),
                Err(_) => None,
            }
        } else {
            None
        };

        // Generate per-metric summary
        let summary = generate_metric_summary(metric_type, &latest, &trend, &stats);

        metrics.insert(
            metric_type.to_string(),
            MetricContext {
                latest,
                trend,
                stats,
                summary,
            },
        );
    }

    // 3. Goals
    let goal_statuses = crate::core::goal::goal_status(db, None)?;
    let goals: Vec<GoalContext> = goal_statuses
        .into_iter()
        .filter(|g| type_filter.is_none() || type_filter.unwrap().contains(&g.metric_type.as_str()))
        .map(|g| {
            let summary = if g.is_met {
                format!(
                    "{} goal met ({} {})",
                    g.metric_type, g.direction, g.target_value
                )
            } else if let Some(current) = g.current_value {
                format!(
                    "{}: {:.1} / {:.1} ({})",
                    g.metric_type, current, g.target_value, g.direction
                )
            } else {
                format!("{} goal: no data yet", g.metric_type)
            };
            GoalContext {
                metric_type: g.metric_type,
                target: g.target_value,
                direction: g.direction,
                timeframe: g.timeframe,
                current: g.current_value,
                is_met: g.is_met,
                summary,
            }
        })
        .collect();

    // 4. Medications
    let medications = match crate::core::med::adherence_status(db, None, 7) {
        Ok(med_statuses) if !med_statuses.is_empty() => {
            let active_count = med_statuses.len();
            let total_scheduled: usize = med_statuses
                .iter()
                .filter(|s| s.adherent_today.is_some())
                .count();
            let adherent_count: usize = med_statuses
                .iter()
                .filter(|s| s.adherent_today == Some(true))
                .count();
            let adherence_today = if total_scheduled > 0 {
                adherent_count as f64 / total_scheduled as f64
            } else {
                1.0
            };

            let adherence_values: Vec<f64> =
                med_statuses.iter().filter_map(|s| s.adherence_7d).collect();
            let adherence_7d = if adherence_values.is_empty() {
                None
            } else {
                Some(adherence_values.iter().sum::<f64>() / adherence_values.len() as f64)
            };

            let meds: Vec<MedBrief> = med_statuses
                .iter()
                .map(|s| MedBrief {
                    name: s.name.clone(),
                    adherent_today: s.adherent_today,
                    adherence_7d: s.adherence_7d,
                    streak: s.streak_days,
                })
                .collect();

            let summary = format!(
                "{} active medication(s). {}/{} taken today.{}",
                active_count,
                adherent_count,
                total_scheduled,
                adherence_7d
                    .map(|a| format!(" {:.0}% adherence (7d).", a * 100.0))
                    .unwrap_or_default()
            );

            Some(MedicationContext {
                active_count,
                adherence_today,
                adherence_7d,
                medications: meds,
                summary,
            })
        }
        _ => None,
    };

    // 5. Streaks
    let streaks = status::compute_streaks(db, today)?;

    // 6. Alerts
    let mut alerts = Vec::new();
    let today_entries = db.query_by_date(today)?;
    let threshold = config.alerts.pain_threshold as f64;
    for entry in &today_entries {
        if (entry.metric_type == "pain" || entry.metric_type == "soreness")
            && entry.value >= threshold
        {
            alerts.push(AlertItem {
                alert_type: "pain_elevated".to_string(),
                message: format!(
                    "{} at {}/10, above threshold of {}",
                    entry.metric_type, entry.value, threshold
                ),
            });
        }
    }

    let consecutive = status::check_consecutive_pain(db, today, &config.alerts)?;
    for alert in &consecutive {
        alerts.push(AlertItem {
            alert_type: "consecutive_pain".to_string(),
            message: format!(
                "{} above threshold for {} consecutive days (latest: {})",
                alert.metric_type, alert.consecutive_days, alert.latest_value
            ),
        });
    }

    // 7. Anomalies (use days as baseline window, moderate threshold)
    let anomaly_result = anomaly::detect(db, None, days.max(14), Threshold::Moderate)?;
    // Filter anomalies to match type_filter if active
    let anomalies: Vec<Anomaly> = anomaly_result
        .anomalies
        .into_iter()
        .filter(|a| type_filter.is_none() || type_filter.unwrap().contains(&a.metric_type.as_str()))
        .collect();

    for a in &anomalies {
        alerts.push(AlertItem {
            alert_type: "anomaly".to_string(),
            message: a.summary.clone(),
        });
    }

    // 8. Generate top-level summary
    let summary = generate_top_summary(&metrics, &goals, &medications, &streaks, &anomalies);

    Ok(ContextResult {
        generated_at: now.to_rfc3339(),
        period: ContextPeriod {
            start: start_date.to_string(),
            end: today.to_string(),
            days,
        },
        summary,
        metrics,
        goals,
        medications,
        streaks,
        alerts,
        anomalies,
    })
}

fn generate_metric_summary(
    metric_type: &str,
    latest: &Option<LatestValue>,
    trend: &Option<TrendInfo>,
    stats: &MetricStats,
) -> String {
    let mut parts = Vec::new();

    if let Some(l) = latest {
        parts.push(format!("{} at {:.1}", metric_type, l.value));
    }

    if let Some(t) = trend {
        if t.direction != "stable" {
            parts.push(format!(
                "{} {:.1} {}",
                t.direction,
                t.rate.abs(),
                t.rate_unit
            ));
        } else {
            parts.push("stable".to_string());
        }
    }

    if stats.count > 1 {
        parts.push(format!("{} readings", stats.count));
    }

    if parts.is_empty() {
        "no data".to_string()
    } else {
        parts.join(", ")
    }
}

fn generate_top_summary(
    metrics: &HashMap<String, MetricContext>,
    goals: &[GoalContext],
    medications: &Option<MedicationContext>,
    streaks: &status::Streaks,
    anomalies: &[Anomaly],
) -> String {
    let mut parts = Vec::new();

    if !metrics.is_empty() {
        parts.push(format!("Tracking {} metric type(s).", metrics.len()));
    } else {
        parts.push("No metrics tracked in this period.".to_string());
    }

    if !goals.is_empty() {
        let met = goals.iter().filter(|g| g.is_met).count();
        parts.push(format!("{}/{} goal(s) met.", met, goals.len()));
    }

    if let Some(meds) = medications {
        parts.push(meds.summary.clone());
    }

    if streaks.logging_days > 0 {
        parts.push(format!("Logging streak: {} day(s).", streaks.logging_days));
    }

    if !anomalies.is_empty() {
        parts.push(format!(
            "{} anomal{} detected.",
            anomalies.len(),
            if anomalies.len() == 1 { "y" } else { "ies" }
        ));
    }

    parts.join(" ")
}
