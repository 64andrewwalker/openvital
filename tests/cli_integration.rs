/// CLI integration tests for openvital.
///
/// Each test spawns the compiled binary via the `assert_cmd::cargo_bin_cmd!`
/// macro and sets `OPENVITAL_HOME` to a fresh `TempDir` so tests are fully
/// isolated from the developer's real `~/.openvital` data.
use assert_cmd::cargo_bin_cmd;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Returns a `Command` with `OPENVITAL_HOME` pointing at `dir`.
fn cmd_in(dir: &TempDir) -> assert_cmd::Command {
    let mut c = cargo_bin_cmd!("openvital");
    c.env("OPENVITAL_HOME", dir.path());
    c
}

/// Run `openvital init --skip` in the given temp dir so the config and DB
/// exist before subsequent commands.
fn init_dir(dir: &TempDir) {
    cmd_in(dir).args(["init", "--skip"]).assert().success();
}

/// Parse stdout JSON and return the root `Value`.
fn parse_json(output: &assert_cmd::assert::Assert) -> Value {
    let bytes = output.get_output().stdout.clone();
    serde_json::from_slice(&bytes).expect("stdout is not valid JSON")
}

/// Parse stderr JSON and return the root `Value`.
fn parse_stderr_json(output: &assert_cmd::assert::Assert) -> Value {
    let bytes = output.get_output().stderr.clone();
    serde_json::from_slice(&bytes).expect("stderr is not valid JSON")
}

// ── init ─────────────────────────────────────────────────────────────────────

#[test]
fn test_init_skip_creates_config_file() {
    let dir = TempDir::new().unwrap();
    cmd_in(&dir)
        .args(["init", "--skip"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Config initialized"));

    let config_path = dir.path().join("config.toml");
    assert!(
        config_path.exists(),
        "config.toml should be created by init --skip"
    );
}

#[test]
fn test_init_skip_is_idempotent() {
    let dir = TempDir::new().unwrap();
    cmd_in(&dir).args(["init", "--skip"]).assert().success();
    // Running init again should not fail
    cmd_in(&dir).args(["init", "--skip"]).assert().success();
}

// ── log ──────────────────────────────────────────────────────────────────────

#[test]
fn test_log_weight_json_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["log", "weight", "82.5"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "log");
    assert_eq!(json["data"]["entry"]["type"], "weight");
    assert!((json["data"]["entry"]["value"].as_f64().unwrap() - 82.5).abs() < f64::EPSILON);
    assert_eq!(json["data"]["entry"]["unit"], "kg");
    assert!(json["data"]["entry"]["id"].as_str().is_some());
    assert!(json["data"]["entry"]["timestamp"].as_str().is_some());
}

#[test]
fn test_log_weight_human_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--human", "log", "weight", "80.0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Logged:"));
}

#[test]
fn test_log_with_note_and_tags() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args([
            "log",
            "pain",
            "3.0",
            "--note",
            "left knee",
            "--tags",
            "knee,post-run",
        ])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["entry"]["type"], "pain");
}

#[test]
fn test_log_with_source() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["log", "cardio", "45.0", "--source", "garmin"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_log_with_date_override() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["--date", "2026-01-15", "log", "weight", "79.0"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    let ts = json["data"]["entry"]["timestamp"].as_str().unwrap();
    assert!(ts.contains("2026-01-15"));
}

#[test]
fn test_log_alias_resolves_to_canonical_type() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // "w" is the default alias for "weight"
    let assert = cmd_in(&dir).args(["log", "w", "77.0"]).assert().success();

    let json = parse_json(&assert);
    assert_eq!(json["data"]["entry"]["type"], "weight");
}

#[test]
fn test_log_batch_json_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let batch = r#"[{"type":"weight","value":80.0},{"type":"water","value":2000.0}]"#;
    let assert = cmd_in(&dir)
        .args(["log", "--batch", batch])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    let entries = json["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["type"], "weight");
    assert_eq!(entries[1]["type"], "water");
}

#[test]
fn test_log_batch_invalid_json_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "--batch", "not-valid-json"])
        .assert()
        .failure();
}

#[test]
fn test_log_missing_value_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Missing value argument — clap should reject this
    cmd_in(&dir).args(["log", "weight"]).assert().failure();
}

// ── show ─────────────────────────────────────────────────────────────────────

#[test]
fn test_show_all_returns_empty_initially() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir).args(["show"]).assert().success();
    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_show_by_type_after_log() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();
    cmd_in(&dir)
        .args(["log", "weight", "81.5"])
        .assert()
        .success();

    // `show <type>` defaults to --last 1; use --last explicitly to get all.
    let assert = cmd_in(&dir)
        .args(["show", "weight", "--last", "10"])
        .assert()
        .success();
    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "show");
    let entries = json["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_show_with_last_flag() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    for v in [80.0, 81.0, 82.0, 83.0] {
        cmd_in(&dir)
            .args(["log", "weight", &v.to_string()])
            .assert()
            .success();
    }

    let assert = cmd_in(&dir)
        .args(["show", "weight", "--last", "2"])
        .assert()
        .success();

    let json = parse_json(&assert);
    let entries = json["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_show_human_flag() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "80.0"])
        .assert()
        .success();

    // Human mode should not output JSON
    cmd_in(&dir)
        .args(["--human", "show", "weight"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"weight|80").unwrap());
}

#[test]
fn test_show_empty_type_human() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--human", "show", "sleep_hours"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No entries"));
}

// ── status ───────────────────────────────────────────────────────────────────

#[test]
fn test_status_json_empty_db() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir).args(["status"]).assert().success();
    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "status");
    // data should have today's date or at minimum be a valid status object
    assert!(json["data"].is_object());
}

#[test]
fn test_status_human_flag() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir).args(["--human", "status"]).assert().success();
}

