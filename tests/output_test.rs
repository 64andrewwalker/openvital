mod common;

use chrono::{NaiveDate, NaiveTime, TimeZone, Utc};
use openvital::core::status::{
    ConsecutivePainAlert, ProfileStatus, StatusData, Streaks, TodayStatus,
};
use openvital::models::Metric;
use openvital::models::config::Units;
use openvital::output::human::{format_metric, format_status};
use openvital::output::{error, success};
use serde_json::json;

// ─── output::success tests ────────────────────────────────────────────────────

/// success() produces a well-formed JSON envelope with status "ok".
#[test]
fn test_success_envelope_structure() {
    let data = json!({"value": 42});
    let result = success("log", data.clone());

    assert_eq!(result["status"], "ok");
    assert_eq!(result["command"], "log");
    assert_eq!(result["data"], data);
    assert!(result["error"].is_null());
}

/// success() with a string payload.
#[test]
fn test_success_with_string_data() {
    let result = success("init", json!("profile created"));
    assert_eq!(result["status"], "ok");
    assert_eq!(result["command"], "init");
    assert_eq!(result["data"], "profile created");
    assert!(result["error"].is_null());
}

/// success() with a null data payload.
#[test]
fn test_success_with_null_data() {
    let result = success("config", json!(null));
    assert_eq!(result["status"], "ok");
    assert_eq!(result["command"], "config");
    assert!(result["data"].is_null());
    assert!(result["error"].is_null());
}

/// success() with an array payload.
#[test]
fn test_success_with_array_data() {
    let data = json!([1, 2, 3]);
    let result = success("show", data.clone());
    assert_eq!(result["status"], "ok");
    assert_eq!(result["data"], data);
}

/// success() with an empty object payload.
#[test]
fn test_success_with_empty_object() {
    let result = success("status", json!({}));
    assert_eq!(result["status"], "ok");
    assert_eq!(result["data"], json!({}));
}

/// success() preserves the command name exactly.
#[test]
fn test_success_command_name_preserved() {
    let result = success("goal_set", json!(null));
    assert_eq!(result["command"], "goal_set");
}

/// success() result is a valid JSON object (not null, not array).
#[test]
fn test_success_result_is_object() {
    let result = success("trend", json!({}));
    assert!(result.is_object());
}

// ─── output::error tests ──────────────────────────────────────────────────────

/// error() produces a well-formed JSON envelope with status "error".
#[test]
fn test_error_envelope_structure() {
    let result = error("log", "DB_ERROR", "database not found");
    assert_eq!(result["status"], "error");
    assert_eq!(result["command"], "log");
    assert!(result["data"].is_null());
    assert_eq!(result["error"]["code"], "DB_ERROR");
    assert_eq!(result["error"]["message"], "database not found");
}

/// error() data field is always null.
#[test]
fn test_error_data_is_null() {
    let result = error("show", "NOT_FOUND", "metric not found");
    assert!(result["data"].is_null());
}

/// error() preserves code and message exactly.
#[test]
fn test_error_code_and_message() {
    let result = error("import", "PARSE_ERROR", "invalid CSV on line 42");
    assert_eq!(result["error"]["code"], "PARSE_ERROR");
    assert_eq!(result["error"]["message"], "invalid CSV on line 42");
}

/// error() result is a valid JSON object.
#[test]
fn test_error_result_is_object() {
    let result = error("export", "IO_ERROR", "cannot write file");
    assert!(result.is_object());
}

/// error() with empty strings still produces valid structure.
#[test]
fn test_error_with_empty_strings() {
    let result = error("", "", "");
    assert_eq!(result["status"], "error");
    assert_eq!(result["command"], "");
    assert_eq!(result["error"]["code"], "");
    assert_eq!(result["error"]["message"], "");
}

/// error() command field matches the given command name.
#[test]
fn test_error_command_preserved() {
    let result = error("trend", "NO_DATA", "no data for period");
    assert_eq!(result["command"], "trend");
}

// ─── Success and error are structurally distinguishable ───────────────────────

#[test]
fn test_success_and_error_have_distinct_status() {
    let ok = success("log", json!({}));
    let err = error("log", "ERR", "fail");
    assert_ne!(ok["status"], err["status"]);
}

/// The envelope always contains exactly the four mandated keys.
#[test]
fn test_envelope_has_required_keys() {
    for envelope in [success("x", json!(1)), error("x", "C", "m")] {
        let obj = envelope.as_object().unwrap();
        assert!(obj.contains_key("status"));
        assert!(obj.contains_key("command"));
        assert!(obj.contains_key("data"));
        assert!(obj.contains_key("error"));
        assert_eq!(obj.len(), 4, "envelope must have exactly 4 keys");
    }
}

// ─── format_metric tests ──────────────────────────────────────────────────────

