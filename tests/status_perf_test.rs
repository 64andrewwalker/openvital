mod common;
use openvital::models::config::Alerts;
use chrono::{Duration, Local, Utc};

#[test]
fn test_check_consecutive_pain_optimized_logic() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Test case 1: 3 consecutive days of pain
    for i in 0..3 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("pain", 6.0, date)).unwrap();
    }

    let alerts = Alerts {
        pain_threshold: 5,
        pain_consecutive_days: 3,
    };

    let results = openvital::core::status::check_consecutive_pain(&db, today, &alerts).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].metric_type, "pain");
    assert_eq!(results[0].consecutive_days, 3);
}

#[test]
fn test_check_consecutive_pain_timezone_aware() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Create an entry with a timestamp that resolves to 'today' locally.
    let local_now = Local::now();
    let m = openvital::models::metric::Metric {
        id: "test".to_string(),
        timestamp: local_now.with_timezone(&Utc),
        category: openvital::models::metric::Category::Pain,
        metric_type: "pain".to_string(),
        value: 7.0,
        unit: "0-10".to_string(),
        note: None,
        tags: vec![],
        source: "test".to_string(),
    };
    db.insert_metric(&m).unwrap();

    let alerts = Alerts {
        pain_threshold: 5,
        pain_consecutive_days: 1,
    };

    let results = openvital::core::status::check_consecutive_pain(&db, today, &alerts).unwrap();
    assert_eq!(results.len(), 1, "Should have 1 alert for today's pain");
    assert_eq!(results[0].consecutive_days, 1);
}

#[test]
fn test_check_consecutive_pain_multi_type() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    db.insert_metric(&common::make_metric("pain", 6.0, today)).unwrap();
    db.insert_metric(&common::make_metric("soreness", 8.0, today)).unwrap();

    let alerts = Alerts {
        pain_threshold: 5,
        pain_consecutive_days: 1,
    };

    let results = openvital::core::status::check_consecutive_pain(&db, today, &alerts).unwrap();
    assert_eq!(results.len(), 2);
}