#[test]
fn test_status_with_logged_data() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "80.0"])
        .assert()
        .success();
    cmd_in(&dir)
        .args(["log", "water", "2000.0"])
        .assert()
        .success();
    cmd_in(&dir).args(["log", "pain", "2.0"]).assert().success();

    let assert = cmd_in(&dir).args(["status"]).assert().success();
    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

// ── trend ─────────────────────────────────────────────────────────────────────

#[test]
fn test_trend_no_data_returns_ok() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir).args(["trend", "weight"]).assert().success();
    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "trend");
}

#[test]
fn test_trend_with_data_weekly_period() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    for (date, val) in [
        ("2026-01-01", 83.0),
        ("2026-01-08", 82.5),
        ("2026-01-15", 82.0),
        ("2026-01-22", 81.5),
    ] {
        cmd_in(&dir)
            .args(["--date", date, "log", "weight", &val.to_string()])
            .assert()
            .success();
    }

    let assert = cmd_in(&dir)
        .args(["trend", "weight", "--period", "weekly"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert!(json["data"]["data"].is_array());
    assert!(json["data"]["trend"].is_object());
}

#[test]
fn test_trend_daily_period() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--date", "2026-01-10", "log", "weight", "80.0"])
        .assert()
        .success();

    let assert = cmd_in(&dir)
        .args(["trend", "weight", "--period", "daily"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_trend_monthly_period() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    for (date, val) in [("2026-01-05", 83.0), ("2026-02-05", 82.0)] {
        cmd_in(&dir)
            .args(["--date", date, "log", "weight", &val.to_string()])
            .assert()
            .success();
    }

    let assert = cmd_in(&dir)
        .args(["trend", "weight", "--period", "monthly"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_trend_with_last_flag() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    for (date, val) in [
        ("2026-01-01", 83.0),
        ("2026-01-08", 82.5),
        ("2026-01-15", 82.0),
    ] {
        cmd_in(&dir)
            .args(["--date", date, "log", "weight", &val.to_string()])
            .assert()
            .success();
    }

    let assert = cmd_in(&dir)
        .args(["trend", "weight", "--last", "2"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_trend_human_flag() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--human", "trend", "weight"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No data").or(predicate::str::contains("Trend:")));
}

#[test]
fn test_trend_invalid_period_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["trend", "weight", "--period", "yearly"])
        .assert()
        .failure();
}

#[test]
fn test_trend_correlate_json() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Log both metrics on the same dates so correlation has data points
    for (date, w, p) in [
        ("2026-01-01", 83.0, 3.0),
        ("2026-01-08", 82.5, 4.0),
        ("2026-01-15", 82.0, 2.0),
    ] {
        cmd_in(&dir)
            .args(["--date", date, "log", "weight", &w.to_string()])
            .assert()
            .success();
        cmd_in(&dir)
            .args(["--date", date, "log", "pain", &p.to_string()])
            .assert()
            .success();
    }

    let assert = cmd_in(&dir)
        .args(["trend", "--correlate", "weight,pain"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "correlate");
    assert!(json["data"]["coefficient"].is_number());
    assert!(json["data"]["data_points"].is_number());
}

#[test]
fn test_trend_correlate_human_flag() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    for (date, w, p) in [("2026-01-01", 83.0, 3.0), ("2026-01-08", 82.5, 4.0)] {
        cmd_in(&dir)
            .args(["--date", date, "log", "weight", &w.to_string()])
            .assert()
            .success();
        cmd_in(&dir)
            .args(["--date", date, "log", "pain", &p.to_string()])
            .assert()
            .success();
    }

    cmd_in(&dir)
        .args(["--human", "trend", "--correlate", "weight,pain"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Correlation:"));
}

#[test]
fn test_trend_correlate_bad_format_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Only one metric supplied — should fail
    cmd_in(&dir)
        .args(["trend", "--correlate", "weight"])
        .assert()
        .failure();
}

// ── goal ─────────────────────────────────────────────────────────────────────

#[test]
fn test_goal_set_json_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "75.0",
            "--direction",
            "below",
            "--timeframe",
            "monthly",
        ])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "goal");
    assert_eq!(json["data"]["goal"]["metric_type"], "weight");
    assert!((json["data"]["goal"]["target_value"].as_f64().unwrap() - 75.0).abs() < f64::EPSILON);
}

#[test]
fn test_goal_set_human_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args([
            "--human",
            "goal",
            "set",
            "water",
            "--target",
            "2000.0",
            "--direction",
            "above",
            "--timeframe",
            "daily",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal set:"));
}

#[test]
fn test_goal_status_empty() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir).args(["goal", "status"]).assert().success();
    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    let goals = json["data"]["goals"].as_array().unwrap();
    assert!(goals.is_empty());
}

#[test]
fn test_goal_status_after_set() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "75.0",
            "--direction",
            "below",
            "--timeframe",
            "monthly",
        ])
        .assert()
        .success();

    let assert = cmd_in(&dir).args(["goal", "status"]).assert().success();
    let json = parse_json(&assert);
    let goals = json["data"]["goals"].as_array().unwrap();
    assert_eq!(goals.len(), 1);
    assert_eq!(goals[0]["metric_type"], "weight");
}

#[test]
fn test_goal_status_filtered_by_type() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "75.0",
            "--direction",
            "below",
            "--timeframe",
            "monthly",
        ])
        .assert()
        .success();

    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "water",
            "--target",
            "2000.0",
            "--direction",
            "above",
            "--timeframe",
            "daily",
        ])
        .assert()
        .success();

    let assert = cmd_in(&dir)
        .args(["goal", "status", "weight"])
        .assert()
        .success();

    let json = parse_json(&assert);
    let goals = json["data"]["goals"].as_array().unwrap();
    assert_eq!(goals.len(), 1);
    assert_eq!(goals[0]["metric_type"], "weight");
}

#[test]
fn test_goal_status_human_no_goals() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--human", "goal", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No active goals"));
}

