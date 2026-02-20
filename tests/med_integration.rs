mod common;

use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use openvital::core::export;
use openvital::core::goal;
use openvital::core::med::{self, AddMedicationParams};
use openvital::core::status;
use openvital::core::trend::{self, TrendPeriod};
use openvital::models::config::Config;
use openvital::models::goal::{Direction, Timeframe};
use openvital::models::metric::{Category, Metric};
use uuid::Uuid;

fn default_config() -> Config {
    Config::default()
}

/// Helper: insert a medication metric directly (for controlled timestamps).
fn insert_med_metric(db: &openvital::db::Database, name: &str, date: NaiveDate) {
    let dt = date.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    let ts = Utc.from_utc_datetime(&dt);
    let m = Metric {
        id: Uuid::new_v4().to_string(),
        timestamp: ts,
        category: Category::Medication,
        metric_type: name.to_string(),
        value: 1.0,
        unit: "dose".to_string(),
        note: None,
        tags: Vec::new(),
        source: "med_take".to_string(),
    };
    db.insert_metric(&m).unwrap();
}

// ---------------------------------------------------------------------------
// 1. trend_medication_uses_sum
// ---------------------------------------------------------------------------

#[test]
fn trend_medication_uses_sum() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "ibuprofen",
            dose: Some("400mg"),
            freq: "3x_daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Take 3 times today
    let today = Utc::now().date_naive();
    insert_med_metric(&db, "ibuprofen", today);
    insert_med_metric(&db, "ibuprofen", today);
    insert_med_metric(&db, "ibuprofen", today);

    let result = trend::compute(&db, "ibuprofen", TrendPeriod::Daily, Some(7)).unwrap();
    assert_eq!(result.data.len(), 1);
    // For medications, avg should be sum (3.0), not average (1.0)
    let day = &result.data[0];
    assert!(
        (day.avg - 3.0).abs() < f64::EPSILON,
        "Expected sum=3.0 for medication trend, got {}",
        day.avg
    );
}

// ---------------------------------------------------------------------------
// 2. goal_medication_cumulative
// ---------------------------------------------------------------------------

#[test]
fn goal_medication_cumulative() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "metformin",
            dose: Some("500mg"),
            freq: "2x_daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Take twice today
    med::take_medication(&db, &config, "metformin", None, None, None, None).unwrap();
    med::take_medication(&db, &config, "metformin", None, None, None, None).unwrap();

    // Set goal: above 2 daily
    goal::set_goal(
        &db,
        "metformin".to_string(),
        2.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let statuses = goal::goal_status(&db, Some("metformin")).unwrap();
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];
    // Should use sum (cumulative) = 2.0, not latest = 1.0
    assert_eq!(
        s.current_value,
        Some(2.0),
        "Medication goal should use cumulative sum"
    );
    assert!(s.is_met, "Goal should be met with 2 doses taken");
}

// ---------------------------------------------------------------------------
// 3. status_includes_medications
// ---------------------------------------------------------------------------

#[test]
fn status_includes_medications() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "aspirin",
            dose: Some("100mg"),
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    let status_data = status::compute(&db, &config).unwrap();
    assert!(
        status_data.medications.is_some(),
        "Status should include medications when meds exist"
    );
    let meds = status_data.medications.unwrap();
    assert_eq!(meds.active_count, 1);
}

// ---------------------------------------------------------------------------
// 4. export_default_no_medications
// ---------------------------------------------------------------------------

#[test]
fn export_default_no_medications() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "aspirin",
            dose: None,
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Default export should not include medications key
    let json_str = export::to_json(&db, None, None, None).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    // Default to_json returns an array of metrics, no "medications" key
    assert!(
        parsed.is_array(),
        "Default export should be a plain array of metrics"
    );
}

// ---------------------------------------------------------------------------
// 5. export_with_medications
// ---------------------------------------------------------------------------

