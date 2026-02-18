use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use openvital::db::Database;
use openvital::models::config::Config;
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

/// Scenario: Streak counts consecutive logging days
///   Given entries logged on 5 consecutive days ending today
///   When I compute streaks
///   Then logging_days streak is 5
#[test]
fn test_streak_consecutive_days() {
    let (_dir, db) = setup_db();
    let today = chrono::Local::now().date_naive();
    for i in 0..5 {
        let date = today - chrono::Duration::days(i);
        let m = make_metric("weight", 85.0 - i as f64 * 0.1, date);
        db.insert_metric(&m).unwrap();
    }

    let streaks = openvital::core::status::compute_streaks(&db, today).unwrap();
    assert_eq!(streaks.logging_days, 5);
}

/// Scenario: Streak breaks when a day is missed
///   Given entries on today, yesterday, and 3 days ago (gap on day before yesterday)
///   When I compute streaks
///   Then logging_days streak is 2 (today + yesterday only)
#[test]
fn test_streak_breaks_on_gap() {
    let (_dir, db) = setup_db();
    let today = chrono::Local::now().date_naive();
    // Today and yesterday
    let m1 = make_metric("weight", 85.0, today);
    let m2 = make_metric("weight", 85.1, today - chrono::Duration::days(1));
    // Skip day-2, log day-3
    let m3 = make_metric("weight", 85.2, today - chrono::Duration::days(3));
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();
    db.insert_metric(&m3).unwrap();

    let streaks = openvital::core::status::compute_streaks(&db, today).unwrap();
    assert_eq!(streaks.logging_days, 2);
}

/// Scenario: Pain alert triggers when pain exceeds threshold for N consecutive days
///   Given pain logged at 6 (above threshold 5) for 3 consecutive days
///   When I check pain alerts
///   Then a consecutive pain alert is generated
#[test]
fn test_pain_consecutive_alert() {
    let (_dir, db) = setup_db();
    let today = chrono::Local::now().date_naive();
    for i in 0..3 {
        let date = today - chrono::Duration::days(i);
        let m = make_metric("pain", 6.0, date);
        db.insert_metric(&m).unwrap();
    }

    let config = Config::default(); // threshold=5, consecutive=3
    let alerts =
        openvital::core::status::check_consecutive_pain(&db, today, &config.alerts).unwrap();
    assert!(
        !alerts.is_empty(),
        "Should have a consecutive pain alert when pain >= threshold for 3+ days"
    );
}

/// Scenario: No pain alert when days are not consecutive
///   Given pain logged at 6 on today and 2 days ago (gap yesterday)
///   When I check pain alerts with consecutive_days=3
///   Then no consecutive pain alert
#[test]
fn test_pain_no_alert_non_consecutive() {
    let (_dir, db) = setup_db();
    let today = chrono::Local::now().date_naive();
    let m1 = make_metric("pain", 6.0, today);
    let m2 = make_metric("pain", 6.0, today - chrono::Duration::days(2));
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