fn make_test_metric(metric_type: &str, value: f64) -> Metric {
    let dt = NaiveDate::from_ymd_opt(2026, 2, 15)
        .unwrap()
        .and_time(NaiveTime::from_hms_opt(14, 30, 0).unwrap());
    let ts = Utc.from_utc_datetime(&dt);
    let mut m = Metric::new(metric_type.to_string(), value);
    m.timestamp = ts;
    m
}

/// format_metric produces the expected pipe-delimited format.
#[test]
fn test_format_metric_basic() {
    let m = make_test_metric("weight", 85.0);
    let line = format_metric(&m);
    assert!(
        line.contains("2026-02-15 14:30"),
        "should contain timestamp"
    );
    assert!(line.contains("weight"), "should contain metric type");
    assert!(line.contains("85"), "should contain value");
    assert!(line.contains("kg"), "should contain unit");
    assert!(line.contains("|"), "should use pipe separator");
}

/// format_metric does not append note section when note is None.
#[test]
fn test_format_metric_no_note() {
    let m = make_test_metric("pain", 3.0);
    let line = format_metric(&m);
    assert!(!line.contains('#'), "no '#' when note is None");
}

/// format_metric appends the note after '#' when present.
#[test]
fn test_format_metric_with_note() {
    let mut m = make_test_metric("pain", 5.0);
    m.note = Some("lower back".to_string());
    let line = format_metric(&m);
    assert!(line.contains("# lower back"), "note should follow '#'");
}

/// format_metric does not append tag section when tags are empty.
#[test]
fn test_format_metric_no_tags() {
    let m = make_test_metric("cardio", 30.0);
    let line = format_metric(&m);
    assert!(!line.contains('['), "no '[' when tags are empty");
}

/// format_metric appends bracketed comma-joined tags when tags are present.
#[test]
fn test_format_metric_with_tags() {
    let mut m = make_test_metric("cardio", 30.0);
    m.tags = vec!["morning".to_string(), "outdoor".to_string()];
    let line = format_metric(&m);
    assert!(line.contains('['), "should contain '['");
    assert!(line.contains("morning"), "should contain first tag");
    assert!(line.contains("outdoor"), "should contain second tag");
    assert!(line.contains(", "), "tags should be comma-space separated");
    assert!(line.contains(']'), "should contain closing ']'");
}

/// format_metric with both note and tags includes both decorations.
#[test]
fn test_format_metric_with_note_and_tags() {
    let mut m = make_test_metric("strength", 45.0);
    m.note = Some("legs day".to_string());
    m.tags = vec!["gym".to_string()];
    let line = format_metric(&m);
    assert!(line.contains("# legs day"));
    assert!(line.contains("[gym]"));
}

/// format_metric for a custom type with empty unit still formats cleanly.
#[test]
fn test_format_metric_custom_type_empty_unit() {
    let m = make_test_metric("mood", 8.0);
    let line = format_metric(&m);
    assert!(line.contains("mood"));
    assert!(line.contains("8"));
    // unit is empty; line should not have trailing garbage
    assert!(line.contains('|'));
}

// ─── format_status tests ─────────────────────────────────────────────────────

fn make_status(
    date: NaiveDate,
    logged: Vec<String>,
    pain_alerts: Vec<serde_json::Value>,
    logging_days: u32,
    consecutive_pain_alerts: Vec<ConsecutivePainAlert>,
    height_cm: Option<f64>,
    latest_weight_kg: Option<f64>,
    bmi: Option<f64>,
    bmi_category: Option<&'static str>,
) -> StatusData {
    StatusData {
        date,
        profile: ProfileStatus {
            height_cm,
            latest_weight_kg,
            bmi,
            bmi_category,
        },
        today: TodayStatus {
            logged,
            pain_alerts,
        },
        streaks: Streaks { logging_days },
        consecutive_pain_alerts,
    }
}

/// format_status includes the date in the header.
#[test]
fn test_format_status_contains_date() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let s = make_status(date, vec![], vec![], 0, vec![], None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(out.contains("2026-02-15"), "header should contain date");
}

/// format_status shows "No entries logged today" when nothing logged.
#[test]
fn test_format_status_no_entries() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let s = make_status(date, vec![], vec![], 0, vec![], None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(out.contains("No entries logged today"));
}

/// format_status lists logged metric types joined by ", ".
#[test]
fn test_format_status_logged_types() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let logged = vec!["weight".to_string(), "cardio".to_string()];
    let s = make_status(date, logged, vec![], 0, vec![], None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(out.contains("weight"), "should list weight");
    assert!(out.contains("cardio"), "should list cardio");
    assert!(out.contains("Logged today"));
}

