mod common;

use chrono::NaiveDate;
use openvital::core::trend;

/// Scenario: Positive correlation between pain and screen time
#[test]
fn test_positive_correlation() {
    let (_dir, db) = common::setup_db();
    for i in 0..7 {
        let date = NaiveDate::from_ymd_opt(2026, 1, 1 + i).unwrap();
        let pain = 2.0 + i as f64 * 0.8;
        let screen = 6.0 + i as f64 * 1.0;
        db.insert_metric(&common::make_metric("pain", pain, date))
            .unwrap();
        db.insert_metric(&common::make_metric("screen_time", screen, date))
            .unwrap();
    }

    let result = trend::correlate(&db, "pain", "screen_time", None).unwrap();
    assert!(
        result.coefficient > 0.5,
        "Expected positive correlation, got {}",
        result.coefficient
    );
}

/// Scenario: No correlation between unrelated metrics
#[test]
fn test_no_correlation_constant_metric() {
    let (_dir, db) = common::setup_db();
    let screen_values = [8.0, 4.0, 10.0, 6.0, 12.0, 3.0, 7.0];
    for (i, &screen) in screen_values.iter().enumerate() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 1 + i as u32).unwrap();
        db.insert_metric(&common::make_metric("pain", 5.0, date))
            .unwrap();
        db.insert_metric(&common::make_metric("screen_time", screen, date))
            .unwrap();
    }

    let result = trend::correlate(&db, "pain", "screen_time", None).unwrap();
    assert!(
        result.coefficient.abs() < 0.1,
        "Expected near-zero correlation for constant metric, got {}",
        result.coefficient
    );
}

/// Scenario: Correlation with insufficient data
#[test]
fn test_correlation_insufficient_data() {
    let (_dir, db) = common::setup_db();
    let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    db.insert_metric(&common::make_metric("pain", 5.0, date))
        .unwrap();
    db.insert_metric(&common::make_metric("screen_time", 8.0, date))
        .unwrap();

    let result = trend::correlate(&db, "pain", "screen_time", None).unwrap();
    assert!(
        result.coefficient.abs() < 0.01,
        "Insufficient data should yield ~0 correlation"
    );
    assert!(result.data_points <= 1);
}
