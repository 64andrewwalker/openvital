# AI Health Context & Anomaly Detection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add two new commands (`openvital context` and `openvital anomaly`) that give AI agents a single-call health state briefing and personal baseline anomaly detection.

**Architecture:** Both features follow the existing 4-layer pattern (CLI → cmd → core → DB). `context` composes existing core functions into a single structured response with natural language summaries. `anomaly` adds IQR-based anomaly detection computed on-the-fly from existing metrics — no new DB tables. A new `db::metrics::distinct_metric_types()` query is needed for scanning all tracked types.

**Tech Stack:** Rust 2024 edition, rusqlite, chrono, serde, anyhow, clap (existing stack — no new dependencies)

---

## Task 1: Add `distinct_metric_types()` DB Query

Both commands need to know what metric types exist. This is a shared prerequisite.

**Files:**
- Modify: `src/db/metrics.rs`
- Test: `tests/anomaly_test.rs` (created in Task 2, but this query is tested here implicitly)

**Step 1: Add the query method to `Database`**

In `src/db/metrics.rs`, add at the end of the `impl Database` block:

```rust
/// Get distinct metric types that have entries, ordered alphabetically.
pub fn distinct_metric_types(&self) -> Result<Vec<String>> {
    let mut stmt = self
        .conn
        .prepare("SELECT DISTINCT type FROM metrics ORDER BY type ASC")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut types = Vec::new();
    for row in rows {
        types.push(row?);
    }
    Ok(types)
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src/db/metrics.rs
git commit -m "feat(db): add distinct_metric_types query"
```

---

## Task 2: Anomaly Detection — Models & Core Logic

Build the anomaly detection engine first since `context` depends on it.

**Files:**
- Create: `src/models/anomaly.rs`
- Modify: `src/models/mod.rs`
- Create: `src/core/anomaly.rs`
- Modify: `src/core/mod.rs`
- Create: `tests/anomaly_test.rs`

**Step 1: Write the failing integration test**

Create `tests/anomaly_test.rs`:

```rust
mod common;

use chrono::{Duration, Local};
use openvital::core::anomaly::{self, AnomalyResult, Severity, Threshold};
use openvital::db::Database;

#[test]
fn test_anomaly_detect_flags_outlier() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Build a 14-day baseline of heart_rate around 70-76
    for i in 1..=14 {
        let date = today - Duration::days(i);
        let m = common::make_metric("heart_rate", 70.0 + (i % 7) as f64, date);
        db.insert_metric(&m).unwrap();
    }

    // Add an anomalous reading today
    let outlier = common::make_metric("heart_rate", 95.0, today);
    db.insert_metric(&outlier).unwrap();

    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(!result.anomalies.is_empty(), "should detect the outlier");
    assert_eq!(result.anomalies[0].metric_type, "heart_rate");
    assert!(matches!(
        result.anomalies[0].severity,
        Severity::Warning | Severity::Alert
    ));
}

#[test]
fn test_anomaly_no_data_returns_empty() {
    let (_dir, db) = common::setup_db();
    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(result.anomalies.is_empty());
    assert!(result.scanned_types.is_empty());
}

#[test]
fn test_anomaly_insufficient_data_skips() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Only 3 data points — below minimum of 7
    for i in 1..=3 {
        let date = today - Duration::days(i);
        let m = common::make_metric("weight", 80.0 + i as f64, date);
        db.insert_metric(&m).unwrap();
    }

    let result = anomaly::detect(&db, Some("weight"), 30, Threshold::Moderate).unwrap();
    assert!(result.anomalies.is_empty());
}

#[test]
fn test_anomaly_normal_value_not_flagged() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // 14-day baseline of weight around 80-82
    for i in 1..=14 {
        let date = today - Duration::days(i);
        let m = common::make_metric("weight", 80.0 + (i % 3) as f64, date);
        db.insert_metric(&m).unwrap();
    }

    // Today's value is within normal range
    let normal = common::make_metric("weight", 81.0, today);
    db.insert_metric(&normal).unwrap();

    let result = anomaly::detect(&db, Some("weight"), 30, Threshold::Moderate).unwrap();
    assert!(result.anomalies.is_empty(), "normal value should not be flagged");
}

#[test]
fn test_anomaly_threshold_strict_catches_more() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Build baseline around 70-76
    for i in 1..=14 {
        let date = today - Duration::days(i);
        let m = common::make_metric("heart_rate", 70.0 + (i % 7) as f64, date);
        db.insert_metric(&m).unwrap();
    }

    // A mildly elevated reading
    let mild = common::make_metric("heart_rate", 82.0, today);
    db.insert_metric(&mild).unwrap();

    // Strict should catch it, relaxed should not
    let strict = anomaly::detect(&db, Some("heart_rate"), 30, Threshold::Strict).unwrap();
    let relaxed = anomaly::detect(&db, Some("heart_rate"), 30, Threshold::Relaxed).unwrap();

    assert!(
        !strict.anomalies.is_empty(),
        "strict threshold should flag mild elevation"
    );
    assert!(
        relaxed.anomalies.is_empty(),
        "relaxed threshold should not flag mild elevation"
    );
}

#[test]
fn test_anomaly_filter_by_type() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Baseline for two types
    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 80.0, date)).unwrap();
        db.insert_metric(&common::make_metric("heart_rate", 72.0, date)).unwrap();
    }

    // Anomaly in heart_rate only
    db.insert_metric(&common::make_metric("heart_rate", 110.0, today)).unwrap();
    db.insert_metric(&common::make_metric("weight", 80.0, today)).unwrap();

    // Filter to weight only — should find nothing
    let result = anomaly::detect(&db, Some("weight"), 30, Threshold::Moderate).unwrap();
    assert!(result.anomalies.is_empty());

    // No filter — should find heart_rate anomaly
    let result_all = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(!result_all.anomalies.is_empty());
    assert_eq!(result_all.anomalies[0].metric_type, "heart_rate");
}

#[test]
fn test_anomaly_baseline_stats() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Known values: 10, 20, 30, 40, 50, 60, 70
    for (i, val) in [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0].iter().enumerate() {
        let date = today - Duration::days(i as i64 + 1);
        db.insert_metric(&common::make_metric("test_metric", *val, date)).unwrap();
    }

    // Value of 200 is clearly an outlier
    db.insert_metric(&common::make_metric("test_metric", 200.0, today)).unwrap();

    let result = anomaly::detect(&db, Some("test_metric"), 30, Threshold::Moderate).unwrap();
    assert!(!result.anomalies.is_empty());

    let a = &result.anomalies[0];
    // Q1=20, Q3=60, IQR=40, median=40
    assert!((a.baseline.q1 - 20.0).abs() < 5.0, "Q1 should be around 20");
    assert!((a.baseline.q3 - 60.0).abs() < 5.0, "Q3 should be around 60");
    assert!(a.baseline.iqr > 0.0, "IQR should be positive");
}

#[test]
fn test_anomaly_summary_generated() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("pain", 3.0, date)).unwrap();
    }
    db.insert_metric(&common::make_metric("pain", 9.0, today)).unwrap();

    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(!result.summary.is_empty());
    assert!(!result.anomalies[0].summary.is_empty());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test anomaly_test 2>&1 | head -30`
