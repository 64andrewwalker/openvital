mod common;

use openvital::models::config::{Alerts, Config, Profile};
use openvital::models::metric::Metric;

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

/// Create a metric entry with the current UTC timestamp (so it matches today's local date).
fn make_metric_today(metric_type: &str, value: f64) -> Metric {
    Metric::new(metric_type.to_string(), value)
}

/// Scenario: compute() returns today's date and logged metric types
#[test]
fn test_compute_returns_today_and_logged_types() {
    let (_dir, db) = common::setup_db();

    // Insert entries for today using current UTC time (matches today's local date)
    let weight = make_metric_today("weight", 80.0);
    let water = make_metric_today("water", 2000.0);
    db.insert_metric(&weight).unwrap();
    db.insert_metric(&water).unwrap();

    let config = Config::default();
    let status = openvital::core::status::compute(&db, &config).unwrap();

    let today_local = chrono::Local::now().date_naive();
    assert_eq!(status.date, today_local);
    assert!(
        status.today.logged.contains(&"weight".to_string()),
        "weight should appear in today's logged types"
    );
    assert!(
        status.today.logged.contains(&"water".to_string()),
        "water should appear in today's logged types"
    );
}

/// Scenario: compute() calculates BMI when height_cm is set and weight is logged
#[test]
fn test_compute_bmi_with_height_and_weight() {
    let (_dir, db) = common::setup_db();

    // Log a weight entry for today
    let weight = make_metric_today("weight", 75.0);
    db.insert_metric(&weight).unwrap();

    let mut config = Config::default();
    config.profile = Profile {
        height_cm: Some(180.0),
        ..Default::default()
    };

    let status = openvital::core::status::compute(&db, &config).unwrap();

    assert_eq!(status.profile.height_cm, Some(180.0));
    assert_eq!(status.profile.latest_weight_kg, Some(75.0));

    // BMI = 75 / (1.80 * 1.80) = 75 / 3.24 ≈ 23.1
    let bmi = status.profile.bmi.expect("BMI should be computed");
    assert!(
        (bmi - 23.1).abs() < 0.2,
        "BMI should be approximately 23.1, got {bmi}"
    );
    assert_eq!(
        status.profile.bmi_category,
        Some("normal"),
        "BMI ~23.1 should be categorized as normal"
    );
}

/// Scenario: compute() returns no BMI when no weight is logged
#[test]
fn test_compute_no_bmi_without_weight() {
    let (_dir, db) = common::setup_db();

    let mut config = Config::default();
    config.profile = Profile {
        height_cm: Some(175.0),
        ..Default::default()
    };

    let status = openvital::core::status::compute(&db, &config).unwrap();

    assert!(
        status.profile.bmi.is_none(),
        "BMI should be None when no weight entry exists"
    );
    assert!(
        status.profile.bmi_category.is_none(),
        "BMI category should be None when BMI is None"
    );
}

/// Scenario: compute() returns no BMI when height_cm is not configured
#[test]
fn test_compute_no_bmi_without_height() {
    let (_dir, db) = common::setup_db();

    // Log weight but no height configured
    let weight = make_metric_today("weight", 80.0);
    db.insert_metric(&weight).unwrap();

    let config = Config::default(); // height_cm is None by default

    let status = openvital::core::status::compute(&db, &config).unwrap();

    assert!(
        status.profile.height_cm.is_none(),
        "height_cm should be None in default config"
    );
    assert!(
        status.profile.bmi.is_none(),
        "BMI should be None when height_cm is not set"
    );
}

/// Scenario: compute() detects today's pain entry above threshold and reports pain_alerts
#[test]
fn test_compute_pain_alerts_today() {
    let (_dir, db) = common::setup_db();

    // Pain value of 7, which exceeds default threshold of 5
    let pain = make_metric_today("pain", 7.0);
    db.insert_metric(&pain).unwrap();

    let config = Config::default();
    let status = openvital::core::status::compute(&db, &config).unwrap();

    assert_eq!(
        status.today.pain_alerts.len(),
        1,
        "One pain alert expected when pain >= threshold"
    );
    let alert = &status.today.pain_alerts[0];
    assert_eq!(alert["type"], "pain");
    assert_eq!(alert["value"], 7.0);
}

