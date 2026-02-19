mod common;

use openvital::core::med::{self, AddMedicationParams};
use openvital::models::config::Config;
use openvital::models::med::Frequency;
use openvital::models::metric::Category;

fn default_config() -> Config {
    Config::default()
}

// ---------------------------------------------------------------------------
// 1. add_medication_basic
// ---------------------------------------------------------------------------

#[test]
fn add_medication_basic() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let m = med::add_medication(
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

    assert_eq!(m.name, "ibuprofen");
    assert_eq!(m.dose.as_deref(), Some("400mg"));
    assert_eq!(m.dose_value, Some(400.0));
    assert_eq!(m.dose_unit.as_deref(), Some("mg"));
    assert_eq!(m.frequency, Frequency::Daily);
    assert_eq!(m.route.to_string(), "oral");
    assert!(m.active);
    assert!(!m.id.is_empty());
}

// ---------------------------------------------------------------------------
// 2. add_medication_topical_with_note
// ---------------------------------------------------------------------------

#[test]
fn add_medication_topical_with_note() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let m = med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "hydrocortisone",
            dose: Some("thin layer"),
            freq: "2x_daily",
            route: Some("topical"),
            note: Some("apply to affected area"),
            started: None,
        },
    )
    .unwrap();

    assert_eq!(m.name, "hydrocortisone");
    assert_eq!(m.dose.as_deref(), Some("thin layer"));
    assert_eq!(m.dose_value, None);
    assert_eq!(m.dose_unit.as_deref(), Some("application"));
    assert_eq!(m.route.to_string(), "topical");
    assert_eq!(m.frequency, Frequency::TwiceDaily);
    assert_eq!(m.note.as_deref(), Some("apply to affected area"));
}

// ---------------------------------------------------------------------------
// 3. add_duplicate_active_errors
// ---------------------------------------------------------------------------

#[test]
fn add_duplicate_active_errors() {
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

    let result = med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "aspirin",
            dose: None,
            freq: "weekly",
            route: None,
            note: None,
            started: None,
        },
    );
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("already") || err_msg.contains("active"),
        "Error should mention already active: {err_msg}"
    );
}

// ---------------------------------------------------------------------------
// 4. add_after_stop_allowed
// ---------------------------------------------------------------------------

#[test]
fn add_after_stop_allowed() {
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
    med::stop_medication(&db, "aspirin", None, None).unwrap();

    let m = med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "aspirin",
            dose: None,
            freq: "2x_daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();
    assert_eq!(m.frequency, Frequency::TwiceDaily);
    assert!(m.active);
}

// ---------------------------------------------------------------------------
// 5. take_creates_metric_with_count_semantics
// ---------------------------------------------------------------------------

#[test]
fn take_creates_metric_with_count_semantics() {
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

    let (metric, medication) =
        med::take_medication(&db, &config, "ibuprofen", None, None, None, None).unwrap();

    assert!((metric.value - 1.0).abs() < f64::EPSILON);
    assert_eq!(metric.unit, "dose");
    assert_eq!(metric.category, Category::Medication);
    assert_eq!(metric.source, "med_take");
    assert_eq!(metric.metric_type, "ibuprofen");
    assert_eq!(medication.name, "ibuprofen");

    // Note should contain the medication dose
    assert_eq!(metric.note.as_deref(), Some("400mg"));
}

// ---------------------------------------------------------------------------
// 6. take_with_dose_override
// ---------------------------------------------------------------------------

#[test]
fn take_with_dose_override() {
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

    let (metric, _) =
        med::take_medication(&db, &config, "ibuprofen", Some("200mg"), None, None, None).unwrap();

    assert!(
        metric.note.as_deref().unwrap().contains("200mg"),
        "Note should contain dose override"
    );
    assert!(
        metric.note.as_deref().unwrap().contains("override"),
        "Note should indicate override"
    );
}

// ---------------------------------------------------------------------------
// 7. take_unknown_medication_errors
// ---------------------------------------------------------------------------

#[test]
fn take_unknown_medication_errors() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    let result = med::take_medication(&db, &config, "nonexistent", None, None, None, None);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not found"),
        "Error should say not found: {err_msg}"
    );
}

// ---------------------------------------------------------------------------
// 8. take_stopped_medication_succeeds
// ---------------------------------------------------------------------------

