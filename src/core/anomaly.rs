use std::collections::HashSet;

use anyhow::Result;
use chrono::{Duration, Local};

use crate::db::Database;
use crate::models::anomaly::{
    Anomaly, AnomalyPeriod, AnomalyResult, Baseline, Bounds, Severity, Threshold,
};

/// Minimum data points required to compute a meaningful baseline.
const MIN_DATA_POINTS: usize = 7;

/// Detect anomalies across one or all metric types.
pub fn detect(
    db: &Database,
    metric_type: Option<&str>,
    baseline_days: u32,
    threshold: Threshold,
) -> Result<AnomalyResult> {
    let today = Local::now().date_naive();
    let baseline_start = today - Duration::days(baseline_days as i64);

    let types_to_scan: Vec<String> = if let Some(t) = metric_type {
        vec![t.to_string()]
    } else {
        db.distinct_metric_types()?
    };

    let mut anomalies = Vec::new();
    let mut scanned_types = Vec::new();
    let mut clean_types = Vec::new();

    for metric in &types_to_scan {
        let entries = db.query_all(Some(metric), Some(baseline_start), Some(today))?;

        if entries.len() < MIN_DATA_POINTS {
            continue;
        }

        scanned_types.push(metric.clone());

        // Separate today's entries from baseline
        let baseline_values: Vec<f64> = entries
            .iter()
            .filter(|e| e.timestamp.date_naive() < today)
            .map(|e| e.value)
            .collect();

        if baseline_values.len() < MIN_DATA_POINTS {
            continue;
        }

        let baseline = compute_baseline(&baseline_values);
        let factor = threshold.factor();
        let lower = baseline.q1 - factor * baseline.iqr;
        let upper = baseline.q3 + factor * baseline.iqr;

        // Check today's entries against baseline
        let today_entries: Vec<_> = entries
            .iter()
            .filter(|e| e.timestamp.date_naive() == today)
            .collect();

        let mut found_anomaly = false;
        for entry in &today_entries {
            if entry.value < lower || entry.value > upper {
                found_anomaly = true;
                let deviation = if entry.value > upper {
                    "above"
                } else {
                    "below"
                };

                let severity = compute_severity(entry.value, &baseline, deviation);

                let summary = format!(
                    "{} {:.1} is {} your normal range ({:.1}-{:.1})",
                    metric, entry.value, deviation, lower, upper
                );

                anomalies.push(Anomaly {
                    metric_type: metric.clone(),
                    value: entry.value,
                    timestamp: entry.timestamp,
                    baseline: baseline.clone(),
                    bounds: Bounds { lower, upper },
                    deviation: deviation.to_string(),
                    severity,
                    summary,
                });
            }
        }

        if !found_anomaly {
            clean_types.push(metric.clone());
        }
    }

    let summary = if anomalies.is_empty() {
        if scanned_types.is_empty() {
            "No metrics with sufficient data for anomaly detection.".to_string()
        } else {
            format!(
                "No anomalies detected across {} metric type(s).",
                scanned_types.len()
            )
        }
    } else {
        let types: Vec<&str> = anomalies
            .iter()
            .map(|a| a.metric_type.as_str())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        format!(
            "{} anomal{} detected across {} metric type(s). Affected: {}.",
            anomalies.len(),
            if anomalies.len() == 1 { "y" } else { "ies" },
            scanned_types.len(),
            types.join(", ")
        )
    };

    Ok(AnomalyResult {
        period: AnomalyPeriod {
            baseline_start: baseline_start.to_string(),
            baseline_end: today.to_string(),
            days: baseline_days,
        },
        threshold,
        anomalies,
        scanned_types,
        clean_types,
        summary,
    })
}

/// Compute IQR-based baseline statistics.
fn compute_baseline(values: &[f64]) -> Baseline {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let median = percentile(&sorted, 50.0);
    let q1 = percentile(&sorted, 25.0);
    let q3 = percentile(&sorted, 75.0);
    let iqr = q3 - q1;

    Baseline {
        q1,
        median,
        q3,
        iqr,
    }
}

/// Compute percentile using linear interpolation.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let k = (p / 100.0) * (sorted.len() - 1) as f64;
    let f = k.floor() as usize;
    let c = k.ceil() as usize;
    if f == c {
        sorted[f]
    } else {
        sorted[f] + (k - f as f64) * (sorted[c] - sorted[f])
    }
}

/// Determine severity based on how far the value is from bounds.
fn compute_severity(value: f64, baseline: &Baseline, deviation: &str) -> Severity {
    let distance = if deviation == "above" {
        (value - baseline.q3) / baseline.iqr.max(0.01)
    } else {
        (baseline.q1 - value) / baseline.iqr.max(0.01)
    };

    if distance > 2.0 {
        Severity::Alert
    } else if distance > 1.5 {
        Severity::Warning
    } else {
        Severity::Info
    }
}
