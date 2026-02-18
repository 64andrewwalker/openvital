mod common;

use chrono::NaiveDate;
use openvital::core::trend::{self, TrendPeriod};
use std::str::FromStr;

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

#[test]
fn test_trend_period_from_str() {
    assert_eq!(TrendPeriod::from_str("daily").unwrap(), TrendPeriod::Daily);
    assert_eq!(
        TrendPeriod::from_str("weekly").unwrap(),
        TrendPeriod::Weekly
    );
    assert_eq!(
        TrendPeriod::from_str("monthly").unwrap(),
        TrendPeriod::Monthly
    );
    assert!(TrendPeriod::from_str("invalid").is_err());
}

#[test]
fn test_monthly_period_bucketing() {
    let (_dir, db) = common::setup_db();

    // January entries
    for (day, val) in [(5u32, 80.0), (15, 79.0), (25, 78.0)] {
        let m = common::make_metric(
            "weight",
            val,
            NaiveDate::from_ymd_opt(2026, 1, day).unwrap(),
        );
        db.insert_metric(&m).unwrap();
    }
    // February entries
    for (day, val) in [(2u32, 77.5), (12, 77.0)] {
        let m = common::make_metric(
            "weight",
            val,
            NaiveDate::from_ymd_opt(2026, 2, day).unwrap(),
        );
        db.insert_metric(&m).unwrap();
    }

    let result = trend::compute(&db, "weight", TrendPeriod::Monthly, Some(12)).unwrap();

    assert_eq!(result.period, "monthly");
    assert_eq!(result.data.len(), 2);

    // January bucket: label "2026-01", avg = (80+79+78)/3 = 79.0
    assert_eq!(result.data[0].label, "2026-01");
    assert_eq!(result.data[0].count, 3);
    assert!((result.data[0].avg - 79.0).abs() < 0.01);

    // February bucket: label "2026-02", avg = (77.5+77.0)/2 = 77.25
    assert_eq!(result.data[1].label, "2026-02");
    assert_eq!(result.data[1].count, 2);
    assert!((result.data[1].avg - 77.25).abs() < 0.01);

    assert_eq!(result.trend.direction, "decreasing");
    assert_eq!(result.trend.rate_unit, "per monthly");
}

#[test]
fn test_trend_last_parameter_limits_periods() {
    let (_dir, db) = common::setup_db();

    // Insert 5 weeks of data
    let weeks = [
        (2u32, 100.0_f64),
        (9, 102.0),
        (16, 104.0),
        (23, 106.0),
        (30, 108.0),
    ];
    for (day, val) in weeks {
        let m = common::make_metric(
            "cardio",
            val,
            NaiveDate::from_ymd_opt(2026, 3, day).unwrap(),
        );
        db.insert_metric(&m).unwrap();
    }

    // Request only the last 3 periods
    let result = trend::compute(&db, "cardio", TrendPeriod::Weekly, Some(3)).unwrap();

    assert_eq!(result.data.len(), 3);
    // Should be the last 3 weeks: weeks ending on Mar 16, 23, 30
    assert!((result.data[0].avg - 104.0).abs() < 0.01);
    assert!((result.data[1].avg - 106.0).abs() < 0.01);
    assert!((result.data[2].avg - 108.0).abs() < 0.01);
}

#[test]
fn test_stable_trend_constant_values() {
    let (_dir, db) = common::setup_db();

    // Same value every day for a week — slope should be ~0, direction = stable
    for day in 1u32..=7 {
        let m = common::make_metric(
            "resting_hr",
            60.0,
            NaiveDate::from_ymd_opt(2026, 2, day).unwrap(),
        );
        db.insert_metric(&m).unwrap();
    }

    let result = trend::compute(&db, "resting_hr", TrendPeriod::Daily, Some(30)).unwrap();

    assert_eq!(result.trend.direction, "stable");
    assert!((result.trend.rate).abs() < 0.01);
    // With slope ~0, projected ≈ 60.0 (clamped to [30.0, 90.0])
    assert!(result.trend.projected_30d.is_some());
    let projected = result.trend.projected_30d.unwrap();
    assert!((projected - 60.0).abs() < 0.2);
}

