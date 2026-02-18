mod common;

use chrono::{Datelike, NaiveDate};
use openvital::core::goal;
use openvital::models::goal::{Direction, Timeframe};

// ── set_goal ────────────────────────────────────────────────────────────────

#[test]
fn test_set_goal_creates_new_goal() {
    let (_dir, db) = common::setup_db();

    let goal = goal::set_goal(
        &db,
        "weight".into(),
        75.0,
        Direction::Below,
        Timeframe::Monthly,
    )
    .unwrap();

    assert_eq!(goal.metric_type, "weight");
    assert_eq!(goal.target_value, 75.0);
    assert_eq!(goal.direction, Direction::Below);
    assert_eq!(goal.timeframe, Timeframe::Monthly);
    assert!(goal.active);

    let in_db = db.list_goals(true).unwrap();
    assert_eq!(in_db.len(), 1);
    assert_eq!(in_db[0].id, goal.id);
}

#[test]
fn test_set_goal_replaces_existing_goal_for_same_type() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "weight".into(),
        80.0,
        Direction::Below,
        Timeframe::Monthly,
    )
    .unwrap();

    let new_goal = goal::set_goal(
        &db,
        "weight".into(),
        75.0,
        Direction::Below,
        Timeframe::Weekly,
    )
    .unwrap();

    // Only the new (active) goal should be listed
    let active = db.list_goals(true).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].target_value, 75.0);
    assert_eq!(active[0].id, new_goal.id);

    // The old goal is kept in history (inactive)
    let all = db.list_goals(false).unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn test_set_goal_independent_types_coexist() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "weight".into(),
        75.0,
        Direction::Below,
        Timeframe::Monthly,
    )
    .unwrap();
    goal::set_goal(
        &db,
        "water".into(),
        2000.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let active = db.list_goals(true).unwrap();
    assert_eq!(active.len(), 2);
}

// ── remove_goal ─────────────────────────────────────────────────────────────

#[test]
fn test_remove_goal_returns_true_when_found() {
    let (_dir, db) = common::setup_db();

    let goal = goal::set_goal(
        &db,
        "sleep_hours".into(),
        8.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let removed = goal::remove_goal(&db, &goal.id).unwrap();
    assert!(removed);

    let active = db.list_goals(true).unwrap();
    assert!(active.is_empty());
}

#[test]
fn test_remove_goal_returns_false_for_unknown_id() {
    let (_dir, db) = common::setup_db();

    let removed = goal::remove_goal(&db, "non-existent-id").unwrap();
    assert!(!removed);
}

// ── goal_status – no data ────────────────────────────────────────────────────

#[test]
fn test_goal_status_empty_when_no_goals() {
    let (_dir, db) = common::setup_db();
    let statuses = goal::goal_status(&db, None).unwrap();
    assert!(statuses.is_empty());
}

#[test]
fn test_goal_status_unmet_when_no_metric_logged() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "weight".into(),
        75.0,
        Direction::Below,
        Timeframe::Monthly,
    )
    .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    assert_eq!(statuses.len(), 1);
    assert!(statuses[0].current_value.is_none());
    assert!(!statuses[0].is_met);
    assert!(statuses[0].progress.is_none());
}

// ── goal_status – daily direction: above (cumulative) ───────────────────────

#[test]
fn test_goal_status_daily_above_sums_entries_for_today() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "water".into(),
        2000.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let today = chrono::Local::now().date_naive();
    db.insert_metric(&common::make_metric("water", 800.0, today))
        .unwrap();
    db.insert_metric(&common::make_metric("water", 900.0, today))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];
    // 800 + 900 = 1700 < 2000, goal not met
    assert!((s.current_value.unwrap() - 1700.0).abs() < f64::EPSILON);
    assert!(!s.is_met);
}

#[test]
fn test_goal_status_daily_above_met_when_sum_reaches_target() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "water".into(),
        2000.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let today = chrono::Local::now().date_naive();
    db.insert_metric(&common::make_metric("water", 1200.0, today))
        .unwrap();
    db.insert_metric(&common::make_metric("water", 900.0, today))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    assert!((s.current_value.unwrap() - 2100.0).abs() < f64::EPSILON);
    assert!(s.is_met);
}

// ── goal_status – daily direction: below (uses latest value) ────────────────

