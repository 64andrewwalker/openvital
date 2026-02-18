mod common;

use chrono::Utc;
use openvital::models::config::{Alerts, Config, Profile, Units};
use openvital::models::goal::{Direction, Goal, Timeframe};
use openvital::models::metric::{Category, Metric, default_unit};
use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::{LazyLock, Mutex};
use tempfile::TempDir;

static CONFIG_ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

struct OpenVitalHomeGuard {
    previous: Option<OsString>,
}

impl OpenVitalHomeGuard {
    fn set(path: &std::path::Path) -> Self {
        let previous = std::env::var_os("OPENVITAL_HOME");
        // SAFETY: tests that touch OPENVITAL_HOME are serialized by CONFIG_ENV_LOCK.
        unsafe { std::env::set_var("OPENVITAL_HOME", path) };
        Self { previous }
    }
}

impl Drop for OpenVitalHomeGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => {
                // SAFETY: tests that touch OPENVITAL_HOME are serialized by CONFIG_ENV_LOCK.
                unsafe { std::env::set_var("OPENVITAL_HOME", value) };
            }
            None => {
                // SAFETY: tests that touch OPENVITAL_HOME are serialized by CONFIG_ENV_LOCK.
                unsafe { std::env::remove_var("OPENVITAL_HOME") };
            }
        }
    }
}

fn with_temp_openvital_home<T>(f: impl FnOnce() -> T) -> T {
    let _lock = CONFIG_ENV_LOCK.lock().unwrap();
    let dir = TempDir::new().unwrap();
    let _home = OpenVitalHomeGuard::set(dir.path());
    f()
}

// ─── Config tests ────────────────────────────────────────────────────────────

/// Config::default() produces sensible zero-value / default-unit values.
#[test]
fn test_config_default() {
    let cfg = Config::default();
    assert_eq!(cfg.units.weight, "kg");
    assert_eq!(cfg.units.height, "cm");
    assert_eq!(cfg.units.water, "ml");
    assert_eq!(cfg.units.temperature, "celsius");
    assert_eq!(cfg.alerts.pain_threshold, 5);
    assert_eq!(cfg.alerts.pain_consecutive_days, 3);
    assert!(cfg.aliases.is_empty());
    assert!(cfg.profile.height_cm.is_none());
    assert!(cfg.profile.birth_year.is_none());
    assert!(cfg.profile.conditions.is_empty());
}

/// Units::default() sets the expected strings.
#[test]
fn test_units_default() {
    let u = Units::default();
    assert_eq!(u.weight, "kg");
    assert_eq!(u.height, "cm");
    assert_eq!(u.water, "ml");
    assert_eq!(u.temperature, "celsius");
}

/// Alerts::default() matches spec thresholds.
#[test]
fn test_alerts_default() {
    let a = Alerts::default();
    assert_eq!(a.pain_threshold, 5);
    assert_eq!(a.pain_consecutive_days, 3);
}

/// Config::default_aliases() returns all expected short-codes.
#[test]
fn test_default_aliases_completeness() {
    let aliases = Config::default_aliases();
    let expected = [
        ("w", "weight"),
        ("bf", "body_fat"),
        ("c", "cardio"),
        ("s", "strength"),
        ("sl", "sleep_hours"),
        ("sq", "sleep_quality"),
        ("wa", "water"),
        ("p", "pain"),
        ("so", "soreness"),
        ("cal", "calories_in"),
        ("st", "screen_time"),
    ];
    for (short, full) in &expected {
        assert_eq!(
            aliases.get(*short).map(|s| s.as_str()),
            Some(*full),
            "alias '{}' should map to '{}'",
            short,
            full
        );
    }
    assert_eq!(aliases.len(), expected.len());
}

/// resolve_alias returns the mapped value when the alias exists.
#[test]
fn test_resolve_alias_known() {
    let mut cfg = Config::default();
    cfg.aliases = Config::default_aliases();
    assert_eq!(cfg.resolve_alias("w"), "weight");
    assert_eq!(cfg.resolve_alias("bf"), "body_fat");
    assert_eq!(cfg.resolve_alias("p"), "pain");
    assert_eq!(cfg.resolve_alias("sl"), "sleep_hours");
}

/// resolve_alias returns the input unchanged for unknown keys.
#[test]
fn test_resolve_alias_unknown_passthrough() {
    let cfg = Config::default();
    assert_eq!(cfg.resolve_alias("weight"), "weight");
    assert_eq!(cfg.resolve_alias("unknown_metric"), "unknown_metric");
    assert_eq!(cfg.resolve_alias(""), "");
}