#[test]
fn test_increasing_trend_direction() {
    let (_dir, db) = common::setup_db();

    // Steadily increasing values over four weeks
    for (day, val) in [(2u32, 50.0_f64), (9, 55.0), (16, 60.0), (23, 65.0)] {
        let m = common::make_metric(
            "vo2max",
            val,
            NaiveDate::from_ymd_opt(2026, 3, day).unwrap(),
        );
        db.insert_metric(&m).unwrap();
    }

    let result = trend::compute(&db, "vo2max", TrendPeriod::Weekly, Some(12)).unwrap();

    assert_eq!(result.trend.direction, "increasing");
    assert!(result.trend.rate > 0.0);
    // With slope=5.0, periods_in_30d=30/7≈4.29, last_avg=65.0:
    // raw_projected = 65.0 + 5.0 * 4.29 ≈ 86.4, clamped to [32.5, 97.5]
    assert!(result.trend.projected_30d.is_some());
    let projected = result.trend.projected_30d.unwrap();
    assert!(projected > 65.0, "projection should be above last avg");
    assert!(projected <= 97.5, "projection should be clamped to 1.5x");
}

#[test]
fn test_single_period_trend_is_stable() {
    let (_dir, db) = common::setup_db();

    // Only one day of data → single period bucket → trend direction = stable
    let m = common::make_metric(
        "sleep_hours",
        7.5,
        NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
    );
    db.insert_metric(&m).unwrap();

    let result = trend::compute(&db, "sleep_hours", TrendPeriod::Daily, Some(12)).unwrap();

    assert_eq!(result.data.len(), 1);
    assert_eq!(result.trend.direction, "stable");
    assert_eq!(result.trend.rate, 0.0);
    // With a single data point, projected_30d equals that point's avg
    assert!(result.trend.projected_30d.is_some());
    assert!((result.trend.projected_30d.unwrap() - 7.5).abs() < 0.01);
}

/// Scenario: correlate() with both metrics having identical constant values on
/// every shared day causes zero variance in both series (denominator → 0).
/// The implementation clamps the coefficient to 0.0 in this case (line 255-258).
#[test]
fn test_correlate_zero_variance_returns_zero_coefficient() {
    let (_dir, db) = common::setup_db();

    // Both metrics have the same constant value on every day — zero variance
    for day in 1u32..=5 {
        let date = NaiveDate::from_ymd_opt(2026, 4, day).unwrap();
        db.insert_metric(&common::make_metric("pain", 3.0, date))
            .unwrap();
        db.insert_metric(&common::make_metric("soreness", 3.0, date))
            .unwrap();
    }

    let result = trend::correlate(&db, "pain", "soreness", None).unwrap();

    // With zero variance in both series the denominator is ~0, so coefficient
    // must be clamped to 0.0 (not NaN or ±Inf)
    assert_eq!(
        result.coefficient, 0.0,
        "Zero-variance denominator should yield coefficient = 0.0, got {}",
        result.coefficient
    );
    assert!(!result.coefficient.is_nan());
    assert!(result.data_points >= 2);
}

