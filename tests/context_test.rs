mod common;

use chrono::{Duration, Local};
use openvital::core::context;
use openvital::db::Database;
use openvital::models::config::Config;

fn make_test_config() -> Config {
    let mut config = Config::default();
    config.profile.height_cm = Some(180.0);
    config
}

#[test]
fn test_context_empty_db() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();

    let result = context::compute(&db, &config, 7, None).unwrap();

    assert_eq!(result.period.days, 7);
    assert!(result.metrics.is_empty());
    assert!(result.goals.is_empty());
    assert!(result.anomalies.is_empty());
    assert!(
        !result.summary.is_empty(),
        "summary should always be present"
    );
}

#[test]
fn test_context_with_metrics() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    for i in 0..7 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 83.0 - i as f64 * 0.3, date))
            .unwrap();
        db.insert_metric(&common::make_metric("pain", 3.0, date))
            .unwrap();
    }

    let result = context::compute(&db, &config, 7, None).unwrap();

    assert!(result.metrics.contains_key("weight"));
    assert!(result.metrics.contains_key("pain"));

    let weight = &result.metrics["weight"];
    assert!(weight.latest.is_some());
    assert!(weight.trend.is_some());
    assert!(weight.stats.count > 0);
    assert!(!weight.summary.is_empty());
}

#[test]
fn test_context_with_goals() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    db.insert_metric(&common::make_metric("weight", 83.0, today))
        .unwrap();

    use openvital::models::goal::{Direction, Timeframe};
    openvital::core::goal::set_goal(
        &db,
        "weight".into(),
        80.0,
        Direction::Below,
        Timeframe::Daily,
    )
    .unwrap();

    let result = context::compute(&db, &config, 7, None).unwrap();

    assert!(!result.goals.is_empty());
    assert_eq!(result.goals[0].metric_type, "weight");
}

#[test]
fn test_context_filter_by_type() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    db.insert_metric(&common::make_metric("weight", 83.0, today))
        .unwrap();
    db.insert_metric(&common::make_metric("pain", 5.0, today))
        .unwrap();

    let result = context::compute(&db, &config, 7, Some(&["weight"])).unwrap();

    assert!(result.metrics.contains_key("weight"));
    assert!(!result.metrics.contains_key("pain"));
}

#[test]
fn test_context_includes_anomalies() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("heart_rate", 72.0, date))
            .unwrap();
    }
    db.insert_metric(&common::make_metric("heart_rate", 110.0, today))
        .unwrap();

    let result = context::compute(&db, &config, 30, None).unwrap();

    assert!(!result.anomalies.is_empty());
}

#[test]
fn test_context_summary_mentions_key_info() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    for i in 0..7 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 83.0 - i as f64 * 0.3, date))
            .unwrap();
    }

    let result = context::compute(&db, &config, 7, None).unwrap();

    assert!(
        result.summary.contains("1") || result.summary.contains("weight"),
        "summary should reference tracked metrics: {}",
        result.summary
    );
}

#[test]
fn test_context_streaks_included() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    for i in 0..5 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 80.0, date))
            .unwrap();
    }

    let result = context::compute(&db, &config, 7, None).unwrap();
    assert!(result.streaks.logging_days >= 5);
}

#[test]
fn test_context_medication_integration() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();

    use openvital::core::med::AddMedicationParams;
    openvital::core::med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "ibuprofen",
            dose: Some("400mg"),
            freq: "daily",
            route: Some("oral"),
            note: None,
            started: None,
        },
    )
    .unwrap();

    openvital::core::med::take_medication(&db, &config, "ibuprofen", None, None, None, None)
        .unwrap();

    let result = context::compute(&db, &config, 7, None).unwrap();
    assert!(result.medications.is_some());
    assert_eq!(result.medications.as_ref().unwrap().active_count, 1);
}

#[test]
fn test_context_pain_alert_included() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    db.insert_metric(&common::make_metric("pain", 7.0, today))
        .unwrap();

    let result = context::compute(&db, &config, 7, None).unwrap();
    assert!(
        result
            .alerts
            .iter()
            .any(|a| a.alert_type == "pain_elevated"),
        "should include pain alert"
    );
}

#[test]
fn test_context_trend_limited_to_window() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // Old data outside 7-day window: high values
    db.insert_metric(&common::make_metric("weight", 100.0, today - Duration::days(30)))
        .unwrap();
    db.insert_metric(&common::make_metric("weight", 95.0, today - Duration::days(20)))
        .unwrap();

    // Recent data within 7-day window: stable low values
    db.insert_metric(&common::make_metric("weight", 80.0, today - Duration::days(3)))
        .unwrap();
    db.insert_metric(&common::make_metric("weight", 80.0, today - Duration::days(1)))
        .unwrap();

    let result = context::compute(&db, &config, 7, None).unwrap();

    let weight = &result.metrics["weight"];
    // Stats should only count 2 entries within the 7-day window
    assert_eq!(
        weight.stats.count, 2,
        "should only count entries within the 7-day window"
    );
    // Trend should be "stable" (both values are 80.0 within the window)
    // Before fix: trend was "decreasing" because old 100/95 entries were included
    assert_eq!(
        weight.trend.as_ref().unwrap().direction,
        "stable",
        "trend should only use entries within the time window"
    );
}

#[test]
fn test_context_multiple_days_of_data() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // Weight declining from 88.0 (30 days ago) to 85.0 (today)
    for i in 0..30 {
        let date = today - Duration::days(i);
        let weight = 85.0 + i as f64 * 0.1; // older = heavier
        db.insert_metric(&common::make_metric("weight", weight, date))
            .unwrap();
    }

    let result = context::compute(&db, &config, 30, None).unwrap();

    let weight = &result.metrics["weight"];
    assert_eq!(weight.stats.count, 30);
    assert!(weight.trend.is_some());
    assert_eq!(weight.trend.as_ref().unwrap().direction, "decreasing");
}