/// Config::path() ends with ".openvital/config.toml".
#[test]
fn test_config_path_suffix() {
    let _lock = CONFIG_ENV_LOCK.lock().unwrap();
    let path = Config::path();
    assert!(
        path.to_string_lossy().contains(".openvital"),
        "path should contain .openvital"
    );
    assert!(path.ends_with("config.toml"));
}

/// Config::db_path() ends with ".openvital/data.db".
#[test]
fn test_db_path_suffix() {
    let _lock = CONFIG_ENV_LOCK.lock().unwrap();
    let path = Config::db_path();
    assert!(path.ends_with("data.db"));
    assert!(
        path.to_string_lossy().contains(".openvital"),
        "db path should contain .openvital"
    );
}

/// data_dir() is consistent between calls.
#[test]
fn test_data_dir_stable() {
    let _lock = CONFIG_ENV_LOCK.lock().unwrap();
    let d1 = Config::data_dir();
    let d2 = Config::data_dir();
    assert_eq!(d1, d2);
}

/// Round-trip: save a config to a temp file and reload it.
/// We test this by constructing a Config, serialising it manually to a
/// temp dir that shadows the standard path, then reading it back via
/// standard TOML de/serialisation (not Config::load, which reads the fixed
/// path).  This keeps the test hermetic.
#[test]
fn test_config_roundtrip_toml() {
    let mut cfg = Config::default();
    cfg.profile.height_cm = Some(180.0);
    cfg.profile.birth_year = Some(1990);
    cfg.profile.gender = Some("male".to_string());
    cfg.profile.conditions = vec!["hypertension".to_string()];
    cfg.profile.primary_exercise = Some("cycling".to_string());
    cfg.units.weight = "lbs".to_string();
    cfg.aliases
        .insert("x".to_string(), "custom_metric".to_string());
    cfg.alerts.pain_threshold = 7;
    cfg.alerts.pain_consecutive_days = 5;

    let serialised = toml::to_string_pretty(&cfg).expect("serialise");
    let reloaded: Config = toml::from_str(&serialised).expect("deserialise");

    assert_eq!(reloaded.profile.height_cm, Some(180.0));
    assert_eq!(reloaded.profile.birth_year, Some(1990));
    assert_eq!(reloaded.profile.gender.as_deref(), Some("male"));
    assert_eq!(reloaded.profile.conditions, vec!["hypertension"]);
    assert_eq!(
        reloaded.profile.primary_exercise.as_deref(),
        Some("cycling")
    );
    assert_eq!(reloaded.units.weight, "lbs");
    assert_eq!(
        reloaded.aliases.get("x").map(|s| s.as_str()),
        Some("custom_metric")
    );
    assert_eq!(reloaded.alerts.pain_threshold, 7);
    assert_eq!(reloaded.alerts.pain_consecutive_days, 5);
}

/// Config can be deserialised from a minimal TOML string (missing sections
/// fall back to Default).
#[test]
fn test_config_partial_toml_uses_defaults() {
    let minimal = r#"
[profile]
height_cm = 170.0
"#;
    let cfg: Config = toml::from_str(minimal).expect("deserialise minimal");
    assert_eq!(cfg.profile.height_cm, Some(170.0));
    // Everything else should be default
    assert_eq!(cfg.units.weight, "kg");
    assert!(cfg.aliases.is_empty());
    assert_eq!(cfg.alerts.pain_threshold, 5);
}

/// Profile fields are individually optional.
#[test]
fn test_profile_optional_fields() {
    let p = Profile::default();
    assert!(p.height_cm.is_none());
    assert!(p.birth_year.is_none());
    assert!(p.gender.is_none());
    assert!(p.conditions.is_empty());
    assert!(p.primary_exercise.is_none());
}

// ─── Direction tests ──────────────────────────────────────────────────────────

#[test]
fn test_direction_from_str_valid() {
    use std::str::FromStr;
    assert_eq!(Direction::from_str("above").unwrap(), Direction::Above);
    assert_eq!(Direction::from_str("below").unwrap(), Direction::Below);
    assert_eq!(Direction::from_str("equal").unwrap(), Direction::Equal);
}

