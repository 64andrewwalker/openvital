mod common;

use chrono::NaiveDate;
use openvital::core::query::{ShowResult, show};
use openvital::models::config::Config;

fn default_config() -> Config {
    Config::default()
}

// ── show – no args → today's entries ────────────────────────────────────────

#[test]
fn test_show_no_args_returns_today() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let today = chrono::Local::now().date_naive();
    db.insert_metric(&common::make_metric("weight", 80.0, today))
        .unwrap();

    let result = show(&db, &config, None, None, None).unwrap();

    match result {
        ShowResult::ByDate { date, entries } => {
            assert_eq!(date, today);
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].metric_type, "weight");
        }
        ShowResult::ByType { .. } => panic!("expected ByDate"),
    }
}

#[test]
fn test_show_no_args_empty_today_returns_empty_list() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Insert data for a past date, not today
    let past = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    db.insert_metric(&common::make_metric("weight", 80.0, past))
        .unwrap();

    let result = show(&db, &config, None, None, None).unwrap();

    match result {
        ShowResult::ByDate { entries, .. } => assert!(entries.is_empty()),
        ShowResult::ByType { .. } => panic!("expected ByDate"),
    }
}

// ── show "today" keyword ──────────────────────────────────────────────────────

#[test]
fn test_show_today_keyword_returns_by_date() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let today = chrono::Local::now().date_naive();
    db.insert_metric(&common::make_metric("pain", 3.0, today))
        .unwrap();

    let result = show(&db, &config, Some("today"), None, None).unwrap();

    match result {
        ShowResult::ByDate { date, entries } => {
            assert_eq!(date, today);
            assert_eq!(entries.len(), 1);
        }
        ShowResult::ByType { .. } => panic!("expected ByDate"),
    }
}

#[test]
fn test_show_today_keyword_with_explicit_date_uses_that_date() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let specific_date = NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
    db.insert_metric(&common::make_metric("pain", 5.0, specific_date))
        .unwrap();

    // "today" keyword but with an explicit date override
    let result = show(&db, &config, Some("today"), None, Some(specific_date)).unwrap();

    match result {
        ShowResult::ByDate { date, entries } => {
            assert_eq!(date, specific_date);
            assert_eq!(entries.len(), 1);
        }
        ShowResult::ByType { .. } => panic!("expected ByDate"),
    }
}

// ── show – explicit date filter ───────────────────────────────────────────────

#[test]
fn test_show_with_explicit_date_returns_by_date() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let d1 = NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 2, 11).unwrap();

    db.insert_metric(&common::make_metric("weight", 80.0, d1))
        .unwrap();
    db.insert_metric(&common::make_metric("weight", 79.5, d2))
        .unwrap();

    let result = show(&db, &config, None, None, Some(d1)).unwrap();

    match result {
        ShowResult::ByDate { date, entries } => {
            assert_eq!(date, d1);
            assert_eq!(entries.len(), 1);
            assert!((entries[0].value - 80.0).abs() < f64::EPSILON);
        }
        ShowResult::ByType { .. } => panic!("expected ByDate"),
    }
}

#[test]
fn test_show_with_date_returns_all_types_for_that_day() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let d = NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
    db.insert_metric(&common::make_metric("weight", 80.0, d))
        .unwrap();
    db.insert_metric(&common::make_metric("water", 1500.0, d))
        .unwrap();
    db.insert_metric(&common::make_metric("pain", 2.0, d))
        .unwrap();

    let result = show(&db, &config, None, None, Some(d)).unwrap();

    match result {
        ShowResult::ByDate { entries, .. } => assert_eq!(entries.len(), 3),
        ShowResult::ByType { .. } => panic!("expected ByDate"),
    }
}

// ── show – by metric type ─────────────────────────────────────────────────────

#[test]
fn test_show_by_type_returns_by_type_variant() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let d = NaiveDate::from_ymd_opt(2026, 2, 5).unwrap();
    db.insert_metric(&common::make_metric("weight", 82.0, d))
        .unwrap();

    let result = show(&db, &config, Some("weight"), None, None).unwrap();

    match result {
        ShowResult::ByType {
            metric_type,
            entries,
        } => {
            assert_eq!(metric_type, "weight");
            assert_eq!(entries.len(), 1);
            assert!((entries[0].value - 82.0).abs() < f64::EPSILON);
        }
        ShowResult::ByDate { .. } => panic!("expected ByType"),
    }
}