#[test]
fn test_goal_status_human_with_goals() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "75.0",
            "--direction",
            "below",
            "--timeframe",
            "monthly",
        ])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["--human", "goal", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("weight"));
}

#[test]
fn test_goal_remove_json_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert_set = cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "75.0",
            "--direction",
            "below",
            "--timeframe",
            "monthly",
        ])
        .assert()
        .success();

    let set_json = parse_json(&assert_set);
    let goal_id = set_json["data"]["goal"]["id"].as_str().unwrap().to_string();

    let assert_remove = cmd_in(&dir)
        .args(["goal", "remove", &goal_id])
        .assert()
        .success();

    let remove_json = parse_json(&assert_remove);
    assert_eq!(remove_json["status"], "ok");
    assert_eq!(remove_json["data"]["removed"], goal_id.as_str());
}

#[test]
fn test_goal_remove_human_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert_set = cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "75.0",
            "--direction",
            "below",
            "--timeframe",
            "monthly",
        ])
        .assert()
        .success();

    let set_json = parse_json(&assert_set);
    let goal_id = set_json["data"]["goal"]["id"].as_str().unwrap().to_string();

    cmd_in(&dir)
        .args(["--human", "goal", "remove", &goal_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal removed:"));
}

#[test]
fn test_goal_remove_nonexistent_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["goal", "remove", "00000000-0000-0000-0000-000000000000"])
        .assert()
        .failure();
}

#[test]
fn test_goal_invalid_direction_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "75.0",
            "--direction",
            "sideways",
            "--timeframe",
            "monthly",
        ])
        .assert()
        .failure();
}

#[test]
fn test_goal_invalid_timeframe_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "75.0",
            "--direction",
            "below",
            "--timeframe",
            "yearly",
        ])
        .assert()
        .failure();
}

// ── config ────────────────────────────────────────────────────────────────────

#[test]
fn test_config_show_json() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir).args(["config", "show"]).assert().success();
    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "config");
    assert!(json["data"]["config"].is_object());
}

#[test]
fn test_config_show_human() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--human", "config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[profile]").or(predicate::str::contains("[units]")));
}

#[test]
fn test_config_set_height() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["config", "set", "height", "175"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["key"], "height");
    assert_eq!(json["data"]["value"], "175");
}

#[test]
fn test_config_set_birth_year() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["config", "set", "birth_year", "1990"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_config_set_gender() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["config", "set", "gender", "male"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["value"], "male");
}

#[test]
fn test_config_set_conditions() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["config", "set", "conditions", "diabetes,hypertension"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_config_set_primary_exercise() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["config", "set", "primary_exercise", "running"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_config_set_alias() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["config", "set", "alias.hr", "heart_rate"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["key"], "alias.hr");

    // Use the new alias to log and verify it resolves
    let log_assert = cmd_in(&dir).args(["log", "hr", "72.0"]).assert().success();

    let log_json = parse_json(&log_assert);
    assert_eq!(log_json["data"]["entry"]["type"], "heart_rate");
}

#[test]
fn test_config_set_unknown_key_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["config", "set", "nonexistent_key", "value"])
        .assert()
        .failure();
}

#[test]
fn test_config_persists_across_invocations() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Set height
    cmd_in(&dir)
        .args(["config", "set", "height", "180"])
        .assert()
        .success();

    // Read it back via show
    let assert = cmd_in(&dir).args(["config", "show"]).assert().success();
    let json = parse_json(&assert);
    let height = json["data"]["config"]["profile"]["height_cm"]
        .as_f64()
        .unwrap();
    assert!((height - 180.0).abs() < f64::EPSILON);
}

// ── report ───────────────────────────────────────────────────────────────────

#[test]
fn test_report_week_empty_db() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["report", "--period", "week"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "report");
    assert!(json["data"]["from"].is_string());
    assert!(json["data"]["to"].is_string());
}

#[test]
fn test_report_month_period() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["report", "--period", "month"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

#[test]
fn test_report_specific_month() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--date", "2026-01-10", "log", "weight", "82.0"])
        .assert()
        .success();

    let assert = cmd_in(&dir)
        .args(["report", "--period", "month", "--month", "2026-01"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["from"], "2026-01-01");
    assert_eq!(json["data"]["to"], "2026-01-31");
    assert!(json["data"]["total_entries"].as_u64().unwrap() >= 1);
}

#[test]
fn test_report_custom_date_range() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    for (date, val) in [("2026-01-05", 83.0), ("2026-01-10", 82.5)] {
        cmd_in(&dir)
            .args(["--date", date, "log", "weight", &val.to_string()])
            .assert()
            .success();
    }

    let assert = cmd_in(&dir)
        .args(["report", "--from", "2026-01-01", "--to", "2026-01-15"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["from"], "2026-01-01");
    assert_eq!(json["data"]["to"], "2026-01-15");
    assert!(json["data"]["total_entries"].as_u64().unwrap() >= 2);
}

#[test]
fn test_report_human_flag() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--human", "report", "--period", "week"])
        .assert()
        .success()
        .stdout(predicate::str::contains("OpenVital Report:"));
}

#[test]
fn test_report_invalid_period_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["report", "--period", "year"])
        .assert()
        .failure();
}