#[test]
fn export_with_medications() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "ibuprofen",
            dose: Some("400mg"),
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    med::take_medication(&db, &config, "ibuprofen", None, None, None, None).unwrap();

    let json_str = export::to_json_with_medications(&db, None, None, None).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert!(
        parsed.get("metrics").is_some(),
        "Export with medications should have 'metrics' key"
    );
    assert!(
        parsed.get("medications").is_some(),
        "Export with medications should have 'medications' key"
    );

    let meds = parsed.get("medications").unwrap().as_array().unwrap();
    assert_eq!(meds.len(), 1);
    assert_eq!(meds[0]["name"], "ibuprofen");
}

// ---------------------------------------------------------------------------
// 6. import_auto_detect_new_format
// ---------------------------------------------------------------------------

#[test]
fn import_auto_detect_new_format() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // First, create data in one DB
    let (_dir2, db2) = common::setup_db();

    med::add_medication(
        &db2,
        &config,
        AddMedicationParams {
            name: "aspirin",
            dose: Some("100mg"),
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Log a regular metric
    let m = Metric::new("weight".to_string(), 75.0);
    db2.insert_metric(&m).unwrap();

    // Export with medications
    let json_str = export::to_json_with_medications(&db2, None, None, None).unwrap();

    // Import into the first DB
    let (metric_count, med_count) = export::import_json_auto(&db, &json_str).unwrap();
    assert!(metric_count >= 1, "Should import at least 1 metric");
    assert_eq!(med_count, 1, "Should import 1 medication");

    // Verify medication was imported
    let meds = db.list_medications(true).unwrap();
    assert!(
        meds.iter().any(|m| m.name == "aspirin"),
        "Imported medication should be in DB"
    );
}

// ---------------------------------------------------------------------------
// 7. import_old_format_still_works
// ---------------------------------------------------------------------------

#[test]
fn import_old_format_still_works() {
    let (_dir, db) = common::setup_db();

    // Old format: plain array of metrics
    let old_json = r#"[
        {"type": "weight", "value": 80.0},
        {"type": "sleep_hours", "value": 7.5}
    ]"#;

    let (metric_count, med_count) = export::import_json_auto(&db, old_json).unwrap();
    assert_eq!(metric_count, 2, "Should import 2 metrics from old format");
    assert_eq!(med_count, 0, "No medications in old format");
}

// ---------------------------------------------------------------------------
// 8. goal_medication_monthly_uses_sum
// ---------------------------------------------------------------------------

#[test]
fn goal_medication_monthly_uses_sum() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Add a daily medication
    let params = openvital::core::med::AddMedicationParams {
        name: "vitamin_d",
        dose: Some("1000iu"),
        freq: "daily",
        route: None,
        note: None,
        started: None,
    };
    openvital::core::med::add_medication(&db, &config, params).unwrap();

    // Take it 5 times
    for _ in 0..5 {
        openvital::core::med::take_medication(&db, &config, "vitamin_d", None, None, None, None)
            .unwrap();
    }

    // Set monthly goal: at least 20 intakes
    openvital::core::goal::set_goal(
        &db,
        "vitamin_d".to_string(),
        20.0,
        openvital::models::goal::Direction::Above,
        openvital::models::goal::Timeframe::Monthly,
    )
    .unwrap();

    let statuses = openvital::core::goal::goal_status(&db, Some("vitamin_d")).unwrap();
    assert_eq!(statuses.len(), 1);
    // Should be sum of 5 intakes, not just 1.0
    assert_eq!(statuses[0].current_value, Some(5.0));
    assert!(!statuses[0].is_met); // 5 < 20
}

// ---------------------------------------------------------------------------
// 9. name_conflict_existing_metric_unchanged
// ---------------------------------------------------------------------------

