use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use std::collections::BTreeMap;
use std::str::FromStr;

use crate::db::Database;

#[derive(Debug, Clone, PartialEq)]
pub enum TrendPeriod {
    Daily,
    Weekly,
    Monthly,
}

impl FromStr for TrendPeriod {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            _ => anyhow::bail!("invalid period: {} (expected daily/weekly/monthly)", s),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TrendResult {
    #[serde(rename = "type")]
    pub metric_type: String,
    pub period: String,
    pub data: Vec<PeriodData>,
    pub trend: TrendSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodData {
    pub label: String,
    pub avg: f64,
    pub min: f64,
    pub max: f64,
    pub count: u32,
}

#[derive(Debug, Serialize)]
pub struct TrendSummary {
    pub direction: String,
    pub rate: f64,
    pub rate_unit: String,
    pub projected_30d: Option<f64>,
}

/// Compute trend data for a metric type.
pub fn compute(
    db: &Database,
    metric_type: &str,
    period: TrendPeriod,
    last: Option<u32>,
) -> Result<TrendResult> {
    // Fetch all entries in ascending order for bucketing
    let entries = db.query_by_type_asc(metric_type, None)?;

    let limit = last.unwrap_or(12) as usize;

    if entries.is_empty() {
        return Ok(TrendResult {
            metric_type: metric_type.to_string(),
            period: period_label(&period),
            data: Vec::new(),
            trend: TrendSummary {
                direction: "stable".to_string(),
                rate: 0.0,
                rate_unit: format!("per {}", period_label(&period)),
                projected_30d: None,
            },
        });
    }

    // Group entries by period bucket
    let mut buckets: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for entry in &entries {
        let date = entry.timestamp.date_naive();
        let key = period_key(date, &period);
        buckets.entry(key).or_default().push(entry.value);
    }

    // Convert to PeriodData, sorted by label, limited
    let mut data: Vec<PeriodData> = buckets
        .into_iter()
        .map(|(label, values)| {
            let count = values.len() as u32;
            let sum: f64 = values.iter().sum();
            let avg = sum / values.len() as f64;
            let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            PeriodData {
                label,
                avg,
                min,
                max,
                count,
            }
        })
        .collect();

    // Keep only last N periods
    if data.len() > limit {
        let start = data.len() - limit;
        data = data[start..].to_vec();
    }

    // Compute trend (linear regression on period averages)
    let trend = compute_trend(&data, &period);

    Ok(TrendResult {
        metric_type: metric_type.to_string(),
        period: period_label(&period),
        data,
        trend,
    })
}

fn period_key(date: NaiveDate, period: &TrendPeriod) -> String {
    match period {
        TrendPeriod::Daily => date.format("%Y-%m-%d").to_string(),
        TrendPeriod::Weekly => {
            let iso = date.iso_week();
            format!("{}-W{:02}", iso.year(), iso.week())
        }
        TrendPeriod::Monthly => date.format("%Y-%m").to_string(),
    }
}

fn period_label(period: &TrendPeriod) -> String {
    match period {
        TrendPeriod::Daily => "daily".to_string(),
        TrendPeriod::Weekly => "weekly".to_string(),
        TrendPeriod::Monthly => "monthly".to_string(),
    }
}

fn compute_trend(data: &[PeriodData], period: &TrendPeriod) -> TrendSummary {
    if data.len() < 2 {
        let last_val = data.first().map(|d| d.avg);
        return TrendSummary {
            direction: "stable".to_string(),
            rate: 0.0,
            rate_unit: format!("per {}", period_label(period)),
            projected_30d: last_val,
        };
    }

    // Simple linear regression: y = slope * x + intercept
    let n = data.len() as f64;
    let xs: Vec<f64> = (0..data.len()).map(|i| i as f64).collect();
    let ys: Vec<f64> = data.iter().map(|d| d.avg).collect();

    let sum_x: f64 = xs.iter().sum();
    let sum_y: f64 = ys.iter().sum();
    let sum_xy: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| x * y).sum();
    let sum_xx: f64 = xs.iter().map(|x| x * x).sum();

    let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);

    let direction = if slope < -0.01 {
        "decreasing"
    } else if slope > 0.01 {
        "increasing"
    } else {
        "stable"
    };

    // Round to 1 decimal
    let rate = (slope * 10.0).round() / 10.0;

    // Project 30 days out
    let periods_in_30d = match period {
        TrendPeriod::Daily => 30.0,
        TrendPeriod::Weekly => 30.0 / 7.0,
        TrendPeriod::Monthly => 1.0,
    };
    let last_avg = ys.last().unwrap();
    let projected = (last_avg + slope * periods_in_30d * 10.0).round() / 10.0;

    TrendSummary {
        direction: direction.to_string(),
        rate,
        rate_unit: format!("per {}", period_label(period)),
        projected_30d: Some(projected),
    }
}
