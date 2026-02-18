mod common;

use chrono::NaiveDate;
use openvital::core::trend::{self, TrendPeriod};

#[test]
fn test_weekly_weight_trend() {
    let (_dir, db) = common::setup_db();

    // Week 1 (Mon 2026-02-02 to Sun 2026-02-08)
    for (d, v) in [(2, 86.0), (4, 85.8), (6, 85.5)] {
        let m = common::make_metric("weight", v, NaiveDate::from_ymd_opt(2026, 2, d).unwrap());
        db.insert_metric(&m).unwrap();
    }
    // Week 2 (Mon 2026-02-09 to Sun 2026-02-15)
    for (d, v) in [(9, 85.2), (11, 85.0), (13, 84.8)] {
        let m = common::make_metric("weight", v, NaiveDate::from_ymd_opt(2026, 2, d).unwrap());
        db.insert_metric(&m).unwrap();
    }

    let result = trend::compute(&db, "weight", TrendPeriod::Weekly, Some(12)).unwrap();

    assert_eq!(result.metric_type, "weight");
    assert_eq!(result.data.len(), 2);

    let w1 = &result.data[0];
    assert_eq!(w1.count, 3);
    assert!((w1.avg - 85.77).abs() < 0.1);
    assert!((w1.min - 85.5).abs() < f64::EPSILON);
    assert!((w1.max - 86.0).abs() < f64::EPSILON);

    let w2 = &result.data[1];
    assert_eq!(w2.count, 3);
    assert!((w2.avg - 85.0).abs() < 0.1);

    assert_eq!(result.trend.direction, "decreasing");
    assert!(result.trend.rate < 0.0);
}

#[test]
fn test_trend_empty_data() {
    let (_dir, db) = common::setup_db();
    let result = trend::compute(&db, "weight", TrendPeriod::Weekly, Some(12)).unwrap();
    assert!(result.data.is_empty());
    assert_eq!(result.trend.direction, "stable");
}

#[test]
fn test_daily_trend_aggregates_same_day() {
    let (_dir, db) = common::setup_db();

    let day1 = NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
    let day2 = NaiveDate::from_ymd_opt(2026, 2, 11).unwrap();
    db.insert_metric(&common::make_metric("water", 500.0, day1))
        .unwrap();
    db.insert_metric(&common::make_metric("water", 800.0, day1))
        .unwrap();
    db.insert_metric(&common::make_metric("water", 700.0, day2))
        .unwrap();

    let result = trend::compute(&db, "water", TrendPeriod::Daily, Some(30)).unwrap();

    assert_eq!(result.data.len(), 2);
    assert!((result.data[0].avg - 650.0).abs() < f64::EPSILON);
    assert_eq!(result.data[0].count, 2);
    assert!((result.data[1].avg - 700.0).abs() < f64::EPSILON);
}
