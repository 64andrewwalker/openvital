use serde_json::{json, Value};

/// Standard JSON envelope per spec section 5.3.
pub fn success(command: &str, data: Value) -> Value {
    json!({
        "status": "ok",
        "command": command,
        "data": data,
        "error": null
    })
}

pub fn error(command: &str, code: &str, message: &str) -> Value {
    json!({
        "status": "error",
        "command": command,
        "data": null,
        "error": {
            "code": code,
            "message": message
        }
    })
}

/// Pretty-print a metric entry for --human output.
pub fn human_metric(m: &crate::models::Metric) -> String {
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