#[test]
fn test_show_by_type_empty_when_no_entries() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let result = show(&db, &config, Some("weight"), None, None).unwrap();

    match result {
        ShowResult::ByType { entries, .. } => assert!(entries.is_empty()),
        ShowResult::ByDate { .. } => panic!("expected ByType"),
    }
}

// ── show – last N limit ───────────────────────────────────────────────────────

#[test]
fn test_show_by_type_default_last_is_one() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    for (d, v) in [
        (NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(), 83.0),
        (NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(), 82.5),
        (NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(), 82.0),
    ] {
        db.insert_metric(&common::make_metric("weight", v, d))
            .unwrap();
    }

    // No `last` param → defaults to 1
    let result = show(&db, &config, Some("weight"), None, None).unwrap();

    match result {
        ShowResult::ByType { entries, .. } => {
            assert_eq!(entries.len(), 1);
            // query_by_type returns DESC, so most-recent first
            assert!((entries[0].value - 82.0).abs() < f64::EPSILON);
        }
        ShowResult::ByDate { .. } => panic!("expected ByType"),
    }
}

#[test]
fn test_show_by_type_with_last_n() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    for (d, v) in [
        (NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(), 83.0),
        (NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(), 82.5),
        (NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(), 82.0),
    ] {
        db.insert_metric(&common::make_metric("weight", v, d))
            .unwrap();
    }

    let result = show(&db, &config, Some("weight"), Some(3), None).unwrap();

    match result {
        ShowResult::ByType { entries, .. } => assert_eq!(entries.len(), 3),
        ShowResult::ByDate { .. } => panic!("expected ByType"),
    }
}

#[test]
fn test_show_by_type_last_exceeds_available_returns_all() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let d = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    db.insert_metric(&common::make_metric("weight", 83.0, d))
        .unwrap();

    // Request more than available
    let result = show(&db, &config, Some("weight"), Some(10), None).unwrap();

    match result {
        ShowResult::ByType { entries, .. } => assert_eq!(entries.len(), 1),
        ShowResult::ByDate { .. } => panic!("expected ByType"),
    }
}

// ── show – alias resolution ───────────────────────────────────────────────────

#[test]
fn test_show_resolves_alias_to_canonical_type() {
    let (_dir, db) = common::setup_db();
    let mut config = default_config();
    config.aliases = Config::default_aliases();

    let d = NaiveDate::from_ymd_opt(2026, 2, 8).unwrap();
    db.insert_metric(&common::make_metric("weight", 81.0, d))
        .unwrap();

    // "w" is the alias for "weight"
    let result = show(&db, &config, Some("w"), None, None).unwrap();

    match result {
        ShowResult::ByType {
            metric_type,
            entries,
        } => {
            assert_eq!(metric_type, "weight");
            assert_eq!(entries.len(), 1);
        }
        ShowResult::ByDate { .. } => panic!("expected ByType"),
    }
}

// ── show – multiple metric types on the same day ──────────────────────────────

#[test]
fn test_show_by_type_only_returns_matching_type() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let d = NaiveDate::from_ymd_opt(2026, 2, 12).unwrap();
    db.insert_metric(&common::make_metric("weight", 80.0, d))
        .unwrap();
    db.insert_metric(&common::make_metric("water", 1200.0, d))
        .unwrap();

    let result = show(&db, &config, Some("water"), Some(5), None).unwrap();

    match result {
        ShowResult::ByType {
            metric_type,
            entries,
        } => {
            assert_eq!(metric_type, "water");
            assert_eq!(entries.len(), 1);
            assert!((entries[0].value - 1200.0).abs() < f64::EPSILON);
        }
        ShowResult::ByDate { .. } => panic!("expected ByType"),
    }
}

// ── show – custom type not in known list ──────────────────────────────────────

#[test]
fn test_show_by_type_works_for_custom_type() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let d = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
    db.insert_metric(&common::make_metric("my_custom_metric", 42.0, d))
        .unwrap();

    let result = show(&db, &config, Some("my_custom_metric"), Some(5), None).unwrap();

    match result {
        ShowResult::ByType {
            metric_type,
            entries,
        } => {
            assert_eq!(metric_type, "my_custom_metric");
            assert_eq!(entries.len(), 1);
        }
        ShowResult::ByDate { .. } => panic!("expected ByType"),
    }
}
