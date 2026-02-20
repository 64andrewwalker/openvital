mod common;

use chrono::{Duration, Local};
use openvital::core::anomaly;
use openvital::models::anomaly::{Severity, Threshold};

#[test]
fn test_anomaly_detect_flags_outlier() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Build a 14-day baseline of heart_rate around 70-76
    for i in 1..=14 {
        let date = today - Duration::days(i);
        let m = common::make_metric("heart_rate", 70.0 + (i % 7) as f64, date);
        db.insert_metric(&m).unwrap();
    }

    // Add an anomalous reading today
    let outlier = common::make_metric("heart_rate", 95.0, today);
    db.insert_metric(&outlier).unwrap();

    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(!result.anomalies.is_empty(), "should detect the outlier");
    assert_eq!(result.anomalies[0].metric_type, "heart_rate");
    assert!(matches!(
        result.anomalies[0].severity,
        Severity::Warning | Severity::Alert
    ));
}

#[test]
fn test_anomaly_no_data_returns_empty() {
    let (_dir, db) = common::setup_db();
    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(result.anomalies.is_empty());
    assert!(result.scanned_types.is_empty());
}

#[test]
fn test_anomaly_insufficient_data_skips() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Only 3 data points — below minimum of 7
    for i in 1..=3 {
        let date = today - Duration::days(i);
        let m = common::make_metric("weight", 80.0 + i as f64, date);
        db.insert_metric(&m).unwrap();
    }

    let result = anomaly::detect(&db, Some("weight"), 30, Threshold::Moderate).unwrap();
    assert!(result.anomalies.is_empty());
}

#[test]
fn test_anomaly_normal_value_not_flagged() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // 14-day baseline of weight around 80-82
    for i in 1..=14 {
        let date = today - Duration::days(i);
        let m = common::make_metric("weight", 80.0 + (i % 3) as f64, date);
        db.insert_metric(&m).unwrap();
    }

    // Today's value is within normal range
    let normal = common::make_metric("weight", 81.0, today);
    db.insert_metric(&normal).unwrap();

    let result = anomaly::detect(&db, Some("weight"), 30, Threshold::Moderate).unwrap();
    assert!(
        result.anomalies.is_empty(),
        "normal value should not be flagged"
    );
}

#[test]
fn test_anomaly_threshold_strict_catches_more() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Build baseline around 70-76
    for i in 1..=14 {
        let date = today - Duration::days(i);
        let m = common::make_metric("heart_rate", 70.0 + (i % 7) as f64, date);
        db.insert_metric(&m).unwrap();
    }

    // A mildly elevated reading — between strict and relaxed bounds
    // Baseline values: 71,72,73,74,75,76,70,71,72,73,74,75,76,70
    // Q1 ≈ 71, Q3 ≈ 75, IQR ≈ 4
    // Strict upper = 75 + 1.0*4 = 79
    // Relaxed upper = 75 + 2.0*4 = 83
    // So 81 is above strict but below relaxed
    let mild = common::make_metric("heart_rate", 81.0, today);
    db.insert_metric(&mild).unwrap();

    let strict = anomaly::detect(&db, Some("heart_rate"), 30, Threshold::Strict).unwrap();
    let relaxed = anomaly::detect(&db, Some("heart_rate"), 30, Threshold::Relaxed).unwrap();

    assert!(
        !strict.anomalies.is_empty(),
        "strict threshold should flag mild elevation"
    );
    assert!(
        relaxed.anomalies.is_empty(),
        "relaxed threshold should not flag mild elevation"
    );
}

#[test]
fn test_anomaly_filter_by_type() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Baseline for two types
    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 80.0, date))
            .unwrap();
        db.insert_metric(&common::make_metric("heart_rate", 72.0, date))
            .unwrap();
    }

    // Anomaly in heart_rate only
    db.insert_metric(&common::make_metric("heart_rate", 110.0, today))
        .unwrap();
    db.insert_metric(&common::make_metric("weight", 80.0, today))
        .unwrap();

    // Filter to weight only — should find nothing
    let result = anomaly::detect(&db, Some("weight"), 30, Threshold::Moderate).unwrap();
    assert!(result.anomalies.is_empty());

    // No filter — should find heart_rate anomaly
    let result_all = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(!result_all.anomalies.is_empty());
    assert_eq!(result_all.anomalies[0].metric_type, "heart_rate");
}

#[test]
fn test_anomaly_baseline_stats() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Known values: 10, 20, 30, 40, 50, 60, 70
    for (i, val) in [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0]
        .iter()
        .enumerate()
    {
        let date = today - Duration::days(i as i64 + 1);
        db.insert_metric(&common::make_metric("test_metric", *val, date))
            .unwrap();
    }

    // Value of 200 is clearly an outlier
    db.insert_metric(&common::make_metric("test_metric", 200.0, today))
        .unwrap();

    let result = anomaly::detect(&db, Some("test_metric"), 30, Threshold::Moderate).unwrap();
    assert!(!result.anomalies.is_empty());

    let a = &result.anomalies[0];
    // With linear interpolation: Q1~25, Q3~55, IQR~30, median=40
    assert!(
        (a.baseline.q1 - 25.0).abs() < 6.0,
        "Q1 should be around 25, got {}",
        a.baseline.q1
    );
    assert!(
        (a.baseline.q3 - 55.0).abs() < 6.0,
        "Q3 should be around 55, got {}",
        a.baseline.q3
    );
    assert!(a.baseline.iqr > 0.0, "IQR should be positive");
}

#[test]
fn test_anomaly_summary_generated() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("pain", 3.0, date))
            .unwrap();
    }
    db.insert_metric(&common::make_metric("pain", 9.0, today))
        .unwrap();

    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(!result.summary.is_empty());
    assert!(!result.anomalies[0].summary.is_empty());
}

#[test]
fn test_anomaly_all_identical_values() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // All values are 72.0 — IQR is 0
    for i in 0..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("heart_rate", 72.0, date))
            .unwrap();
    }

    let result = anomaly::detect(&db, Some("heart_rate"), 30, Threshold::Moderate).unwrap();
    // With IQR=0 and today's value=72 matching the baseline, no anomaly
    assert!(result.anomalies.is_empty());
}

#[test]
fn test_anomaly_below_baseline() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Baseline: 70-76
    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("heart_rate", 70.0 + (i % 7) as f64, date))
            .unwrap();
    }

    // Abnormally low value
    db.insert_metric(&common::make_metric("heart_rate", 40.0, today))
        .unwrap();

    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(!result.anomalies.is_empty());
    assert_eq!(result.anomalies[0].deviation, "below");
}

#[test]
fn test_anomaly_multiple_types_scanned() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 80.0, date))
            .unwrap();
        db.insert_metric(&common::make_metric("sleep", 7.5, date))
            .unwrap();
        db.insert_metric(&common::make_metric("pain", 3.0, date))
            .unwrap();
    }

    // Normal values today
    db.insert_metric(&common::make_metric("weight", 80.0, today))
        .unwrap();
    db.insert_metric(&common::make_metric("sleep", 7.5, today))
        .unwrap();
    db.insert_metric(&common::make_metric("pain", 3.0, today))
        .unwrap();

    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(result.scanned_types.len() >= 3);
    assert!(result.anomalies.is_empty());
}

#[test]
fn test_anomaly_clean_types_populated() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 80.0, date))
            .unwrap();
    }
    db.insert_metric(&common::make_metric("weight", 80.0, today))
        .unwrap();

    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(result.clean_types.contains(&"weight".to_string()));
}
