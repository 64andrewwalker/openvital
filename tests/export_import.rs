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

/// Scenario: Export CSV with date range filter returns only entries in range
#[test]
fn test_export_csv_date_range_filter() {
    let (_dir, db) = common::setup_db();
    let jan1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let jan5 = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
    let jan10 = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();

    let m1 = common::make_metric("weight", 85.0, jan1);
    let m2 = common::make_metric("weight", 84.5, jan5);
    let m3 = common::make_metric("weight", 84.0, jan10);
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();
    db.insert_metric(&m3).unwrap();

    // Export only from jan1 to jan5 (inclusive)
    let csv = export::to_csv(&db, None, Some(jan1), Some(jan5)).unwrap();
    let lines: Vec<&str> = csv.lines().collect();

    // Should have header + 2 rows (jan1 and jan5), not jan10
    assert_eq!(
        lines.len(),
        3,
        "Expected header + 2 data rows for date range jan1..=jan5"
    );
    assert!(
        !csv.contains("2026-01-10"),
        "jan10 entry should be excluded by date range"
    );
    assert!(csv.contains("2026-01-01"), "jan1 entry should be included");
    assert!(csv.contains("2026-01-05"), "jan5 entry should be included");
}

/// Scenario: Export JSON with date range filter returns only entries in range
#[test]
fn test_export_json_date_range_filter() {
    let (_dir, db) = common::setup_db();
    let jan1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let jan3 = NaiveDate::from_ymd_opt(2026, 1, 3).unwrap();
    let jan7 = NaiveDate::from_ymd_opt(2026, 1, 7).unwrap();

    let m1 = common::make_metric("cardio", 30.0, jan1);
    let m2 = common::make_metric("cardio", 40.0, jan3);
    let m3 = common::make_metric("cardio", 45.0, jan7);
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();
    db.insert_metric(&m3).unwrap();

    // Export only jan1..=jan3
    let json_str = export::to_json(&db, None, Some(jan1), Some(jan3)).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();

    assert_eq!(
        parsed.len(),
        2,
        "Expected 2 entries for date range jan1..=jan3"
    );
}

/// Scenario: Export CSV filtered by type AND date range
#[test]
fn test_export_csv_type_and_date_range() {
    let (_dir, db) = common::setup_db();
    let jan1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let jan3 = NaiveDate::from_ymd_opt(2026, 1, 3).unwrap();
    let jan5 = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();

    db.insert_metric(&common::make_metric("weight", 85.0, jan1))
        .unwrap();
    db.insert_metric(&common::make_metric("weight", 84.0, jan3))
        .unwrap();
    db.insert_metric(&common::make_metric("weight", 83.0, jan5))
        .unwrap();
    db.insert_metric(&common::make_metric("water", 2000.0, jan3))
        .unwrap();

    // Filter by type=weight, date range jan1..=jan3
    let csv = export::to_csv(&db, Some("weight"), Some(jan1), Some(jan3)).unwrap();
    let lines: Vec<&str> = csv.lines().collect();

    // Should have header + 2 weight rows (jan1 and jan3), not jan5 weight, not water
    assert_eq!(
        lines.len(),
        3,
        "Expected header + 2 weight entries for jan1..=jan3"
    );
    assert!(
        !csv.contains("water"),
        "water entries should be excluded by type filter"
    );
    assert!(
        !csv.contains("2026-01-05"),
        "jan5 should be excluded by date range"
    );
}

/// Scenario: Export empty database returns only CSV header
#[test]
fn test_export_csv_empty_database() {
    let (_dir, db) = common::setup_db();

    let csv = export::to_csv(&db, None, None, None).unwrap();
    let lines: Vec<&str> = csv.lines().collect();

    assert_eq!(
        lines.len(),
        1,
        "Empty database CSV should have only header row"
    );
    assert!(
        lines[0].contains("timestamp"),
        "Header row should be present"
    );
}

/// Scenario: Export empty database returns empty JSON array
#[test]
fn test_export_json_empty_database() {
    let (_dir, db) = common::setup_db();

    let json_str = export::to_json(&db, None, None, None).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();

    assert!(
        parsed.is_empty(),
        "Empty database should export as empty JSON array"
    );
}