#[test]
fn name_conflict_existing_metric_unchanged() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Log "water" as nutrition (normal metric)
    let water_metric = Metric::new("water".to_string(), 500.0);
    db.insert_metric(&water_metric).unwrap();

    // Add "water" as a medication
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "water",
            dose: Some("1 tablet"),
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Take the "water" medication
    let (med_metric, _) =
        med::take_medication(&db, &config, "water", None, None, None, None).unwrap();

    // The med take metric should be Medication category
    assert_eq!(med_metric.category, Category::Medication);

    // The original water metric should still be Nutrition
    let all_water = db.query_by_type("water", Some(10)).unwrap();
    let nutrition_waters: Vec<_> = all_water
        .iter()
        .filter(|m| m.category == Category::Nutrition)
        .collect();
    assert_eq!(
        nutrition_waters.len(),
        1,
        "Original nutrition water metric should be unchanged"
    );

    let medication_waters: Vec<_> = all_water
        .iter()
        .filter(|m| m.category == Category::Medication)
        .collect();
    assert_eq!(
        medication_waters.len(),
        1,
        "Med take should create medication category entry"
    );
}

// ---------------------------------------------------------------------------
// 9. correlate_medication_uses_daily_sum
// ---------------------------------------------------------------------------

#[test]
fn correlate_medication_uses_daily_sum() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Add medication
    let params = med::AddMedicationParams {
        name: "aspirin",
        dose: Some("100mg"),
        freq: "3x_daily",
        route: None,
        note: None,
        started: None,
    };
    med::add_medication(&db, &config, params).unwrap();

    let today = Utc::now().date_naive();

    // Take aspirin 3 times today and log pain
    for _ in 0..3 {
        med::take_medication(&db, &config, "aspirin", None, None, None, None).unwrap();
    }
    // Log a pain value
    let entry = openvital::core::logging::LogEntry {
        metric_type: "pain",
        value: 5.0,
        note: None,
        tags: None,
        source: None,
        date: None,
    };
    openvital::core::logging::log_metric(&db, &config, entry).unwrap();

    // Need at least 3 data points for correlation
    // Log 2 more days with different amounts
    let day1 = today - chrono::Duration::days(1);
    let day2 = today - chrono::Duration::days(2);

    for day in [day1, day2] {
        // Take aspirin and log pain for each day
        med::take_medication(&db, &config, "aspirin", None, None, None, Some(day)).unwrap();
        let entry = openvital::core::logging::LogEntry {
            metric_type: "pain",
            value: 3.0,
            note: None,
            tags: None,
            source: None,
            date: Some(day),
        };
        openvital::core::logging::log_metric(&db, &config, entry).unwrap();
    }

    // Run correlation
    let result = trend::correlate(&db, "aspirin", "pain", Some(7)).unwrap();

    // The aspirin daily sums should be: today=3, day1=1, day2=1
    // This should NOT be: today=1, day1=1, day2=1 (which would mean "no correlation")
    // With different sums, correlation should detect something
    assert_ne!(result.interpretation, "insufficient data");
    // Just verify it computed without error - the specific coefficient depends on pain values
}

// ===========================================================================
// Bug 1: Name collision between medication and non-medication metrics
// ===========================================================================

// ---------------------------------------------------------------------------
// 10. trend_excludes_medication_entries_for_non_med_metric
// ---------------------------------------------------------------------------

#[test]
fn trend_excludes_medication_entries_for_non_med_metric() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Log mood=4 as a regular (non-medication) metric
    let entry = openvital::core::logging::LogEntry {
        metric_type: "mood",
        value: 4.0,
        note: None,
        tags: None,
        source: None,
        date: None,
    };
    openvital::core::logging::log_metric(&db, &config, entry).unwrap();

    // Add a medication also named "mood"
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "mood",
            dose: Some("10mg"),
            freq: "2x_daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Take the "mood" medication twice (creates entries with value=1.0, Category::Medication)
    med::take_medication(&db, &config, "mood", None, None, None, None).unwrap();
    med::take_medication(&db, &config, "mood", None, None, None, None).unwrap();

    // Run trend for "mood" — should only see the non-medication entry
    let result = trend::compute(&db, "mood", TrendPeriod::Daily, Some(7)).unwrap();
    assert_eq!(result.data.len(), 1, "Should have exactly 1 day of data");

    let day = &result.data[0];
    assert!(
        (day.avg - 4.0).abs() < f64::EPSILON,
        "Trend avg should be 4.0 (non-med only), got {}",
        day.avg
    );
}