#[test]
fn test_direction_from_str_invalid() {
    use std::str::FromStr;
    assert!(Direction::from_str("up").is_err());
    assert!(Direction::from_str("").is_err());
    assert!(Direction::from_str("Above").is_err()); // case-sensitive
    assert!(Direction::from_str("BELOW").is_err());
}

#[test]
fn test_direction_display() {
    assert_eq!(Direction::Above.to_string(), "above");
    assert_eq!(Direction::Below.to_string(), "below");
    assert_eq!(Direction::Equal.to_string(), "equal");
}

/// Direction round-trips through Display → FromStr.
#[test]
fn test_direction_display_fromstr_roundtrip() {
    use std::str::FromStr;
    for d in [Direction::Above, Direction::Below, Direction::Equal] {
        let s = d.to_string();
        let back = Direction::from_str(&s).unwrap();
        assert_eq!(back, d);
    }
}

/// Direction serialises to its snake_case JSON representation.
#[test]
fn test_direction_serde_json() {
    let j = serde_json::to_string(&Direction::Above).unwrap();
    assert_eq!(j, "\"above\"");
    let d: Direction = serde_json::from_str("\"below\"").unwrap();
    assert_eq!(d, Direction::Below);
}

// ─── Timeframe tests ─────────────────────────────────────────────────────────

#[test]
fn test_timeframe_from_str_valid() {
    use std::str::FromStr;
    assert_eq!(Timeframe::from_str("daily").unwrap(), Timeframe::Daily);
    assert_eq!(Timeframe::from_str("weekly").unwrap(), Timeframe::Weekly);
    assert_eq!(Timeframe::from_str("monthly").unwrap(), Timeframe::Monthly);
}

#[test]
fn test_timeframe_from_str_invalid() {
    use std::str::FromStr;
    assert!(Timeframe::from_str("yearly").is_err());
    assert!(Timeframe::from_str("").is_err());
    assert!(Timeframe::from_str("Daily").is_err());
}

#[test]
fn test_timeframe_display() {
    assert_eq!(Timeframe::Daily.to_string(), "daily");
    assert_eq!(Timeframe::Weekly.to_string(), "weekly");
    assert_eq!(Timeframe::Monthly.to_string(), "monthly");
}

#[test]
fn test_timeframe_display_fromstr_roundtrip() {
    use std::str::FromStr;
    for t in [Timeframe::Daily, Timeframe::Weekly, Timeframe::Monthly] {
        let s = t.to_string();
        let back = Timeframe::from_str(&s).unwrap();
        assert_eq!(back, t);
    }
}

#[test]
fn test_timeframe_serde_json() {
    let j = serde_json::to_string(&Timeframe::Weekly).unwrap();
    assert_eq!(j, "\"weekly\"");
    let t: Timeframe = serde_json::from_str("\"monthly\"").unwrap();
    assert_eq!(t, Timeframe::Monthly);
}

// ─── Goal tests ───────────────────────────────────────────────────────────────

#[test]
fn test_goal_new_fields() {
    let g = Goal::new("weight".into(), 75.0, Direction::Below, Timeframe::Monthly);
    assert_eq!(g.metric_type, "weight");
    assert_eq!(g.target_value, 75.0);
    assert_eq!(g.direction, Direction::Below);
    assert_eq!(g.timeframe, Timeframe::Monthly);
    assert!(g.active);
    // id should be a non-empty UUID string
    assert!(!g.id.is_empty());
    // created_at should be close to now
    let diff = Utc::now().signed_duration_since(g.created_at).num_seconds();
    assert!(diff < 5, "created_at should be within last 5 seconds");
}

#[test]
fn test_goal_new_generates_unique_ids() {
    let g1 = Goal::new("weight".into(), 75.0, Direction::Below, Timeframe::Monthly);
    let g2 = Goal::new("weight".into(), 75.0, Direction::Below, Timeframe::Monthly);
    assert_ne!(g1.id, g2.id);
}

/// Goal::is_met covers all three direction variants.
#[test]
fn test_goal_is_met_above() {
    let g = Goal::new("cardio".into(), 150.0, Direction::Above, Timeframe::Weekly);
    assert!(g.is_met(150.0)); // exactly at threshold
    assert!(g.is_met(200.0)); // above
    assert!(!g.is_met(149.9)); // just below
}

#[test]
fn test_goal_is_met_below() {
    let g = Goal::new("weight".into(), 80.0, Direction::Below, Timeframe::Monthly);
    assert!(g.is_met(80.0)); // exactly at threshold
    assert!(g.is_met(60.0)); // well below
    assert!(!g.is_met(80.1)); // just above
}