Expected: FAIL — `anomaly` module does not exist

**Step 3: Create the anomaly model**

Create `src/models/anomaly.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Alert,
}

#[derive(Debug, Clone, Copy)]
pub enum Threshold {
    Relaxed,
    Moderate,
    Strict,
}

impl Threshold {
    /// IQR multiplier for determining bounds.
    pub fn factor(self) -> f64 {
        match self {
            Self::Relaxed => 2.0,
            Self::Moderate => 1.5,
            Self::Strict => 1.0,
        }
    }
}

impl FromStr for Threshold {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "relaxed" => Ok(Self::Relaxed),
            "moderate" => Ok(Self::Moderate),
            "strict" => Ok(Self::Strict),
            _ => anyhow::bail!("invalid threshold: {} (expected relaxed/moderate/strict)", s),
        }
    }
}

impl std::fmt::Display for Threshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Relaxed => write!(f, "relaxed"),
            Self::Moderate => write!(f, "moderate"),
            Self::Strict => write!(f, "strict"),
        }
    }
}

impl Serialize for Threshold {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Baseline {
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub iqr: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Anomaly {
    pub metric_type: String,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
    pub baseline: Baseline,
    pub bounds: Bounds,
    pub deviation: String,
    pub severity: Severity,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bounds {
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Serialize)]
pub struct AnomalyResult {
    pub period: AnomalyPeriod,
    pub threshold: Threshold,
    pub anomalies: Vec<Anomaly>,
    pub scanned_types: Vec<String>,
    pub clean_types: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct AnomalyPeriod {
    pub baseline_start: String,
    pub baseline_end: String,
    pub days: u32,
}
```

**Step 4: Register the model module**

In `src/models/mod.rs`, add:

```rust
pub mod anomaly;
```

**Step 5: Create the core anomaly detection logic**

Create `src/core/anomaly.rs`:

```rust
use anyhow::Result;
use chrono::{Duration, Local, NaiveDate};

use crate::db::Database;
use crate::models::anomaly::{
    Anomaly, AnomalyPeriod, AnomalyResult, Baseline, Bounds, Severity, Threshold,
};

/// Minimum data points required to compute a meaningful baseline.
const MIN_DATA_POINTS: usize = 7;

/// Detect anomalies across one or all metric types.
pub fn detect(
    db: &Database,
    metric_type: Option<&str>,
    baseline_days: u32,
    threshold: Threshold,
) -> Result<AnomalyResult> {
    let today = Local::now().date_naive();
    let baseline_start = today - Duration::days(baseline_days as i64);

    let types_to_scan: Vec<String> = if let Some(t) = metric_type {
        vec![t.to_string()]
    } else {
        db.distinct_metric_types()?
    };

    let mut anomalies = Vec::new();
    let mut scanned_types = Vec::new();
    let mut clean_types = Vec::new();

    for metric in &types_to_scan {
        let entries = db.query_all(Some(metric), Some(baseline_start), Some(today))?;

        if entries.len() < MIN_DATA_POINTS {
            continue;
        }

        scanned_types.push(metric.clone());

        // Separate today's entries from baseline
        let baseline_values: Vec<f64> = entries
            .iter()
            .filter(|e| e.timestamp.date_naive() < today)
            .map(|e| e.value)
            .collect();

        if baseline_values.len() < MIN_DATA_POINTS {
            continue;
        }

        let baseline = compute_baseline(&baseline_values);
        let factor = threshold.factor();
        let lower = baseline.q1 - factor * baseline.iqr;
        let upper = baseline.q3 + factor * baseline.iqr;

        // Check today's entries against baseline
        let today_entries: Vec<_> = entries
            .iter()
            .filter(|e| e.timestamp.date_naive() == today)
            .collect();

        let mut found_anomaly = false;
        for entry in &today_entries {
            if entry.value < lower || entry.value > upper {
                found_anomaly = true;
                let deviation = if entry.value > upper {
                    "above"
                } else {
                    "below"
                };

                let severity = compute_severity(entry.value, &baseline, deviation);

                let summary = format!(
                    "{} {:.1} is {} your normal range ({:.1}-{:.1})",
                    metric, entry.value, deviation, lower, upper
                );

                anomalies.push(Anomaly {
                    metric_type: metric.clone(),
                    value: entry.value,
                    timestamp: entry.timestamp,
                    baseline: baseline.clone(),
                    bounds: Bounds { lower, upper },
                    deviation: deviation.to_string(),
                    severity,
                    summary,
                });
            }
        }

        if !found_anomaly {
            clean_types.push(metric.clone());
        }
    }

    let summary = if anomalies.is_empty() {
        if scanned_types.is_empty() {
            "No metrics with sufficient data for anomaly detection.".to_string()
        } else {
            format!(
                "No anomalies detected across {} metric type(s).",
                scanned_types.len()
            )
        }
    } else {
        let types: Vec<&str> = anomalies
            .iter()
            .map(|a| a.metric_type.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        format!(
            "{} anomal{} detected across {} metric type(s). Affected: {}.",
            anomalies.len(),
            if anomalies.len() == 1 { "y" } else { "ies" },
            scanned_types.len(),
            types.join(", ")
        )
    };

    Ok(AnomalyResult {
        period: AnomalyPeriod {
            baseline_start: baseline_start.to_string(),
            baseline_end: today.to_string(),
            days: baseline_days,
        },
        threshold,
        anomalies,
        scanned_types,
        clean_types,
        summary,
    })
}

/// Compute IQR-based baseline statistics.
fn compute_baseline(values: &[f64]) -> Baseline {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = sorted.len();

    let median = percentile(&sorted, 50.0);
    let q1 = percentile(&sorted, 25.0);
    let q3 = percentile(&sorted, 75.0);
    let iqr = q3 - q1;

    Baseline {
        q1,
        median,
        q3,
        iqr,
    }
}

/// Compute percentile using linear interpolation.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let k = (p / 100.0) * (sorted.len() - 1) as f64;
    let f = k.floor() as usize;
    let c = k.ceil() as usize;
    if f == c {
        sorted[f]
    } else {
        sorted[f] + (k - f as f64) * (sorted[c] - sorted[f])
    }
}

/// Determine severity based on how far the value is from bounds.
fn compute_severity(value: f64, baseline: &Baseline, deviation: &str) -> Severity {
    let distance = if deviation == "above" {
        (value - baseline.q3) / baseline.iqr.max(0.01)
    } else {
        (baseline.q1 - value) / baseline.iqr.max(0.01)
    };

    if distance > 2.0 {
        Severity::Alert
    } else if distance > 1.5 {
        Severity::Warning
    } else {
        Severity::Info
    }
}
```

