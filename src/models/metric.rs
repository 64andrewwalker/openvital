use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Body,
    Exercise,
    Sleep,
    Nutrition,
    Pain,
    Habit,
    Medication,
    Custom,
}

impl Category {
    pub fn from_type(metric_type: &str) -> Self {
        match metric_type {
            "weight" | "body_fat" | "waist" => Self::Body,
            "cardio" | "strength" | "calories_burned" => Self::Exercise,
            "sleep_hours" | "sleep_quality" | "bed_time" | "wake_time" => Self::Sleep,
            "calories" | "calories_in" | "calories_out" | "water" => Self::Nutrition,
            "pain" | "soreness" => Self::Pain,
            "standing_breaks" | "screen_time" => Self::Habit,
            _ => Self::Custom,
        }
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Body => write!(f, "body"),
            Self::Exercise => write!(f, "exercise"),
            Self::Sleep => write!(f, "sleep"),
            Self::Nutrition => write!(f, "nutrition"),
            Self::Pain => write!(f, "pain"),
            Self::Habit => write!(f, "habit"),
            Self::Medication => write!(f, "medication"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// Default unit for a known metric type.
pub fn default_unit(metric_type: &str) -> &str {
    match metric_type {
        "weight" => "kg",
        "body_fat" => "%",
        "waist" => "cm",
        "cardio" | "strength" => "min",
        "calories" | "calories_out" | "calories_burned" | "calories_in" => "kcal",
        "sleep_hours" => "hours",
        "sleep_quality" => "1-5",
        "bed_time" | "wake_time" => "HH:MM",
        "water" => "ml",
        "sleep" => "hours",
        "steps" => "steps",
        "mood" => "1-10",
        "heart_rate" => "bpm",
        "bp_systolic" | "bp_diastolic" => "mmHg",
        "pain" => "0-10",
        "soreness" => "0-10",
        "standing_breaks" => "count",
        "screen_time" => "hours",
        _ => "",
    }
}

/// Whether a metric type is cumulative (sum values) vs snapshot (use latest).
pub fn is_cumulative(metric_type: &str) -> bool {
    matches!(
        metric_type,
        "water" | "steps" | "calories_in" | "calories_burned" | "standing_breaks"
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub category: Category,
    #[serde(rename = "type")]
    pub metric_type: String,
    pub value: f64,
    pub unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub source: String,
}

impl Metric {
    pub fn new(metric_type: String, value: f64) -> Self {
        let category = Category::from_type(&metric_type);
        let unit = default_unit(&metric_type).to_string();
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            category,
            metric_type,
            value,
            unit,
            note: None,
            tags: Vec::new(),
            source: "manual".to_string(),
        }
    }
}