#[test]
fn test_goal_is_met_equal() {
    let g = Goal::new(
        "sleep_quality".into(),
        4.0,
        Direction::Equal,
        Timeframe::Daily,
    );
    assert!(g.is_met(4.0));
    assert!(!g.is_met(3.999));
    assert!(!g.is_met(4.001));
}

/// is_met with equal uses f64::EPSILON tolerance (not an arbitrary margin).
#[test]
fn test_goal_is_met_equal_epsilon_boundary() {
    let g = Goal::new("pain".into(), 3.0, Direction::Equal, Timeframe::Daily);
    // Exactly f64::EPSILON away should NOT be met
    assert!(!g.is_met(3.0 + f64::EPSILON * 2.0));
    // A value that rounds to exactly 3.0 in IEEE754 IS met
    assert!(g.is_met(3.0));
}

/// Goal serialises and deserialises correctly via JSON.
#[test]
fn test_goal_serde_json_roundtrip() {
    let g = Goal::new("water".into(), 2000.0, Direction::Above, Timeframe::Daily);
    let json = serde_json::to_string(&g).unwrap();
    let back: Goal = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, g.id);
    assert_eq!(back.metric_type, g.metric_type);
    assert_eq!(back.target_value, g.target_value);
    assert_eq!(back.direction, g.direction);
    assert_eq!(back.timeframe, g.timeframe);
    assert_eq!(back.active, g.active);
}

// ─── Category tests ───────────────────────────────────────────────────────────

#[test]
fn test_category_from_type_body() {
    assert_eq!(Category::from_type("weight"), Category::Body);
    assert_eq!(Category::from_type("body_fat"), Category::Body);
    assert_eq!(Category::from_type("waist"), Category::Body);
}

#[test]
fn test_category_from_type_exercise() {
    assert_eq!(Category::from_type("cardio"), Category::Exercise);
    assert_eq!(Category::from_type("strength"), Category::Exercise);
    assert_eq!(Category::from_type("calories_burned"), Category::Exercise);
}

#[test]
fn test_category_from_type_sleep() {
    assert_eq!(Category::from_type("sleep_hours"), Category::Sleep);
    assert_eq!(Category::from_type("sleep_quality"), Category::Sleep);
    assert_eq!(Category::from_type("bed_time"), Category::Sleep);
    assert_eq!(Category::from_type("wake_time"), Category::Sleep);
}

#[test]
fn test_category_from_type_nutrition() {
    assert_eq!(Category::from_type("calories_in"), Category::Nutrition);
    assert_eq!(Category::from_type("water"), Category::Nutrition);
}

#[test]
fn test_category_from_type_pain() {
    assert_eq!(Category::from_type("pain"), Category::Pain);
    assert_eq!(Category::from_type("soreness"), Category::Pain);
}

#[test]
fn test_category_from_type_habit() {
    assert_eq!(Category::from_type("standing_breaks"), Category::Habit);
    assert_eq!(Category::from_type("screen_time"), Category::Habit);
}

#[test]
fn test_category_from_type_custom() {
    assert_eq!(Category::from_type("mood"), Category::Custom);
    assert_eq!(Category::from_type(""), Category::Custom);
    assert_eq!(Category::from_type("my_custom_metric"), Category::Custom);
}

#[test]
fn test_category_display() {
    assert_eq!(Category::Body.to_string(), "body");
    assert_eq!(Category::Exercise.to_string(), "exercise");
    assert_eq!(Category::Sleep.to_string(), "sleep");
    assert_eq!(Category::Nutrition.to_string(), "nutrition");
    assert_eq!(Category::Pain.to_string(), "pain");
    assert_eq!(Category::Habit.to_string(), "habit");
    assert_eq!(Category::Custom.to_string(), "custom");
}

/// Category serialises to snake_case via serde.
#[test]
fn test_category_serde_json() {
    let j = serde_json::to_string(&Category::Body).unwrap();
    assert_eq!(j, "\"body\"");
    let c: Category = serde_json::from_str("\"exercise\"").unwrap();
    assert_eq!(c, Category::Exercise);
}

// ─── default_unit tests ───────────────────────────────────────────────────────