**Step 6: Register the core module**

In `src/core/mod.rs`, add:

```rust
pub mod anomaly;
```

**Step 7: Run tests to verify they pass**

Run: `cargo test --test anomaly_test`
Expected: all 8 tests PASS

**Step 8: Commit**

```bash
git add src/models/anomaly.rs src/models/mod.rs src/core/anomaly.rs src/core/mod.rs tests/anomaly_test.rs
git commit -m "feat(anomaly): add IQR-based anomaly detection engine with tests"
```

---

## Task 3: Anomaly CLI Command

Wire the anomaly detection into the CLI layer.

**Files:**
- Modify: `src/cli.rs`
- Create: `src/cmd/anomaly.rs`
- Modify: `src/cmd/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/output/human.rs`

**Step 1: Write CLI integration test**

Add to `tests/anomaly_test.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_anomaly_cli_json_output() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[profile]\n[units]\nsystem = \"metric\"\n[aliases]\n[alerts]\npain_threshold = 5\npain_consecutive_days = 3\n",
    )
    .unwrap();

    // Create DB and populate baseline
    {
        let db = Database::open(&db_path).unwrap();
        let today = Local::now().date_naive();
        for i in 1..=14 {
            let date = today - Duration::days(i);
            db.insert_metric(&common::make_metric("heart_rate", 72.0, date)).unwrap();
        }
        db.insert_metric(&common::make_metric("heart_rate", 110.0, today)).unwrap();
    }

    let mut cmd = Command::cargo_bin("openvital").unwrap();
    cmd.env("OPENVITAL_HOME", dir.path())
        .arg("anomaly");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"ok\""))
        .stdout(predicate::str::contains("\"command\":\"anomaly\""))
        .stdout(predicate::str::contains("heart_rate"));
}

#[test]
fn test_anomaly_cli_human_output() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[profile]\n[units]\nsystem = \"metric\"\n[aliases]\n[alerts]\npain_threshold = 5\npain_consecutive_days = 3\n",
    )
    .unwrap();

    {
        let db = Database::open(&db_path).unwrap();
        let today = Local::now().date_naive();
        for i in 1..=14 {
            let date = today - Duration::days(i);
            db.insert_metric(&common::make_metric("heart_rate", 72.0, date)).unwrap();
        }
        db.insert_metric(&common::make_metric("heart_rate", 110.0, today)).unwrap();
    }

    let mut cmd = Command::cargo_bin("openvital").unwrap();
    cmd.env("OPENVITAL_HOME", dir.path())
        .arg("anomaly")
        .arg("--human");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Anomaly"))
        .stdout(predicate::str::contains("heart_rate"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test anomaly_test test_anomaly_cli 2>&1 | head -20`
Expected: FAIL — `anomaly` not a valid subcommand

**Step 3: Add CLI variant**

In `src/cli.rs`, add to the `Commands` enum (after `Med`):

```rust
/// Detect anomalous health readings against personal baselines
Anomaly {
    /// Metric type to check (all if omitted)
    r#type: Option<String>,

    /// Baseline window in days (default: 30)
    #[arg(long, default_value = "30")]
    days: u32,

    /// Sensitivity: relaxed, moderate, strict (default: moderate)
    #[arg(long, default_value = "moderate")]
    threshold: String,
},
```

**Step 4: Create command handler**

Create `src/cmd/anomaly.rs`:

```rust
use anyhow::Result;
use std::str::FromStr;

use openvital::core::anomaly;
use openvital::db::Database;
use openvital::models::anomaly::Threshold;
use openvital::models::config::Config;
use openvital::output;
use openvital::output::human;

pub fn run(
    metric_type: Option<&str>,
    days: u32,
    threshold: &str,
    human_flag: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let threshold = Threshold::from_str(threshold)?;

    let result = anomaly::detect(&db, metric_type, days, threshold)?;

    if human_flag {
        println!("{}", human::format_anomaly(&result));
    } else {
        let out = output::success("anomaly", serde_json::to_value(&result)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
```

**Step 5: Register command module**

In `src/cmd/mod.rs`, add:

```rust
pub mod anomaly;
```

**Step 6: Add dispatch to main.rs**

In `src/main.rs`, add the import and match arm. After `Commands::Med { action } => ...`:

```rust
Commands::Anomaly {
    r#type,
    days,
    threshold,
} => cmd::anomaly::run(r#type.as_deref(), days, &threshold, cli.human),
```

**Step 7: Add human formatting**

In `src/output/human.rs`, add:

```rust
use crate::models::anomaly::{AnomalyResult, Severity};

/// Format anomaly detection results for human display.
pub fn format_anomaly(result: &AnomalyResult) -> String {
    let mut out = format!(
        "=== Anomaly Scan ({} days, {} threshold) ===\n",
        result.period.days, result.threshold
    );

    if result.anomalies.is_empty() {
        out.push_str(&format!("\n{}", result.summary));
        return out;
    }

    for a in &result.anomalies {
        let severity_marker = match a.severity {
            Severity::Alert => "!!!",
            Severity::Warning => "!!",
            Severity::Info => "!",
        };
        out.push_str(&format!(
            "\n{} {} {:.1} (normal: {:.1}-{:.1}, {} baseline)",
            severity_marker,
            a.metric_type,
            a.value,
            a.bounds.lower,
            a.bounds.upper,
            a.deviation,
        ));
    }

    out.push_str(&format!("\n\n{}", result.summary));

    if !result.clean_types.is_empty() {
        out.push_str(&format!(
            "\nNormal: {}",
            result.clean_types.join(", ")
        ));
    }

    out
}
```

**Step 8: Run all tests**

Run: `cargo test --test anomaly_test`
Expected: all tests PASS

**Step 9: Run clippy and fmt**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: no errors

**Step 10: Commit**

```bash
git add src/cli.rs src/cmd/anomaly.rs src/cmd/mod.rs src/main.rs src/output/human.rs
git commit -m "feat(anomaly): add anomaly CLI command with human/JSON output"
```

---

## Task 4: Context Command — Core Logic

The main "AI briefing" command. Composes existing core functions into one structured response.

**Files:**
- Create: `src/core/context.rs`
- Modify: `src/core/mod.rs`
- Create: `tests/context_test.rs`

**Step 1: Write the failing integration tests**

Create `tests/context_test.rs`:

```rust
mod common;

use chrono::{Duration, Local};
use openvital::core::context::{self, ContextResult};
use openvital::db::Database;
use openvital::models::config::Config;

fn make_test_config() -> Config {
    let mut config = Config::default();
    config.profile.height_cm = Some(180.0);
    config
}

#[test]
fn test_context_empty_db() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();

    let result = context::compute(&db, &config, 7, None).unwrap();

    assert_eq!(result.period.days, 7);
    assert!(result.metrics.is_empty());
    assert!(result.goals.is_empty());
    assert!(result.anomalies.is_empty());
    assert!(!result.summary.is_empty(), "summary should always be present");
}

#[test]
fn test_context_with_metrics() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // Add some metric data
    for i in 0..7 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 83.0 - i as f64 * 0.3, date)).unwrap();
        db.insert_metric(&common::make_metric("pain", 3.0, date)).unwrap();
    }

    let result = context::compute(&db, &config, 7, None).unwrap();

    assert!(result.metrics.contains_key("weight"));
    assert!(result.metrics.contains_key("pain"));

    let weight = &result.metrics["weight"];
    assert!(weight.latest.is_some());
    assert!(weight.trend.is_some());
    assert!(weight.stats.count > 0);
    assert!(!weight.summary.is_empty());
}

#[test]
fn test_context_with_goals() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // Add weight data and a goal
    db.insert_metric(&common::make_metric("weight", 83.0, today)).unwrap();

    use openvital::models::goal::{Direction, Timeframe};
    openvital::core::goal::set_goal(&db, "weight".into(), 80.0, Direction::Below, Timeframe::Daily).unwrap();

    let result = context::compute(&db, &config, 7, None).unwrap();

    assert!(!result.goals.is_empty());
    assert_eq!(result.goals[0].metric_type, "weight");
}

#[test]
fn test_context_filter_by_type() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    db.insert_metric(&common::make_metric("weight", 83.0, today)).unwrap();
    db.insert_metric(&common::make_metric("pain", 5.0, today)).unwrap();

    let result = context::compute(&db, &config, 7, Some(&["weight"])).unwrap();

    assert!(result.metrics.contains_key("weight"));
    assert!(!result.metrics.contains_key("pain"));
}

#[test]
fn test_context_includes_anomalies() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // Build a baseline
    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("heart_rate", 72.0, date)).unwrap();
    }
    // Add anomalous value today
    db.insert_metric(&common::make_metric("heart_rate", 110.0, today)).unwrap();

    let result = context::compute(&db, &config, 30, None).unwrap();

    assert!(!result.anomalies.is_empty());
}

#[test]
fn test_context_summary_mentions_key_info() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // Weight with trend
    for i in 0..7 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 83.0 - i as f64 * 0.3, date)).unwrap();
    }

    let result = context::compute(&db, &config, 7, None).unwrap();

    // Summary should mention the number of metric types tracked
    assert!(
        result.summary.contains("1") || result.summary.contains("weight"),
        "summary should reference tracked metrics: {}",
        result.summary
    );
}

#[test]
fn test_context_streaks_included() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // 5 consecutive days of logging
    for i in 0..5 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 80.0, date)).unwrap();
    }

    let result = context::compute(&db, &config, 7, None).unwrap();
    assert!(result.streaks.logging_days >= 5);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test context_test 2>&1 | head -10`