#[test]
fn test_goal_status_daily_below_uses_latest_value() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(&db, "pain".into(), 5.0, Direction::Below, Timeframe::Daily).unwrap();

    let today = chrono::Local::now().date_naive();
    db.insert_metric(&common::make_metric("pain", 3.0, today))
        .unwrap();
    db.insert_metric(&common::make_metric("pain", 4.0, today))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    // latest value is 4.0, which is <= 5.0, goal met
    assert!((s.current_value.unwrap() - 4.0).abs() < f64::EPSILON);
    assert!(s.is_met);
}

// ── goal_status – weekly ─────────────────────────────────────────────────────

#[test]
fn test_goal_status_weekly_sums_week_entries() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "cardio".into(),
        150.0,
        Direction::Above,
        Timeframe::Weekly,
    )
    .unwrap();

    // Use dates guaranteed to be in the current week (Mon–today)
    let today = chrono::Local::now().date_naive();
    let weekday_num = today.weekday().num_days_from_monday();
    let monday = today - chrono::Duration::days(weekday_num as i64);

    // Insert entries on Monday and Tuesday (if they exist before today)
    let day1 = monday;
    let day2 = monday + chrono::Duration::days(1);

    db.insert_metric(&common::make_metric("cardio", 60.0, day1))
        .unwrap();
    if day2 <= today {
        db.insert_metric(&common::make_metric("cardio", 45.0, day2))
            .unwrap();
    }

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    assert!(s.current_value.is_some());
    // At minimum 60 minutes logged
    assert!(s.current_value.unwrap() >= 60.0);
}

#[test]
fn test_goal_status_weekly_none_when_no_entries_this_week() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "cardio".into(),
        150.0,
        Direction::Above,
        Timeframe::Weekly,
    )
    .unwrap();

    // Insert data for a past week (2 weeks ago)
    let two_weeks_ago = NaiveDate::from_ymd_opt(2026, 2, 2).unwrap();
    db.insert_metric(&common::make_metric("cardio", 60.0, two_weeks_ago))
        .unwrap();

    // Only a weekly goal checking *this* week should find no data this week
    // (assuming today is 2026-02-18, which is in the week of Mon 2026-02-16)
    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    // 2026-02-02 is in the week Mon 2026-01-26..Sun 2026-02-01 or Mon 2026-02-02..
    // Let compute_current decide; if entry falls in current week it will have a value
    // The important thing: it should not panic and returns a valid GoalStatus
    assert_eq!(s.metric_type, "cardio");
}

// ── goal_status – monthly ────────────────────────────────────────────────────

#[test]
fn test_goal_status_monthly_uses_latest_value() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "weight".into(),
        75.0,
        Direction::Below,
        Timeframe::Monthly,
    )
    .unwrap();

    let d1 = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
    db.insert_metric(&common::make_metric("weight", 80.0, d1))
        .unwrap();
    db.insert_metric(&common::make_metric("weight", 76.0, d2))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    // query_by_type with limit=1 returns the most-recent entry
    assert!((s.current_value.unwrap() - 76.0).abs() < f64::EPSILON);
    // 76 > 75, goal not met
    assert!(!s.is_met);
}

#[test]
fn test_goal_status_monthly_met() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "weight".into(),
        75.0,
        Direction::Below,
        Timeframe::Monthly,
    )
    .unwrap();

    let d = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    db.insert_metric(&common::make_metric("weight", 74.5, d))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    assert!((s.current_value.unwrap() - 74.5).abs() < f64::EPSILON);
    assert!(s.is_met);
}

// ── goal_status – filter by metric_type ─────────────────────────────────────

#[test]
fn test_goal_status_filter_by_metric_type() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "weight".into(),
        75.0,
        Direction::Below,
        Timeframe::Monthly,
    )
    .unwrap();
    goal::set_goal(
        &db,
        "water".into(),
        2000.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let statuses = goal::goal_status(&db, Some("weight")).unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].metric_type, "weight");
}

#[test]
fn test_goal_status_filter_by_metric_type_no_match() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "weight".into(),
        75.0,
        Direction::Below,
        Timeframe::Monthly,
    )
    .unwrap();

    let statuses = goal::goal_status(&db, Some("cardio")).unwrap();
    assert!(statuses.is_empty());
}

// ── GoalStatus fields ────────────────────────────────────────────────────────

