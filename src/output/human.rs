use crate::core::status::StatusData;
use crate::models::Metric;

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

/// Pretty-print the status overview.
pub fn format_status(s: &StatusData) -> String {
    let mut out = format!("=== OpenVital Status — {} ===\n\n", s.date);
    if let (Some(w), Some(b)) = (s.profile.latest_weight_kg, s.profile.bmi) {
        out.push_str(&format!(
            "Weight: {} kg | BMI: {} ({})\n",
            w,
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