Expected: FAIL — `context` module does not exist

**Step 3: Create the context core logic**

Create `src/core/context.rs`:

```rust
use std::collections::HashMap;

use anyhow::Result;
use chrono::{Duration, Local, NaiveDate};
use serde::Serialize;

use crate::core::anomaly;
use crate::core::goal::GoalStatus;
use crate::core::status;
use crate::core::trend::{TrendPeriod, TrendSummary};
use crate::db::Database;
use crate::models::anomaly::{Anomaly, Threshold};
use crate::models::config::Config;

#[derive(Debug, Serialize)]
pub struct ContextResult {
    pub generated_at: String,
    pub period: ContextPeriod,
    pub summary: String,
    pub metrics: HashMap<String, MetricContext>,
    pub goals: Vec<GoalContext>,
    pub medications: Option<MedicationContext>,
    pub streaks: status::Streaks,
    pub alerts: Vec<AlertItem>,
    pub anomalies: Vec<Anomaly>,
}

#[derive(Debug, Serialize)]
pub struct ContextPeriod {
    pub start: String,
    pub end: String,
    pub days: u32,
}

#[derive(Debug, Serialize)]
pub struct MetricContext {
    pub latest: Option<LatestValue>,
    pub trend: Option<TrendInfo>,
    pub stats: MetricStats,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct LatestValue {
    pub value: f64,
    pub unit: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct TrendInfo {
    pub direction: String,
    pub rate: f64,
    pub rate_unit: String,
}

#[derive(Debug, Serialize)]
pub struct MetricStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub count: u32,
}

#[derive(Debug, Serialize)]
pub struct GoalContext {
    pub metric_type: String,
    pub target: f64,
    pub direction: String,
    pub timeframe: String,
    pub current: Option<f64>,
    pub is_met: bool,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct MedicationContext {
    pub active_count: usize,
    pub adherence_today: f64,
    pub adherence_7d: Option<f64>,
    pub medications: Vec<MedBrief>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct MedBrief {
    pub name: String,
    pub adherent_today: Option<bool>,
    pub adherence_7d: Option<f64>,
    pub streak: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct AlertItem {
    #[serde(rename = "type")]
    pub alert_type: String,
    pub message: String,
}

/// Compute the full health context briefing.
pub fn compute(
    db: &Database,
    config: &Config,
    days: u32,
    type_filter: Option<&[&str]>,
) -> Result<ContextResult> {
    let today = Local::now().date_naive();
    let start_date = today - Duration::days(days as i64);
    let now = chrono::Utc::now();

    // 1. Get all distinct metric types
    let all_types = db.distinct_metric_types()?;
    let types: Vec<&str> = if let Some(filter) = type_filter {
        all_types
            .iter()
            .filter(|t| filter.contains(&t.as_str()))
            .map(|t| t.as_str())
            .collect()
    } else {
        all_types.iter().map(|t| t.as_str()).collect()
    };

    // 2. Build per-metric context
    let mut metrics = HashMap::new();
    for metric_type in &types {
        let entries = db.query_all(Some(metric_type), Some(start_date), Some(today))?;
        if entries.is_empty() {
            continue;
        }

        let latest = entries.last().map(|e| LatestValue {
            value: e.value,
            unit: e.unit.clone(),
            timestamp: e.timestamp.to_rfc3339(),
        });

        let values: Vec<f64> = entries.iter().map(|e| e.value).collect();
        let count = values.len() as u32;
        let sum: f64 = values.iter().sum();
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg = (sum / values.len() as f64 * 10.0).round() / 10.0;

        let stats = MetricStats {
            min,
            max,
            avg,
            count,
        };

        // Compute trend if enough data
        let trend = if count >= 2 {
            match crate::core::trend::compute(db, metric_type, TrendPeriod::Daily, Some(days)) {
                Ok(t) => Some(TrendInfo {
                    direction: t.trend.direction.clone(),
                    rate: t.trend.rate,
                    rate_unit: t.trend.rate_unit.clone(),
                }),
                Err(_) => None,
            }
        } else {
            None
        };

        // Generate per-metric summary
        let summary = generate_metric_summary(metric_type, &latest, &trend, &stats);

        metrics.insert(
            metric_type.to_string(),
            MetricContext {
                latest,
                trend,
                stats,
                summary,
            },
        );
    }

    // 3. Goals
    let goal_statuses = crate::core::goal::goal_status(db, None)?;
    let goals: Vec<GoalContext> = goal_statuses
        .into_iter()
        .filter(|g| {
            type_filter.is_none()
                || type_filter
                    .unwrap()
                    .contains(&g.metric_type.as_str())
        })
        .map(|g| {
            let summary = if g.is_met {
                format!("{} goal met ({} {})", g.metric_type, g.direction, g.target_value)
            } else if let Some(current) = g.current_value {
                format!(
                    "{}: {:.1} / {:.1} ({})",
                    g.metric_type, current, g.target_value, g.direction
                )
            } else {
                format!("{} goal: no data yet", g.metric_type)
            };
            GoalContext {
                metric_type: g.metric_type,
                target: g.target_value,
                direction: g.direction,
                timeframe: g.timeframe,
                current: g.current_value,
                is_met: g.is_met,
                summary,
            }
        })
        .collect();

    // 4. Medications
    let medications = match crate::core::med::adherence_status(db, None, 7) {
        Ok(med_statuses) if !med_statuses.is_empty() => {
            let active_count = med_statuses.len();
            let total_scheduled: usize = med_statuses
                .iter()
                .filter(|s| s.adherent_today.is_some())
                .count();
            let adherent_count: usize = med_statuses
                .iter()
                .filter(|s| s.adherent_today == Some(true))
                .count();
            let adherence_today = if total_scheduled > 0 {
                adherent_count as f64 / total_scheduled as f64
            } else {
                1.0
            };

            let adherence_values: Vec<f64> =
                med_statuses.iter().filter_map(|s| s.adherence_7d).collect();
            let adherence_7d = if adherence_values.is_empty() {
                None
            } else {
                Some(adherence_values.iter().sum::<f64>() / adherence_values.len() as f64)
            };

            let meds: Vec<MedBrief> = med_statuses
                .iter()
                .map(|s| MedBrief {
                    name: s.name.clone(),
                    adherent_today: s.adherent_today,
                    adherence_7d: s.adherence_7d,
                    streak: s.streak_days,
                })
                .collect();

            let summary = format!(
                "{} active medication(s). {}/{} taken today.{}",
                active_count,
                adherent_count,
                total_scheduled,
                adherence_7d
                    .map(|a| format!(" {:.0}% adherence (7d).", a * 100.0))
                    .unwrap_or_default()
            );

            Some(MedicationContext {
                active_count,
                adherence_today,
                adherence_7d,
                medications: meds,
                summary,
            })
        }
        _ => None,
    };

    // 5. Streaks
    let streaks = status::compute_streaks(db, today)?;

    // 6. Alerts
    let mut alerts = Vec::new();
    let today_entries = db.query_by_date(today)?;
    let threshold = config.alerts.pain_threshold as f64;
    for entry in &today_entries {
        if (entry.metric_type == "pain" || entry.metric_type == "soreness")
            && entry.value >= threshold
        {
            alerts.push(AlertItem {
                alert_type: "pain_elevated".to_string(),
                message: format!(
                    "{} at {}/10, above threshold of {}",
                    entry.metric_type, entry.value, threshold
                ),
            });
        }
    }

    let consecutive = status::check_consecutive_pain(db, today, &config.alerts)?;
    for alert in &consecutive {
        alerts.push(AlertItem {
            alert_type: "consecutive_pain".to_string(),
            message: format!(
                "{} above threshold for {} consecutive days (latest: {})",
                alert.metric_type, alert.consecutive_days, alert.latest_value
            ),
        });
    }

    // 7. Anomalies (use days as baseline window, moderate threshold)
    let anomaly_result = anomaly::detect(db, None, days.max(14), Threshold::Moderate)?;
    let anomalies = anomaly_result.anomalies;

    // Add anomaly alerts
    for a in &anomalies {
        alerts.push(AlertItem {
            alert_type: "anomaly".to_string(),
            message: a.summary.clone(),
        });
    }

    // 8. Generate top-level summary
    let summary = generate_top_summary(&metrics, &goals, &medications, &streaks, &anomalies);

    Ok(ContextResult {
        generated_at: now.to_rfc3339(),
        period: ContextPeriod {
            start: start_date.to_string(),
            end: today.to_string(),
            days,
        },
        summary,
        metrics,
        goals,
        medications,
        streaks,
        alerts,
        anomalies,
    })
}

fn generate_metric_summary(
    metric_type: &str,
    latest: &Option<LatestValue>,
    trend: &Option<TrendInfo>,
    stats: &MetricStats,
) -> String {
    let mut parts = Vec::new();

    if let Some(ref l) = latest {
        parts.push(format!("{} at {:.1}", metric_type, l.value));
    }

    if let Some(ref t) = trend {
        if t.direction != "stable" {
            parts.push(format!("{} {:.1} {}", t.direction, t.rate.abs(), t.rate_unit));
        } else {
            parts.push("stable".to_string());
        }
    }

    if stats.count > 1 {
        parts.push(format!("{} readings", stats.count));
    }

    if parts.is_empty() {
        "no data".to_string()
    } else {
        parts.join(", ")
    }
}

fn generate_top_summary(
    metrics: &HashMap<String, MetricContext>,
    goals: &[GoalContext],
    medications: &Option<MedicationContext>,
    streaks: &status::Streaks,
    anomalies: &[Anomaly],
) -> String {
    let mut parts = Vec::new();

    // Metrics overview
    if !metrics.is_empty() {
        parts.push(format!("Tracking {} metric type(s).", metrics.len()));
    } else {
        parts.push("No metrics tracked in this period.".to_string());
    }

    // Goals
    if !goals.is_empty() {
        let met = goals.iter().filter(|g| g.is_met).count();
        parts.push(format!("{}/{} goal(s) met.", met, goals.len()));
    }

    // Medications
    if let Some(ref meds) = medications {
        parts.push(meds.summary.clone());
    }

    // Streaks
    if streaks.logging_days > 0 {
        parts.push(format!("Logging streak: {} day(s).", streaks.logging_days));
    }

    // Anomalies
    if !anomalies.is_empty() {
        parts.push(format!("{} anomal{} detected.", anomalies.len(), if anomalies.len() == 1 { "y" } else { "ies" }));
    }

    parts.join(" ")
}
```

