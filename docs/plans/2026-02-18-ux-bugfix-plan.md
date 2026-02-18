# OpenVital v0.2: Bug Fixes & UX Improvements — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix 4 bugs and deliver 5 UX improvements discovered during product trial.

**Architecture:** Changes span all 4 layers (CLI → Command → Core → DB is untouched). Most changes are in `models/metric.rs` (new helpers), `core/goal.rs` (aggregation fix), `cmd/log.rs` (BP + batch), and `core/trend.rs` (projection clamp). Each task is independently testable.

**Tech Stack:** Rust, clap, chrono, serde_json, rusqlite, tempfile (tests)

**Test helpers:** `tests/common/mod.rs` provides `setup_db()` → `(TempDir, Database)` and `make_metric(type, value, date)` → `Metric`.

---

### Task 1: Add `is_cumulative()` helper + extend `default_unit()` (BUG-1 + UX-2)

**Files:**
- Modify: `src/models/metric.rs:46-63`
- Test: `tests/models_test.rs`

**Step 1: Write the failing tests**

Add to `tests/models_test.rs`:

```rust
#[test]
fn test_is_cumulative_water_steps() {
    assert!(openvital::models::metric::is_cumulative("water"));
    assert!(openvital::models::metric::is_cumulative("steps"));
    assert!(openvital::models::metric::is_cumulative("calories_in"));
    assert!(openvital::models::metric::is_cumulative("calories_burned"));
    assert!(openvital::models::metric::is_cumulative("standing_breaks"));
}

#[test]
fn test_is_cumulative_false_for_snapshot_metrics() {
    assert!(!openvital::models::metric::is_cumulative("weight"));
    assert!(!openvital::models::metric::is_cumulative("sleep"));
    assert!(!openvital::models::metric::is_cumulative("mood"));
    assert!(!openvital::models::metric::is_cumulative("heart_rate"));
    assert!(!openvital::models::metric::is_cumulative("body_fat"));
}

#[test]
fn test_default_unit_sleep_steps_mood_hr_bp() {
    use openvital::models::metric::default_unit;
    assert_eq!(default_unit("sleep"), "hours");
    assert_eq!(default_unit("steps"), "steps");
    assert_eq!(default_unit("mood"), "1-10");
    assert_eq!(default_unit("heart_rate"), "bpm");
    assert_eq!(default_unit("bp_systolic"), "mmHg");
    assert_eq!(default_unit("bp_diastolic"), "mmHg");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_is_cumulative test_default_unit_sleep -- --test-threads=1 2>&1 | tail -10`
Expected: compilation error — `is_cumulative` not found, new unit assertions fail

**Step 3: Implement**

In `src/models/metric.rs`, add after the `default_unit` function:

```rust
/// Whether a metric type is cumulative (sum values) vs snapshot (use latest).
pub fn is_cumulative(metric_type: &str) -> bool {
    matches!(metric_type, "water" | "steps" | "calories_in" | "calories_burned" | "standing_breaks")
}
```

And extend the `default_unit` match arms — add before the `_ => ""` catch-all:

```rust
        "sleep" => "hours",
        "steps" => "steps",
        "mood" => "1-10",
        "heart_rate" => "bpm",
        "bp_systolic" | "bp_diastolic" => "mmHg",
```

**Step 4: Run tests to verify they pass**

Run: `cargo test test_is_cumulative test_default_unit_sleep 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/models/metric.rs tests/models_test.rs
git commit -m "feat(models): add is_cumulative() and extend default_unit() for common metrics"
```

---

### Task 2: Fix goal aggregation for snapshot metrics (BUG-1)

**Files:**
- Modify: `src/core/goal.rs:72-118`
- Test: `tests/core_goal.rs`

**Step 1: Write the failing test**

Add to `tests/core_goal.rs`:

```rust
#[test]
fn test_daily_above_goal_snapshot_uses_last_value_not_sum() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();

    // Log sleep twice: 7.5 + 7.5 = 15 if summed, but should use last (7.5)
    let m1 = common::make_metric("sleep", 7.5, today);
    db.insert_metric(&m1).unwrap();
    let m2 = common::make_metric("sleep", 7.5, today);
    db.insert_metric(&m2).unwrap();

    goal::set_goal(&db, "sleep".into(), 8.0, Direction::Above, Timeframe::Daily).unwrap();
    let statuses = goal::goal_status(&db, Some("sleep")).unwrap();

    assert_eq!(statuses.len(), 1);
    // sleep is NOT cumulative, so current should be 7.5 (last), NOT 15 (sum)
    assert_eq!(statuses[0].current_value, Some(7.5));
    assert!(!statuses[0].is_met); // 7.5 < 8.0
}

#[test]
fn test_daily_above_goal_cumulative_uses_sum() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();

    // Log water twice: 500 + 800 = 1300
    let m1 = common::make_metric("water", 500.0, today);
    db.insert_metric(&m1).unwrap();
    let m2 = common::make_metric("water", 800.0, today);
    db.insert_metric(&m2).unwrap();

    goal::set_goal(&db, "water".into(), 2000.0, Direction::Above, Timeframe::Daily).unwrap();
    let statuses = goal::goal_status(&db, Some("water")).unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].current_value, Some(1300.0)); // summed
    assert!(!statuses[0].is_met); // 1300 < 2000
}

#[test]
fn test_weekly_goal_snapshot_uses_latest_value() {
    let (_dir, db) = common::setup_db();
    let today = chrono::Local::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);

    let m1 = common::make_metric("weight", 73.0, yesterday);
    db.insert_metric(&m1).unwrap();
    let m2 = common::make_metric("weight", 72.5, today);
    db.insert_metric(&m2).unwrap();

    goal::set_goal(&db, "weight".into(), 70.0, Direction::Below, Timeframe::Weekly).unwrap();
    let statuses = goal::goal_status(&db, Some("weight")).unwrap();

    assert_eq!(statuses.len(), 1);
    // For snapshot metrics weekly, use the latest value (72.5), not sum (145.5)
    assert_eq!(statuses[0].current_value, Some(72.5));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_daily_above_goal_snapshot test_daily_above_goal_cumulative test_weekly_goal_snapshot_uses 2>&1 | tail -10`
Expected: `test_daily_above_goal_snapshot_uses_last_value_not_sum` FAILS (gets 15.0 instead of 7.5)

**Step 3: Implement**

Replace the `compute_current` function in `src/core/goal.rs` (lines 72-119):

```rust
fn compute_current(db: &Database, goal: &Goal, today: NaiveDate) -> Result<Option<f64>> {
    use crate::models::metric::is_cumulative;
    let cumulative = is_cumulative(&goal.metric_type);

    match goal.timeframe {
        Timeframe::Daily => {
            let entries = db.query_by_date(today)?;
            let day_entries: Vec<_> = entries
                .iter()
                .filter(|m| m.metric_type == goal.metric_type)
                .collect();
            if day_entries.is_empty() {
                return Ok(None);
            }
            if cumulative {
                Ok(Some(day_entries.iter().map(|m| m.value).sum()))
            } else {
                Ok(Some(day_entries.last().unwrap().value))
            }
        }
        Timeframe::Weekly => {
            let weekday = today.weekday().num_days_from_monday();
            let week_start = today - chrono::Duration::days(weekday as i64);
            let mut values = Vec::new();
            for i in 0..7 {
                let date = week_start + chrono::Duration::days(i);
                if date > today {
                    break;
                }
                let entries = db.query_by_date(date)?;
                for m in &entries {
                    if m.metric_type == goal.metric_type {
                        values.push(m.value);
                    }
                }
            }
            if values.is_empty() {
                Ok(None)
            } else if cumulative {
                Ok(Some(values.iter().sum()))
            } else {
                Ok(Some(*values.last().unwrap()))
            }
        }
        Timeframe::Monthly => {
            let entries = db.query_by_type(&goal.metric_type, Some(1))?;
            Ok(entries.first().map(|m| m.value))
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test core_goal 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/core/goal.rs tests/core_goal.rs
git commit -m "fix(goal): use is_cumulative() to decide sum vs latest for goal aggregation"
```

---

### Task 3: Show default `--last 10` (UX-1)

**Files:**
- Modify: `src/core/query.rs:41`
- Test: `tests/core_query.rs`

**Step 1: Write the failing test**

Add to `tests/core_query.rs`:

```rust
#[test]
fn test_show_by_type_defaults_to_10_entries() {
    let (_dir, db) = common::setup_db();
    let config = openvital::models::config::Config::default();
    let base_date = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();

    // Insert 15 weight entries on different days
    for i in 0..15 {
        let date = base_date + chrono::Duration::days(i);
        let m = common::make_metric("weight", 70.0 + i as f64 * 0.1, date);
        db.insert_metric(&m).unwrap();
    }

    // show weight with no --last → should get 10 (not 1)
    let result = openvital::core::query::show(&db, &config, Some("weight"), None, None).unwrap();
    match result {
        openvital::core::query::ShowResult::ByType { entries, .. } => {
            assert_eq!(entries.len(), 10);
        }
        _ => panic!("expected ByType"),
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_show_by_type_defaults_to_10 2>&1 | tail -5`
Expected: FAIL — gets 1 entry instead of 10

**Step 3: Implement**

In `src/core/query.rs` line 41, change `unwrap_or(1)` to `unwrap_or(10)`:

```rust
    let entries = db.query_by_type(&resolved, Some(last.unwrap_or(10)))?;
```

**Step 4: Run tests**

Run: `cargo test core_query 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/core/query.rs tests/core_query.rs
git commit -m "feat(show): default --last from 1 to 10 entries"
```

---

### Task 4: Clamp trend projection (BUG-4)

**Files:**
- Modify: `src/core/trend.rs:186-200`
- Test: `tests/trend.rs`

**Step 1: Write the failing test**

Add to `tests/trend.rs`:

```rust
#[test]
fn test_projection_clamped_to_reasonable_range() {
    let (_dir, db) = common::setup_db();

    // Create data with steep downward trend: 80, 60 over 2 weeks
    let w1_date = chrono::NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();
    let w2_date = chrono::NaiveDate::from_ymd_opt(2026, 1, 13).unwrap();

    let m1 = common::make_metric("weight", 80.0, w1_date);
    db.insert_metric(&m1).unwrap();
    let m2 = common::make_metric("weight", 60.0, w2_date);
    db.insert_metric(&m2).unwrap();

    let result = openvital::core::trend::compute(
        &db,
        "weight",
        openvital::core::trend::TrendPeriod::Weekly,
        None,
    )
    .unwrap();

    let projected = result.trend.projected_30d.unwrap();
    // Without clamp, projection would be 60 + (-20 * 4.3) ≈ -26 (absurd)
    // With clamp, should be >= 60 * 0.5 = 30
    assert!(projected >= 30.0, "projection {} should be >= 30.0", projected);
    assert!(projected >= 0.0, "projection should never be negative");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_projection_clamped 2>&1 | tail -10`
Expected: FAIL — projection is negative or unreasonably low

**Step 3: Implement**

In `src/core/trend.rs`, replace lines 192-193 with:

```rust
    let last_avg = ys.last().unwrap();
    let raw_projected = last_avg + slope * periods_in_30d;
    // Clamp: never negative, never beyond ±50% of current value
    let min_proj = (last_avg * 0.5).max(0.0);
    let max_proj = last_avg * 1.5;
    let projected = (raw_projected.clamp(min_proj, max_proj) * 10.0).round() / 10.0;
```

**Step 4: Run tests**

Run: `cargo test trend 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/core/trend.rs tests/trend.rs
git commit -m "fix(trend): clamp 30-day projection to ±50% of current value"
```

---

### Task 5: Blood pressure compound value support (BUG-2)

**Files:**
- Modify: `src/cli.rs:48-49` (VALUE type change)
- Modify: `src/cmd/log.rs:11-53` (BP detection + split)
- Modify: `src/main.rs:23-34` (pass String value)
- Modify: `src/core/logging.rs` (no change needed — BP splits in cmd layer)
- Test: `tests/cli_integration.rs`

**Step 1: Write the failing test**

Add to `tests/cli_integration.rs`:

```rust
#[test]
fn test_log_blood_pressure_compound_value() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let db = openvital::db::Database::open(&db_path).unwrap();
    let config = openvital::models::config::Config::default();

    // Simulate logging blood_pressure 120/80 — should create 2 entries
    let systolic = openvital::core::logging::log_metric(
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

    let diastolic = openvital::core::logging::log_metric(
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

    assert_eq!(systolic.unit, "mmHg");
    assert_eq!(diastolic.unit, "mmHg");
    assert_eq!(systolic.value, 120.0);
    assert_eq!(diastolic.value, 80.0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_log_blood_pressure_compound 2>&1 | tail -5`
Expected: FAIL — unit is "" not "mmHg" (until Task 1 is done, then this passes for units but we still need the CLI parsing)