#[test]
fn test_report_with_data_shows_metrics_summary() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Log entries within the current week range
    for v in [83.0_f64, 82.5, 82.0] {
        cmd_in(&dir)
            .args(["log", "weight", &v.to_string()])
            .assert()
            .success();
    }

    let assert = cmd_in(&dir)
        .args(["report", "--period", "week"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert!(json["data"]["total_entries"].as_u64().unwrap() >= 3);
    let metrics = json["data"]["metrics"].as_array().unwrap();
    assert!(!metrics.is_empty());

    // MetricSummary serialises the type field as "type" (not "metric_type")
    let weight_metric = metrics.iter().find(|m| m["type"] == "weight").unwrap();
    assert!(weight_metric["avg"].as_f64().is_some());
    assert!(weight_metric["min"].as_f64().is_some());
    assert!(weight_metric["max"].as_f64().is_some());
    assert!(weight_metric["count"].as_u64().is_some());
}

// ── export / import ───────────────────────────────────────────────────────────

#[test]
fn test_export_json_stdout() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();

    // Without --output, export prints raw JSON to stdout (not in envelope)
    let output = cmd_in(&dir)
        .args(["export", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: Value = serde_json::from_slice(&output).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 1);
}

#[test]
fn test_export_csv_stdout() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();

    let output = cmd_in(&dir)
        .args(["export", "--format", "csv"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("timestamp"), "CSV should have header");
    assert!(text.contains("weight"));
}

#[test]
fn test_export_json_to_file_then_human_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();

    let out_file = dir.path().join("export.json");
    cmd_in(&dir)
        .args([
            "--human",
            "export",
            "--format",
            "json",
            "--output",
            out_file.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported to"));

    assert!(out_file.exists());
    let content = fs::read_to_string(&out_file).unwrap();
    let parsed: Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.is_array());
}

#[test]
fn test_export_json_to_file_json_envelope() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();

    let out_file = dir.path().join("export2.json");
    let assert = cmd_in(&dir)
        .args([
            "export",
            "--format",
            "json",
            "--output",
            out_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "export");
    assert_eq!(json["data"]["format"], "json");
}

#[test]
fn test_export_csv_to_file() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir).args(["log", "pain", "3.0"]).assert().success();

    let out_file = dir.path().join("export.csv");
    cmd_in(&dir)
        .args([
            "export",
            "--format",
            "csv",
            "--output",
            out_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let content = fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("pain"));
}

#[test]
fn test_export_filter_by_type() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();
    cmd_in(&dir)
        .args(["log", "water", "2000.0"])
        .assert()
        .success();

    let output = cmd_in(&dir)
        .args(["export", "--format", "json", "--type", "weight"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: Value = serde_json::from_slice(&output).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["type"], "weight");
}

#[test]
fn test_export_invalid_format_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["export", "--format", "xml"])
        .assert()
        .failure();
}