#[test]
fn test_default_unit_known_types() {
    assert_eq!(default_unit("weight"), "kg");
    assert_eq!(default_unit("body_fat"), "%");
    assert_eq!(default_unit("waist"), "cm");
    assert_eq!(default_unit("cardio"), "min");
    assert_eq!(default_unit("strength"), "min");
    assert_eq!(default_unit("calories_burned"), "kcal");
    assert_eq!(default_unit("calories_in"), "kcal");
    assert_eq!(default_unit("sleep_hours"), "hours");
    assert_eq!(default_unit("sleep_quality"), "1-5");
    assert_eq!(default_unit("bed_time"), "HH:MM");
    assert_eq!(default_unit("wake_time"), "HH:MM");
    assert_eq!(default_unit("water"), "ml");
    assert_eq!(default_unit("pain"), "0-10");
    assert_eq!(default_unit("soreness"), "0-10");
    assert_eq!(default_unit("standing_breaks"), "count");
    assert_eq!(default_unit("screen_time"), "hours");
}

#[test]
fn test_default_unit_unknown_returns_empty() {
    assert_eq!(default_unit("mood"), "");
    assert_eq!(default_unit(""), "");
    assert_eq!(default_unit("unknown"), "");
}

// ─── Metric tests ─────────────────────────────────────────────────────────────

#[test]
fn test_metric_new_infers_category_and_unit() {
    let m = Metric::new("weight".to_string(), 85.0);
    assert_eq!(m.metric_type, "weight");
    assert_eq!(m.value, 85.0);
    assert_eq!(m.category, Category::Body);
    assert_eq!(m.unit, "kg");
    assert_eq!(m.source, "manual");
    assert!(m.note.is_none());
    assert!(m.tags.is_empty());
    assert!(!m.id.is_empty());
}

#[test]
fn test_metric_new_generates_unique_ids() {
    let m1 = Metric::new("weight".to_string(), 85.0);
    let m2 = Metric::new("weight".to_string(), 85.0);
    assert_ne!(m1.id, m2.id);
}

#[test]
fn test_metric_new_timestamp_is_recent() {
    let m = Metric::new("pain".to_string(), 3.0);
    let diff = Utc::now().signed_duration_since(m.timestamp).num_seconds();
    assert!(diff < 5, "timestamp should be within the last 5 seconds");
}

#[test]
fn test_metric_new_custom_type_has_empty_unit() {
    let m = Metric::new("mood".to_string(), 7.0);
    assert_eq!(m.unit, "");
    assert_eq!(m.category, Category::Custom);
}

#[test]
fn test_metric_new_exercise_types() {
    let cardio = Metric::new("cardio".to_string(), 30.0);
    assert_eq!(cardio.category, Category::Exercise);
    assert_eq!(cardio.unit, "min");

    let strength = Metric::new("strength".to_string(), 45.0);
    assert_eq!(strength.category, Category::Exercise);
    assert_eq!(strength.unit, "min");
}

#[test]
fn test_metric_new_sleep_types() {
    let sh = Metric::new("sleep_hours".to_string(), 7.5);
    assert_eq!(sh.category, Category::Sleep);
    assert_eq!(sh.unit, "hours");

    let sq = Metric::new("sleep_quality".to_string(), 4.0);
    assert_eq!(sq.category, Category::Sleep);
    assert_eq!(sq.unit, "1-5");
}

/// Metric serialises tags only when non-empty (skip_serializing_if).
#[test]
fn test_metric_serde_tags_omitted_when_empty() {
    let m = Metric::new("water".to_string(), 500.0);
    let json = serde_json::to_string(&m).unwrap();
    assert!(
        !json.contains("\"tags\""),
        "tags field should be omitted when empty"
    );
}

#[test]
fn test_metric_serde_tags_present_when_non_empty() {
    let mut m = Metric::new("water".to_string(), 500.0);
    m.tags = vec!["morning".to_string(), "filtered".to_string()];
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("\"tags\""));
    assert!(json.contains("morning"));
}

/// Metric serialises note only when Some (skip_serializing_if).
#[test]
fn test_metric_serde_note_omitted_when_none() {
    let m = Metric::new("pain".to_string(), 3.0);
    let json = serde_json::to_string(&m).unwrap();
    assert!(
        !json.contains("\"note\""),
        "note should be omitted when None"
    );
}

#[test]
fn test_metric_serde_note_present_when_some() {
    let mut m = Metric::new("pain".to_string(), 3.0);
    m.note = Some("post workout".to_string());
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("\"note\""));
    assert!(json.contains("post workout"));
}