**Step 3: Implement CLI changes**

In `src/cli.rs` line 48-49, change VALUE from `f64` to `String`:

```rust
        /// Metric value
        #[arg(required_unless_present = "batch")]
        value: Option<String>,
```

In `src/main.rs` lines 21-35, update the Log branch:

```rust
        Commands::Log {
            r#type,
            value,
            note,
            tags,
            source,
            batch,
        } => {
            if let Some(batch_json) = batch {
                cmd::log::run_batch(&batch_json, cli.human)
            } else {
                let t = r#type.as_deref().expect("type is required");
                let v = value.as_deref().expect("value is required");
                cmd::log::run(
                    t,
                    v,
                    note.as_deref(),
                    tags.as_deref(),
                    source.as_deref(),
                    cli.date,
                    cli.human,
                )
            }
        }
```

In `src/cmd/log.rs`, change `run()` signature and add BP logic:

```rust
pub fn run(
    metric_type: &str,
    value_str: &str,
    note: Option<&str>,
    tags: Option<&str>,
    source: Option<&str>,
    date: Option<NaiveDate>,
    human_flag: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;

    // Check for blood pressure compound value (e.g., "120/80")
    if (metric_type == "blood_pressure" || metric_type == "bp")
        && value_str.contains('/')
    {
        let parts: Vec<&str> = value_str.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!("blood pressure format must be SYSTOLIC/DIASTOLIC (e.g., 120/80)");
        }
        let systolic: f64 = parts[0].parse().map_err(|_| anyhow::anyhow!("invalid systolic value"))?;
        let diastolic: f64 = parts[1].parse().map_err(|_| anyhow::anyhow!("invalid diastolic value"))?;

        let m1 = openvital::core::logging::log_metric(
            &db, &config,
            LogEntry { metric_type: "bp_systolic", value: systolic, note, tags, source, date },
        )?;
        let m2 = openvital::core::logging::log_metric(
            &db, &config,
            LogEntry { metric_type: "bp_diastolic", value: diastolic, note, tags, source, date },
        )?;

        if human_flag {
            println!("Logged: BP {}/{} {}", m1.value, m2.value, m1.unit);
        } else {
            let out = output::success("log", json!({
                "entries": [
                    {"id": m1.id, "type": m1.metric_type, "value": m1.value, "unit": m1.unit},
                    {"id": m2.id, "type": m2.metric_type, "value": m2.value, "unit": m2.unit}
                ]
            }));
            println!("{}", serde_json::to_string(&out)?);
        }
        return Ok(());
    }

    // Normal single-value log
    let value: f64 = value_str.parse().map_err(|_| anyhow::anyhow!("invalid value: {}", value_str))?;
    let m = openvital::core::logging::log_metric(
        &db, &config,
        LogEntry { metric_type, value, note, tags, source, date },
    )?;

    if human_flag {
        println!("Logged: {}", human::format_metric(&m));
    } else {
        let out = output::success("log", json!({
            "entry": {
                "id": m.id,
                "timestamp": m.timestamp.to_rfc3339(),
                "type": m.metric_type,
                "value": m.value,
                "unit": m.unit
            }
        }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
```

**Step 4: Run tests**

Run: `cargo test 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/cli.rs src/main.rs src/cmd/log.rs tests/cli_integration.rs
git commit -m "feat(log): support blood pressure compound value (120/80)"
```

---

### Task 6: Batch `--human` flag + simple format (BUG-3 + UX-5)

**Files:**
- Modify: `src/cmd/log.rs:55-75` (run_batch)
- Modify: `src/main.rs` (pass human flag to run_batch — already done in Task 5)
- Test: `tests/cli_integration.rs`

**Step 1: Write the failing tests**

Add to `tests/cli_integration.rs`:

```rust
#[test]
fn test_batch_simple_format_parsing() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let db = openvital::db::Database::open(&db_path).unwrap();
    let config = openvital::models::config::Config::default();

    // Simple format: "weight:72.5,sleep:7.5,mood:8"
    let simple = "weight:72.5,sleep:7.5,mood:8";
    let json = openvital::core::logging::parse_simple_batch(simple).unwrap();
    let metrics = openvital::core::logging::log_batch(&db, &config, &json).unwrap();

    assert_eq!(metrics.len(), 3);
    assert_eq!(metrics[0].metric_type, "weight");
    assert_eq!(metrics[0].value, 72.5);
    assert_eq!(metrics[1].metric_type, "sleep");
    assert_eq!(metrics[2].metric_type, "mood");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_batch_simple_format 2>&1 | tail -5`