**Step 4: Register the module**

In `src/core/mod.rs`, add:

```rust
pub mod context;
```

**Step 5: Run tests**

Run: `cargo test --test context_test`
Expected: all 7 tests PASS

**Step 6: Commit**

```bash
git add src/core/context.rs tests/context_test.rs
git commit -m "feat(context): add health context briefing core logic with tests"
```

---

## Task 5: Context CLI Command

Wire the context command into the CLI layer.

**Files:**
- Modify: `src/cli.rs`
- Create: `src/cmd/context.rs`
- Modify: `src/cmd/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/output/human.rs`

**Step 1: Write CLI integration test**

Add to `tests/context_test.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_context_cli_json_output() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[profile]\nheight_cm = 180.0\n[units]\nsystem = \"metric\"\n[aliases]\n[alerts]\npain_threshold = 5\npain_consecutive_days = 3\n",
    )
    .unwrap();

    {
        let db = Database::open(&db_path).unwrap();
        let today = Local::now().date_naive();
        for i in 0..7 {
            let date = today - Duration::days(i);
            db.insert_metric(&common::make_metric("weight", 83.0, date)).unwrap();
        }
    }

    let mut cmd = Command::cargo_bin("openvital").unwrap();
    cmd.env("OPENVITAL_HOME", dir.path())
        .arg("context");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"ok\""))
        .stdout(predicate::str::contains("\"command\":\"context\""))
        .stdout(predicate::str::contains("weight"));
}

#[test]
fn test_context_cli_human_output() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[profile]\nheight_cm = 180.0\n[units]\nsystem = \"metric\"\n[aliases]\n[alerts]\npain_threshold = 5\npain_consecutive_days = 3\n",
    )
    .unwrap();

    {
        let db = Database::open(&db_path).unwrap();
        let today = Local::now().date_naive();
        db.insert_metric(&common::make_metric("weight", 83.0, today)).unwrap();
    }

    let mut cmd = Command::cargo_bin("openvital").unwrap();
    cmd.env("OPENVITAL_HOME", dir.path())
        .arg("context")
        .arg("--human");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Health Context"));
}

#[test]
fn test_context_cli_with_days_flag() {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("data.db");
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[profile]\n[units]\nsystem = \"metric\"\n[aliases]\n[alerts]\npain_threshold = 5\npain_consecutive_days = 3\n",
    )
    .unwrap();

    {
        let db = Database::open(&db_path).unwrap();
        let today = Local::now().date_naive();
        db.insert_metric(&common::make_metric("weight", 83.0, today)).unwrap();
    }

    let mut cmd = Command::cargo_bin("openvital").unwrap();
    cmd.env("OPENVITAL_HOME", dir.path())
        .arg("context")
        .arg("--days")
        .arg("14");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"days\":14"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test context_test test_context_cli 2>&1 | head -10`