// ---------------------------------------------------------------------------
// 11. goal_excludes_medication_entries_for_non_med_metric
// ---------------------------------------------------------------------------

#[test]
fn goal_excludes_medication_entries_for_non_med_metric() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Log mood=4 as a regular metric
    let entry = openvital::core::logging::LogEntry {
        metric_type: "mood",
        value: 4.0,
        note: None,
        tags: None,
        source: None,
        date: None,
    };
    openvital::core::logging::log_metric(&db, &config, entry).unwrap();

    // Add medication named "mood" and take it twice
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "mood",
            dose: Some("10mg"),
            freq: "2x_daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();
    med::take_medication(&db, &config, "mood", None, None, None, None).unwrap();
    med::take_medication(&db, &config, "mood", None, None, None, None).unwrap();

    // Set goal: mood above 3 daily
    goal::set_goal(
        &db,
        "mood".to_string(),
        3.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let statuses = goal::goal_status(&db, Some("mood")).unwrap();
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];
    // current_value should be 4.0 (the non-med entry), NOT 6.0 (4+1+1)
    assert_eq!(
        s.current_value,
        Some(4.0),
        "Goal should use non-med mood entry (4.0), not mixed with med entries"
    );
    assert!(s.is_met, "Goal should be met: 4.0 > 3.0");
}

// ---------------------------------------------------------------------------
// 12. goal_for_medication_excludes_non_med_entries
// ---------------------------------------------------------------------------

#[test]
fn goal_for_medication_excludes_non_med_entries() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Log mood=4 as a regular metric first
    let entry = openvital::core::logging::LogEntry {
        metric_type: "mood",
        value: 4.0,
        note: None,
        tags: None,
        source: None,
        date: None,
    };
    openvital::core::logging::log_metric(&db, &config, entry).unwrap();

    // Add medication named "mood" and take it twice
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "mood",
            dose: Some("10mg"),
            freq: "2x_daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();
    med::take_medication(&db, &config, "mood", None, None, None, None).unwrap();
    med::take_medication(&db, &config, "mood", None, None, None, None).unwrap();

    // Set goal for mood above 3 daily — since non-med entries exist,
    // the goal will be treated as non-med, so current_value = 4.0
    goal::set_goal(
        &db,
        "mood".to_string(),
        3.0,
        Direction::Above,
        Timeframe::Daily,
    )
    .unwrap();

    let statuses = goal::goal_status(&db, Some("mood")).unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(
        statuses[0].current_value,
        Some(4.0),
        "With both med and non-med entries, goal uses non-med value (4.0)"
    );
}

// ===========================================================================
// Bug 2: Weekly adherence consistency
// ===========================================================================

// ---------------------------------------------------------------------------
// 13. weekly_med_no_takes_shows_not_adherent
// ---------------------------------------------------------------------------

#[test]
fn weekly_med_no_takes_shows_not_adherent() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Add weekly medication started a few days ago
    let started = Utc::now().date_naive() - chrono::Duration::days(3);
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "weekly_vitamin",
            dose: Some("5000iu"),
            freq: "weekly",
            route: None,
            note: None,
            started: Some(started),
        },
    )
    .unwrap();

    // No takes at all — check adherence
    let statuses = med::adherence_status(&db, Some("weekly_vitamin"), 7).unwrap();
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];

    assert_eq!(
        s.adherent_today,
        Some(false),
        "Weekly med with no takes should be NOT adherent, got {:?}",
        s.adherent_today
    );
    assert_eq!(
        s.adherence_7d,
        Some(0.0),
        "Weekly med with 0 takes should have 0% 7d adherence, got {:?}",
        s.adherence_7d
    );
}

// ---------------------------------------------------------------------------
// 14. weekly_med_one_take_shows_adherent
// ---------------------------------------------------------------------------