Expected: FAIL — `parse_simple_batch` not found

**Step 3: Implement**

Add to `src/core/logging.rs`:

```rust
/// Convert simple batch format ("weight:72.5,sleep:7.5") to JSON array string.
pub fn parse_simple_batch(input: &str) -> Result<String> {
    let entries: Vec<serde_json::Value> = input
        .split(',')
        .map(|pair| {
            let parts: Vec<&str> = pair.trim().splitn(2, ':').collect();
            if parts.len() != 2 {
                anyhow::bail!("invalid batch entry: '{}' (expected type:value)", pair);
            }
            let value: f64 = parts[1].parse().map_err(|_| anyhow::anyhow!("invalid value in '{}'", pair))?;
            Ok(serde_json::json!({"type": parts[0].trim(), "value": value}))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(serde_json::to_string(&entries)?)
}
```

Update `src/cmd/log.rs` `run_batch`:

```rust
pub fn run_batch(batch_input: &str, human_flag: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;

    // Detect format: JSON array starts with '[', otherwise simple format
    let batch_json = if batch_input.trim_start().starts_with('[') {
        batch_input.to_string()
    } else {
        openvital::core::logging::parse_simple_batch(batch_input)?
    };

    let metrics = openvital::core::logging::log_batch(&db, &config, &batch_json)?;

    if human_flag {
        for m in &metrics {
            println!("Logged: {}", human::format_metric(m));
        }
    } else {
        let entries: Vec<_> = metrics
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "type": m.metric_type,
                    "value": m.value,
                    "unit": m.unit
                })
            })
            .collect();
        let out = output::success("log", json!({ "entries": entries }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
```

**Step 4: Run tests**

Run: `cargo test 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/core/logging.rs src/cmd/log.rs
git commit -m "feat(batch): support --human flag and simple key:value format"
```

---

### Task 7: Status deduplicates "Logged today" (UX-4)

**Files:**
- Modify: `src/output/human.rs:28-32`
- Test: `tests/status_enhanced.rs`

**Step 1: Write the failing test**

Add to `tests/status_enhanced.rs`:

```rust
#[test]
fn test_status_human_deduplicates_logged_today() {
    use openvital::core::status::{StatusData, ProfileStatus, TodayStatus, Streaks};

    let status = StatusData {
        date: chrono::NaiveDate::from_ymd_opt(2026, 2, 18).unwrap(),
        profile: ProfileStatus {
            height_cm: None,
            latest_weight_kg: None,
            bmi: None,
            bmi_category: None,
        },
        today: TodayStatus {
            logged: vec![
                "water".into(), "water".into(), "water".into(),
                "weight".into(), "weight".into(),
                "sleep".into(),
            ],
            pain_alerts: vec![],
        },
        streaks: Streaks { logging_days: 1 },
        consecutive_pain_alerts: vec![],
    };

    let output = openvital::output::human::format_status(&status);
    // Should contain deduplicated counts, not raw list
    assert!(output.contains("water(3)"), "expected water(3), got: {}", output);
    assert!(output.contains("weight(2)"), "expected weight(2), got: {}", output);
    assert!(output.contains("sleep(1)"), "expected sleep(1), got: {}", output);
    // Should NOT contain the raw comma-separated duplicate list
    assert!(!output.contains("water, water, water"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_status_human_deduplicates 2>&1 | tail -10`
Expected: FAIL — output contains raw duplicates

**Step 3: Implement**

In `src/output/human.rs`, replace lines 28-32:

```rust
    if s.today.logged.is_empty() {
        out.push_str("No entries logged today.");
    } else {
        // Deduplicate: count occurrences, preserve insertion order
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for t in &s.today.logged {
            if let Some(entry) = counts.iter_mut().find(|(name, _)| *name == t.as_str()) {
                entry.1 += 1;
            } else {
                counts.push((t.as_str(), 1));
            }
        }
        let parts: Vec<String> = counts.iter().map(|(name, count)| format!("{}({})", name, count)).collect();
        out.push_str(&format!("Logged today: {}", parts.join(", ")));
    }
```

**Step 4: Run tests**