#[test]
fn test_import_json_round_trip() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Log some data, export it, import it into fresh home
    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();
    cmd_in(&dir)
        .args(["log", "cardio", "45.0"])
        .assert()
        .success();

    let export_file = dir.path().join("data.json");
    cmd_in(&dir)
        .args([
            "export",
            "--format",
            "json",
            "--output",
            export_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Import into a second fresh home
    let dir2 = TempDir::new().unwrap();
    init_dir(&dir2);

    let assert = cmd_in(&dir2)
        .args([
            "import",
            "--source",
            "json",
            "--file",
            export_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "import");
    assert_eq!(json["data"]["metric_count"], 2);
    assert_eq!(json["data"]["medication_count"], 0);
}

#[test]
fn test_import_csv_round_trip() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();

    let export_file = dir.path().join("data.csv");
    cmd_in(&dir)
        .args([
            "export",
            "--format",
            "csv",
            "--output",
            export_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let dir2 = TempDir::new().unwrap();
    init_dir(&dir2);

    let assert = cmd_in(&dir2)
        .args([
            "import",
            "--source",
            "csv",
            "--file",
            export_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["count"], 1);
}

#[test]
fn test_import_human_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();

    let export_file = dir.path().join("data.json");
    cmd_in(&dir)
        .args([
            "export",
            "--format",
            "json",
            "--output",
            export_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let dir2 = TempDir::new().unwrap();
    init_dir(&dir2);

    cmd_in(&dir2)
        .args([
            "--human",
            "import",
            "--source",
            "json",
            "--file",
            export_file.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported"));
}

#[test]
fn test_import_nonexistent_file_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args([
            "import",
            "--source",
            "json",
            "--file",
            "/nonexistent/path/data.json",
        ])
        .assert()
        .failure();
}

#[test]
fn test_import_invalid_source_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Create a dummy file to avoid "file not found" error before source check
    let dummy_file = dir.path().join("dummy.xml");
    fs::write(&dummy_file, "<data/>").unwrap();

    cmd_in(&dir)
        .args([
            "import",
            "--source",
            "xml",
            "--file",
            dummy_file.to_str().unwrap(),
        ])
        .assert()
        .failure();
}

// ── completions ───────────────────────────────────────────────────────────────

#[test]
fn test_completions_bash() {
    let dir = TempDir::new().unwrap();
    cmd_in(&dir)
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("openvital"));
}

#[test]
fn test_completions_zsh() {
    let dir = TempDir::new().unwrap();
    cmd_in(&dir)
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("openvital"));
}

#[test]
fn test_completions_fish() {
    let dir = TempDir::new().unwrap();
    cmd_in(&dir)
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_completions_invalid_shell_fails() {
    let dir = TempDir::new().unwrap();
    cmd_in(&dir)
        .args(["completions", "powershell_invalid"])
        .assert()
        .failure();
}

// ── error cases / main.rs error path ─────────────────────────────────────────

#[test]
fn test_unknown_subcommand_fails() {
    let dir = TempDir::new().unwrap();
    cmd_in(&dir).args(["notacommand"]).assert().failure();
}

#[test]
fn test_no_args_fails() {
    let dir = TempDir::new().unwrap();
    cmd_in(&dir).assert().failure();
}

#[test]
fn test_error_output_goes_to_stderr_not_stdout() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // An operation that must fail: remove a non-existent goal
    cmd_in(&dir)
        .args(["goal", "remove", "bad-id"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error").or(predicate::str::contains("not found")))
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_log_then_show_full_pipeline() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Log three entries on different dates
    for (date, val) in [
        ("2026-01-01", 83.0_f64),
        ("2026-01-15", 82.5),
        ("2026-02-01", 82.0),
    ] {
        cmd_in(&dir)
            .args(["--date", date, "log", "weight", &val.to_string()])
            .assert()
            .success();
    }

    // Show all — pass --last with a high count since the default is 1
    let assert = cmd_in(&dir)
        .args(["show", "weight", "--last", "10"])
        .assert()
        .success();
    let json = parse_json(&assert);
    let entries = json["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);

    // Show today's date explicitly
    let assert2 = cmd_in(&dir).args(["show", "today"]).assert().success();
    let json2 = parse_json(&assert2);
    assert_eq!(json2["status"], "ok");
}

#[test]
fn test_status_with_goals_and_data() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Set a goal and log some data, then check status
    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "80.0",
            "--direction",
            "below",
            "--timeframe",
            "monthly",
        ])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["log", "weight", "79.5"])
        .assert()
        .success();

    let assert = cmd_in(&dir).args(["status"]).assert().success();
    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
}

// ── init (interactive path via stdin pipe) ────────────────────────────────────

#[test]
fn test_init_interactive_via_stdin_logs_initial_weight() {
    let dir = TempDir::new().unwrap();

    // Feed interactive prompts: height, weight, birth_year, gender, conditions, exercise
    let stdin_input = "175\n80.0\n1990\nmale\ndiabetes\nrunning\n";

    cmd_in(&dir)
        .args(["init"])
        .write_stdin(stdin_input)
        .assert()
        .success()
        .stdout(predicate::str::contains("Setup complete"));

    // A weight entry should have been logged during interactive init
    let assert = cmd_in(&dir).args(["show", "weight"]).assert().success();
    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    let entries = json["data"]["entries"].as_array().unwrap();
    assert!(
        !entries.is_empty(),
        "Initial weight should have been logged"
    );
    assert!((entries[0]["value"].as_f64().unwrap() - 80.0).abs() < f64::EPSILON);
}

#[test]
fn test_init_interactive_retry_on_bad_height() {
    let dir = TempDir::new().unwrap();

    // First height input is bad ("abc"), then a valid one
    let stdin_input = "abc\n175\n80.0\n1990\nmale\n\nrunning\n";

    cmd_in(&dir)
        .args(["init"])
        .write_stdin(stdin_input)
        .assert()
        .success()
        .stdout(predicate::str::contains("Setup complete"));
}

#[test]
fn test_init_interactive_retry_on_bad_birth_year() {
    let dir = TempDir::new().unwrap();

    // birth_year has a bad value first ("xyz"), then valid
    let stdin_input = "175\n80.0\nxyz\n1990\nmale\n\nrunning\n";

    cmd_in(&dir)
        .args(["init"])
        .write_stdin(stdin_input)
        .assert()
        .success()
        .stdout(predicate::str::contains("Setup complete"));
}

#[test]
fn test_init_interactive_empty_conditions() {
    let dir = TempDir::new().unwrap();

    // conditions left empty (just a newline)
    let stdin_input = "175\n80.0\n1990\nmale\n\nrunning\n";

    cmd_in(&dir)
        .args(["init"])
        .write_stdin(stdin_input)
        .assert()
        .success()
        .stdout(predicate::str::contains("Setup complete"));

    // Config should have been written with no conditions
    let assert = cmd_in(&dir).args(["config", "show"]).assert().success();
    let json = parse_json(&assert);
    let conditions = json["data"]["config"]["profile"]["conditions"]
        .as_array()
        .unwrap();
    assert!(
        conditions.is_empty(),
        "Conditions should be empty when skipped"
    );
}

// ── show (ByDate paths) ───────────────────────────────────────────────────────

#[test]
fn test_show_by_date_with_entries_human() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--date", "2026-01-10", "log", "weight", "82.0"])
        .assert()
        .success();

    // show with --date triggers ByDate path
    cmd_in(&dir)
        .args(["--human", "show", "--date", "2026-01-10"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2026-01-10"));
}

#[test]
fn test_show_by_date_no_entries_human() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // No data logged for this date
    cmd_in(&dir)
        .args(["--human", "show", "--date", "2025-01-01"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No entries"));
}

#[test]
fn test_show_by_date_json() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--date", "2026-01-10", "log", "pain", "3.0"])
        .assert()
        .success();

    let assert = cmd_in(&dir)
        .args(["show", "--date", "2026-01-10"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "show");
    // ByDate result has "date" and "entries" keys
    assert_eq!(json["data"]["date"], "2026-01-10");
    let entries = json["data"]["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert_eq!(entries[0]["type"], "pain");
}

// ── config (error paths) ──────────────────────────────────────────────────────

#[test]
fn test_config_set_height_invalid_value_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // "tall" is not a valid f64
    cmd_in(&dir)
        .args(["config", "set", "height", "tall"])
        .assert()
        .failure();
}

#[test]
fn test_config_set_birth_year_invalid_value_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // "nineteen-ninety" is not a valid u16
    cmd_in(&dir)
        .args(["config", "set", "birth_year", "nineteen-ninety"])
        .assert()
        .failure();
}

// ── report (additional coverage paths) ────────────────────────────────────────

#[test]
fn test_report_human_no_data_shows_no_data_message() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["--human", "report", "--period", "week"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No data").or(predicate::str::contains("Days with")));
}

#[test]
fn test_report_december_month_boundary() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // December month boundary: December should span 2025-12-01 to 2025-12-31
    let assert = cmd_in(&dir)
        .args(["report", "--period", "month", "--month", "2025-12"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["from"], "2025-12-01");
    assert_eq!(json["data"]["to"], "2025-12-31");
}

#[test]
fn test_report_invalid_month_format_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // "January" is not in YYYY-MM format
    cmd_in(&dir)
        .args(["report", "--period", "month", "--month", "January"])
        .assert()
        .failure();
}

#[test]
fn test_report_human_with_data_shows_metrics() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.5"])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["--human", "report", "--period", "week"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("OpenVital Report:").and(predicate::str::contains("weight")),
        );
}

