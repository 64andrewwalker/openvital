use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Above,
    Below,
    Equal,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Above => write!(f, "above"),
            Self::Below => write!(f, "below"),
            Self::Equal => write!(f, "equal"),
        }
    }
}

impl FromStr for Direction {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "above" => Ok(Self::Above),
            "below" => Ok(Self::Below),
            "equal" => Ok(Self::Equal),
            _ => anyhow::bail!("invalid direction: {} (expected above/below/equal)", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Timeframe {
    Daily,
    Weekly,
    Monthly,
}

impl std::fmt::Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Daily => write!(f, "daily"),
            Self::Weekly => write!(f, "weekly"),
            Self::Monthly => write!(f, "monthly"),
        }
    }
}

impl FromStr for Timeframe {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            _ => anyhow::bail!("invalid timeframe: {} (expected daily/weekly/monthly)", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub metric_type: String,
    pub target_value: f64,
    pub direction: Direction,
    pub timeframe: Timeframe,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

impl Goal {
    pub fn new(
        metric_type: String,
        target_value: f64,
        direction: Direction,
        timeframe: Timeframe,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            metric_type,
            target_value,
            direction,
            timeframe,
            active: true,
            created_at: Utc::now(),
        }
    }

    /// Check if a value meets the goal target.
    pub fn is_met(&self, value: f64) -> bool {
        match self.direction {
            Direction::Above => value >= self.target_value,
            Direction::Below => value <= self.target_value,
            Direction::Equal => (value - self.target_value).abs() < f64::EPSILON,
        }
    }
}
