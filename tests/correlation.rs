use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use openvital::core::trend;
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

/// Scenario: Positive correlation between pain and screen time
///   Given matching daily data where pain increases with screen time
///   When I compute correlation between pain and screen_time
///   Then the correlation coefficient is positive (> 0.5)
#[test]
fn test_positive_correlation() {
    let (_dir, db) = setup_db();
    for i in 0..7 {
        let date = NaiveDate::from_ymd_opt(2026, 1, 1 + i).unwrap();
        let pain = 2.0 + i as f64 * 0.8; // increasing
        let screen = 6.0 + i as f64 * 1.0; // also increasing
        let m1 = make_metric("pain", pain, date);
        let m2 = make_metric("screen_time", screen, date);
        db.insert_metric(&m1).unwrap();
        db.insert_metric(&m2).unwrap();
    }

    let result = trend::correlate(&db, "pain", "screen_time", None).unwrap();
    assert!(
        result.coefficient > 0.5,
        "Expected positive correlation, got {}",
        result.coefficient
    );
}

/// Scenario: No correlation between unrelated metrics
///   Given pain is constant while screen_time varies randomly
///   When I compute correlation
///   Then the correlation coefficient is near zero
#[test]
fn test_no_correlation_constant_metric() {
    let (_dir, db) = setup_db();
    let screen_values = [8.0, 4.0, 10.0, 6.0, 12.0, 3.0, 7.0];
    for (i, &screen) in screen_values.iter().enumerate() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 1 + i as u32).unwrap();
        let m1 = make_metric("pain", 5.0, date); // constant
        let m2 = make_metric("screen_time", screen, date);
        db.insert_metric(&m1).unwrap();
        db.insert_metric(&m2).unwrap();
    }

    let result = trend::correlate(&db, "pain", "screen_time", None).unwrap();
    assert!(
        result.coefficient.abs() < 0.1,
        "Expected near-zero correlation for constant metric, got {}",
        result.coefficient
    );
}

/// Scenario: Correlation with insufficient data
///   Given only 1 day of data
///   When I compute correlation
///   Then coefficient is 0 (insufficient data)
#[test]
fn test_correlation_insufficient_data() {
    let (_dir, db) = setup_db();
    let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let m1 = make_metric("pain", 5.0, date);
    let m2 = make_metric("screen_time", 8.0, date);
    db.insert_metric(&m1).unwrap();
    db.insert_metric(&m2).unwrap();

    let result = trend::correlate(&db, "pain", "screen_time", None).unwrap();
    assert!(
        result.coefficient.abs() < 0.01,
        "Insufficient data should yield ~0 correlation"
    );
    assert!(result.data_points <= 1);
}
