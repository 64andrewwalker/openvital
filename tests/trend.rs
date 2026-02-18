use std::path::Path;
use tempfile::TempDir;

use openvital::core::trend::{self, TrendPeriod};
use openvital::db::Database;
use openvital::models::metric::Metric;

fn open_db(dir: &Path) -> Database {
    let db_path = dir.join("test.db");
    Database::open(&db_path).unwrap()
}

fn insert_metric_on(db: &Database, metric_type: &str, value: f64, date: &str) {
    let mut m = Metric::new(metric_type.into(), value);
    m.timestamp = chrono::DateTime::parse_from_rfc3339(&format!("{}T12:00:00+00:00", date))
        .unwrap()
        .with_timezone(&chrono::Utc);
    db.insert_metric(&m).unwrap();
}

#[test]
fn test_weekly_weight_trend() {
    let tmp = TempDir::new().unwrap();
    let db = open_db(tmp.path());

    // Week 1 (Mon 2026-02-02 to Sun 2026-02-08)
    insert_metric_on(&db, "weight", 86.0, "2026-02-02");
    insert_metric_on(&db, "weight", 85.8, "2026-02-04");
    insert_metric_on(&db, "weight", 85.5, "2026-02-06");

    // Week 2 (Mon 2026-02-09 to Sun 2026-02-15)
    insert_metric_on(&db, "weight", 85.2, "2026-02-09");
    insert_metric_on(&db, "weight", 85.0, "2026-02-11");
    insert_metric_on(&db, "weight", 84.8, "2026-02-13");

    let result = trend::compute(&db, "weight", TrendPeriod::Weekly, Some(12)).unwrap();

    assert_eq!(result.metric_type, "weight");
    assert_eq!(result.data.len(), 2);

    // Week 1 averages
    let w1 = &result.data[0];
    assert_eq!(w1.count, 3);
    assert!((w1.avg - 85.77).abs() < 0.1);
    assert!((w1.min - 85.5).abs() < f64::EPSILON);
    assert!((w1.max - 86.0).abs() < f64::EPSILON);

    // Week 2 averages
    let w2 = &result.data[1];
    assert_eq!(w2.count, 3);
    assert!((w2.avg - 85.0).abs() < 0.1);

    // Trend should be decreasing
    let t = &result.trend;
    assert_eq!(t.direction, "decreasing");
    assert!(t.rate < 0.0);
}

#[test]
fn test_trend_empty_data() {
    let tmp = TempDir::new().unwrap();
    let db = open_db(tmp.path());

    let result = trend::compute(&db, "weight", TrendPeriod::Weekly, Some(12)).unwrap();
    assert!(result.data.is_empty());
    assert_eq!(result.trend.direction, "stable");
}

#[test]
fn test_daily_trend_aggregates_same_day() {
    let tmp = TempDir::new().unwrap();
    let db = open_db(tmp.path());

    // Two entries on the same day
    insert_metric_on(&db, "water", 500.0, "2026-02-10");
    insert_metric_on(&db, "water", 800.0, "2026-02-10");
    insert_metric_on(&db, "water", 700.0, "2026-02-11");

    let result = trend::compute(&db, "water", TrendPeriod::Daily, Some(30)).unwrap();

    assert_eq!(result.data.len(), 2);
    // Day 1: avg of 500+800 = 650
    assert!((result.data[0].avg - 650.0).abs() < f64::EPSILON);
    assert_eq!(result.data[0].count, 2);
    // Day 2: just 700
    assert!((result.data[1].avg - 700.0).abs() < f64::EPSILON);
}