// ── export (additional coverage: csv with output file, human output) ──────────

#[test]
fn test_export_csv_to_file_human_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "82.0"])
        .assert()
        .success();

    let out_file = dir.path().join("export_human.csv");
    cmd_in(&dir)
        .args([
            "--human",
            "export",
            "--format",
            "csv",
            "--output",
            out_file.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported to"));

    assert!(out_file.exists());
    let content = fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("weight"));
}

#[test]
fn test_export_csv_to_file_json_envelope() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir).args(["log", "pain", "4.0"]).assert().success();

    let out_file = dir.path().join("export_envelope.csv");
    let assert = cmd_in(&dir)
        .args([
            "export",
            "--format",
            "csv",
            "--output",
            out_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "export");
    assert_eq!(json["data"]["format"], "csv");
}

// ── trend (additional human-mode paths) ──────────────────────────────────────

#[test]
fn test_trend_human_with_data_shows_direction_and_projection() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Log enough data for a trend to have direction + potentially a projection
    for (date, val) in [
        ("2026-01-01", 83.0_f64),
        ("2026-01-08", 82.5),
        ("2026-01-15", 82.0),
        ("2026-01-22", 81.5),
    ] {
        cmd_in(&dir)
            .args(["--date", date, "log", "weight", &val.to_string()])
            .assert()
            .success();
    }

    cmd_in(&dir)
        .args(["--human", "trend", "weight", "--period", "weekly"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Trend: weight"));
}

#[test]
fn test_trend_correlate_with_last_flag() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    for (date, w, p) in [
        ("2026-01-01", 83.0, 3.0),
        ("2026-01-08", 82.5, 4.0),
        ("2026-01-15", 82.0, 2.0),
    ] {
        cmd_in(&dir)
            .args(["--date", date, "log", "weight", &w.to_string()])
            .assert()
            .success();
        cmd_in(&dir)
            .args(["--date", date, "log", "pain", &p.to_string()])
            .assert()
            .success();
    }

    let assert = cmd_in(&dir)
        .args(["trend", "--correlate", "weight,pain", "--last", "30"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "correlate");
}

// ── goal (additional coverage: status human with met goal) ───────────────────

#[test]
fn test_goal_status_human_with_met_goal() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Set a goal and then satisfy it so is_met = true
    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "100.0",
            "--direction",
            "below",
            "--timeframe",
            "monthly",
        ])
        .assert()
        .success();

    // Log a value well below target so it is met
    cmd_in(&dir)
        .args(["log", "weight", "80.0"])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["--human", "goal", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("MET").or(predicate::str::contains("weight")));
}

#[test]
fn test_goal_status_human_with_specific_type() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "water",
            "--target",
            "2000.0",
            "--direction",
            "above",
            "--timeframe",
            "daily",
        ])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["--human", "goal", "status", "water"])
        .assert()
        .success()
        .stdout(predicate::str::contains("water"));
}

// ── goal positional args ─────────────────────────────────────────────────────

#[test]
fn test_goal_set_positional_args() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Goal set with positional: goal set weight 70 below daily
    cmd_in(&dir)
        .args(["goal", "set", "weight", "70", "below", "daily", "--human"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal set: weight below 70"));
}

#[test]
fn test_goal_set_named_args_still_works() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Goal set with named args (original syntax)
    cmd_in(&dir)
        .args([
            "goal",
            "set",
            "water",
            "--target",
            "2000",
            "--direction",
            "above",
            "--timeframe",
            "daily",
            "--human",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Goal set: water above 2000"));
}

#[test]
fn test_goal_set_missing_required_value_returns_json_error_not_panic() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["goal", "set", "weight"])
        .assert()
        .failure();

    let json = parse_stderr_json(&assert);
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["code"], "general_error");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("target is required")
    );
}

// ── blood pressure ───────────────────────────────────────────────────────────

#[test]
fn test_log_blood_pressure_splits_into_two_entries() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let db = openvital::db::Database::open(&db_path).unwrap();
    let config = openvital::models::config::Config::default();

    // Simulate the split that cmd/log.rs does for "blood_pressure" "120/80"
    let m1 = openvital::core::logging::log_metric(
        &db,
        &config,
        openvital::core::logging::LogEntry {
            metric_type: "bp_systolic",
            value: 120.0,
            note: None,
            tags: None,
            source: None,
            date: None,
        },
    )
    .unwrap();
    let m2 = openvital::core::logging::log_metric(
        &db,
        &config,
        openvital::core::logging::LogEntry {
            metric_type: "bp_diastolic",
            value: 80.0,
            note: None,
            tags: None,
            source: None,
            date: None,
        },
    )
    .unwrap();

    assert_eq!(m1.unit, "mmHg");
    assert_eq!(m2.unit, "mmHg");
    assert_eq!(m1.value, 120.0);
    assert_eq!(m2.value, 80.0);
    assert_eq!(m1.metric_type, "bp_systolic");
    assert_eq!(m2.metric_type, "bp_diastolic");
}

#[test]
fn test_log_blood_pressure_cli_json_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["log", "blood_pressure", "120/80"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "log");
    let entries = json["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["type"], "bp_systolic");
    assert_eq!(entries[0]["value"], 120.0);
    assert_eq!(entries[0]["unit"], "mmHg");
    assert_eq!(entries[1]["type"], "bp_diastolic");
    assert_eq!(entries[1]["value"], 80.0);
    assert_eq!(entries[1]["unit"], "mmHg");
}

#[test]
fn test_log_blood_pressure_bp_alias_json_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["log", "bp", "130/85"])
        .assert()
        .success();

    let json = parse_json(&assert);
    assert_eq!(json["status"], "ok");
    let entries = json["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["value"], 130.0);
    assert_eq!(entries[1]["value"], 85.0);
}

