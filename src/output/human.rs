use crate::core::context::ContextResult;
use crate::core::med::MedStatus;
use crate::core::status::StatusData;
use crate::models::Metric;
use crate::models::anomaly::{AnomalyResult, Severity};
use crate::models::config::Units;
use crate::models::med::Medication;

/// Format a value with its unit, handling scale units like "0-10" → "7/10".
fn format_value_with_unit(val: f64, unit: &str) -> String {
    match unit {
        "0-10" | "1-10" => format!("{}/10", val),
        "1-5" => format!("{}/5", val),
        "" => format!("{}", val),
        u => format!("{} {}", val, u),
    }
}

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
    let value_display = format_value_with_unit(display_val, &display_unit);
    let mut line = format!("{} | {} = {}", ts, m.metric_type, value_display);
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

    // Medications
    if let Some(ref meds) = s.medications {
        out.push_str(&format!("\nMedications: {} active", meds.active_count));
        if !meds.missed.is_empty() {
            out.push_str(&format!(" | Missed: {}", meds.missed.join(", ")));
        }
        if let Some(adherence) = meds.overall_adherence_7d {
            out.push_str(&format!(" | 7d adherence: {:.0}%", adherence * 100.0));
        }
    }

    out
}

/// Format medication list for human display.
pub fn format_med_list(meds: &[Medication], include_stopped: bool) -> String {
    if meds.is_empty() {
        return "No medications found.".to_string();
    }

    let header = if include_stopped {
        "All Medications"
    } else {
        "Active Medications"
    };
    let separator = "=".repeat(header.len());
    let mut out = format!("{}\n{}\n", header, separator);
    for med in meds {
        let dose_str = med.dose.as_deref().unwrap_or("");
        let route_str = med.route.to_string();
        let freq_display = match med.frequency.to_string().as_str() {
            "daily" => "daily",
            "2x_daily" => "2x daily",
            "3x_daily" => "3x daily",
            "weekly" => "weekly",
            "as_needed" => "as needed",
            _ => "unknown",
        }
        .to_string();
        let since = med.started_at.format("%b %d");
        let note_part = med
            .note
            .as_ref()
            .map(|n| format!("  \"{}\"", n))
            .unwrap_or_default();
        let stopped_marker = if !med.active { " [STOPPED]" } else { "" };

        out.push_str(&format!(
            "  {:<14}{} {}  {:<11}since {}{}{}",
            med.name, dose_str, route_str, freq_display, since, note_part, stopped_marker,
        ));
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Format medication take confirmation.
pub fn format_med_take(name: &str, dose: &str, route: &str, timestamp: &str) -> String {
    format!(
        "Took {} {} ({})\n  Recorded at {}",
        name, dose, route, timestamp
    )
}

/// Format medication status overview.
pub fn format_med_status(statuses: &[MedStatus], date: chrono::NaiveDate) -> String {
    if statuses.is_empty() {
        return "No active medications.".to_string();
    }

    let header = format!("Medication Adherence \u{2014} {}", date.format("%b %d, %Y"));
    let separator = "=".repeat(header.len());
    let mut out = format!("{}\n{}\n", header, separator);

    for s in statuses {
        let taken_display = if let Some(req) = s.required_today {
            format!("{}/{} taken today", s.taken_today, req)
        } else {
            format!("{} taken today", s.taken_today)
        };

        let adherence_marker = if s.frequency == "as_needed" {
            "(as needed)".to_string()
        } else if let Some(true) = s.adherent_today {
            "OK".to_string()
        } else if let Some(false) = s.adherent_today {
            "MISSED".to_string()
        } else {
            String::new()
        };

        let streak_str = s
            .streak_days
            .map(|d| format!("streak: {} days", d))
            .unwrap_or_default();

        let adh_7d_str = s
            .adherence_7d
            .map(|a| format!("7d: {:.0}%", a * 100.0))
            .unwrap_or_default();

        let parts: Vec<&str> = [
            taken_display.as_str(),
            adherence_marker.as_str(),
            streak_str.as_str(),
            adh_7d_str.as_str(),
        ]
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect();

        out.push_str(&format!("  {:<14}{}\n", s.name, parts.join("    ")));
    }

    // Overall adherence (exclude as_needed)
    let adherence_values: Vec<f64> = statuses.iter().filter_map(|s| s.adherence_7d).collect();
    if !adherence_values.is_empty() {
        let overall = adherence_values.iter().sum::<f64>() / adherence_values.len() as f64;
        out.push_str(&format!(
            "\nOverall 7-day adherence: {:.0}%",
            overall * 100.0
        ));
    }

    out.trim_end().to_string()
}

/// Format medication stop.
pub fn format_med_stop(name: &str, reason: Option<&str>) -> String {
    match reason {
        Some(r) => format!("Stopped {}: {}", name, r),
        None => format!("Stopped {}", name),
    }
}

/// Format anomaly detection results for human display.
pub fn format_anomaly(result: &AnomalyResult) -> String {
    let mut out = format!(
        "=== Anomaly Scan ({} days, {} threshold) ===\n",
        result.period.days, result.threshold
    );

    if result.anomalies.is_empty() {
        out.push_str(&format!("\n{}", result.summary));
        return out;
    }

    for a in &result.anomalies {
        let severity_marker = match a.severity {
            Severity::Alert => "!!!",
            Severity::Warning => "!!",
            Severity::Info => "!",
        };
        out.push_str(&format!(
            "\n{} {} {:.1} (normal: {:.1}-{:.1}, {} baseline)",
            severity_marker, a.metric_type, a.value, a.bounds.lower, a.bounds.upper, a.deviation,
        ));
    }

    out.push_str(&format!("\n\n{}", result.summary));

    if !result.clean_types.is_empty() {
        out.push_str(&format!("\nNormal: {}", result.clean_types.join(", ")));
    }

    out
}

/// Format health context briefing for human display.
pub fn format_context(result: &ContextResult) -> String {
    let mut out = format!(
        "=== Health Context ({} days: {} to {}) ===\n",
        result.period.days, result.period.start, result.period.end
    );

    out.push_str(&format!("\n{}\n", result.summary));

    // Metrics
    if !result.metrics.is_empty() {
        out.push_str("\n--- Metrics ---\n");
        let mut sorted_keys: Vec<&String> = result.metrics.keys().collect();
        sorted_keys.sort();
        for key in sorted_keys {
            let m = &result.metrics[key];
            out.push_str(&format!("  {}: {}\n", key, m.summary));
        }
    }

    // Goals
    if !result.goals.is_empty() {
        out.push_str("\n--- Goals ---\n");
        for g in &result.goals {
            let status = if g.is_met { "MET" } else { "..." };
            out.push_str(&format!("  [{}] {}\n", status, g.summary));
        }
    }

    // Medications
    if let Some(ref meds) = result.medications {
        out.push_str(&format!("\n--- Medications ---\n  {}\n", meds.summary));
    }

    // Streaks
    if result.streaks.logging_days > 0 {
        out.push_str(&format!(
            "\n--- Streaks ---\n  Logging: {} day(s)\n",
            result.streaks.logging_days
        ));
    }

    // Alerts
    if !result.alerts.is_empty() {
        out.push_str("\n--- Alerts ---\n");
        for a in &result.alerts {
            out.push_str(&format!("  [{}] {}\n", a.alert_type, a.message));
        }
    }

    out.trim_end().to_string()
}
