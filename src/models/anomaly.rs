use chrono::{DateTime, Utc};
use serde::Serialize;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Alert,
}

#[derive(Debug, Clone, Copy)]
pub enum Threshold {
    Relaxed,
    Moderate,
    Strict,
}

impl Threshold {
    /// IQR multiplier for determining bounds.
    pub fn factor(self) -> f64 {
        match self {
            Self::Relaxed => 2.0,
            Self::Moderate => 1.5,
            Self::Strict => 1.0,
        }
    }
}

impl FromStr for Threshold {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "relaxed" => Ok(Self::Relaxed),
            "moderate" => Ok(Self::Moderate),
            "strict" => Ok(Self::Strict),
            _ => anyhow::bail!(
                "invalid threshold: {} (expected relaxed/moderate/strict)",
                s
            ),
        }
    }
}

impl std::fmt::Display for Threshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Relaxed => write!(f, "relaxed"),
            Self::Moderate => write!(f, "moderate"),
            Self::Strict => write!(f, "strict"),
        }
    }
}

impl Serialize for Threshold {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Baseline {
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub iqr: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Anomaly {
    pub metric_type: String,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
    pub baseline: Baseline,
    pub bounds: Bounds,
    pub deviation: String,
    pub severity: Severity,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bounds {
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Serialize)]
pub struct AnomalyResult {
    pub period: AnomalyPeriod,
    pub threshold: Threshold,
    pub anomalies: Vec<Anomaly>,
    pub scanned_types: Vec<String>,
    pub clean_types: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct AnomalyPeriod {
    pub baseline_start: String,
    pub baseline_end: String,
    pub days: u32,
}