#[test]
fn test_log_blood_pressure_custom_alias_json_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["config", "set", "alias.press", "blood_pressure"])
        .assert()
        .success();

    let assert = cmd_in(&dir)
        .args(["log", "press", "128/84"])
        .assert()
        .success();

    let json = parse_json(&assert);
    let entries = json["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["type"], "bp_systolic");
    assert_eq!(entries[0]["value"], 128.0);
    assert_eq!(entries[1]["type"], "bp_diastolic");
    assert_eq!(entries[1]["value"], 84.0);
}

#[test]
fn test_log_blood_pressure_human_output() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["--human", "log", "blood_pressure", "120/80"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("bp_systolic") && stdout.contains("bp_diastolic"),
        "BP human output should show bp_systolic and bp_diastolic, got: {}",
        stdout
    );
    assert!(
        stdout.contains("mmHg"),
        "BP human output should show mmHg unit, got: {}",
        stdout
    );
}

// ── imperial unit conversions ────────────────────────────────────────────────

#[test]
fn test_config_set_units_system_imperial() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["config", "set", "units.system", "imperial"])
        .assert()
        .success();

    // Verify config shows imperial
    let assert = cmd_in(&dir)
        .args(["--human", "config", "show"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("imperial"),
        "config should show imperial, got: {}",
        stdout
    );
    assert!(
        stdout.contains("lbs"),
        "config should show lbs, got: {}",
        stdout
    );
}

#[test]
fn test_config_set_units_system_invalid_fails() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["config", "set", "units.system", "martian"])
        .assert()
        .failure();
}

#[test]
fn test_log_weight_imperial_converts_and_displays() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Set imperial
    cmd_in(&dir)
        .args(["config", "set", "units.system", "imperial"])
        .assert()
        .success();

    // Log 160 lbs in human mode — should display lbs
    cmd_in(&dir)
        .args(["--human", "log", "weight", "160"])
        .assert()
        .success()
        .stdout(predicate::str::contains("lbs"));

    // JSON output should show metric (kg) — value stored in kg
    let assert = cmd_in(&dir)
        .args(["log", "weight", "160"])
        .assert()
        .success();
    let json = parse_json(&assert);
    assert_eq!(json["data"]["entry"]["unit"], "kg");
    // 160 lbs ~ 72.57 kg
    let stored = json["data"]["entry"]["value"].as_f64().unwrap();
    assert!(
        stored < 100.0,
        "stored value should be in kg (< 100), got: {}",
        stored
    );
}

#[test]
fn test_log_batch_imperial_converts_to_metric_storage() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["config", "set", "units.system", "imperial"])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["log", "--batch", r#"[{"type":"weight","value":160.0}]"#])
        .assert()
        .success();

    let assert = cmd_in(&dir)
        .args(["show", "weight", "--last", "1"])
        .assert()
        .success();
    let json = parse_json(&assert);
    let stored = json["data"]["entries"][0]["value"].as_f64().unwrap();
    assert!(
        stored < 100.0,
        "batch entry should be stored in kg (< 100), got: {}",
        stored
    );
}

#[test]
fn test_show_weight_imperial_displays_lbs() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["config", "set", "units.system", "imperial"])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["log", "weight", "160"])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["--human", "show", "weight"])
        .assert()
        .success()
        .stdout(predicate::str::contains("lbs"));
}

#[test]
fn test_goal_set_imperial_converts_target() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["config", "set", "units.system", "imperial"])
        .assert()
        .success();

    // Set goal: weight below 155 lbs
    cmd_in(&dir)
        .args(["goal", "set", "weight", "155", "below", "daily", "--human"])
        .assert()
        .success()
        .stdout(predicate::str::contains("155").and(predicate::str::contains("lbs")));

    // JSON output: target_value should be stored in kg
    let assert = cmd_in(&dir)
        .args([
            "goal",
            "set",
            "weight",
            "--target",
            "155",
            "--direction",
            "below",
            "--timeframe",
            "daily",
        ])
        .assert()
        .success();
    let json = parse_json(&assert);
    let stored_target = json["data"]["goal"]["target_value"].as_f64().unwrap();
    // 155 lbs ~ 70.3 kg
    assert!(
        stored_target < 100.0,
        "goal target should be stored in kg (< 100), got: {}",
        stored_target
    );
}

#[test]
fn test_goal_status_imperial_progress_uses_display_units() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["config", "set", "units.system", "imperial"])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["goal", "set", "weight", "155", "below", "daily"])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["log", "weight", "160"])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["--human", "goal", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("155.0 lbs").and(predicate::str::contains("160.0")));
}

#[test]
fn test_trend_imperial_rate_uses_display_unit() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["config", "set", "units.system", "imperial"])
        .assert()
        .success();

    cmd_in(&dir)
        .args(["--date", "2026-02-16", "log", "weight", "160"])
        .assert()
        .success();
    cmd_in(&dir)
        .args(["--date", "2026-02-17", "log", "weight", "158"])
        .assert()
        .success();

    cmd_in(&dir)
        .args([
            "--human", "trend", "weight", "--period", "daily", "--last", "2",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("lbs per day"));
}

// ── batch simple format ──────────────────────────────────────────────────────

#[test]
fn test_batch_simple_format_parsing() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let db = openvital::db::Database::open(&db_path).unwrap();
    let config = openvital::models::config::Config::default();

    // Simple format: "weight:72.5,sleep:7.5,mood:8"
    let simple = "weight:72.5,sleep:7.5,mood:8";
    let json_str = openvital::core::logging::parse_simple_batch(simple).unwrap();
    let metrics = openvital::core::logging::log_batch(&db, &config, &json_str).unwrap();

    assert_eq!(metrics.len(), 3);
    assert_eq!(metrics[0].metric_type, "weight");
    assert_eq!(metrics[0].value, 72.5);
    assert_eq!(metrics[1].metric_type, "sleep");
    assert_eq!(metrics[1].value, 7.5);
    assert_eq!(metrics[2].metric_type, "mood");
    assert_eq!(metrics[2].value, 8.0);
}