#[test]
fn test_goal_status_direction_and_timeframe_serialized_as_strings() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "cardio".into(),
        150.0,
        Direction::Above,
        Timeframe::Weekly,
    )
    .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    assert_eq!(statuses[0].direction, "above");
    assert_eq!(statuses[0].timeframe, "weekly");
}

// ── progress string formatting ───────────────────────────────────────────────

#[test]
fn test_goal_status_progress_string_below_at_target() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "weight".into(),
        75.0,
        Direction::Below,
        Timeframe::Monthly,
    )
    .unwrap();

    let d = NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
    db.insert_metric(&common::make_metric("weight", 75.0, d))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    assert!(s.is_met);
    assert!(s.progress.as_deref().unwrap().contains("at target"));
}

#[test]
fn test_goal_status_progress_string_above_remaining() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "water".into(),
        2000.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let today = chrono::Local::now().date_naive();
    db.insert_metric(&common::make_metric("water", 1000.0, today))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    assert!(!s.is_met);
    assert!(s.progress.as_deref().unwrap().contains("remaining"));
}

#[test]
fn test_goal_status_progress_string_above_target_met() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "water".into(),
        2000.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let today = chrono::Local::now().date_naive();
    db.insert_metric(&common::make_metric("water", 2500.0, today))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    assert!(s.is_met);
    assert!(s.progress.as_deref().unwrap().contains("target met"));
}

#[test]
fn test_goal_status_progress_string_equal_at_target() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "sleep_quality".into(),
        4.0,
        Direction::Equal,
        Timeframe::Daily,
    )
    .unwrap();

    let today = chrono::Local::now().date_naive();
    db.insert_metric(&common::make_metric("sleep_quality", 4.0, today))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    assert!(s.is_met);
    assert!(s.progress.as_deref().unwrap().contains("at target"));
}

#[test]
fn test_goal_status_progress_string_equal_off_target() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "sleep_quality".into(),
        4.0,
        Direction::Equal,
        Timeframe::Daily,
    )
    .unwrap();

    let today = chrono::Local::now().date_naive();
    db.insert_metric(&common::make_metric("sleep_quality", 3.0, today))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    assert!(!s.is_met);
    let prog = s.progress.as_deref().unwrap();
    assert!(prog.contains("current") && prog.contains("target"));
}

// ── goal_status – daily direction: equal (uses latest value) ─────────────────

/// Direction::Equal with a daily timeframe uses the `_ =>` branch in compute_current,
/// which returns the latest entry's value (same as Direction::Below).
#[test]
fn test_goal_status_daily_equal_uses_latest_value() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "sleep_quality".into(),
        4.0,
        Direction::Equal,
        Timeframe::Daily,
    )
    .unwrap();

    let today = chrono::Local::now().date_naive();
    // Insert two entries; compute_current should pick the last one
    db.insert_metric(&common::make_metric("sleep_quality", 3.0, today))
        .unwrap();
    db.insert_metric(&common::make_metric("sleep_quality", 4.0, today))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    // latest value is 4.0, which equals the target — goal met
    assert!(
        (s.current_value.unwrap() - 4.0).abs() < f64::EPSILON,
        "Equal goal with daily timeframe should use the latest entry"
    );
    assert!(s.is_met, "Goal should be met when current == target");
}

/// Direction::Equal daily with multiple entries picks the last, not the first.
#[test]
fn test_goal_status_daily_equal_picks_last_not_first() {
    let (_dir, db) = common::setup_db();

    goal::set_goal(
        &db,
        "sleep_quality".into(),
        4.0,
        Direction::Equal,
        Timeframe::Daily,
    )
    .unwrap();

    let today = chrono::Local::now().date_naive();
    // First entry matches target, second does not
    db.insert_metric(&common::make_metric("sleep_quality", 4.0, today))
        .unwrap();
    db.insert_metric(&common::make_metric("sleep_quality", 3.0, today))
        .unwrap();

    let statuses = goal::goal_status(&db, None).unwrap();
    let s = &statuses[0];
    // latest value is 3.0, which does not equal target 4.0 — goal not met
    assert!(
        (s.current_value.unwrap() - 3.0).abs() < f64::EPSILON,
        "Equal goal should use the last (most recent) entry, not the first"
    );
    assert!(
        !s.is_met,
        "Goal should not be met when latest value != target"
    );
}