#[test]
fn take_stopped_medication_succeeds() {
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
    med::stop_medication(&db, "aspirin", Some("side effects"), None).unwrap();

    let (metric, _) =
        med::take_medication(&db, &config, "aspirin", None, None, None, None).unwrap();

    assert_eq!(metric.metric_type, "aspirin");
    assert!((metric.value - 1.0).abs() < f64::EPSILON);
    // Note should indicate stopped
    let note = metric.note.as_deref().unwrap_or("");
    assert!(
        note.contains("stopped"),
        "Note should mention stopped: {note}"
    );
}

// ---------------------------------------------------------------------------
// 9. take_resolves_alias
// ---------------------------------------------------------------------------

#[test]
fn take_resolves_alias() {
    let (_dir, db) = common::setup_db();
    let mut config = default_config();
    config
        .aliases
        .insert("ibu".to_string(), "ibuprofen".to_string());

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

    let (metric, _) = med::take_medication(&db, &config, "ibu", None, None, None, None).unwrap();
    assert_eq!(metric.metric_type, "ibuprofen");
}

// ---------------------------------------------------------------------------
// 10. stop_medication_with_reason
// ---------------------------------------------------------------------------

#[test]
fn stop_medication_with_reason() {
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

    let stopped = med::stop_medication(&db, "aspirin", Some("side effects"), None).unwrap();
    assert!(stopped);

    // Verify medication is stopped
    let meds = med::list_medications(&db, true).unwrap();
    let aspirin = meds.iter().find(|m| m.name == "aspirin").unwrap();
    assert!(!aspirin.active);
    assert_eq!(aspirin.stop_reason.as_deref(), Some("side effects"));
}

// ---------------------------------------------------------------------------
// 11. remove_preserves_metric_entries
// ---------------------------------------------------------------------------

#[test]
fn remove_preserves_metric_entries() {
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

    // Take it once
    med::take_medication(&db, &config, "ibuprofen", None, None, None, None).unwrap();

    // Remove the medication
    let removed = med::remove_medication(&db, "ibuprofen").unwrap();
    assert!(removed);

    // Medication should be gone
    let meds = med::list_medications(&db, true).unwrap();
    assert!(meds.iter().all(|m| m.name != "ibuprofen"));

    // But metric entries should remain
    let metrics = db.query_by_type("ibuprofen", Some(10)).unwrap();
    assert_eq!(metrics.len(), 1);
}

// ---------------------------------------------------------------------------
// 12. adherence_daily_med
// ---------------------------------------------------------------------------

#[test]
fn adherence_daily_med() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "metformin",
            dose: None,
            freq: "2x_daily",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    // Take once today
    med::take_medication(&db, &config, "metformin", None, None, None, None).unwrap();

    let statuses = med::adherence_status(&db, Some("metformin"), 7).unwrap();
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];
    assert_eq!(s.name, "metformin");
    assert_eq!(s.required_today, Some(2));
    assert_eq!(s.taken_today, 1);
    // Not adherent because took 1 of required 2
    assert_eq!(s.adherent_today, Some(false));
}

// ---------------------------------------------------------------------------
// 13. adherence_as_needed_null
// ---------------------------------------------------------------------------

#[test]
fn adherence_as_needed_null() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(
        &db,
        &config,
        AddMedicationParams {
            name: "tylenol",
            dose: None,
            freq: "as_needed",
            route: None,
            note: None,
            started: None,
        },
    )
    .unwrap();

    let statuses = med::adherence_status(&db, Some("tylenol"), 7).unwrap();
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];
    assert!(s.required_today.is_none());
    assert!(s.adherent_today.is_none());
    assert!(s.streak_days.is_none());
    assert!(s.adherence_7d.is_none());
}

// ---------------------------------------------------------------------------
// 14. name_conflict_category — "water" med take creates Medication category
// ---------------------------------------------------------------------------

#[test]
fn name_conflict_category() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Add a medication named "water"
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

    let (metric, _) = med::take_medication(&db, &config, "water", None, None, None, None).unwrap();

    // The med take should create a Medication category, not Nutrition
    assert_eq!(metric.category, Category::Medication);
}

// ---------------------------------------------------------------------------
// 15. from_type_unchanged — Category::from_type("water") still returns Nutrition
// ---------------------------------------------------------------------------

#[test]
fn from_type_unchanged() {
    // Ensure the generic from_type hasn't been altered
    assert_eq!(Category::from_type("water"), Category::Nutrition);
}