#[test]
fn test_batch_simple_format_invalid_entry() {
    let result = openvital::core::logging::parse_simple_batch("weight72.5");
    assert!(result.is_err());
}

#[test]
fn test_batch_simple_format_invalid_value() {
    let result = openvital::core::logging::parse_simple_batch("weight:abc");
    assert!(result.is_err());
}

/// config set height with imperial units should convert feet to cm before storing.
#[test]
fn test_config_set_height_imperial_converts_feet_to_cm() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Switch to imperial
    cmd_in(&dir)
        .args(["config", "set", "units.system", "imperial"])
        .assert()
        .success();

    // Set height to 5.83 feet
    cmd_in(&dir)
        .args(["config", "set", "height", "5.83"])
        .assert()
        .success();

    // Read back config — height_cm should be ~177.7, not 5.83
    let assert = cmd_in(&dir).args(["config", "show"]).assert().success();
    let json = parse_json(&assert);
    let height_cm = json["data"]["config"]["profile"]["height_cm"]
        .as_f64()
        .expect("height_cm should be a number");
    assert!(
        (height_cm - 177.7).abs() < 1.0,
        "expected ~177.7 cm, got {} (imperial feet were not converted)",
        height_cm
    );
}

// ─── Fix 3: init with empty stdin should fail, not loop ──────────────────────

#[test]
fn test_init_empty_stdin_fails_not_loops() {
    let dir = TempDir::new().unwrap();

    // Empty stdin (EOF immediately) — should fail, not loop
    cmd_in(&dir)
        .args(["init"])
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(5))
        .assert()
        .failure();
}

// ─── Fix 4: init weight has source="init" ────────────────────────────────────

#[test]
fn test_init_weight_has_source_init() {
    let dir = TempDir::new().unwrap();

    let stdin_input = "175\n80.0\n1990\nmale\n\nrunning\n";
    cmd_in(&dir)
        .args(["init"])
        .write_stdin(stdin_input)
        .assert()
        .success();

    let assert = cmd_in(&dir).args(["show", "weight"]).assert().success();
    let json = parse_json(&assert);
    let entries = json["data"]["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert_eq!(
        entries[0]["source"], "init",
        "init weight should have source='init'"
    );
}

// ─── Fix 5: --batch conflicts with TYPE/VALUE ────────────────────────────────

#[test]
fn test_log_batch_conflicts_with_type_value() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    cmd_in(&dir)
        .args(["log", "weight", "74.5", "--batch", "steps:9000"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

// ─── Fix 7: goal remove by metric type ───────────────────────────────────────

#[test]
fn test_goal_remove_by_metric_type() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Set a goal for steps
    cmd_in(&dir)
        .args(["goal", "set", "steps", "10000", "above", "daily"])
        .assert()
        .success();

    // Remove by metric type name (not UUID)
    cmd_in(&dir)
        .args(["goal", "remove", "steps"])
        .assert()
        .success();

    // Verify goal is removed
    let assert = cmd_in(&dir).args(["goal", "status"]).assert().success();
    let json = parse_json(&assert);
    let goals = json["data"]["goals"].as_array().unwrap();
    assert!(
        goals.is_empty(),
        "goal should be removed when using metric type"
    );
}

// ─── Fix 9: trend rate_unit uses noun form ───────────────────────────────────

#[test]
fn test_trend_rate_unit_uses_noun_form() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Log enough data for trend
    cmd_in(&dir)
        .args(["log", "weight", "80", "--date", "2026-02-10"])
        .assert()
        .success();
    cmd_in(&dir)
        .args(["log", "weight", "79.5", "--date", "2026-02-17"])
        .assert()
        .success();

    let assert = cmd_in(&dir)
        .args(["trend", "weight", "--period", "weekly"])
        .assert()
        .success();
    let json = parse_json(&assert);
    let rate_unit = json["data"]["trend"]["rate_unit"].as_str().unwrap();
    assert_eq!(
        rate_unit, "per week",
        "rate_unit should be 'per week', not 'per weekly'"
    );
}

// ─── Fix 10: correlation with 2 data points → insufficient data ──────────────

#[test]
fn test_correlation_two_points_insufficient_data() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Log exactly 2 matching data points
    cmd_in(&dir)
        .args(["log", "weight", "80", "--date", "2026-02-10"])
        .assert()
        .success();
    cmd_in(&dir)
        .args(["log", "steps", "8000", "--date", "2026-02-10"])
        .assert()
        .success();
    cmd_in(&dir)
        .args(["log", "weight", "79", "--date", "2026-02-11"])
        .assert()
        .success();
    cmd_in(&dir)
        .args(["log", "steps", "10000", "--date", "2026-02-11"])
        .assert()
        .success();

    let assert = cmd_in(&dir)
        .args(["trend", "--correlate", "weight,steps"])
        .assert()
        .success();
    let json = parse_json(&assert);
    let interpretation = json["data"]["interpretation"].as_str().unwrap();
    assert_eq!(
        interpretation, "insufficient data",
        "2 data points should be insufficient"
    );
}

// ─── Fix 11: config set unknown key lists valid keys ─────────────────────────

#[test]
fn test_config_set_unknown_key_lists_valid_keys() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    let assert = cmd_in(&dir)
        .args(["config", "set", "invalid_key", "val"])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("height"),
        "error should list valid keys, got: {}",
        stderr
    );
}

// ─── Fix 12: show with no args shows tip in human mode ───────────────────────

#[test]
fn test_show_no_args_human_shows_tip() {
    let dir = TempDir::new().unwrap();
    init_dir(&dir);

    // Log something so show has data
    cmd_in(&dir)
        .args(["log", "weight", "80"])
        .assert()
        .success();

    let assert = cmd_in(&dir).args(["show", "--human"]).assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("Tip:"),
        "show with no type should display a tip, got: {}",
        stdout
    );
}