Expected: FAIL — `context` not a valid subcommand

**Step 3: Add CLI variant**

In `src/cli.rs`, add to the `Commands` enum (after `Anomaly`):

```rust
/// AI health briefing — complete health state in one response
Context {
    /// Lookback window in days (default: 7)
    #[arg(long, default_value = "7")]
    days: u32,

    /// Filter to specific metric types (comma-separated)
    #[arg(long)]
    types: Option<String>,
},
```

**Step 4: Create command handler**

Create `src/cmd/context.rs`:

```rust
use anyhow::Result;

use openvital::core::context;
use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;
use openvital::output::human;

pub fn run(days: u32, types: Option<&str>, human_flag: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;

    let type_filter: Option<Vec<&str>> = types.map(|t| t.split(',').collect());
    let type_refs: Option<&[&str]> = type_filter.as_deref();

    let result = context::compute(&db, &config, days, type_refs)?;

    if human_flag {
        println!("{}", human::format_context(&result));
    } else {
        let out = output::success("context", serde_json::to_value(&result)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
```

**Step 5: Register command module**

In `src/cmd/mod.rs`, add:

```rust
pub mod context;
```

**Step 6: Add dispatch to main.rs**

In `src/main.rs`, add the match arm after `Commands::Anomaly`:

```rust
Commands::Context { days, types } => {
    cmd::context::run(days, types.as_deref(), cli.human)
}
```

**Step 7: Add human formatting**

In `src/output/human.rs`, add:

```rust
use crate::core::context::ContextResult;

/// Format health context briefing for human display.
pub fn format_context(result: &ContextResult) -> String {
    let mut out = format!(
        "=== Health Context ({} days: {} to {}) ===\n",
        result.period.days, result.period.start, result.period.end
    );

    out.push_str(&format!("\n{}\n", result.summary));

    // Metrics
    if !result.metrics.is_empty() {
        out.push_str("\n--- Metrics ---\n");
        let mut sorted_keys: Vec<&String> = result.metrics.keys().collect();
        sorted_keys.sort();
        for key in sorted_keys {
            let m = &result.metrics[key];
            out.push_str(&format!("  {}: {}\n", key, m.summary));
        }
    }

    // Goals
    if !result.goals.is_empty() {
        out.push_str("\n--- Goals ---\n");
        for g in &result.goals {
            let status = if g.is_met { "MET" } else { "..." };
            out.push_str(&format!("  [{}] {}\n", status, g.summary));
        }
    }

    // Medications
    if let Some(ref meds) = result.medications {
        out.push_str(&format!("\n--- Medications ---\n  {}\n", meds.summary));
    }

    // Streaks
    if result.streaks.logging_days > 0 {
        out.push_str(&format!(
            "\n--- Streaks ---\n  Logging: {} day(s)\n",
            result.streaks.logging_days
        ));
    }

    // Alerts
    if !result.alerts.is_empty() {
        out.push_str("\n--- Alerts ---\n");
        for a in &result.alerts {
            out.push_str(&format!("  [{}] {}\n", a.alert_type, a.message));
        }
    }

    out.trim_end().to_string()
}
```

**Step 8: Run all tests**

Run: `cargo test`
Expected: ALL tests PASS (existing + new)

**Step 9: Run clippy and fmt**

Run: `cargo fmt --all && cargo clippy -- -D warnings`
Expected: no errors

**Step 10: Commit**

```bash
git add src/cli.rs src/cmd/context.rs src/cmd/mod.rs src/main.rs src/output/human.rs tests/context_test.rs
git commit -m "feat(context): add AI health briefing CLI command"
```

---

## Task 6: Edge Case & Variant Tests

Add comprehensive edge case testing for both features.

