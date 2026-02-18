use crate::core::status::StatusData;
use crate::models::Metric;
use crate::models::config::Units;

/// Pretty-print a single metric entry.
pub fn format_metric(m: &Metric) -> String {
    let ts = m.timestamp.format("%Y-%m-%d %H:%M");
    let mut line = format!("{} | {} = {} {}", ts, m.metric_type, m.value, m.unit);
    if let Some(ref note) = m.note {
        line.push_str(&format!("  # {}", note));
    }
    if !m.tags.is_empty() {
        line.push_str(&format!("  [{}]", m.tags.join(", ")));
    }
    line
}

/// Pretty-print a single metric entry, converting to user's preferred unit system.
pub fn format_metric_with_units(m: &Metric, user_units: &Units) -> String {
    let ts = m.timestamp.format("%Y-%m-%d %H:%M");
    let (display_val, display_unit) =
        crate::core::units::to_display(m.value, &m.metric_type, user_units);
    let mut line = format!(
        "{} | {} = {} {}",
        ts, m.metric_type, display_val, display_unit
    );
    if let Some(ref note) = m.note {
        line.push_str(&format!("  # {}", note));
    }
    if !m.tags.is_empty() {
        line.push_str(&format!("  [{}]", m.tags.join(", ")));
    }
    line
}

/// Format goal progress for human-readable output with unit conversion.
pub fn format_progress_human(status: &crate::core::goal::GoalStatus, units: &Units) -> String {
    let Some(current_raw) = status.current_value else {
        return "no data".to_string();
    };

    let (current, unit) = crate::core::units::to_display(current_raw, &status.metric_type, units);
    let (target, _) =
        crate::core::units::to_display(status.target_value, &status.metric_type, units);

    match status.direction.as_str() {
        "below" => {
            if current_raw <= status.target_value {
                format!("at target ({:.1} <= {:.1} {})", current, target, unit)
            } else {
                format!(
                    "{:.1} to go ({:.1} -> {:.1} {})",
                    current - target,
                    current,
                    target,
                    unit
                )
            }
        }
        "above" => {
            if current_raw >= status.target_value {
                format!("target met ({:.1} >= {:.1} {})", current, target, unit)
            } else {
                format!(
                    "{:.1} remaining ({:.1}/{:.1} {})",
                    target - current,
                    current,
                    target,
                    unit
                )
            }
        }
        "equal" => {
            if (current_raw - status.target_value).abs() < 0.01 {
                format!("at target ({:.1} {})", current, unit)
            } else {
                format!(
                    "current: {:.1} {}, target: {:.1} {}",
                    current, unit, target, unit
                )
            }
        }
        _ => status
            .progress
            .clone()
            .unwrap_or_else(|| "no data".to_string()),
    }
}

/// Pretty-print the status overview.
pub fn format_status(s: &StatusData, user_units: &Units) -> String {
    let mut out = format!("=== OpenVital Status — {} ===\n\n", s.date);
    if let (Some(w), Some(b)) = (s.profile.latest_weight_kg, s.profile.bmi) {
        let (display_w, display_wu) = crate::core::units::to_display(w, "weight", user_units);
        out.push_str(&format!(
            "Weight: {} {} | BMI: {} ({})\n",
            display_w,
            display_wu,
            b,
            s.profile.bmi_category.unwrap_or("?")
        ));
    }
    if s.today.logged.is_empty() {
        out.push_str("No entries logged today.");
    } else {
        // Deduplicate: count occurrences, preserve insertion order
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for t in &s.today.logged {
            if let Some(entry) = counts.iter_mut().find(|(name, _)| *name == t.as_str()) {
                entry.1 += 1;
            } else {
                counts.push((t.as_str(), 1));
            }
        }
        let parts: Vec<String> = counts
            .iter()
            .map(|(name, count)| format!("{}({})", name, count))
            .collect();
        out.push_str(&format!("Logged today: {}", parts.join(", ")));
    }
    if !s.today.pain_alerts.is_empty() {
        out.push_str(&format!(
            "\nPain alerts: {} active",
            s.today.pain_alerts.len()
        ));
    }

    // Streaks
    if s.streaks.logging_days > 0 {
        out.push_str(&format!(
            "\nLogging streak: {} day(s)",
            s.streaks.logging_days
        ));
    }

    // Consecutive pain alerts
    for alert in &s.consecutive_pain_alerts {
        out.push_str(&format!(
            "\n!! {} above threshold for {} consecutive days (latest: {})",
            alert.metric_type, alert.consecutive_days, alert.latest_value
        ));
    }

    out
}
