mod common;

use openvital::models::config::Config;

/// Scenario: Streak counts consecutive logging days
#[test]
fn test_streak_consecutive_days() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();
    for i in 0..5 {
        let date = today - chrono::Duration::days(i);
        let m = common::make_metric("weight", 85.0 - i as f64 * 0.1, date);
        db.insert_metric(&m).unwrap();
    }

    let streaks = openvital::core::status::compute_streaks(&db, today).unwrap();
    assert_eq!(streaks.logging_days, 5);
}

/// Scenario: Streak breaks when a day is missed
#[test]
fn test_streak_breaks_on_gap() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();
    let m1 = common::make_metric("weight", 85.0, today);
    let m2 = common::make_metric("weight", 85.1, today - chrono::Duration::days(1));
    let m3 = common::make_metric("weight", 85.2, today - chrono::Duration::days(3));
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();
    db.insert_metric(&m3).unwrap();

    let streaks = openvital::core::status::compute_streaks(&db, today).unwrap();
    assert_eq!(streaks.logging_days, 2);
}

/// Scenario: Pain alert triggers when pain exceeds threshold for N consecutive days
#[test]
fn test_pain_consecutive_alert() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();
    for i in 0..3 {
        let date = today - chrono::Duration::days(i);
        let m = common::make_metric("pain", 6.0, date);
        db.insert_metric(&m).unwrap();
    }

    let config = Config::default();
    let alerts =
        openvital::core::status::check_consecutive_pain(&db, today, &config.alerts).unwrap();
    assert!(
        !alerts.is_empty(),
        "Should have a consecutive pain alert when pain >= threshold for 3+ days"
    );
}

/// Scenario: No pain alert when days are not consecutive
#[test]
fn test_pain_no_alert_non_consecutive() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();
    let m1 = common::make_metric("pain", 6.0, today);
    let m2 = common::make_metric("pain", 6.0, today - chrono::Duration::days(2));
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();

    let config = Config::default();
    let alerts =
        openvital::core::status::check_consecutive_pain(&db, today, &config.alerts).unwrap();
    assert!(
        alerts.is_empty(),
        "Should not trigger alert when pain days are not consecutive"
    );
}
