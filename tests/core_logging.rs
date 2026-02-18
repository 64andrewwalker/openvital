mod common;

use chrono::{NaiveDate, Timelike};
use openvital::core::logging::{LogEntry, log_batch, log_metric};
use openvital::models::config::Config;

fn default_config() -> Config {
    Config::default()
}

// ── log_metric – basic fields ────────────────────────────────────────────────

#[test]
fn test_log_metric_basic_fields_persisted() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let entry = LogEntry {
        metric_type: "weight",
        value: 82.5,
        note: None,
        tags: None,
        source: None,
        date: None,
    };

    let m = log_metric(&db, &config, entry).unwrap();

    assert_eq!(m.metric_type, "weight");
    assert!((m.value - 82.5).abs() < f64::EPSILON);
    assert_eq!(m.unit, "kg");
    assert_eq!(m.source, "manual");
    assert!(m.note.is_none());
    assert!(m.tags.is_empty());

    // Verify persisted to DB
    let stored = db.query_by_type("weight", Some(1)).unwrap();
    assert_eq!(stored.len(), 1);
    assert!((stored[0].value - 82.5).abs() < f64::EPSILON);
}

#[test]
fn test_log_metric_note_stored() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let entry = LogEntry {
        metric_type: "pain",
        value: 3.0,
        note: Some("lower back"),
        tags: None,
        source: None,
        date: None,
    };

    let m = log_metric(&db, &config, entry).unwrap();
    assert_eq!(m.note.as_deref(), Some("lower back"));
}

#[test]
fn test_log_metric_tags_split_on_comma() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let entry = LogEntry {
        metric_type: "cardio",
        value: 45.0,
        note: None,
        tags: Some("morning, outdoor, run"),
        source: None,
        date: None,
    };

    let m = log_metric(&db, &config, entry).unwrap();
    assert_eq!(m.tags.len(), 3);
    assert_eq!(m.tags[0], "morning");
    assert_eq!(m.tags[1], "outdoor");
    assert_eq!(m.tags[2], "run");
}

#[test]
fn test_log_metric_source_overridden() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let entry = LogEntry {
        metric_type: "sleep_hours",
        value: 7.5,
        note: None,
        tags: None,
        source: Some("apple_health"),
        date: None,
    };

    let m = log_metric(&db, &config, entry).unwrap();
    assert_eq!(m.source, "apple_health");
}

#[test]
fn test_log_metric_custom_date_sets_noon_utc() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let entry = LogEntry {
        metric_type: "weight",
        value: 80.0,
        note: None,
        tags: None,
        source: None,
        date: Some(date),
    };

    let m = log_metric(&db, &config, entry).unwrap();

    assert_eq!(m.timestamp.date_naive(), date);
    assert_eq!(m.timestamp.time().hour(), 12);
}

#[test]
fn test_log_metric_no_date_defaults_to_now() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let entry = LogEntry {
        metric_type: "water",
        value: 500.0,
        note: None,
        tags: None,
        source: None,
        date: None,
    };

    let before = chrono::Utc::now();
    let m = log_metric(&db, &config, entry).unwrap();
    let after = chrono::Utc::now();

    assert!(m.timestamp >= before);
    assert!(m.timestamp <= after);
}

// ── log_metric – alias resolution ────────────────────────────────────────────

#[test]
fn test_log_metric_resolves_alias() {
    let (_dir, db) = common::setup_db();
    let mut config = default_config();
    config.aliases = Config::default_aliases();

    let entry = LogEntry {
        metric_type: "w", // alias for "weight"
        value: 79.0,
        note: None,
        tags: None,
        source: None,
        date: None,
    };

    let m = log_metric(&db, &config, entry).unwrap();
    assert_eq!(m.metric_type, "weight");

    // Stored under the resolved type
    let stored = db.query_by_type("weight", Some(1)).unwrap();
    assert_eq!(stored.len(), 1);
}

#[test]
fn test_log_metric_unknown_alias_passes_through() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let entry = LogEntry {
        metric_type: "custom_metric",
        value: 42.0,
        note: None,
        tags: None,
        source: None,
        date: None,
    };

    let m = log_metric(&db, &config, entry).unwrap();
    assert_eq!(m.metric_type, "custom_metric");
}

// ── log_metric – multiple entries for the same type ─────────────────────────

#[test]
fn test_log_metric_multiple_entries_accumulate() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    for v in [500.0, 600.0, 700.0] {
        let entry = LogEntry {
            metric_type: "water",
            value: v,
            note: None,
            tags: None,
            source: None,
            date: None,
        };
        log_metric(&db, &config, entry).unwrap();
    }

    let stored = db.query_by_type("water", Some(10)).unwrap();
    assert_eq!(stored.len(), 3);
}

// ── log_batch ────────────────────────────────────────────────────────────────

#[test]
fn test_log_batch_basic() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let json = r#"[
        {"type": "weight", "value": 80.5},
        {"type": "water",  "value": 600.0},
        {"type": "pain",   "value": 2.0}
    ]"#;

    let results = log_batch(&db, &config, json).unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].metric_type, "weight");
    assert_eq!(results[1].metric_type, "water");
    assert_eq!(results[2].metric_type, "pain");
}

#[test]
fn test_log_batch_with_note() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let json = r#"[{"type": "pain", "value": 4.0, "note": "knee"}]"#;

    let results = log_batch(&db, &config, json).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].note.as_deref(), Some("knee"));
}

#[test]
fn test_log_batch_with_tags_array() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let json = r#"[{"type": "cardio", "value": 30.0, "tags": ["morning", "run"]}]"#;

    let results = log_batch(&db, &config, json).unwrap();
    assert_eq!(results[0].tags, vec!["morning", "run"]);
}

#[test]
fn test_log_batch_resolves_aliases() {
    let (_dir, db) = common::setup_db();
    let mut config = default_config();
    config.aliases = Config::default_aliases();

    let json = r#"[{"type": "w", "value": 78.0}]"#;

    let results = log_batch(&db, &config, json).unwrap();
    assert_eq!(results[0].metric_type, "weight");
}

#[test]
fn test_log_batch_all_persisted_to_db() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let json = r#"[
        {"type": "sleep_hours", "value": 7.0},
        {"type": "sleep_hours", "value": 8.0}
    ]"#;

    log_batch(&db, &config, json).unwrap();

    let stored = db.query_by_type("sleep_hours", Some(10)).unwrap();
    assert_eq!(stored.len(), 2);
}

#[test]
fn test_log_batch_error_on_missing_type_field() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let json = r#"[{"value": 80.0}]"#;

    let result = log_batch(&db, &config, json);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("missing 'type'"));
}

#[test]
fn test_log_batch_error_on_missing_value_field() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let json = r#"[{"type": "weight"}]"#;

    let result = log_batch(&db, &config, json);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("missing 'value'"));
}

#[test]
fn test_log_batch_error_on_invalid_json() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let result = log_batch(&db, &config, "not json");
    assert!(result.is_err());
}

#[test]
fn test_log_batch_empty_array_succeeds() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let results = log_batch(&db, &config, "[]").unwrap();
    assert!(results.is_empty());
}