#[test]
fn weekly_med_one_take_shows_adherent() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Add weekly medication
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "weekly_iron",
            dose: Some("65mg"),
            freq: "weekly",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Take it once today
    med::take_medication(&db, &config, "weekly_iron", None, None, None, None).unwrap();

    // Check adherence
    let statuses = med::adherence_status(&db, Some("weekly_iron"), 7).unwrap();
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];

    assert_eq!(
        s.adherent_today,
        Some(true),
        "Weekly med with 1 take this week should be adherent"
    );
    assert_eq!(
        s.streak_days,
        Some(1),
        "Weekly med with 1 take should have streak of 1 (one week)"
    );
}

// ---------------------------------------------------------------------------
// 15. weekly_adherence_history_uses_weekly_entries
// ---------------------------------------------------------------------------

#[test]
fn weekly_adherence_history_uses_weekly_entries() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Add weekly medication started 3 weeks ago
    let started = Utc::now().date_naive() - chrono::Duration::days(21);
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "weekly_b12",
            dose: Some("1000mcg"),
            freq: "weekly",
            route: None,
            note: None,
            started: Some(started),
        },
    )
    .unwrap();

    // Take it once today
    med::take_medication(&db, &config, "weekly_b12", None, None, None, None).unwrap();

    // Check single-med adherence with last=14 days
    let statuses = med::adherence_status(&db, Some("weekly_b12"), 14).unwrap();
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];

    let history = s.adherence_history.as_ref().expect("Should have history");

    // History entries should have required=1 for weekly meds
    for entry in history {
        assert_eq!(
            entry.required, 1,
            "Weekly history entries should have required=1, got {}",
            entry.required
        );
    }

    // Should NOT have 14 daily entries — should have weekly entries instead
    // 14 days = 2 weeks, so at most 2-3 weekly entries (depending on start date)
    assert!(
        history.len() <= 4,
        "Weekly history should have weekly entries (got {} entries, expected <= 4 for ~14 days)",
        history.len()
    );
}

// ===========================================================================
// Bug 3: Status missed formatting for weekly meds
// ===========================================================================

// ---------------------------------------------------------------------------
// 16. status_weekly_missed_format_no_zero_slash_zero
// ---------------------------------------------------------------------------

#[test]
fn status_weekly_missed_format_no_zero_slash_zero() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Add weekly medication, no takes
    let started = Utc::now().date_naive() - chrono::Duration::days(3);
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "weekly_test",
            dose: Some("100mg"),
            freq: "weekly",
            route: None,
            note: None,
            started: Some(started),
        },
    )
    .unwrap();

    // Compute status
    let status_data = status::compute(&db, &config).unwrap();
    let meds = status_data
        .medications
        .expect("Should have medication status");

    // The missed list should exist and contain our weekly med
    assert!(
        !meds.missed.is_empty(),
        "Missed list should contain the weekly med with no takes"
    );

    // Verify the missed entry does NOT contain "0/0" pattern
    for entry in &meds.missed {
        assert!(
            !entry.contains("0/0"),
            "Missed entry should not contain '0/0', got: {}",
            entry
        );
    }

    // Verify the missed entry uses "taken this week" format for weekly meds
    let weekly_entry = meds
        .missed
        .iter()
        .find(|e| e.contains("weekly_test"))
        .expect("Should have weekly_test in missed list");
    assert!(
        weekly_entry.contains("taken this week"),
        "Weekly missed entry should say 'taken this week', got: {}",
        weekly_entry
    );
}

// ===========================================================================
// Fix: correlate is_med_b guard consistency
// ===========================================================================

