mod common;

use chrono::NaiveDate;
use openvital::core::report;

/// Scenario: Generate a weekly report with multiple metric types
#[test]
fn test_report_weekly_multiple_types() {
    let (_dir, db) = common::setup_db();
    for (day, val) in [
        (5, 85.0),
        (6, 84.8),
        (7, 84.5),
        (8, 84.6),
        (9, 84.3),
        (10, 84.0),
    ] {
        let m = common::make_metric(
            "weight",
            val,
            NaiveDate::from_ymd_opt(2026, 1, day).unwrap(),
        );
        db.insert_metric(&m).unwrap();
    }
    for (day, val) in [(6, 2000.0), (7, 2500.0), (8, 1800.0), (9, 2200.0)] {
        let m = common::make_metric("water", val, NaiveDate::from_ymd_opt(2026, 1, day).unwrap());
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
#[test]
fn test_report_empty_range() {
    let (_dir, db) = common::setup_db();
    let from = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
    let result = report::generate(&db, from, to).unwrap();
    assert!(result.metrics.is_empty());
    assert_eq!(result.days_with_entries, 0);
}

/// Scenario: Report includes logging day count
#[test]
fn test_report_counts_distinct_days() {
    let (_dir, db) = common::setup_db();
    let m1 = common::make_metric("weight", 85.0, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
    let m2 = common::make_metric(
        "water",
        2000.0,
        NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
    );
    let m3 = common::make_metric("weight", 84.5, NaiveDate::from_ymd_opt(2026, 2, 3).unwrap());
    let m4 = common::make_metric("cardio", 30.0, NaiveDate::from_ymd_opt(2026, 2, 5).unwrap());
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();
    db.insert_metric(&m3).unwrap();
    db.insert_metric(&m4).unwrap();

    let from = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 2, 7).unwrap();
    let result = report::generate(&db, from, to).unwrap();
    assert_eq!(result.days_with_entries, 3);
}