/// Scenario: compute() does not raise pain_alerts when pain is below threshold
#[test]
fn test_compute_no_pain_alerts_below_threshold() {
    let (_dir, db) = common::setup_db();

    // Pain value of 3, which is below default threshold of 5
    let pain = make_metric_today("pain", 3.0);
    db.insert_metric(&pain).unwrap();

    let config = Config::default();
    let status = openvital::core::status::compute(&db, &config).unwrap();

    assert!(
        status.today.pain_alerts.is_empty(),
        "No pain alerts expected when pain < threshold"
    );
}

/// Scenario: compute() includes soreness in pain_alerts when above threshold
#[test]
fn test_compute_soreness_alert_today() {
    let (_dir, db) = common::setup_db();

    let soreness = make_metric_today("soreness", 8.0);
    db.insert_metric(&soreness).unwrap();

    let config = Config::default();
    let status = openvital::core::status::compute(&db, &config).unwrap();

    assert_eq!(
        status.today.pain_alerts.len(),
        1,
        "One pain alert expected for soreness above threshold"
    );
    assert_eq!(status.today.pain_alerts[0]["type"], "soreness");
}

/// Scenario: compute() shows a streak when entries were logged for several consecutive days
#[test]
fn test_compute_streak_included() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();

    // Log entries for today and 2 prior days
    for i in 0..3i64 {
        let date = today - chrono::Duration::days(i);
        let m = common::make_metric("weight", 80.0, date);
        db.insert_metric(&m).unwrap();
    }

    let config = Config::default();
    let status = openvital::core::status::compute(&db, &config).unwrap();

    assert_eq!(
        status.streaks.logging_days, 3,
        "Streak should be 3 for 3 consecutive days with entries"
    );
}

/// Scenario: compute() fires consecutive_pain_alerts when pain is above threshold for N days
#[test]
fn test_compute_consecutive_pain_alert_triggered() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();

    // Log pain above threshold for 3 consecutive days (default required_days = 3)
    for i in 0..3i64 {
        let date = today - chrono::Duration::days(i);
        let m = common::make_metric("pain", 6.0, date);
        db.insert_metric(&m).unwrap();
    }

    let config = Config::default();
    let status = openvital::core::status::compute(&db, &config).unwrap();

    assert!(
        !status.consecutive_pain_alerts.is_empty(),
        "consecutive_pain_alerts should fire after 3 days of pain >= threshold"
    );
    let alert = &status.consecutive_pain_alerts[0];
    assert_eq!(alert.metric_type, "pain");
    assert_eq!(alert.consecutive_days, 3);
    assert_eq!(alert.latest_value, 6.0);
}

/// Scenario: compute() returns empty state when database has no entries
#[test]
fn test_compute_empty_database() {
    let (_dir, db) = common::setup_db();

    let config = Config::default();
    let status = openvital::core::status::compute(&db, &config).unwrap();

    assert!(
        status.today.logged.is_empty(),
        "No logged types expected for empty database"
    );
    assert!(
        status.today.pain_alerts.is_empty(),
        "No pain alerts expected for empty database"
    );
    assert_eq!(
        status.streaks.logging_days, 0,
        "Streak should be 0 for empty database"
    );
    assert!(
        status.consecutive_pain_alerts.is_empty(),
        "No consecutive pain alerts expected for empty database"
    );
    assert!(
        status.profile.bmi.is_none(),
        "BMI should be None for empty database"
    );
}

/// Scenario: BMI category boundaries — underweight, overweight, obese
#[test]
fn test_compute_bmi_categories() {
    // Underweight: BMI < 18.5 — height 180cm, weight 55kg → BMI ≈ 17.0
    {
        let (_dir, db) = common::setup_db();
        let weight = make_metric_today("weight", 55.0);
        db.insert_metric(&weight).unwrap();
        let mut config = Config::default();
        config.profile = Profile {
            height_cm: Some(180.0),
            ..Default::default()
        };
        let status = openvital::core::status::compute(&db, &config).unwrap();
        assert_eq!(
            status.profile.bmi_category,
            Some("underweight"),
            "55 kg at 180 cm should be underweight"
        );
    }

    // Overweight: BMI 25-29.9 — height 180cm, weight 85kg → BMI ≈ 26.2
    {
        let (_dir, db) = common::setup_db();
        let weight = make_metric_today("weight", 85.0);
        db.insert_metric(&weight).unwrap();
        let mut config = Config::default();
        config.profile = Profile {
            height_cm: Some(180.0),
            ..Default::default()
        };
        let status = openvital::core::status::compute(&db, &config).unwrap();
        assert_eq!(
            status.profile.bmi_category,
            Some("overweight"),
            "85 kg at 180 cm should be overweight"
        );
    }

    // Obese: BMI >= 30 — height 170cm, weight 100kg → BMI ≈ 34.6
    {
        let (_dir, db) = common::setup_db();
        let weight = make_metric_today("weight", 100.0);
        db.insert_metric(&weight).unwrap();
        let mut config = Config::default();
        config.profile = Profile {
            height_cm: Some(170.0),
            ..Default::default()
        };
        let status = openvital::core::status::compute(&db, &config).unwrap();
        assert_eq!(
            status.profile.bmi_category,
            Some("obese"),
            "100 kg at 170 cm should be obese"
        );
    }
}