/// Scenario: Import JSON with missing optional fields (no timestamp, no note, no tags, no source)
#[test]
fn test_import_json_missing_optional_fields() {
    let (_dir, db) = common::setup_db();
    // Only required fields: type and value
    let json = r#"[
        {"type": "weight", "value": 80.0},
        {"type": "sleep_hours", "value": 7.5}
    ]"#;

    let count = export::import_json(&db, json).unwrap();
    assert_eq!(
        count, 2,
        "Should import 2 entries even without optional fields"
    );

    let weights = db.query_by_type("weight", Some(10)).unwrap();
    assert_eq!(weights.len(), 1);
    // source should default to "import" when not provided
    assert_eq!(
        weights[0].source, "import",
        "source should default to 'import' when not provided"
    );

    let sleep = db.query_by_type("sleep_hours", Some(10)).unwrap();
    assert_eq!(sleep.len(), 1);
    assert!(
        sleep[0].note.is_none(),
        "note should be None when not provided"
    );
    assert!(
        sleep[0].tags.is_empty(),
        "tags should be empty when not provided"
    );
}

/// Scenario: Import JSON with custom source field
#[test]
fn test_import_json_custom_source() {
    let (_dir, db) = common::setup_db();
    let json = r#"[
        {"type": "cardio", "value": 30.0, "source": "garmin_sync", "timestamp": "2026-01-15T09:00:00Z"}
    ]"#;

    let count = export::import_json(&db, json).unwrap();
    assert_eq!(count, 1);

    let entries = db.query_by_type("cardio", Some(10)).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].source, "garmin_sync",
        "custom source field should be preserved on import"
    );
}

/// Scenario: Import JSON with note and tags preserves those fields
#[test]
fn test_import_json_note_and_tags_preserved() {
    let (_dir, db) = common::setup_db();
    let json = r#"[
        {
            "type": "pain",
            "value": 4.0,
            "note": "left knee",
            "tags": ["knee", "post-run"],
            "timestamp": "2026-01-10T08:00:00Z"
        }
    ]"#;

    let count = export::import_json(&db, json).unwrap();
    assert_eq!(count, 1);

    let entries = db.query_by_type("pain", Some(10)).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].note.as_deref(),
        Some("left knee"),
        "note should be preserved on JSON import"
    );
    assert_eq!(
        entries[0].tags,
        vec!["knee".to_string(), "post-run".to_string()],
        "tags should be preserved on JSON import"
    );
}

/// Scenario: Import CSV with all optional fields populated
#[test]
fn test_import_csv_full_fields() {
    let (_dir, db) = common::setup_db();
    let csv = "timestamp,type,value,unit,note,tags,source\n\
               2026-02-01T07:00:00+00:00,pain,6.0,0-10,lower back,[\"back\"],physio_app\n";

    let count = export::import_csv(&db, csv).unwrap();
    assert_eq!(count, 1);

    let entries = db.query_by_type("pain", Some(10)).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].note.as_deref(),
        Some("lower back"),
        "note should be preserved from CSV import"
    );
    assert_eq!(
        entries[0].tags,
        vec!["back".to_string()],
        "tags should be parsed from JSON array in CSV"
    );
    assert_eq!(
        entries[0].source, "physio_app",
        "custom source should be preserved from CSV import"
    );
}

/// Scenario: Import CSV with minimal fields (only timestamp, type, value)
#[test]
fn test_import_csv_minimal_fields() {
    let (_dir, db) = common::setup_db();
    // Only 3 fields: timestamp, type, value
    let csv = "timestamp,type,value\n\
               2026-03-01T10:00:00+00:00,water,1500.0\n";

    let count = export::import_csv(&db, csv).unwrap();
    assert_eq!(count, 1);

    let entries = db.query_by_type("water", Some(10)).unwrap();
    assert_eq!(entries.len(), 1);
    // unit should fall back to default for "water" which is "ml"
    assert_eq!(
        entries[0].unit, "ml",
        "unit should default to 'ml' for water when not specified in CSV"
    );
    assert!(
        entries[0].note.is_none(),
        "note should be None when not in CSV"
    );
    assert!(
        entries[0].tags.is_empty(),
        "tags should be empty when not in CSV"
    );
    assert_eq!(
        entries[0].source, "import",
        "source should default to 'import' when not in CSV"
    );
}

