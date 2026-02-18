# Imperial Units Support — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add native imperial unit support — DB always stores metric, input/output converts per user config.

**Architecture:** New `core/units.rs` module handles all conversions. `Units.system` field in config controls metric/imperial. Conversion is applied at the cmd layer boundaries (input parsing and human output), keeping core logic unit-agnostic.

**Tech Stack:** Rust, clap, serde, toml

**Test helpers:** `tests/common/mod.rs` has `setup_db()` → `(TempDir, Database)` and `make_metric(type, value, date)`.

---

### Task 1: Add UnitSystem to config and update Units struct

**Files:**
- Modify: `src/models/config.rs:27-44`
- Test: `tests/models_test.rs`

**Step 1: Write the failing tests**

Add to `tests/models_test.rs`:

```rust
#[test]
fn test_units_default_is_metric() {
    let config = openvital::models::config::Config::default();
    assert_eq!(config.units.system, "metric");
    assert_eq!(config.units.weight, "kg");
}

#[test]
fn test_units_imperial_defaults() {
    let units = openvital::models::config::Units::imperial();
    assert_eq!(units.system, "metric");  // system field says what user chose
    // Wait, actually system should be "imperial"
    assert_eq!(units.system, "imperial");
    assert_eq!(units.weight, "lbs");
    assert_eq!(units.height, "ft");
    assert_eq!(units.water, "fl_oz");
    assert_eq!(units.temperature, "fahrenheit");
}

#[test]
fn test_units_is_imperial() {
    let metric = openvital::models::config::Units::default();
    assert!(!metric.is_imperial());

    let imperial = openvital::models::config::Units::imperial();
    assert!(imperial.is_imperial());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_units_default_is_metric test_units_imperial test_units_is_imperial 2>&1 | tail -10`
Expected: compilation errors — `system` field, `imperial()`, `is_imperial()` don't exist

**Step 3: Implement**

In `src/models/config.rs`, update `Units` struct and impl:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Units {
    #[serde(default = "default_system")]
    pub system: String,
    pub weight: String,
    pub height: String,
    pub water: String,
    pub temperature: String,
}

fn default_system() -> String {
    "metric".to_string()
}

impl Default for Units {
    fn default() -> Self {
        Self {
            system: "metric".to_string(),
            weight: "kg".to_string(),
            height: "cm".to_string(),
            water: "ml".to_string(),
            temperature: "celsius".to_string(),
        }
    }
}

impl Units {
    pub fn imperial() -> Self {
        Self {
            system: "imperial".to_string(),
            weight: "lbs".to_string(),
            height: "ft".to_string(),
            water: "fl_oz".to_string(),
            temperature: "fahrenheit".to_string(),
        }
    }

    pub fn is_imperial(&self) -> bool {
        self.system == "imperial"
    }
}
```

**Step 4: Run tests**

Run: `cargo test --test models_test 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/models/config.rs tests/models_test.rs
git commit -m "feat(config): add UnitSystem to Units struct with imperial() constructor

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 2: Create core/units.rs conversion module

**Files:**
- Create: `src/core/units.rs`
- Modify: `src/core/mod.rs` (add `pub mod units;`)
- Test: `tests/units_test.rs` (new file)
- Create: `tests/units_test.rs`

**Step 1: Write the failing tests**

Create `tests/units_test.rs`:

```rust
use openvital::core::units;
use openvital::models::config::Units;

// ── to_display ──────────────────────────────────────────────────────────────

#[test]
fn test_to_display_weight_metric_no_change() {
    let u = Units::default();
    let (val, unit) = units::to_display(72.5, "weight", &u);
    assert!((val - 72.5).abs() < 0.01);
    assert_eq!(unit, "kg");
}

#[test]
fn test_to_display_weight_imperial() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(72.5, "weight", &u);
    // 72.5 kg * 2.20462 = 159.8
    assert!((val - 159.8).abs() < 0.2);
    assert_eq!(unit, "lbs");
}

#[test]
fn test_to_display_waist_imperial() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(80.0, "waist", &u);
    // 80 cm / 2.54 = 31.5 in
    assert!((val - 31.5).abs() < 0.1);
    assert_eq!(unit, "in");
}

#[test]
fn test_to_display_water_imperial() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(2000.0, "water", &u);
    // 2000 ml / 29.5735 = 67.6 fl oz
    assert!((val - 67.6).abs() < 0.2);
    assert_eq!(unit, "fl oz");
}

#[test]
fn test_to_display_temperature_imperial() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(37.0, "temperature", &u);
    // 37 C * 1.8 + 32 = 98.6 F
    assert!((val - 98.6).abs() < 0.1);
    assert_eq!(unit, "°F");
}

#[test]
fn test_to_display_unaffected_metric() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(8.0, "sleep", &u);
    assert!((val - 8.0).abs() < 0.01);
    assert_eq!(unit, "hours");
}

#[test]
fn test_to_display_heart_rate_unaffected() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(68.0, "heart_rate", &u);
    assert!((val - 68.0).abs() < 0.01);
    assert_eq!(unit, "bpm");
}

// ── from_input ──────────────────────────────────────────────────────────────

#[test]
fn test_from_input_weight_metric_no_change() {
    let u = Units::default();
    let val = units::from_input(72.5, "weight", &u);
    assert!((val - 72.5).abs() < 0.01);
}

#[test]
fn test_from_input_weight_imperial() {
    let u = Units::imperial();
    let val = units::from_input(160.0, "weight", &u);
    // 160 lbs / 2.20462 = 72.57 kg
    assert!((val - 72.57).abs() < 0.1);
}

#[test]
fn test_from_input_water_imperial() {
    let u = Units::imperial();
    let val = units::from_input(67.6, "water", &u);
    // 67.6 fl_oz * 29.5735 = ~2000 ml
    assert!((val - 2000.0).abs() < 5.0);
}

#[test]
fn test_from_input_temperature_imperial() {
    let u = Units::imperial();
    let val = units::from_input(98.6, "temperature", &u);
    // (98.6 - 32) / 1.8 = 37.0 C
    assert!((val - 37.0).abs() < 0.1);
}

#[test]
fn test_from_input_sleep_unaffected() {
    let u = Units::imperial();
    let val = units::from_input(8.0, "sleep", &u);
    assert!((val - 8.0).abs() < 0.01);
}

// ── round-trip ──────────────────────────────────────────────────────────────

#[test]
fn test_round_trip_weight_imperial() {
    let u = Units::imperial();
    let stored = units::from_input(160.0, "weight", &u);
    let (displayed, _) = units::to_display(stored, "weight", &u);
    assert!((displayed - 160.0).abs() < 0.1, "round-trip got {}", displayed);
}

#[test]
fn test_round_trip_temperature_imperial() {
    let u = Units::imperial();
    let stored = units::from_input(98.6, "temperature", &u);
    let (displayed, _) = units::to_display(stored, "temperature", &u);
    assert!((displayed - 98.6).abs() < 0.1, "round-trip got {}", displayed);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test units_test 2>&1 | tail -10`
Expected: compilation error — `core::units` module doesn't exist

**Step 3: Implement**

Add to `src/core/mod.rs`:
```rust
pub mod units;
```

Create `src/core/units.rs`:

```rust
use crate::models::config::Units;
use crate::models::metric::default_unit;

const KG_TO_LBS: f64 = 2.20462;
const CM_TO_IN: f64 = 2.54;
const ML_TO_FLOZ: f64 = 29.5735;

/// Convert a stored (metric) value to display value + display unit string.
/// For metric config, returns the value unchanged with its default unit.
/// For imperial config, converts applicable metric types.
pub fn to_display(value: f64, metric_type: &str, units: &Units) -> (f64, String) {
    if !units.is_imperial() {
        return (value, default_unit(metric_type).to_string());
    }

    match metric_type {
        "weight" => (round1(value * KG_TO_LBS), "lbs".to_string()),
        "waist" => (round1(value / CM_TO_IN), "in".to_string()),
        "water" => (round1(value / ML_TO_FLOZ), "fl oz".to_string()),
        "temperature" => (round1(value * 1.8 + 32.0), "°F".to_string()),
        _ => (value, default_unit(metric_type).to_string()),
    }
}

/// Convert a user-input value (in their configured unit system) to metric for storage.
/// For metric config, returns the value unchanged.
/// For imperial config, converts applicable metric types to metric.
pub fn from_input(value: f64, metric_type: &str, units: &Units) -> f64 {
    if !units.is_imperial() {
        return value;
    }

    match metric_type {
        "weight" => value / KG_TO_LBS,
        "waist" => value * CM_TO_IN,
        "water" => value * ML_TO_FLOZ,
        "temperature" => (value - 32.0) / 1.8,
        _ => value,
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
```