/// format_status shows weight and BMI line when both are present.
#[test]
fn test_format_status_weight_and_bmi() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let s = make_status(
        date,
        vec![],
        vec![],
        0,
        vec![],
        Some(175.0),
        Some(75.0),
        Some(24.5),
        Some("normal"),
    );
    let out = format_status(&s, &Units::default());
    assert!(out.contains("75"), "should show weight");
    assert!(
        out.contains("24.5") || out.contains("BMI"),
        "should show BMI"
    );
    assert!(out.contains("normal"), "should show BMI category");
}

/// format_status omits weight/BMI line when values are absent.
#[test]
fn test_format_status_no_weight_no_bmi_line() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let s = make_status(date, vec![], vec![], 0, vec![], None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(!out.contains("BMI"), "BMI line should be absent");
    assert!(!out.contains("kg"), "weight line should be absent");
}

/// format_status shows pain alert count when pain alerts exist.
#[test]
fn test_format_status_pain_alerts_shown() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let pain_alerts = vec![json!({"type": "pain", "value": 7})];
    let s = make_status(
        date,
        vec!["pain".to_string()],
        pain_alerts,
        0,
        vec![],
        None,
        None,
        None,
        None,
    );
    let out = format_status(&s, &Units::default());
    assert!(out.contains("Pain alerts"), "should mention pain alerts");
    assert!(out.contains('1'), "should show count of 1 alert");
}

/// format_status omits pain alert section when there are no alerts.
#[test]
fn test_format_status_no_pain_alerts_section() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let s = make_status(date, vec![], vec![], 0, vec![], None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(!out.contains("Pain alerts"));
}

/// format_status shows logging streak when greater than zero.
#[test]
fn test_format_status_streak_shown() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let s = make_status(date, vec![], vec![], 7, vec![], None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(out.contains("Logging streak"), "should mention streak");
    assert!(out.contains('7'), "should show streak count");
}

/// format_status omits streak line when logging_days is zero.
#[test]
fn test_format_status_streak_zero_omitted() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let s = make_status(date, vec![], vec![], 0, vec![], None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(
        !out.contains("Logging streak"),
        "streak line should be absent when zero"
    );
}

/// format_status shows consecutive pain alert with '!!' prefix.
#[test]
fn test_format_status_consecutive_pain_alert() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let alert = ConsecutivePainAlert {
        metric_type: "pain".to_string(),
        consecutive_days: 4,
        latest_value: 7.0,
    };
    let s = make_status(date, vec![], vec![], 0, vec![alert], None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(
        out.contains("!!"),
        "consecutive pain alert should use '!!' prefix"
    );
    assert!(out.contains("pain"), "should mention metric type");
    assert!(out.contains('4'), "should show consecutive days");
    assert!(out.contains('7'), "should show latest value");
}

/// format_status with multiple consecutive pain alerts lists each one.
#[test]
fn test_format_status_multiple_consecutive_pain_alerts() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let alerts = vec![
        ConsecutivePainAlert {
            metric_type: "pain".to_string(),
            consecutive_days: 3,
            latest_value: 6.0,
        },
        ConsecutivePainAlert {
            metric_type: "soreness".to_string(),
            consecutive_days: 5,
            latest_value: 8.0,
        },
    ];
    let s = make_status(date, vec![], vec![], 0, alerts, None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(out.contains("pain"), "should mention pain");
    assert!(out.contains("soreness"), "should mention soreness");
    // Two '!!' markers expected
    assert_eq!(out.matches("!!").count(), 2);
}

/// format_status output always starts with the '===' header.
#[test]
fn test_format_status_starts_with_header() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let s = make_status(date, vec![], vec![], 0, vec![], None, None, None, None);
    let out = format_status(&s, &Units::default());
    assert!(
        out.starts_with("=== OpenVital Status"),
        "should start with header"
    );
}

/// format_status with all fields populated does not panic and contains all sections.
#[test]
fn test_format_status_full() {
    let date = NaiveDate::from_ymd_opt(2026, 2, 18).unwrap();
    let pain_alerts = vec![json!({"type": "pain", "value": 6})];
    let consecutive = vec![ConsecutivePainAlert {
        metric_type: "pain".to_string(),
        consecutive_days: 3,
        latest_value: 6.0,
    }];
    let s = make_status(
        date,
        vec!["weight".to_string(), "pain".to_string()],
        pain_alerts,
        10,
        consecutive,
        Some(178.0),
        Some(82.0),
        Some(25.9),
        Some("overweight"),
    );
    let out = format_status(&s, &Units::default());
    assert!(out.contains("2026-02-18"));
    assert!(out.contains("82"));
    assert!(out.contains("25.9"));
    assert!(out.contains("overweight"));
    assert!(out.contains("weight"));
    assert!(out.contains("Pain alerts"));
    assert!(out.contains("Logging streak"));
    assert!(out.contains("10"));
    assert!(out.contains("!!"));
}
