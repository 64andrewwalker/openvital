use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use openvital::core::report;
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

/// Scenario: Generate a weekly report with multiple metric types
///   Given weight entries on 2026-01-05..2026-01-10
///   And water entries on 2026-01-06..2026-01-09
///   When I generate a report from 2026-01-05 to 2026-01-11
///   Then the report includes summaries for both weight and water
///   And each metric has count, avg, min, max
#[test]
fn test_report_weekly_multiple_types() {
    let (_dir, db) = setup_db();
    // Weight entries
    for (day, val) in [
        (5, 85.0),
        (6, 84.8),
        (7, 84.5),
        (8, 84.6),
        (9, 84.3),
        (10, 84.0),
    ] {
        let m = make_metric(
            "weight",
            val,
            NaiveDate::from_ymd_opt(2026, 1, day).unwrap(),
        );
        db.insert_metric(&m).unwrap();
    }
    // Water entries
    for (day, val) in [(6, 2000.0), (7, 2500.0), (8, 1800.0), (9, 2200.0)] {
        let m = make_metric("water", val, NaiveDate::from_ymd_opt(2026, 1, day).unwrap());
        db.insert_metric(&m).unwrap();
    }

    let from = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 1, 11).unwrap();
    let result = report::generate(&db, from, to).unwrap();

    assert_eq!(result.from, from);
    assert_eq!(result.to, to);
    assert!(result.metrics.len() >= 2);

    let weight_summary = result
        .metrics
        .iter()
        .find(|s| s.metric_type == "weight")
        .unwrap();
    assert_eq!(weight_summary.count, 6);
    assert!(weight_summary.avg < 85.0);
    assert!((weight_summary.min - 84.0).abs() < 0.01);
    assert!((weight_summary.max - 85.0).abs() < 0.01);

    let water_summary = result
        .metrics
        .iter()
        .find(|s| s.metric_type == "water")
        .unwrap();
    assert_eq!(water_summary.count, 4);
}

/// Scenario: Empty report for date range with no data
///   Given an empty database
///   When I generate a report for any date range
///   Then the report has zero metrics
#[test]
fn test_report_empty_range() {
    let (_dir, db) = setup_db();
    let from = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
    let result = report::generate(&db, from, to).unwrap();
    assert!(result.metrics.is_empty());
    assert_eq!(result.days_with_entries, 0);
}

/// Scenario: Report includes logging day count
///   Given entries on 3 distinct days
///   When I generate a report for that range
///   Then days_with_entries is 3
#[test]
fn test_report_counts_distinct_days() {
    let (_dir, db) = setup_db();
    // 3 distinct days, some with multiple entries
    let m1 = make_metric("weight", 85.0, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
    let m2 = make_metric(
        "water",
        2000.0,
        NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
    );
    let m3 = make_metric("weight", 84.5, NaiveDate::from_ymd_opt(2026, 2, 3).unwrap());
    let m4 = make_metric("cardio", 30.0, NaiveDate::from_ymd_opt(2026, 2, 5).unwrap());
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();
    db.insert_metric(&m3).unwrap();
    db.insert_metric(&m4).unwrap();

    let from = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 2, 7).unwrap();
    let result = report::generate(&db, from, to).unwrap();
    assert_eq!(result.days_with_entries, 3);
}