/// Scenario: check_consecutive_pain requires threshold to be met; pain below threshold does not count
#[test]
fn test_pain_below_threshold_not_counted() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();

    // Log pain at exactly the threshold value (5) for 3 days
    // Default threshold is 5, and the code uses `value >= threshold` so 5 should trigger
    for i in 0..3i64 {
        let date = today - chrono::Duration::days(i);
        let m = common::make_metric("pain", 5.0, date);
        db.insert_metric(&m).unwrap();
    }

    let config = Config::default();
    let alerts =
        openvital::core::status::check_consecutive_pain(&db, today, &config.alerts).unwrap();
    assert!(
        !alerts.is_empty(),
        "Pain at exactly the threshold (5) for 3 days should trigger alert"
    );
}

/// Scenario: check_consecutive_pain does not alert when pain is one below threshold
#[test]
fn test_pain_one_below_threshold_no_alert() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();

    // Log pain at 4 (below threshold of 5) for 3 consecutive days
    for i in 0..3i64 {
        let date = today - chrono::Duration::days(i);
        let m = common::make_metric("pain", 4.0, date);
        db.insert_metric(&m).unwrap();
    }

    let config = Config::default();
    let alerts =
        openvital::core::status::check_consecutive_pain(&db, today, &config.alerts).unwrap();
    assert!(
        alerts.is_empty(),
        "Pain below threshold should never trigger alert"
    );
}

/// Scenario: check_consecutive_pain respects custom thresholds and required_days
#[test]
fn test_pain_custom_alert_config() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();

    // Custom: threshold=7, required_days=2 — log pain=8 for 2 days
    for i in 0..2i64 {
        let date = today - chrono::Duration::days(i);
        let m = common::make_metric("pain", 8.0, date);
        db.insert_metric(&m).unwrap();
    }

    let alerts_config = Alerts {
        pain_threshold: 7,
        pain_consecutive_days: 2,
    };
    let alerts =
        openvital::core::status::check_consecutive_pain(&db, today, &alerts_config).unwrap();
    assert!(
        !alerts.is_empty(),
        "Should alert with custom threshold=7, required_days=2 when pain=8 for 2 days"
    );

    // Now log pain=6 (below custom threshold of 7) — should NOT alert
    let (_dir2, db2) = common::setup_db();
    for i in 0..2i64 {
        let date = today - chrono::Duration::days(i);
        let m = common::make_metric("pain", 6.0, date);
        db2.insert_metric(&m).unwrap();
    }
    let alerts2 =
        openvital::core::status::check_consecutive_pain(&db2, today, &alerts_config).unwrap();
    assert!(
        alerts2.is_empty(),
        "Should not alert when pain is below custom threshold"
    );
}

/// Scenario: compute() uses the most recent weight for BMI even if older entries exist
#[test]
fn test_compute_uses_latest_weight_for_bmi() {
    let (_dir, db) = common::setup_db();

    // Insert an old weight entry (past)
    let old_date = chrono::Local::now().date_naive() - chrono::Duration::days(10);
    let old_weight = common::make_metric("weight", 100.0, old_date);
    db.insert_metric(&old_weight).unwrap();

    // Insert a recent weight entry (today)
    let recent_weight = make_metric_today("weight", 75.0);
    db.insert_metric(&recent_weight).unwrap();

    let mut config = Config::default();
    config.profile = Profile {
        height_cm: Some(180.0),
        ..Default::default()
    };

    let status = openvital::core::status::compute(&db, &config).unwrap();

    // Should use the latest weight (75.0, not 100.0)
    assert_eq!(
        status.profile.latest_weight_kg,
        Some(75.0),
        "compute() should use the most recent weight entry"
    );

    let bmi = status.profile.bmi.expect("BMI should be computed");
    // BMI = 75 / (1.80^2) ≈ 23.1
    assert!(
        (bmi - 23.1).abs() < 0.2,
        "BMI should be based on latest weight (75 kg), got {bmi}"
    );
}