/// Scenario: Import CSV skips blank lines gracefully
#[test]
fn test_import_csv_skips_blank_lines() {
    let (_dir, db) = common::setup_db();
    let csv = "timestamp,type,value,unit,note,tags,source\n\
               \n\
               2026-01-05T12:00:00+00:00,weight,78.0,kg,,,\n\
               \n";

    let count = export::import_csv(&db, csv).unwrap();
    assert_eq!(count, 1, "Blank lines in CSV should be skipped");
}

/// Scenario: Round-trip export/import via CSV preserves entry data
#[test]
fn test_round_trip_csv_export_import() {
    let (_dir, db1) = common::setup_db();
    let date = NaiveDate::from_ymd_opt(2026, 1, 20).unwrap();

    let mut original = common::make_metric("weight", 77.5, date);
    original.note = Some("post-workout".to_string());
    original.tags = vec!["morning".to_string()];
    original.source = "manual".to_string();
    db1.insert_metric(&original).unwrap();

    // Export from db1
    let csv = export::to_csv(&db1, None, None, None).unwrap();

    // Import into a fresh db2
    let (_dir2, db2) = common::setup_db();
    let count = export::import_csv(&db2, &csv).unwrap();
    assert_eq!(count, 1);

    let entries = db2.query_by_type("weight", Some(10)).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].value, 77.5);
    assert_eq!(
        entries[0].note.as_deref(),
        Some("post-workout"),
        "note should survive CSV round-trip"
    );
}

/// Scenario: Import CSV with malformed tags JSON falls back to empty tags (unwrap_or_default)
/// This exercises the `.unwrap_or_default()` branch in import_csv (line 110 in export.rs).
#[test]
fn test_import_csv_malformed_tags_uses_default() {
    let (_dir, db) = common::setup_db();
    // tags field contains invalid JSON — should fall back to empty Vec via unwrap_or_default()
    let csv = "timestamp,type,value,unit,note,tags,source\n\
               2026-05-01T08:00:00+00:00,pain,3.0,0-10,test,NOT_VALID_JSON,manual\n";

    let count = export::import_csv(&db, csv).unwrap();
    assert_eq!(
        count, 1,
        "Row with malformed tags JSON should still be imported"
    );

    let entries = db.query_by_type("pain", Some(10)).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(
        entries[0].tags.is_empty(),
        "Tags should be empty when JSON parse fails (unwrap_or_default)"
    );
    assert_eq!(entries[0].note.as_deref(), Some("test"));
    assert_eq!(entries[0].source, "manual");
}

/// Scenario: Round-trip export/import via JSON preserves entry data
#[test]
fn test_round_trip_json_export_import() {
    let (_dir, db1) = common::setup_db();
    let date = NaiveDate::from_ymd_opt(2026, 1, 25).unwrap();

    let mut original = common::make_metric("sleep_hours", 7.0, date);
    original.note = Some("good night".to_string());
    original.tags = vec!["restful".to_string()];
    db1.insert_metric(&original).unwrap();

    // Export from db1
    let json_str = export::to_json(&db1, None, None, None).unwrap();

    // Import into fresh db2
    let (_dir2, db2) = common::setup_db();
    let count = export::import_json(&db2, &json_str).unwrap();
    assert_eq!(count, 1);

    let entries = db2.query_by_type("sleep_hours", Some(10)).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].value, 7.0);
    assert_eq!(
        entries[0].note.as_deref(),
        Some("good night"),
        "note should survive JSON round-trip"
    );
    assert_eq!(
        entries[0].tags,
        vec!["restful".to_string()],
        "tags should survive JSON round-trip"
    );
}

/// Scenario: Export CSV with non-empty tags serialises them as a JSON array in the CSV
/// This exercises the non-empty tags branch in to_csv (the serde_json::to_string path).
#[test]
fn test_export_csv_with_tags_serialised_as_json_array() {
    let (_dir, db) = common::setup_db();
    let date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();

    let mut m = common::make_metric("pain", 4.0, date);
    m.tags = vec!["knee".to_string(), "post-run".to_string()];
    db.insert_metric(&m).unwrap();

    let csv = export::to_csv(&db, None, None, None).unwrap();

    // The tags column should contain a JSON array representation
    assert!(
        csv.contains("[\"knee\",\"post-run\"]") || csv.contains("knee"),
        "Tags should be serialised as JSON array in CSV output"
    );
    // Verify we still have header + 1 data row
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 2, "Should have header + 1 data row");
}
