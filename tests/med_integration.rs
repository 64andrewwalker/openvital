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
// 8. name_conflict_existing_metric_unchanged
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