Run: `cargo test status 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/output/human.rs tests/status_enhanced.rs
git commit -m "feat(status): deduplicate 'Logged today' with counts"
```

---

### Task 8: Goal set positional arguments (UX-3)

**Files:**
- Modify: `src/cli.rs:181-195` (GoalAction::Set)
- Modify: `src/main.rs:57-64` (goal dispatch)
- Modify: `src/cmd/goal.rs` (adjust run_set if needed)
- Test: `tests/cli_integration.rs`

**Step 1: Write the failing test**

This is a CLI-level change. Test by building and running the binary in a subprocess. Add to `tests/cli_integration.rs`:

```rust
#[test]
fn test_goal_set_positional_args() {
    // Test that positional parsing works at the clap level
    use clap::Parser;

    // We can't easily test clap parsing from integration tests without
    // importing the private cli module. Instead, test via the binary:
    let dir = tempfile::TempDir::new().unwrap();
    std::env::set_var("OPENVITAL_HOME", dir.path());

    // Initialize first
    let bin = env!("CARGO_BIN_EXE_openvital");
    let init = std::process::Command::new(bin)
        .args(["init", "--skip"])
        .env("OPENVITAL_HOME", dir.path())
        .output()
        .unwrap();
    assert!(init.status.success());

    // Goal set with positional: goal set weight 70 below daily
    let output = std::process::Command::new(bin)
        .args(["goal", "set", "weight", "70", "below", "daily", "--human"])
        .env("OPENVITAL_HOME", dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Goal set: weight below 70"), "got: {}", stdout);
}
```

Note: This test uses `std::env::set_var` which requires `unsafe` in current Rust. If the test runner doesn't allow it, use the subprocess approach only (which is already the main mechanism here).

**Step 2: Run test to verify it fails**

Run: `cargo test test_goal_set_positional 2>&1 | tail -10`
Expected: FAIL — positional args not accepted

**Step 3: Implement**

In `src/cli.rs`, change `GoalAction::Set`:

```rust
    /// Set a goal for a metric type
    Set {
        /// Metric type (e.g. weight, cardio, water)
        r#type: String,
        /// Target value (positional)
        #[arg(value_name = "TARGET_POS")]
        target_pos: Option<f64>,
        /// Direction (positional): above, below, or equal
        #[arg(value_name = "DIRECTION_POS")]
        direction_pos: Option<String>,
        /// Timeframe (positional): daily, weekly, or monthly
        #[arg(value_name = "TIMEFRAME_POS")]
        timeframe_pos: Option<String>,
        /// Target value (named)
        #[arg(long)]
        target: Option<f64>,
        /// Direction: above, below, or equal (named)
        #[arg(long)]
        direction: Option<String>,
        /// Timeframe: daily, weekly, or monthly (named)
        #[arg(long)]
        timeframe: Option<String>,
    },
```

In `src/main.rs`, update the GoalAction::Set match:

```rust
            GoalAction::Set {
                r#type,
                target_pos,
                direction_pos,
                timeframe_pos,
                target,
                direction,
                timeframe,
            } => {
                let t = target.or(target_pos).expect("target is required (use positional or --target)");
                let d = direction.or(direction_pos).expect("direction is required (use positional or --direction)");
                let tf = timeframe.or(timeframe_pos).expect("timeframe is required (use positional or --timeframe)");
                cmd::goal::run_set(&r#type, t, &d, &tf, cli.human)
            }
```

**Step 4: Run tests**

Run: `cargo test 2>&1 | tail -5`
Expected: all PASS

**Step 5: Commit**

```bash
git add src/cli.rs src/main.rs tests/cli_integration.rs
git commit -m "feat(goal): support positional args for goal set (type target direction timeframe)"
```

---

### Task 9: Final verification + cleanup

**Step 1: Run full test suite**

Run: `cargo test 2>&1 | tail -10`
Expected: all tests PASS

**Step 2: Run linting**

Run: `cargo fmt --all -- --check && cargo clippy -- -D warnings 2>&1 | tail -5`
Expected: no warnings, no format issues

**Step 3: Run coverage (optional)**

Run: `cargo tarpaulin --skip-clean 2>&1 | tail -5`
Expected: coverage should remain >= 99%

**Step 4: Final commit if any cleanup needed**

```bash
git add -A
git commit -m "chore: final cleanup after v0.2 bug fixes and UX improvements"
```
