#![allow(dead_code)]

use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use openvital::db::Database;
use openvital::models::metric::Metric;
use tempfile::TempDir;

/// Create a temporary database for testing.
pub fn setup_db() -> (TempDir, Database) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    (dir, db)
}

/// Create a metric entry with a specific date (noon UTC).
pub fn make_metric(metric_type: &str, value: f64, date: NaiveDate) -> Metric {
    let dt = date.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    let ts = Utc.from_utc_datetime(&dt);
    let mut m = Metric::new(metric_type.to_string(), value);
    m.timestamp = ts;
    m
}