/// Metric JSON uses "type" as the key for metric_type (rename annotation).
#[test]
fn test_metric_serde_type_key_rename() {
    let m = Metric::new("cardio".to_string(), 30.0);
    let json = serde_json::to_string(&m).unwrap();
    assert!(
        json.contains("\"type\""),
        "metric_type should be serialised as 'type'"
    );
    assert!(!json.contains("\"metric_type\""));
}

/// Full round-trip through JSON preserves all fields.
#[test]
fn test_metric_serde_json_roundtrip() {
    let mut m = Metric::new("cardio".to_string(), 45.0);
    m.note = Some("interval session".to_string());
    m.tags = vec!["hiit".to_string()];

    let json = serde_json::to_string(&m).unwrap();
    let back: Metric = serde_json::from_str(&json).unwrap();

    assert_eq!(back.id, m.id);
    assert_eq!(back.metric_type, m.metric_type);
    assert_eq!(back.value, m.value);
    assert_eq!(back.unit, m.unit);
    assert_eq!(back.category, m.category);
    assert_eq!(back.source, m.source);
    assert_eq!(back.note, m.note);
    assert_eq!(back.tags, m.tags);
}

/// Metric can be cloned and the clone is independent.
#[test]
fn test_metric_clone() {
    let mut m = Metric::new("weight".to_string(), 85.0);
    m.note = Some("morning".to_string());
    let mut cloned = m.clone();
    cloned.value = 90.0;
    assert_eq!(m.value, 85.0);
    assert_eq!(cloned.value, 90.0);
}

// ─── Config save/load with temp dir (via direct file I/O) ─────────────────────

// ─── Config::load() / Config::save() direct coverage ─────────────────────────

/// Config::load() returns defaults when the config file does not exist.
/// This exercises the `else` branch of load().
#[test]
fn test_config_load_returns_default_when_no_file() {
    with_temp_openvital_home(|| {
        let path = Config::path();
        assert!(!path.exists(), "temp config should not exist before load");

        let cfg = Config::load().expect("Config::load() should return defaults");
        assert_eq!(cfg.units.weight, "kg");
        assert_eq!(cfg.alerts.pain_threshold, 5);
        assert!(cfg.aliases.is_empty());
    });
}

/// Config::load() covers the file-exists branch with an isolated OPENVITAL_HOME.
#[test]
fn test_config_load_from_existing_file_branch() {
    with_temp_openvital_home(|| {
        let mut cfg = Config::default();
        cfg.profile.height_cm = Some(168.0);
        cfg.units.weight = "lbs".to_string();
        cfg.save().expect("Config::save() should succeed");

        let loaded = Config::load().expect("Config::load() should read saved file");
        assert_eq!(loaded.profile.height_cm, Some(168.0));
        assert_eq!(loaded.units.weight, "lbs");
    });
}

/// Config::save() and Config::load() round-trip end-to-end in an isolated home.
#[test]
fn test_config_save_and_load_with_temp_openvital_home() {
    with_temp_openvital_home(|| {
        let mut cfg = Config::default();
        cfg.profile.birth_year = Some(1992);
        cfg.units.height = "in".to_string();
        cfg.save().expect("Config::save() should succeed");

        let loaded = Config::load().expect("Config::load() should succeed after save");
        assert_eq!(loaded.profile.birth_year, Some(1992));
        assert_eq!(loaded.units.height, "in");
    });
}

/// We test save/load by writing to a file, then reading back with toml directly,
/// since Config::load() / Config::save() use the fixed ~/.openvital path.
#[test]
fn test_config_save_and_reload_via_toml() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.toml");

    let mut cfg = Config::default();
    cfg.profile.height_cm = Some(175.0);
    cfg.profile.birth_year = Some(1985);
    cfg.units.weight = "lbs".to_string();
    cfg.aliases = {
        let mut m = HashMap::new();
        m.insert("wt".to_string(), "weight".to_string());
        m
    };

    let serialised = toml::to_string_pretty(&cfg).unwrap();
    std::fs::write(&config_path, &serialised).unwrap();

    let loaded_str = std::fs::read_to_string(&config_path).unwrap();
    let loaded: Config = toml::from_str(&loaded_str).unwrap();

    assert_eq!(loaded.profile.height_cm, Some(175.0));
    assert_eq!(loaded.profile.birth_year, Some(1985));
    assert_eq!(loaded.units.weight, "lbs");
    assert_eq!(loaded.aliases.get("wt").map(|s| s.as_str()), Some("weight"));
}