#[test]
fn correlate_name_conflict_b_side_uses_non_med() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Log "mood" as a regular metric on 3 separate days
    for i in 0..3 {
        let day = Utc::now().date_naive() - chrono::Duration::days(i);
        let entry = openvital::core::logging::LogEntry {
            metric_type: "mood",
            value: 5.0 + i as f64,
            note: None,
            tags: None,
            source: None,
            date: Some(day),
        };
        openvital::core::logging::log_metric(&db, &config, entry).unwrap();
    }

    // Add medication named "mood" and take it on same days
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "mood",
            dose: Some("10mg"),
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();
    for i in 0..3 {
        let day = Utc::now().date_naive() - chrono::Duration::days(i);
        med::take_medication(&db, &config, "mood", None, None, None, Some(day)).unwrap();
    }

    // Log "pain" on same 3 days
    for i in 0..3 {
        let day = Utc::now().date_naive() - chrono::Duration::days(i);
        let entry = openvital::core::logging::LogEntry {
            metric_type: "pain",
            value: 3.0,
            note: None,
            tags: None,
            source: None,
            date: Some(day),
        };
        openvital::core::logging::log_metric(&db, &config, entry).unwrap();
    }

    // correlate pain,mood — mood is on the B side, has both med and non-med entries
    // Should use non-med values (5.0, 6.0, 7.0), not medication values (1.0)
    let result = trend::correlate(&db, "pain", "mood", Some(7)).unwrap();
    assert_ne!(
        result.interpretation, "insufficient data",
        "Should have enough data points"
    );
    assert_eq!(result.data_points, 3);
}

// ===========================================================================
// Fix: overall_adherence_7d assertion
// ===========================================================================

#[test]
fn status_overall_adherence_7d_computed_correctly() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Add two daily meds + one as_needed
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "med_a",
            dose: None,
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "med_b",
            dose: None,
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "prn_med",
            dose: None,
            freq: "as_needed",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Take med_a today (adherent), skip med_b (not adherent)
    med::take_medication(&db, &config, "med_a", None, None, None, None).unwrap();

    let status_data = status::compute(&db, &config).unwrap();
    let meds = status_data.medications.expect("Should have medications");

    // overall_adherence_7d should average med_a (1.0) and med_b (0.0), excluding as_needed
    // Since both started today, each has only 1 eligible day
    assert!(
        meds.overall_adherence_7d.is_some(),
        "Should have overall adherence value"
    );
    let overall = meds.overall_adherence_7d.unwrap();
    assert!(
        (overall - 0.5).abs() < 0.01,
        "Overall adherence should be ~0.5 (avg of 1.0 and 0.0), got {}",
        overall
    );
}

// ===========================================================================
// Fix: add_medication with --started past date
// ===========================================================================

#[test]
fn add_medication_with_started_date() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let past = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let params = AddMedicationParams {
        name: "old_med",
        dose: Some("100mg"),
        freq: "daily",
        route: None,
        note: None,
        started: Some(past),
    };
    let med = med::add_medication(&db, &config, params).unwrap();
    assert_eq!(
        med.started_at.date_naive(),
        past,
        "started_at should match the provided date"
    );
}

// ===========================================================================
// Bug 4: Med list --all header
// ===========================================================================

// ---------------------------------------------------------------------------
// 17. med_list_all_header
// ---------------------------------------------------------------------------

#[test]
fn med_list_all_header() {
    use openvital::output::human::format_med_list;

    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Create two medications
    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "active_med",
            dose: Some("50mg"),
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "stopped_med",
            dose: Some("25mg"),
            freq: "daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Stop one medication
    med::stop_medication(&db, "stopped_med", Some("no longer needed"), None).unwrap();

    // Format with include_stopped=true → should show "All Medications"
    let all_meds = db.list_medications(true).unwrap();
    let output_all = format_med_list(&all_meds, true);
    assert!(
        output_all.starts_with("All Medications"),
        "include_stopped=true should show 'All Medications' header, got: {}",
        output_all.lines().next().unwrap_or("")
    );

    // Format with include_stopped=false → should show "Active Medications"
    let active_meds = db.list_medications(false).unwrap();
    let output_active = format_med_list(&active_meds, false);
    assert!(
        output_active.starts_with("Active Medications"),
        "include_stopped=false should show 'Active Medications' header, got: {}",
        output_active.lines().next().unwrap_or("")
    );
}
