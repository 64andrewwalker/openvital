use crate::models::config::Units;
use crate::models::metric::default_unit;

const KG_TO_LBS: f64 = 2.20462;
const CM_TO_IN: f64 = 2.54;
const CM_TO_FT: f64 = 30.48;
const ML_TO_FLOZ: f64 = 29.5735;

/// Convert a stored (metric) value to display value + display unit string.
pub fn to_display(value: f64, metric_type: &str, units: &Units) -> (f64, String) {
    if !units.is_imperial() {
        return (value, default_unit(metric_type).to_string());
    }

    match metric_type {
        "weight" => (round1(value * KG_TO_LBS), "lbs".to_string()),
        "waist" => (round1(value / CM_TO_IN), "in".to_string()),
        "height" => (round1(value / CM_TO_FT), "ft".to_string()),
        "water" => (round1(value / ML_TO_FLOZ), "fl oz".to_string()),
        "temperature" => (round1(value * 1.8 + 32.0), "\u{00b0}F".to_string()),
        _ => (value, default_unit(metric_type).to_string()),
    }
}

/// Return the display unit string for a metric in the active unit system.
pub fn display_unit(metric_type: &str, units: &Units) -> String {
    to_display(0.0, metric_type, units).1
}

/// Convert a metric-space change/rate to display-space rate.
pub fn to_display_rate(rate: f64, metric_type: &str, units: &Units) -> f64 {
    if !units.is_imperial() {
        return rate;
    }

    match metric_type {
        "weight" => round1(rate * KG_TO_LBS),
        "waist" => round1(rate / CM_TO_IN),
        "height" => round1(rate / CM_TO_FT),
        "water" => round1(rate / ML_TO_FLOZ),
        "temperature" => round1(rate * 1.8),
        _ => rate,
    }
}

/// Convert a user-input value (in their configured unit system) to metric for storage.
pub fn from_input(value: f64, metric_type: &str, units: &Units) -> f64 {
    if !units.is_imperial() {
        return value;
    }

    match metric_type {
        "weight" => value / KG_TO_LBS,
        "waist" => value * CM_TO_IN,
        "height" => value * CM_TO_FT,
        "water" => value * ML_TO_FLOZ,
        "temperature" => (value - 32.0) / 1.8,
        _ => value,
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
