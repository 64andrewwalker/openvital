mod common;

use chrono::Utc;
use openvital::models::med::{Frequency, Medication, Route};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_med(name: &str, freq: Frequency) -> Medication {
    Medication::new(name, freq)
}

fn make_med_full(name: &str, freq: Frequency, route: Route, dose: &str) -> Medication {
    let mut med = Medication::new(name, freq);
    med.route = route;
    med.dose = Some(dose.to_string());
    med.dose_value = Some(400.0);
    med.dose_unit = Some("mg".to_string());
    med.note = Some("test note".to_string());
    med
}

// ---------------------------------------------------------------------------
// Insert and get — verify all fields roundtrip
// ---------------------------------------------------------------------------

#[test]
fn insert_and_get_roundtrip() {
    let (_dir, db) = common::setup_db();
    let med = make_med_full("ibuprofen", Frequency::Daily, Route::Oral, "400mg");

    db.insert_medication(&med).unwrap();
    let got = db.get_medication_by_name("ibuprofen").unwrap().unwrap();

    assert_eq!(got.id, med.id);
    assert_eq!(got.name, "ibuprofen");
    assert_eq!(got.dose.as_deref(), Some("400mg"));
    assert_eq!(got.dose_value, Some(400.0));
    assert_eq!(got.dose_unit.as_deref(), Some("mg"));
    assert_eq!(got.route, Route::Oral);
    assert_eq!(got.frequency, Frequency::Daily);
    assert!(got.active);
    assert_eq!(got.started_at.to_rfc3339(), med.started_at.to_rfc3339());
    assert!(got.stopped_at.is_none());
    assert!(got.stop_reason.is_none());
    assert_eq!(got.note.as_deref(), Some("test note"));
    assert_eq!(got.created_at.to_rfc3339(), med.created_at.to_rfc3339());
}

// ---------------------------------------------------------------------------
// Get nonexistent returns None
// ---------------------------------------------------------------------------

#[test]
fn get_nonexistent_returns_none() {
    let (_dir, db) = common::setup_db();
    let got = db.get_medication_by_name("does-not-exist").unwrap();
    assert!(got.is_none());
}

// ---------------------------------------------------------------------------
// List active only vs list all
// ---------------------------------------------------------------------------

#[test]
fn list_active_only_vs_all() {
    let (_dir, db) = common::setup_db();

    let m1 = make_med("aspirin", Frequency::Daily);
    let m2 = make_med("metformin", Frequency::TwiceDaily);
    db.insert_medication(&m1).unwrap();
    db.insert_medication(&m2).unwrap();

    // Stop aspirin
    db.stop_medication("aspirin", Utc::now(), Some("side effects"))
        .unwrap();

    // Active only should return 1
    let active = db.list_medications(false).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "metformin");

    // All should return 2
    let all = db.list_medications(true).unwrap();
    assert_eq!(all.len(), 2);
}

// ---------------------------------------------------------------------------
// Stop sets inactive, stopped_at, stop_reason
// ---------------------------------------------------------------------------

#[test]
fn stop_sets_inactive_and_timestamps() {
    let (_dir, db) = common::setup_db();

    let med = make_med("lisinopril", Frequency::Daily);
    db.insert_medication(&med).unwrap();

    let stop_time = Utc::now();
    let updated = db
        .stop_medication("lisinopril", stop_time, Some("switched medication"))
        .unwrap();
    assert!(updated);

    // Should not be found by active-only lookup
    let active = db.get_medication_by_name("lisinopril").unwrap();
    assert!(active.is_none());

    // Should be found by any lookup
    let any = db
        .get_medication_by_name_any("lisinopril")
        .unwrap()
        .unwrap();
    assert!(!any.active);
    assert!(any.stopped_at.is_some());
    assert_eq!(any.stop_reason.as_deref(), Some("switched medication"));
}

// ---------------------------------------------------------------------------
// Stop nonexistent returns false
// ---------------------------------------------------------------------------

#[test]
fn stop_nonexistent_returns_false() {
    let (_dir, db) = common::setup_db();
    let result = db.stop_medication("ghost", Utc::now(), None).unwrap();
    assert!(!result);
}

// ---------------------------------------------------------------------------
// Partial unique index: stop then re-add same name
// ---------------------------------------------------------------------------

#[test]
fn stop_then_readd_same_name() {
    let (_dir, db) = common::setup_db();

    let m1 = make_med("aspirin", Frequency::Daily);
    db.insert_medication(&m1).unwrap();
    db.stop_medication("aspirin", Utc::now(), Some("paused"))
        .unwrap();

    // Re-add with same name — should succeed because unique index only covers active=1
    let m2 = make_med("aspirin", Frequency::TwiceDaily);
    db.insert_medication(&m2).unwrap();

    let got = db.get_medication_by_name("aspirin").unwrap().unwrap();
    assert_eq!(got.frequency, Frequency::TwiceDaily);
    assert!(got.active);
}

// ---------------------------------------------------------------------------
// Duplicate active name rejected (DB constraint)
// ---------------------------------------------------------------------------

#[test]
fn duplicate_active_name_rejected() {
    let (_dir, db) = common::setup_db();

    let m1 = make_med("aspirin", Frequency::Daily);
    db.insert_medication(&m1).unwrap();

    let m2 = make_med("aspirin", Frequency::Weekly);
    let result = db.insert_medication(&m2);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Remove deletes record
// ---------------------------------------------------------------------------

#[test]
fn remove_deletes_record() {
    let (_dir, db) = common::setup_db();

    let med = make_med("tylenol", Frequency::AsNeeded);
    db.insert_medication(&med).unwrap();

    let removed = db.remove_medication("tylenol").unwrap();
    assert!(removed);

    // Should be gone completely
    let got = db.get_medication_by_name_any("tylenol").unwrap();
    assert!(got.is_none());

    // Remove again returns false
    let again = db.remove_medication("tylenol").unwrap();
    assert!(!again);
}

// ---------------------------------------------------------------------------
// Route stored and retrieved correctly (including Other variant)
// ---------------------------------------------------------------------------

#[test]
fn route_roundtrip_standard() {
    let (_dir, db) = common::setup_db();

    let mut med = make_med("eye-drops", Frequency::TwiceDaily);
    med.route = Route::Ophthalmic;
    db.insert_medication(&med).unwrap();

    let got = db.get_medication_by_name("eye-drops").unwrap().unwrap();
    assert_eq!(got.route, Route::Ophthalmic);
}

#[test]
fn route_roundtrip_other() {
    let (_dir, db) = common::setup_db();

    let mut med = make_med("suppository-med", Frequency::Daily);
    med.route = Route::Other("rectal".to_string());
    db.insert_medication(&med).unwrap();

    let got = db
        .get_medication_by_name("suppository-med")
        .unwrap()
        .unwrap();
    assert_eq!(got.route, Route::Other("rectal".to_string()));
}

// ---------------------------------------------------------------------------
// get_medication_by_name_any prefers active over stopped
// ---------------------------------------------------------------------------

#[test]
fn get_by_name_any_prefers_active() {
    let (_dir, db) = common::setup_db();

    // Insert and stop first version
    let m1 = make_med("aspirin", Frequency::Daily);
    db.insert_medication(&m1).unwrap();
    db.stop_medication("aspirin", Utc::now(), None).unwrap();

    // Insert new active version
    let m2 = make_med("aspirin", Frequency::TwiceDaily);
    db.insert_medication(&m2).unwrap();

    let got = db.get_medication_by_name_any("aspirin").unwrap().unwrap();
    assert!(got.active);
    assert_eq!(got.frequency, Frequency::TwiceDaily);
}