/// Scenario: correlate() with a last_days cutoff filters out older pairs.
/// This exercises the `cutoff` / `last_days` branch inside correlate() (lines 219-227).
#[test]
fn test_correlate_last_days_cutoff_filters_old_entries() {
    let (_dir, db) = common::setup_db();

    // Insert 14 days of positively-correlated data in the past
    // Use dates far enough in the past that last_days=7 will exclude them
    let base = chrono::Local::now().date_naive();
    for i in 0..7u32 {
        // Old entries: 30-36 days ago — outside a 7-day window
        let old_date = base - chrono::Duration::days(30 + i as i64);
        db.insert_metric(&common::make_metric("pain", 1.0 + i as f64, old_date))
            .unwrap();
        db.insert_metric(&common::make_metric(
            "screen_time",
            2.0 + i as f64,
            old_date,
        ))
        .unwrap();

        // Recent entries: 0-6 days ago — inside a 7-day window, all constant
        let recent_date = base - chrono::Duration::days(i as i64);
        db.insert_metric(&common::make_metric("pain", 5.0, recent_date))
            .unwrap();
        db.insert_metric(&common::make_metric("screen_time", 5.0, recent_date))
            .unwrap();
    }

    // With last_days=7 the cutoff should exclude the 30-36 day-old pairs
    let result_recent = trend::correlate(&db, "pain", "screen_time", Some(7)).unwrap();
    // Without cutoff we see all 14 days
    let result_all = trend::correlate(&db, "pain", "screen_time", None).unwrap();

    // The recent window only sees the constant (5.0, 5.0) pairs → 0.0 coefficient
    assert_eq!(
        result_recent.coefficient, 0.0,
        "Recent window with constant data should have zero coefficient"
    );
    // All data includes the varying old pairs which should push coefficient non-zero
    assert!(
        result_all.data_points > result_recent.data_points,
        "Without cutoff, more data_points should be included"
    );
}

#[test]
fn test_projection_clamped_to_reasonable_range() {
    let (_dir, db) = common::setup_db();

    // Create data with steep downward trend: 80, 60 over 2 weeks
    let w1_date = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();
    let w2_date = NaiveDate::from_ymd_opt(2026, 1, 13).unwrap();

    let m1 = common::make_metric("weight", 80.0, w1_date);
    db.insert_metric(&m1).unwrap();
    let m2 = common::make_metric("weight", 60.0, w2_date);
    db.insert_metric(&m2).unwrap();

    let result = trend::compute(&db, "weight", TrendPeriod::Weekly, None).unwrap();

    let projected = result.trend.projected_30d.unwrap();
    // Without clamp, projection would be 60 + (-20 * 4.3) ≈ -26 (absurd)
    // With clamp, should be >= 60 * 0.5 = 30
    assert!(
        projected >= 30.0,
        "projection {} should be >= 30.0",
        projected
    );
    assert!(projected >= 0.0, "projection should never be negative");
}

#[test]
fn test_projection_clamped_upper_bound() {
    let (_dir, db) = common::setup_db();

    // Create data with steep upward trend: 50, 100 over 2 weeks
    let w1_date = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();
    let w2_date = NaiveDate::from_ymd_opt(2026, 1, 13).unwrap();

    let m1 = common::make_metric("steps", 50.0, w1_date);
    db.insert_metric(&m1).unwrap();
    let m2 = common::make_metric("steps", 100.0, w2_date);
    db.insert_metric(&m2).unwrap();

    let result = trend::compute(&db, "steps", TrendPeriod::Weekly, None).unwrap();

    let projected = result.trend.projected_30d.unwrap();
    // Without clamp, projection would be 100 + 50 * 4.3 = 315 (absurd)
    // With clamp, should be <= 100 * 1.5 = 150
    assert!(
        projected <= 150.0,
        "projection {} should be <= 150.0",
        projected
    );
}

#[test]
fn test_projection_with_negative_values_stays_bounded() {
    let (_dir, db) = common::setup_db();

    let d1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
    db.insert_metric(&common::make_metric("mood", -5.0, d1))
        .unwrap();
    db.insert_metric(&common::make_metric("mood", -4.0, d2))
        .unwrap();

    let result = trend::compute(&db, "mood", TrendPeriod::Daily, None).unwrap();
    let projected = result.trend.projected_30d.unwrap();

    // last_avg = -4.0, so clamp band should be [-6.0, -2.0]
    assert!(
        projected >= -6.0,
        "projection {} should be >= -6.0",
        projected
    );
    assert!(
        projected <= -2.0,
        "projection {} should be <= -2.0",
        projected
    );
}