**Files:**
- Modify: `tests/anomaly_test.rs`
- Modify: `tests/context_test.rs`

**Step 1: Add anomaly variant tests**

Append to `tests/anomaly_test.rs`:

```rust
#[test]
fn test_anomaly_all_identical_values() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // All values are 72.0 — IQR is 0, no anomalies possible with moderate
    for i in 0..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("heart_rate", 72.0, date)).unwrap();
    }

    let result = anomaly::detect(&db, Some("heart_rate"), 30, Threshold::Moderate).unwrap();
    // With IQR=0 and factor 1.5, bounds are [72, 72], so even 72.0 is not anomalous
    assert!(result.anomalies.is_empty());
}

#[test]
fn test_anomaly_below_baseline() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    // Baseline: 70-76
    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("heart_rate", 70.0 + (i % 7) as f64, date)).unwrap();
    }

    // Abnormally low value
    db.insert_metric(&common::make_metric("heart_rate", 40.0, today)).unwrap();

    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(!result.anomalies.is_empty());
    assert_eq!(result.anomalies[0].deviation, "below");
}

#[test]
fn test_anomaly_multiple_types_scanned() {
    let (_dir, db) = common::setup_db();
    let today = Local::now().date_naive();

    for i in 1..=14 {
        let date = today - Duration::days(i);
        db.insert_metric(&common::make_metric("weight", 80.0, date)).unwrap();
        db.insert_metric(&common::make_metric("sleep", 7.5, date)).unwrap();
        db.insert_metric(&common::make_metric("pain", 3.0, date)).unwrap();
    }

    // Normal values today
    db.insert_metric(&common::make_metric("weight", 80.0, today)).unwrap();
    db.insert_metric(&common::make_metric("sleep", 7.5, today)).unwrap();
    db.insert_metric(&common::make_metric("pain", 3.0, today)).unwrap();

    let result = anomaly::detect(&db, None, 30, Threshold::Moderate).unwrap();
    assert!(result.scanned_types.len() >= 3);
    assert!(result.anomalies.is_empty());
}
```

**Step 2: Add context variant tests**

Append to `tests/context_test.rs`:

```rust
#[test]
fn test_context_medication_integration() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // Add a medication
    use openvital::core::med::AddMedicationParams;
    openvital::core::med::add_medication(
        &db,
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

    // Take the medication
    openvital::core::med::take_medication(&db, "ibuprofen", None, None, None, None).unwrap();

    let result = context::compute(&db, &config, 7, None).unwrap();
    assert!(result.medications.is_some());
    assert_eq!(result.medications.as_ref().unwrap().active_count, 1);
}

#[test]
fn test_context_pain_alert_included() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // Log pain above threshold (5)
    db.insert_metric(&common::make_metric("pain", 7.0, today)).unwrap();

    let result = context::compute(&db, &config, 7, None).unwrap();
    assert!(
        result.alerts.iter().any(|a| a.alert_type == "pain_elevated"),
        "should include pain alert"
    );
}

#[test]
fn test_context_multiple_days_of_data() {
    let (_dir, db) = common::setup_db();
    let config = make_test_config();
    let today = Local::now().date_naive();

    // 30 days of varied weight data
    for i in 0..30 {
        let date = today - Duration::days(i);
        let weight = 85.0 - i as f64 * 0.1; // slow decline
        db.insert_metric(&common::make_metric("weight", weight, date)).unwrap();
    }

    let result = context::compute(&db, &config, 30, None).unwrap();

    let weight = &result.metrics["weight"];
    assert_eq!(weight.stats.count, 30);
    assert!(weight.trend.is_some());
    assert_eq!(weight.trend.as_ref().unwrap().direction, "decreasing");
}
```

**Step 3: Run all tests**

Run: `cargo test`
Expected: ALL tests PASS

**Step 4: Commit**

```bash
git add tests/anomaly_test.rs tests/context_test.rs
git commit -m "test: add edge case and variant tests for anomaly and context"
```

---

## Task 7: Code Quality & Optimization Pass

Final review, clippy cleanup, and documentation.

**Files:**
- All new files from Tasks 1-6

**Step 1: Run full test suite**

Run: `cargo test`
Expected: ALL tests PASS

**Step 2: Run clippy with strict warnings**

Run: `cargo clippy -- -D warnings`
Expected: no warnings

**Step 3: Run formatter**

Run: `cargo fmt --all -- --check`
Expected: no formatting issues

**Step 4: Verify the CLI help output**

Run: `cargo run -- --help`
Expected: `context` and `anomaly` commands appear in help output

Run: `cargo run -- context --help`
Expected: shows `--days` and `--types` flags

Run: `cargo run -- anomaly --help`
Expected: shows `--days`, `--threshold`, and optional type argument

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "refactor: code quality and clippy fixes for context and anomaly"
```

---

## Summary of All Files Changed

### New Files (7)
- `src/models/anomaly.rs` — Anomaly, Baseline, Severity, Threshold types
- `src/core/anomaly.rs` — IQR-based anomaly detection engine
- `src/core/context.rs` — Health context briefing composition
- `src/cmd/anomaly.rs` — Anomaly command handler
- `src/cmd/context.rs` — Context command handler
- `tests/anomaly_test.rs` — 11+ anomaly tests
- `tests/context_test.rs` — 10+ context tests

### Modified Files (6)
- `src/models/mod.rs` — register `anomaly` module
- `src/core/mod.rs` — register `anomaly` and `context` modules
- `src/cmd/mod.rs` — register `anomaly` and `context` modules
- `src/cli.rs` — add `Anomaly` and `Context` command variants
- `src/main.rs` — add dispatch for new commands
- `src/output/human.rs` — add formatting for anomaly and context
- `src/db/metrics.rs` — add `distinct_metric_types()` query

### Estimated Commit Count
7 commits across 7 tasks
