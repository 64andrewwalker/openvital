mod common;

use chrono::NaiveDate;
use openvital::core::export;

/// Scenario: Export to CSV includes all entries
#[test]
fn test_export_csv() {
    let (_dir, db) = common::setup_db();
    let m1 = common::make_metric("weight", 85.0, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let m2 = common::make_metric("weight", 84.5, NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
    let m3 = common::make_metric(
        "water",
        2000.0,
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
    );
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();
    db.insert_metric(&m3).unwrap();

    let csv = export::to_csv(&db, None, None, None).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 4); // header + 3 rows
    assert!(lines[0].contains("timestamp"));
    assert!(lines[0].contains("type"));
    assert!(lines[0].contains("value"));
}

/// Scenario: Export to JSON produces valid array
#[test]
fn test_export_json() {
    let (_dir, db) = common::setup_db();
    let m1 = common::make_metric("weight", 85.0, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let m2 = common::make_metric("cardio", 45.0, NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();

    let json_str = export::to_json(&db, None, None, None).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.len(), 2);
}

/// Scenario: Export filtered by type
#[test]
fn test_export_filter_by_type() {
    let (_dir, db) = common::setup_db();
    let m1 = common::make_metric("weight", 85.0, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let m2 = common::make_metric(
        "water",
        2000.0,
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
    );
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();

    let csv = export::to_csv(&db, Some("weight"), None, None).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 2); // header + 1 weight row
    assert!(!csv.contains("water"));
}

/// Scenario: Import from JSON
#[test]
fn test_import_json() {
    let (_dir, db) = common::setup_db();
    let json = r#"[
        {"type": "weight", "value": 85.0, "timestamp": "2026-01-01T12:00:00Z"},
        {"type": "cardio", "value": 45.0, "timestamp": "2026-01-02T12:00:00Z"}
    ]"#;

    let count = export::import_json(&db, json).unwrap();
    assert_eq!(count, 2);

    let weights = db.query_by_type("weight", Some(10)).unwrap();
    assert_eq!(weights.len(), 1);
}

/// Scenario: Import from CSV
#[test]
fn test_import_csv() {
    let (_dir, db) = common::setup_db();
    let csv = "timestamp,type,value,unit,note,tags,source\n\
               2026-01-01T12:00:00+00:00,weight,85.0,kg,,,[]\n\
               2026-01-02T12:00:00+00:00,cardio,45.0,min,,,[]\n";

    let count = export::import_csv(&db, csv).unwrap();
    assert_eq!(count, 2);
}
