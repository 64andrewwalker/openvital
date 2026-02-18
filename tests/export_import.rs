use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use openvital::core::export;
use openvital::db::Database;
use openvital::models::metric::Metric;
use tempfile::TempDir;

fn setup_db() -> (TempDir, Database) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    (dir, db)
}

fn make_metric(metric_type: &str, value: f64, date: NaiveDate) -> Metric {
    let dt = date.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    let ts = Utc.from_utc_datetime(&dt);
    let mut m = Metric::new(metric_type.to_string(), value);
    m.timestamp = ts;
    m
}

/// Scenario: Export to CSV includes all entries
///   Given 3 metric entries in the database
///   When I export to CSV format
///   Then the CSV has a header row and 3 data rows
#[test]
fn test_export_csv() {
    let (_dir, db) = setup_db();
    let m1 = make_metric("weight", 85.0, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let m2 = make_metric("weight", 84.5, NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
    let m3 = make_metric(
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
///   Given 2 metric entries
///   When I export to JSON format
///   Then the result is a valid JSON array with 2 elements
#[test]
fn test_export_json() {
    let (_dir, db) = setup_db();
    let m1 = make_metric("weight", 85.0, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let m2 = make_metric("cardio", 45.0, NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();

    let json_str = export::to_json(&db, None, None, None).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.len(), 2);
}

/// Scenario: Export filtered by type
///   Given weight and water entries
///   When I export CSV filtered to "weight"
///   Then only weight entries appear
#[test]
fn test_export_filter_by_type() {
    let (_dir, db) = setup_db();
    let m1 = make_metric("weight", 85.0, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let m2 = make_metric(
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
///   Given a JSON string with 2 metric entries
///   When I import from JSON
///   Then 2 entries are inserted into the database
#[test]
fn test_import_json() {
    let (_dir, db) = setup_db();
    let json = r#"[
        {"type": "weight", "value": 85.0, "timestamp": "2026-01-01T12:00:00Z"},
        {"type": "cardio", "value": 45.0, "timestamp": "2026-01-02T12:00:00Z"}
    ]"#;

    let count = export::import_json(&db, json).unwrap();
    assert_eq!(count, 2);

    // Verify they're in the DB
    let weights = db.query_by_type("weight", Some(10)).unwrap();
    assert_eq!(weights.len(), 1);
}

/// Scenario: Import from CSV
///   Given a CSV string with header and 2 data rows
///   When I import from CSV
///   Then 2 entries are inserted into the database
#[test]
fn test_import_csv() {
    let (_dir, db) = setup_db();
    let csv = "timestamp,type,value,unit,note,tags,source\n\
               2026-01-01T12:00:00+00:00,weight,85.0,kg,,,[]\n\
               2026-01-02T12:00:00+00:00,cardio,45.0,min,,,[]\n";

    let count = export::import_csv(&db, csv).unwrap();
    assert_eq!(count, 2);
}