**Step 4: Run tests**

Run: `cargo test --test units_test 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/core/units.rs src/core/mod.rs tests/units_test.rs
git commit -m "feat(units): add core/units.rs with to_display/from_input conversions

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 3: Wire conversion into log input path

**Files:**
- Modify: `src/cmd/log.rs:78-81` (single log value parsing)
- Modify: `src/cmd/log.rs:29-34` (BP values)
- Test: `tests/cli_integration.rs`

**Step 1: Write the failing test**

Add to `tests/cli_integration.rs`:

```rust
#[test]
fn test_log_weight_imperial_converts_to_metric() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let db = openvital::db::Database::open(&db_path).unwrap();

    // Set up imperial config
    let mut config = openvital::models::config::Config::default();
    config.units = openvital::models::config::Units::imperial();

    // User inputs 160 lbs → should store as ~72.57 kg
    let m = openvital::core::logging::log_metric(
        &db, &config,
        openvital::core::logging::LogEntry {
            metric_type: "weight",
            value: openvital::core::units::from_input(160.0, "weight", &config.units),
            note: None, tags: None, source: None, date: None,
        },
    ).unwrap();

    assert_eq!(m.unit, "kg"); // stored in metric
    assert!((m.value - 72.57).abs() < 0.1, "stored value should be ~72.57 kg, got {}", m.value);
}
```

Note: This test verifies the conversion function works in context. The actual wiring into `cmd/log.rs` is tested via the CLI subprocess tests below.

**Step 2: Run test — should pass** (the conversion logic is already implemented in Task 2)

**Step 3: Wire into cmd/log.rs**

In `src/cmd/log.rs`, the `run()` function needs to convert the parsed value before logging. After line 81 (`let value: f64 = value_str.parse()...`), add conversion:

Change lines 78-93 from:
```rust
    // Normal single-value log
    let value: f64 = value_str
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid value: {}", value_str))?;
    let m = openvital::core::logging::log_metric(
        &db,
        &config,
        LogEntry {
            metric_type,
            value,
```

To:
```rust
    // Normal single-value log
    let parsed: f64 = value_str
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid value: {}", value_str))?;
    let resolved_type = config.resolve_alias(metric_type);
    let value = openvital::core::units::from_input(parsed, &resolved_type, &config.units);
    let m = openvital::core::logging::log_metric(
        &db,
        &config,
        LogEntry {
            metric_type,
            value,
```

Also update BP compound values (lines 29-34). After parsing systolic and diastolic as f64, convert them:
```rust
        let systolic = openvital::core::units::from_input(systolic, "bp_systolic", &config.units);
        let diastolic = openvital::core::units::from_input(diastolic, "bp_diastolic", &config.units);
```

(BP values don't need conversion since mmHg is universal, but the code should be consistent.)

**Step 4: Run ALL tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/cmd/log.rs tests/cli_integration.rs
git commit -m "feat(log): convert input values from user unit system to metric for storage

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 4: Wire conversion into human output path

**Files:**
- Modify: `src/output/human.rs:5-15` (format_metric)
- Test: `tests/output_test.rs`

**Step 1: Write the failing test**

Add to `tests/output_test.rs`:

```rust
#[test]
fn test_format_metric_imperial_weight() {
    use openvital::models::config::Units;
    let mut m = openvital::models::metric::Metric::new("weight".to_string(), 72.5);
    // format_metric_with_units should convert 72.5 kg → ~159.8 lbs
    let units = Units::imperial();
    let line = openvital::output::human::format_metric_with_units(&m, &units);
    assert!(line.contains("lbs"), "expected lbs in output, got: {}", line);
    assert!(line.contains("159"), "expected ~159 in output, got: {}", line);
}

#[test]
fn test_format_metric_metric_weight_unchanged() {
    let m = openvital::models::metric::Metric::new("weight".to_string(), 72.5);
    let units = openvital::models::config::Units::default();
    let line = openvital::output::human::format_metric_with_units(&m, &units);
    assert!(line.contains("kg"), "expected kg, got: {}", line);
    assert!(line.contains("72.5"), "expected 72.5, got: {}", line);
}
```

**Step 2: Run tests — should fail** (format_metric_with_units doesn't exist)

**Step 3: Implement**

In `src/output/human.rs`, add `format_metric_with_units`:

```rust
use crate::core::units;
use crate::models::config::Units;

/// Pretty-print a metric entry with unit conversion.
pub fn format_metric_with_units(m: &Metric, user_units: &Units) -> String {
    let ts = m.timestamp.format("%Y-%m-%d %H:%M");
    let (display_val, display_unit) = units::to_display(m.value, &m.metric_type, user_units);
    let mut line = format!("{} | {} = {} {}", ts, m.metric_type, display_val, display_unit);
    if let Some(ref note) = m.note {
        line.push_str(&format!("  # {}", note));
    }
    if !m.tags.is_empty() {
        line.push_str(&format!("  [{}]", m.tags.join(", ")));
    }
    line
}
```

Keep the existing `format_metric()` as-is for backward compatibility (it uses the stored unit directly, which is fine for JSON/metric contexts).

**Step 4: Run tests**

Run: `cargo test --test output_test 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/output/human.rs tests/output_test.rs
git commit -m "feat(output): add format_metric_with_units for imperial display

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 5: Wire format_metric_with_units into cmd layer

**Files:**
- Modify: `src/cmd/log.rs` (pass config.units to human output)
- Modify: `src/cmd/show.rs` (human output with units)
- Modify: `src/cmd/trend.rs` (human output with units)
- Modify: `src/cmd/goal.rs` (human output with units)
- Modify: `src/cmd/status.rs` (no change needed — status doesn't show individual metric values)

**Step 1: Write a CLI integration test**

Add to `tests/cli_integration.rs`:

```rust
#[test]
fn test_cli_log_and_show_imperial_weight() {
    let dir = tempfile::TempDir::new().unwrap();
    let bin = env!("CARGO_BIN_EXE_openvital");

    // Init
    std::process::Command::new(bin)
        .args(["init", "--skip"])
        .env("OPENVITAL_HOME", dir.path())
        .output()
        .unwrap();

    // Set imperial
    std::process::Command::new(bin)
        .args(["config", "set", "units.system", "imperial"])
        .env("OPENVITAL_HOME", dir.path())
        .output()
        .unwrap();

    // Log weight 160 (lbs) → stored as ~72.6 kg
    let log_out = std::process::Command::new(bin)
        .args(["log", "weight", "160", "--human"])
        .env("OPENVITAL_HOME", dir.path())
        .output()
        .unwrap();
    let log_stdout = String::from_utf8_lossy(&log_out.stdout);
    assert!(log_out.status.success(), "stderr: {}", String::from_utf8_lossy(&log_out.stderr));
    assert!(log_stdout.contains("lbs"), "human output should show lbs, got: {}", log_stdout);

    // Show weight --human should also display in lbs
    let show_out = std::process::Command::new(bin)
        .args(["show", "weight", "--human"])
        .env("OPENVITAL_HOME", dir.path())
        .output()
        .unwrap();
    let show_stdout = String::from_utf8_lossy(&show_out.stdout);
    assert!(show_stdout.contains("lbs"), "show should display lbs, got: {}", show_stdout);
    assert!(show_stdout.contains("160"), "show should display ~160, got: {}", show_stdout);
}
```

**Step 2: Run test — should fail** (config set units.system not supported, cmd/log doesn't use format_metric_with_units yet)

**Step 3: Implement**

**3a. Add `units.system` to config set** in `src/cmd/config.rs`:

Add a match arm before the `_ => anyhow::bail!` line:

```rust
        "units.system" => {
            match value {
                "metric" => config.units = Units::default(),
                "imperial" => config.units = Units::imperial(),
                _ => anyhow::bail!("units.system must be 'metric' or 'imperial'"),
            }
        }
```

Add `use openvital::models::config::Units;` at top.

**3b. Update cmd/log.rs** — use `format_metric_with_units` for human output:

Replace the human output in `run()` (line 95-96):
```rust
    if human_flag {
        println!("Logged: {}", human::format_metric_with_units(&m, &config.units));
    }
```

And in BP output (line 61-62), convert display values:
```rust
        if human_flag {
            let (sv, su) = openvital::core::units::to_display(m1.value, "bp_systolic", &config.units);
            let (dv, _) = openvital::core::units::to_display(m2.value, "bp_diastolic", &config.units);
            println!("Logged: BP {}/{} {}", sv, dv, su);
        }
```

And in batch human output (line 129-131):
```rust
        for m in &metrics {
            println!("Logged: {}", human::format_metric_with_units(m, &config.units));
        }
```

**3c. Update cmd/show.rs** — pass units to human formatting:

The show command already loads config. Update the human output to use `format_metric_with_units`:

```rust
// In the human output section where entries are printed, use:
human::format_metric_with_units(&entry, &config.units)
```

(Read cmd/show.rs first to see exact lines to change.)

**3d. Update cmd/trend.rs** — convert values in human output:

In the trend human output (lines 21-25), convert avg/min/max:

```rust
            for d in &result.data {
                let (avg, unit) = openvital::core::units::to_display(d.avg, &resolved, &config.units);
                let (min, _) = openvital::core::units::to_display(d.min, &resolved, &config.units);
                let (max, _) = openvital::core::units::to_display(d.max, &resolved, &config.units);
                println!(
                    "  {} | avg: {:.1}  min: {:.1}  max: {:.1}  (n={})",
                    d.label, avg, min, max, d.count
                );
            }
```

And for projection (line 32):
```rust
            if let Some(p) = result.trend.projected_30d {
                let (pv, pu) = openvital::core::units::to_display(p, &resolved, &config.units);
                println!("  30-day projection: {:.1} {}", pv, pu);
            }
```

**3e. Update cmd/goal.rs** — convert target and current values in human output:

In `run_set` (line 26-28):
```rust
        let (display_target, display_unit) =
            openvital::core::units::to_display(goal.target_value, &goal.metric_type, &config.units);
        println!("Goal set: {} {} {} {} ({})",
            goal.metric_type, goal.direction, display_target, display_unit, goal.timeframe);
```

In `run_status` — the target_value in goal needs conversion. Add to the human output (line 48-54). Convert `s.target_value` for display. Also need to pass config to run_status — this requires a small change: load config in run_status (it already does for alias resolution).

```rust
        for s in &statuses {
            let met = if s.is_met { "MET" } else { "..." };
            let (display_target, _) = openvital::core::units::to_display(
                s.target_value, &s.metric_type, &config.units);
            let progress = s.progress.as_deref().unwrap_or("no data");
            println!(
                "[{}] {} {} {} ({}) — {}",
                met, s.metric_type, s.direction, display_target, s.timeframe, progress
            );
        }
```

Note: The `progress` string contains raw metric values. For full imperial support in progress text, `format_progress` in `core/goal.rs` would need the Units struct too. For now, converting just the target value in the status line is sufficient. Progress string conversion can be a follow-up.

**3f. Wire goal set input conversion** in `src/cmd/goal.rs` `run_set`:

After parsing target_value (line 22), convert from user units to metric:
```rust
    let stored_target = openvital::core::units::from_input(target_value, &resolved, &config.units);
    let goal = openvital::core::goal::set_goal(&db, resolved, stored_target, dir, tf)?;
```

**Step 4: Run ALL tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/cmd/log.rs src/cmd/show.rs src/cmd/trend.rs src/cmd/goal.rs src/cmd/config.rs tests/cli_integration.rs
git commit -m "feat: wire imperial unit conversion into all cmd layer I/O paths

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 6: Add --units flag to init command

**Files:**
- Modify: `src/cli.rs` (add `--units` to Init)
- Modify: `src/cmd/init.rs` (handle imperial init)
- Modify: `src/main.rs` (pass units to init)
- Test: `tests/cli_integration.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_init_with_imperial_flag() {
    let dir = tempfile::TempDir::new().unwrap();
    let bin = env!("CARGO_BIN_EXE_openvital");

    let output = std::process::Command::new(bin)
        .args(["init", "--skip", "--units", "imperial"])
        .env("OPENVITAL_HOME", dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());

    // Verify config was saved with imperial
    let config_path = dir.path().join("config.toml");
    let contents = std::fs::read_to_string(config_path).unwrap();
    assert!(contents.contains("imperial"), "config should contain imperial, got: {}", contents);
    assert!(contents.contains("lbs"), "config should contain lbs, got: {}", contents);
}
```

**Step 2: Run test — should fail**

**Step 3: Implement**

In `src/cli.rs`, add to `Init`:
```rust
    Init {
        #[arg(long)]
        skip: bool,
        /// Unit system: metric or imperial
        #[arg(long, default_value = "metric")]
        units: String,
    },
```

In `src/main.rs`, update Init dispatch:
```rust
        Commands::Init { skip, units } => cmd::init::run(skip, &units),
```

In `src/cmd/init.rs`, update `run()`:
```rust
pub fn run(skip: bool, unit_system: &str) -> Result<()> {
    let mut config = Config::load().unwrap_or_default();

    if config.aliases.is_empty() {
        config.aliases = Config::default_aliases();
    }

    match unit_system {
        "imperial" => config.units = openvital::models::config::Units::imperial(),
        "metric" => {} // keep default
        _ => anyhow::bail!("units must be 'metric' or 'imperial'"),
    }

    // ... rest unchanged
```

Also update the interactive prompts to show correct units (lines 18-19):
```rust
        let height_label = if config.units.is_imperial() { "Height (ft, e.g. 5.83 for 5'10\")" } else { "Height (cm)" };
        let weight_label = if config.units.is_imperial() { "Current weight (lbs)" } else { "Current weight (kg)" };
        config.profile.height_cm = Some({
            let raw = prompt_f64(height_label)?;
            openvital::core::units::from_input(raw, "height", &config.units)
        });
        let weight = {
            let raw = prompt_f64(weight_label)?;
            openvital::core::units::from_input(raw, "weight", &config.units)
        };
```

Note: Height in imperial is tricky (5'10" = 5.833 ft = 177.8 cm). For the `--skip` path, no conversion needed since no values are entered. For the interactive path, accept decimal feet for now (e.g., 5.83). A `5'10"` parser can be a follow-up.

**Step 4: Run tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/cli.rs src/cmd/init.rs src/main.rs tests/cli_integration.rs
git commit -m "feat(init): add --units imperial flag for initial setup

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 7: Add height conversion (ft'in" display)

**Files:**
- Modify: `src/core/units.rs` (add height-specific conversion)
- Test: `tests/units_test.rs`

**Step 1: Write the failing test**

Add to `tests/units_test.rs`:

```rust
#[test]
fn test_to_display_height_imperial() {
    let u = Units::imperial();
    // 178 cm = 5'10"
    let (val, unit) = units::to_display(178.0, "height", &u);
    // For simplicity, display as decimal feet: 5.8
    assert!((val - 5.8).abs() < 0.1);
    assert_eq!(unit, "ft");
}

#[test]
fn test_from_input_height_imperial() {
    let u = Units::imperial();
    // 5.83 ft → 177.7 cm
    let val = units::from_input(5.83, "height", &u);
    assert!((val - 177.7).abs() < 0.5);
}
```

**Step 2: Run test — should fail** (height not handled in units.rs)

**Step 3: Implement**

In `src/core/units.rs`, add height to the match arms:

In `to_display`:
```rust
        "height" => (round1(value / 30.48), "ft".to_string()),
```

In `from_input`:
```rust
        "height" => value * 30.48,
```

(30.48 cm per foot)

**Step 4: Run tests**

Run: `cargo test --test units_test 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/core/units.rs tests/units_test.rs
git commit -m "feat(units): add height cm↔ft conversion

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 8: Final verification

**Step 1: Run full test suite**

Run: `cargo test 2>&1 | tail -10`
Expected: all PASS

**Step 2: Run linting**

Run: `cargo fmt --all -- --check && cargo clippy -- -D warnings 2>&1 | tail -5`
Expected: clean

**Step 3: Manual verification**

```bash
OV=target/release/openvital
export OPENVITAL_HOME=$(mktemp -d)

# Init with imperial
$OV init --skip --units imperial

# Log in imperial
$OV log weight 160 --human           # should show "160 lbs"
$OV log water 67 --human             # should show "67 fl oz"

# Show in imperial
$OV show weight --human              # should show lbs

# Goal in imperial
$OV goal set weight 155 below daily --human  # should show lbs

# Config show
$OV config show --human              # should show system = "imperial"
```

**Step 4: Commit if any cleanup needed**

```bash
git add -A && git commit -m "chore: final cleanup for imperial units support

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```
