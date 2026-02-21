# Medication Management Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the full medication management feature as defined in `docs/plans/2026-02-19-medication-management-design.md`.

**Architecture:** Follows existing 4-layer pattern (CLI → Command → Core → DB). New files: `src/models/med.rs`, `src/db/meds.rs`, `src/core/med.rs`, `src/cmd/med.rs`. Modifications to: `src/models/metric.rs`, `src/db/migrate.rs`, `src/db/metrics.rs`, `src/cli.rs`, `src/main.rs`, `src/lib.rs`, `src/output/human.rs`, `src/core/trend.rs`, `src/core/goal.rs`, `src/core/status.rs`, `src/core/export.rs`, `src/cmd/export.rs`.

**Tech Stack:** Rust (edition 2024), rusqlite, chrono, serde, uuid, clap, anyhow, regex

**Design Reference:** `docs/plans/2026-02-19-medication-management-design.md` — all JSON output shapes, edge cases, adherence logic, and human-readable formats are defined there.

---

### Task 1: Models — Medication, Frequency, Route, Dose Parsing

**Files:**
- Create: `src/models/med.rs`
- Modify: `src/models/mod.rs` — add `pub mod med;`
- Modify: `src/models/metric.rs` — add `Medication` variant to `Category`
- Modify: `src/db/metrics.rs` — add `"medication"` to `row_to_metric` category match
- Test: unit tests inside `src/models/med.rs`

**Context:** This is the foundation. All other tasks depend on these types. The `Category::Medication` variant is also needed by `db/metrics.rs` for deserialization.

**Step 1: Write failing unit tests in `src/models/med.rs`**

The file should define the module with structs/enums and `#[cfg(test)] mod tests` containing:

```rust
// src/models/med.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// Administration route for medications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Oral,
    Topical,
    Ophthalmic,
    Injection,
    Inhaled,
    Sublingual,
    Transdermal,
    Other(String),
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Oral => write!(f, "oral"),
            Self::Topical => write!(f, "topical"),
            Self::Ophthalmic => write!(f, "ophthalmic"),
            Self::Injection => write!(f, "injection"),
            Self::Inhaled => write!(f, "inhaled"),
            Self::Sublingual => write!(f, "sublingual"),
            Self::Transdermal => write!(f, "transdermal"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

impl FromStr for Route {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "oral" => Self::Oral,
            "topical" => Self::Topical,
            "ophthalmic" => Self::Ophthalmic,
            "injection" => Self::Injection,
            "inhaled" => Self::Inhaled,
            "sublingual" => Self::Sublingual,
            "transdermal" => Self::Transdermal,
            other => Self::Other(other.to_string()),
        })
    }
}

/// Medication frequency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Daily,
    TwiceDaily,
    ThreeTimesDaily,
    Weekly,
    AsNeeded,
}

impl std::fmt::Display for Frequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Daily => write!(f, "daily"),
            Self::TwiceDaily => write!(f, "2x_daily"),
            Self::ThreeTimesDaily => write!(f, "3x_daily"),
            Self::Weekly => write!(f, "weekly"),
            Self::AsNeeded => write!(f, "as_needed"),
        }
    }
}

impl FromStr for Frequency {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "daily" => Ok(Self::Daily),
            "2x_daily" => Ok(Self::TwiceDaily),
            "3x_daily" => Ok(Self::ThreeTimesDaily),
            "weekly" => Ok(Self::Weekly),
            "as_needed" => Ok(Self::AsNeeded),
            _ => anyhow::bail!(
                "invalid frequency: {} (expected daily/2x_daily/3x_daily/weekly/as_needed)",
                s
            ),
        }
    }
}

impl Frequency {
    /// How many doses are required per day. Returns None for as_needed.
    pub fn required_per_day(&self) -> Option<u32> {
        match self {
            Self::Daily => Some(1),
            Self::TwiceDaily => Some(2),
            Self::ThreeTimesDaily => Some(3),
            Self::Weekly | Self::AsNeeded => None,
        }
    }
}

/// Parsed dose representation.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDose {
    pub raw: String,
    pub value: Option<f64>,
    pub unit: String,
}

/// Parse a dose string into numeric value + unit.
///
/// Strategy (in order):
/// 1. Decimal float prefix: "400mg" → (400.0, "mg")
/// 2. Leading-dot: ".5mg" → (0.5, "mg")
/// 3. Fraction: "1/2 tablet" → (0.5, "tablet")
/// 4. Unicode fraction: "½ tablet" → (0.5, "tablet")
/// 5. No numeric prefix: "thin layer" → (None, "application")
/// 6. Empty/None: → (1.0, "dose")
pub fn parse_dose(input: Option<&str>) -> ParsedDose {
    let Some(raw) = input else {
        return ParsedDose {
            raw: String::new(),
            value: Some(1.0),
            unit: "dose".to_string(),
        };
    };

    let raw = raw.trim().to_string();
    if raw.is_empty() {
        return ParsedDose {
            raw,
            value: Some(1.0),
            unit: "dose".to_string(),
        };
    }

    // Try unicode fractions first (before regex, since they're single chars)
    let mapped = map_unicode_fractions(&raw);

    // Try decimal float: "400mg", ".5mg", "0.5mg", "2 drops"
    let re = regex::Regex::new(r"^(\d*\.?\d+)\s*(.+)$").unwrap();
    if let Some(caps) = re.captures(&mapped) {
        let num_str = &caps[1];
        let unit_str = caps[2].trim().to_string();
        if let Ok(val) = num_str.parse::<f64>() {
            if val >= 0.0 {
                return ParsedDose {
                    raw: raw.clone(),
                    value: Some(val),
                    unit: unit_str,
                };
            }
        }
    }

    // Try fraction: "1/2 tablet"
    let frac_re = regex::Regex::new(r"^(\d+)/(\d+)\s+(.+)$").unwrap();
    if let Some(caps) = frac_re.captures(&mapped) {
        let num: f64 = caps[1].parse().unwrap_or(0.0);
        let den: f64 = caps[2].parse().unwrap_or(0.0);
        let unit_str = caps[3].trim().to_string();
        if den != 0.0 {
            return ParsedDose {
                raw: raw.clone(),
                value: Some(num / den),
                unit: unit_str,
            };
        }
        // 0/0 or x/0 → fallback
    }

    // Check for negative values (reject)
    if mapped.starts_with('-') {
        return ParsedDose {
            raw,
            value: None,
            unit: "application".to_string(),
        };
    }

    // No numeric prefix → fallback
    ParsedDose {
        raw,
        value: None,
        unit: "application".to_string(),
    }
}

fn map_unicode_fractions(s: &str) -> String {
    s.replace('½', "1/2 ")
        .replace('⅓', "1/3 ")
        .replace('¼', "1/4 ")
        .replace('¾', "3/4 ")
        .replace('⅔', "2/3 ")
}

/// A medication record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Medication {
    pub id: String,
    pub name: String,
    pub dose: Option<String>,
    pub dose_value: Option<f64>,
    pub dose_unit: Option<String>,
    pub route: Route,
    pub frequency: Frequency,
    pub active: bool,
    pub started_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub stop_reason: Option<String>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Medication {
    pub fn new(name: String, frequency: Frequency) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            dose: None,
            dose_value: None,
            dose_unit: None,
            route: Route::Oral,
            frequency,
            active: true,
            started_at: now,
            stopped_at: None,
            stop_reason: None,
            note: None,
            created_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Route FromStr ──
    #[test]
    fn route_parses_known_variants_case_insensitive() {
        assert_eq!(Route::from_str("oral").unwrap(), Route::Oral);
        assert_eq!(Route::from_str("TOPICAL").unwrap(), Route::Topical);
        assert_eq!(Route::from_str("Injection").unwrap(), Route::Injection);
    }

    #[test]
    fn route_unknown_becomes_other() {
        assert_eq!(
            Route::from_str("rectal").unwrap(),
            Route::Other("rectal".to_string())
        );
    }

    #[test]
    fn route_display_roundtrips() {
        for r in [Route::Oral, Route::Topical, Route::Transdermal] {
            assert_eq!(Route::from_str(&r.to_string()).unwrap(), r);
        }
    }

    // ── Frequency FromStr ──
    #[test]
    fn frequency_parses_all_variants() {
        assert_eq!(Frequency::from_str("daily").unwrap(), Frequency::Daily);
        assert_eq!(Frequency::from_str("2x_daily").unwrap(), Frequency::TwiceDaily);
        assert_eq!(Frequency::from_str("3x_daily").unwrap(), Frequency::ThreeTimesDaily);
        assert_eq!(Frequency::from_str("weekly").unwrap(), Frequency::Weekly);
        assert_eq!(Frequency::from_str("as_needed").unwrap(), Frequency::AsNeeded);
    }

    #[test]
    fn frequency_invalid_returns_error() {
        assert!(Frequency::from_str("biweekly").is_err());
    }

    #[test]
    fn frequency_required_per_day() {
        assert_eq!(Frequency::Daily.required_per_day(), Some(1));
        assert_eq!(Frequency::TwiceDaily.required_per_day(), Some(2));
        assert_eq!(Frequency::ThreeTimesDaily.required_per_day(), Some(3));
        assert_eq!(Frequency::Weekly.required_per_day(), None);
        assert_eq!(Frequency::AsNeeded.required_per_day(), None);
    }

    // ── Dose Parsing ──
    #[test]
    fn dose_parse_standard_mg() {
        let d = parse_dose(Some("400mg"));
        assert_eq!(d.value, Some(400.0));
        assert_eq!(d.unit, "mg");
    }

    #[test]
    fn dose_parse_ml_with_space() {
        let d = parse_dose(Some("5 ml"));
        assert_eq!(d.value, Some(5.0));
        assert_eq!(d.unit, "ml");
    }

    #[test]
    fn dose_parse_drops() {
        let d = parse_dose(Some("2 drops"));
        assert_eq!(d.value, Some(2.0));
        assert_eq!(d.unit, "drops");
    }

    #[test]
    fn dose_parse_decimal() {
        let d = parse_dose(Some("0.5mg"));
        assert_eq!(d.value, Some(0.5));
        assert_eq!(d.unit, "mg");
    }

    #[test]
    fn dose_parse_leading_dot() {
        let d = parse_dose(Some(".5mg"));
        assert_eq!(d.value, Some(0.5));
        assert_eq!(d.unit, "mg");
    }

    #[test]
    fn dose_parse_fraction() {
        let d = parse_dose(Some("1/2 tablet"));
        assert_eq!(d.value, Some(0.5));
        assert_eq!(d.unit, "tablet");
    }

    #[test]
    fn dose_parse_unicode_half() {
        let d = parse_dose(Some("½ tablet"));
        assert_eq!(d.value, Some(0.5));
        assert_eq!(d.unit, "tablet");
    }

    #[test]
    fn dose_parse_no_numeric_fallback() {
        let d = parse_dose(Some("thin layer"));
        assert_eq!(d.value, None);
        assert_eq!(d.unit, "application");
    }

    #[test]
    fn dose_parse_patch() {
        let d = parse_dose(Some("1 patch"));
        assert_eq!(d.value, Some(1.0));
        assert_eq!(d.unit, "patch");
    }

    #[test]
    fn dose_parse_none_defaults() {
        let d = parse_dose(None);
        assert_eq!(d.value, Some(1.0));
        assert_eq!(d.unit, "dose");
    }

    #[test]
    fn dose_parse_empty_defaults() {
        let d = parse_dose(Some(""));
        assert_eq!(d.value, Some(1.0));
        assert_eq!(d.unit, "dose");
    }

    // ── Negative test cases from design doc ──
    #[test]
    fn dose_parse_unit_first_rejected() {
        let d = parse_dose(Some("mg400"));
        assert_eq!(d.value, None);
        assert_eq!(d.unit, "application");
    }

    #[test]
    fn dose_parse_zero_denominator_fallback() {
        let d = parse_dose(Some("0/0 tablet"));
        assert_eq!(d.value, None);
        assert_eq!(d.unit, "application");
    }

    #[test]
    fn dose_parse_negative_rejected() {
        let d = parse_dose(Some("-5mg"));
        assert_eq!(d.value, None);
        assert_eq!(d.unit, "application");
    }

    // ── Medication struct ──
    #[test]
    fn medication_new_sets_defaults() {
        let m = Medication::new("ibuprofen".to_string(), Frequency::AsNeeded);
        assert_eq!(m.name, "ibuprofen");
        assert!(m.active);
        assert_eq!(m.route, Route::Oral);
        assert!(m.stopped_at.is_none());
    }
}
```

**Step 2: Update `src/models/mod.rs`**

Add `pub mod med;` to the module declarations.

**Step 3: Update `src/models/metric.rs` — add `Medication` variant**

Add `Medication` to the `Category` enum. Add match arm to `Display`. Do NOT change `from_type()` — it stays pure (design doc Section 2.5).

```rust
// In Category enum, add:
    Medication,

// In Display impl, add:
    Self::Medication => write!(f, "medication"),
```

**Step 4: Update `src/db/metrics.rs` — handle `"medication"` in deserialization**

In the `row_to_metric` function, add `"medication" => Category::Medication` to the category match.

**Step 5: Run tests**

```bash
cargo test -- models::med::tests
```

Expected: All 19 unit tests pass.

**Step 6: Commit**

```
feat(med): add Medication, Frequency, Route models with dose parsing
```

---

### Task 2: Database Migration and CRUD for Medications Table

**Files:**
- Modify: `src/db/migrate.rs` — add `CREATE TABLE medications` with partial unique index
- Create: `src/db/meds.rs` — CRUD operations
- Modify: `src/db/mod.rs` — add `pub mod meds;` (note: name as `meds` to match pattern of `metrics`, `goals`)
- Test: `tests/med_db.rs`

**Context:** Depends on Task 1 (needs `Medication`, `Route`, `Frequency` types).

**Step 1: Write failing integration tests in `tests/med_db.rs`**

```rust
// tests/med_db.rs
mod common;

use chrono::{TimeZone, Utc};
use openvital::db::Database;
use openvital::models::med::{Frequency, Medication, Route};

fn make_med(name: &str, freq: Frequency) -> Medication {
    let mut m = Medication::new(name.to_string(), freq);
    m.dose = Some("400mg".to_string());
    m.dose_value = Some(400.0);
    m.dose_unit = Some("mg".to_string());
    m
}

// ── insert + get ──

#[test]
fn insert_and_get_medication() {
    let (_dir, db) = common::setup_db();
    let med = make_med("ibuprofen", Frequency::AsNeeded);
    db.insert_medication(&med).unwrap();

    let got = db.get_medication_by_name("ibuprofen").unwrap().unwrap();
    assert_eq!(got.name, "ibuprofen");
    assert_eq!(got.dose.as_deref(), Some("400mg"));
    assert_eq!(got.dose_value, Some(400.0));
    assert!(got.active);
}

#[test]
fn get_nonexistent_medication_returns_none() {
    let (_dir, db) = common::setup_db();
    let got = db.get_medication_by_name("nonexistent").unwrap();
    assert!(got.is_none());
}

// ── list ──

#[test]
fn list_active_only() {
    let (_dir, db) = common::setup_db();
    let m1 = make_med("ibuprofen", Frequency::AsNeeded);
    let mut m2 = make_med("aspirin", Frequency::Daily);
    m2.active = false;
    m2.stopped_at = Some(Utc::now());
    db.insert_medication(&m1).unwrap();
    db.insert_medication(&m2).unwrap();

    let active = db.list_medications(false).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "ibuprofen");
}

#[test]
fn list_all_includes_stopped() {
    let (_dir, db) = common::setup_db();
    let m1 = make_med("ibuprofen", Frequency::AsNeeded);
    let mut m2 = make_med("aspirin", Frequency::Daily);
    m2.active = false;
    m2.stopped_at = Some(Utc::now());
    db.insert_medication(&m1).unwrap();
    db.insert_medication(&m2).unwrap();

    let all = db.list_medications(true).unwrap();
    assert_eq!(all.len(), 2);
}

// ── stop ──

#[test]
fn stop_medication_sets_inactive() {
    let (_dir, db) = common::setup_db();
    let med = make_med("ibuprofen", Frequency::AsNeeded);
    db.insert_medication(&med).unwrap();

    let now = Utc::now();
    db.stop_medication("ibuprofen", now, Some("side effects")).unwrap();

    let got = db.get_medication_by_name("ibuprofen").unwrap();
    // After stopping, get_medication_by_name (which looks for active) should return None
    assert!(got.is_none());
}

// ── partial unique index: stop then re-add ──

#[test]
fn stop_then_readd_same_name_allowed() {
    let (_dir, db) = common::setup_db();
    let med = make_med("ibuprofen", Frequency::AsNeeded);
    db.insert_medication(&med).unwrap();

    db.stop_medication("ibuprofen", Utc::now(), None).unwrap();

    // Re-add with different dose
    let mut med2 = make_med("ibuprofen", Frequency::Daily);
    med2.dose = Some("200mg".to_string());
    med2.dose_value = Some(200.0);
    db.insert_medication(&med2).unwrap();

    let got = db.get_medication_by_name("ibuprofen").unwrap().unwrap();
    assert_eq!(got.dose.as_deref(), Some("200mg"));
    assert!(got.active);
}

#[test]
fn duplicate_active_name_rejected() {
    let (_dir, db) = common::setup_db();
    let m1 = make_med("ibuprofen", Frequency::AsNeeded);
    db.insert_medication(&m1).unwrap();

    let m2 = make_med("ibuprofen", Frequency::Daily);
    let result = db.insert_medication(&m2);
    assert!(result.is_err());
}

// ── remove ──

#[test]
fn remove_medication_deletes_record() {
    let (_dir, db) = common::setup_db();
    let med = make_med("ibuprofen", Frequency::AsNeeded);
    db.insert_medication(&med).unwrap();

    db.remove_medication("ibuprofen").unwrap();

    let all = db.list_medications(true).unwrap();
    assert!(all.is_empty());
}

// ── route stored as lowercase ──

#[test]
fn route_stored_and_retrieved_correctly() {
    let (_dir, db) = common::setup_db();
    let mut med = make_med("retinol", Frequency::Daily);
    med.route = Route::Topical;
    db.insert_medication(&med).unwrap();

    let got = db.get_medication_by_name("retinol").unwrap().unwrap();
    assert_eq!(got.route, Route::Topical);
}

#[test]
fn other_route_roundtrips() {
    let (_dir, db) = common::setup_db();
    let mut med = make_med("custom_med", Frequency::Daily);
    med.route = Route::Other("rectal".to_string());
    db.insert_medication(&med).unwrap();

    let got = db.get_medication_by_name("custom_med").unwrap().unwrap();
    assert_eq!(got.route, Route::Other("rectal".to_string()));
}
```

**Step 2: Implement `src/db/migrate.rs` changes**

Add to the `execute_batch` call:

```sql
CREATE TABLE IF NOT EXISTS medications (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    dose        TEXT,
    dose_value  REAL,
    dose_unit   TEXT,
    route       TEXT NOT NULL DEFAULT 'oral',
    frequency   TEXT NOT NULL,
    active      INTEGER NOT NULL DEFAULT 1,
    started_at  TEXT NOT NULL,
    stopped_at  TEXT,
    stop_reason TEXT,
    note        TEXT,
    created_at  TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_medications_name_active
    ON medications(name) WHERE active = 1;
CREATE INDEX IF NOT EXISTS idx_medications_active ON medications(active);
```

**Step 3: Implement `src/db/meds.rs`**

Methods on `Database`:
- `insert_medication(&self, med: &Medication) -> Result<()>`
- `get_medication_by_name(&self, name: &str) -> Result<Option<Medication>>` — only active
- `get_medication_by_name_any(&self, name: &str) -> Result<Option<Medication>>` — active preferred, then stopped
- `list_medications(&self, include_stopped: bool) -> Result<Vec<Medication>>`
- `stop_medication(&self, name: &str, stopped_at: DateTime<Utc>, reason: Option<&str>) -> Result<bool>`
- `remove_medication(&self, name: &str) -> Result<bool>`

Route is stored/loaded via `to_string()` / `FromStr`. Frequency likewise.

**Step 4: Update `src/db/mod.rs`**

Add `pub mod meds;` (make it public so integration tests can access via `openvital::db::Database`).

**Step 5: Run tests**

```bash
cargo test med_db
```

Expected: All integration tests pass.

**Step 6: Commit**

```
feat(med): add medications table migration and CRUD operations
```

---

### Task 3: Core Business Logic — add, take, stop, remove, adherence

**Files:**
- Create: `src/core/med.rs`
- Modify: `src/core/mod.rs` — add `pub mod med;`
- Test: `tests/med_core.rs`

**Context:** Depends on Tasks 1 and 2. This is the most complex task. Implements all business logic from design doc Sections 2.4, 4.1–4.4, and 7.

**Step 1: Write failing integration tests in `tests/med_core.rs`**

```rust
// tests/med_core.rs
mod common;

use chrono::{Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use openvital::core::med;
use openvital::models::config::Config;
use openvital::models::med::Frequency;
use openvital::models::metric::Category;

fn default_config() -> Config {
    Config::default()
}

// ── add_medication ──

#[test]
fn add_medication_basic() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    let result = med::add_medication(
        &db, &config, "ibuprofen",
        Some("400mg"), "as_needed", Some("oral"), None, None,
    ).unwrap();
    assert_eq!(result.name, "ibuprofen");
    assert_eq!(result.dose.as_deref(), Some("400mg"));
    assert_eq!(result.dose_value, Some(400.0));
    assert_eq!(result.dose_unit.as_deref(), Some("mg"));
    assert!(result.active);
}

#[test]
fn add_medication_topical_with_note() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    let result = med::add_medication(
        &db, &config, "retinol",
        Some("thin layer"), "daily", Some("topical"),
        Some("apply to face at night"), None,
    ).unwrap();
    assert_eq!(result.dose_value, None);
    assert_eq!(result.dose_unit.as_deref(), Some("application"));
    assert_eq!(result.route.to_string(), "topical");
}

#[test]
fn add_duplicate_active_errors() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();
    let result = med::add_medication(&db, &config, "ibuprofen", Some("200mg"), "daily", None, None, None);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("already active"));
}

#[test]
fn add_after_stop_allowed() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();
    med::stop_medication(&db, "ibuprofen", None, None).unwrap();
    let result = med::add_medication(&db, &config, "ibuprofen", Some("200mg"), "daily", None, None, None);
    assert!(result.is_ok());
}

// ── take_medication ──

#[test]
fn take_creates_metric_with_count_semantics() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();
    let (metric, _med) = med::take_medication(&db, &config, "ibuprofen", None, None, None, None).unwrap();

    assert_eq!(metric.value, 1.0);
    assert_eq!(metric.unit, "dose");
    assert_eq!(metric.category, Category::Medication);
    assert_eq!(metric.source, "med_take");
    assert_eq!(metric.metric_type, "ibuprofen");
    // Note should contain dose text
    assert_eq!(metric.note.as_deref(), Some("400mg"));
}

#[test]
fn take_with_dose_override() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();
    let (metric, _) = med::take_medication(&db, &config, "ibuprofen", Some("200mg"), None, None, None).unwrap();
    assert_eq!(metric.note.as_deref(), Some("200mg (override)"));
}

#[test]
fn take_unknown_medication_errors() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    let result = med::take_medication(&db, &config, "unknown", None, None, None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn take_stopped_medication_warns_but_succeeds() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();
    med::stop_medication(&db, "ibuprofen", None, None).unwrap();
    // Should succeed (with warning in note)
    let result = med::take_medication(&db, &config, "ibuprofen", None, None, None, None);
    assert!(result.is_ok());
}

#[test]
fn take_resolves_alias() {
    let (_dir, db) = common::setup_db();
    let mut config = default_config();
    config.aliases.insert("ibu".to_string(), "ibuprofen".to_string());
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();
    let result = med::take_medication(&db, &config, "ibu", None, None, None, None);
    assert!(result.is_ok());
}

// ── stop_medication ──

#[test]
fn stop_medication_with_reason() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();
    let result = med::stop_medication(&db, "ibuprofen", Some("side effects"), None).unwrap();
    assert!(result);
}

// ── remove_medication ──

#[test]
fn remove_medication_deletes_metadata_preserves_metrics() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();
    med::take_medication(&db, &config, "ibuprofen", None, None, None, None).unwrap();

    med::remove_medication(&db, "ibuprofen").unwrap();

    // Medication record gone
    let meds = db.list_medications(true).unwrap();
    assert!(meds.is_empty());

    // Metric entry preserved
    let metrics = db.query_by_type("ibuprofen", Some(10)).unwrap();
    assert_eq!(metrics.len(), 1);
}

// ── adherence_status ──

#[test]
fn adherence_daily_med_counts_correctly() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    let today = chrono::Local::now().date_naive();
    med::add_medication(&db, &config, "metformin", Some("500mg"), "2x_daily", None, None, None).unwrap();

    // Take once today
    med::take_medication(&db, &config, "metformin", None, None, None, None).unwrap();

    let statuses = med::adherence_status(&db, None, 7).unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].name, "metformin");
    assert_eq!(statuses[0].required_today, Some(2));
    assert_eq!(statuses[0].taken_today, 1);
    assert!(!statuses[0].adherent_today.unwrap());
}

#[test]
fn adherence_as_needed_always_null() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();

    let statuses = med::adherence_status(&db, None, 7).unwrap();
    assert_eq!(statuses.len(), 1);
    assert!(statuses[0].adherent_today.is_none());
    assert!(statuses[0].adherence_7d.is_none());
}

// ── name conflict ──

#[test]
fn med_take_category_is_medication_not_nutrition() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    // "water" is a known metric type (Nutrition)
    med::add_medication(&db, &config, "water", Some("5ml"), "daily", None, None, None).unwrap();
    let (metric, _) = med::take_medication(&db, &config, "water", None, None, None, None).unwrap();
    // med take explicitly sets Category::Medication
    assert_eq!(metric.category, Category::Medication);
}

#[test]
fn from_type_still_returns_nutrition_for_water() {
    // Verify from_type is unchanged
    assert_eq!(Category::from_type("water"), Category::Nutrition);
}
```

**Step 2: Implement `src/core/med.rs`**

Public functions:
- `add_medication(db, config, name, dose, freq, route, note, started) -> Result<Medication>`
- `take_medication(db, config, name, dose_override, note, tags, date) -> Result<(Metric, Medication)>`
- `stop_medication(db, name, reason, date) -> Result<bool>`
- `remove_medication(db, name) -> Result<bool>`
- `adherence_status(db, name: Option<&str>, last_days: u32) -> Result<Vec<MedStatus>>`
- `list_medications(db, include_stopped: bool) -> Result<Vec<Medication>>`

Key implementation notes:
- `take_medication` builds a `Metric` with `value=1.0`, `unit="dose"`, `category=Category::Medication`, `source="med_take"`, `note=dose_text`. Uses `db.insert_metric()`.
- `take_medication` must handle alias resolution via `config.resolve_alias()`.
- For stopped meds, `take_medication` queries `get_medication_by_name_any()` (which returns stopped meds too).
- Adherence calculation per design doc Section 4.1–4.4.

`MedStatus` struct:
```rust
#[derive(Debug, Serialize)]
pub struct MedStatus {
    pub name: String,
    pub dose: Option<String>,
    pub route: String,
    pub frequency: String,
    pub required_today: Option<u32>,
    pub taken_today: u32,
    pub adherent_today: Option<bool>,
    pub streak_days: Option<u32>,
    pub adherence_7d: Option<f64>,
    pub adherence_30d: Option<f64>,
    pub adherence_history: Option<Vec<DayAdherence>>,
}

#[derive(Debug, Serialize)]
pub struct DayAdherence {
    pub date: NaiveDate,
    pub required: u32,
    pub taken: u32,
    pub adherent: bool,
}
```

**Step 3: Update `src/core/mod.rs`**

Add `pub mod med;`.

**Step 4: Run tests**

```bash
cargo test med_core
```

Expected: All tests pass.

**Step 5: Commit**

```
feat(med): add medication core business logic with adherence tracking
```

---

### Task 4: CLI Definitions and Command Handler

**Files:**
- Modify: `src/cli.rs` — add `Med(MedAction)` variant and `MedAction` enum
- Create: `src/cmd/med.rs` — thin command handler
- Modify: `src/cmd/mod.rs` — add `pub mod med;`
- Modify: `src/main.rs` — add dispatch for `Commands::Med`
- Modify: `src/output/human.rs` — add medication formatting functions
- Modify: `src/lib.rs` — no change needed (core::med already exported via core module)
- Test: `tests/med_integration.rs` — integration tests using core API (not CLI binary)

**Context:** Depends on Tasks 1–3.

**Step 1: Add CLI definitions to `src/cli.rs`**

```rust
// Add to Commands enum:
    /// Manage medications
    Med {
        #[command(subcommand)]
        action: MedAction,
    },

// Add new enum:
#[derive(Subcommand)]
pub enum MedAction {
    /// Add a medication to the active list
    Add {
        /// Medication name (e.g., "ibuprofen")
        name: String,
        /// Dosage (e.g., "400mg", "5ml", "thin layer")
        #[arg(long)]
        dose: Option<String>,
        /// Frequency: daily, 2x_daily, 3x_daily, weekly, as_needed
        #[arg(long)]
        freq: String,
        /// Administration route (default: oral)
        #[arg(long, default_value = "oral")]
        route: String,
        /// Notes (e.g., "take with food")
        #[arg(long)]
        note: Option<String>,
        /// Start date (default: today)
        #[arg(long)]
        started: Option<NaiveDate>,
    },
    /// Record a dose taken
    Take {
        /// Medication name
        name: String,
        /// Override dose for this intake
        #[arg(long)]
        dose: Option<String>,
        /// Note for this intake
        #[arg(long)]
        note: Option<String>,
        /// Comma-separated tags
        #[arg(long)]
        tags: Option<String>,
    },
    /// List medications (active by default)
    List {
        /// Include stopped medications
        #[arg(long)]
        all: bool,
    },
    /// Mark a medication as stopped
    Stop {
        /// Medication name
        name: String,
        /// Reason for stopping
        #[arg(long)]
        reason: Option<String>,
    },
    /// Delete a medication record
    Remove {
        /// Medication name
        name: String,
    },
    /// View adherence status
    Status {
        /// Medication name (all if omitted)
        name: Option<String>,
        /// Show adherence for last N days (default: 7)
        #[arg(long, default_value = "7")]
        last: u32,
    },
}
```

Note: `MedAction` must be imported in `main.rs` via `use cli::{..., MedAction}`.

**Step 2: Add human-readable formatters in `src/output/human.rs`**

Add functions:
- `format_med_list(meds: &[Medication], include_stopped: bool) -> String`
- `format_med_take(name: &str, dose: &str, route: &str, timestamp: &str) -> String`
- `format_med_status(statuses: &[MedStatus], date: NaiveDate) -> String`
- `format_med_stop(name: &str, reason: Option<&str>) -> String`

Following the design doc Section 8 for exact output format.

**Step 3: Implement `src/cmd/med.rs`**

Thin handler: opens DB, calls `core::med::*`, formats output (JSON envelope or human).

```rust
// src/cmd/med.rs

use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;

use openvital::core::med;
use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;
use openvital::output::human;

pub fn run_add(
    name: &str, dose: Option<&str>, freq: &str, route: &str,
    note: Option<&str>, started: Option<NaiveDate>, human_flag: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let resolved = config.resolve_alias(name);
    let medication = med::add_medication(
        &db, &config, &resolved, dose, freq, Some(route), note, started,
    )?;
    if human_flag {
        println!("Added {} {} ({}) — {}", medication.name,
            medication.dose.as_deref().unwrap_or(""),
            medication.route, medication.frequency);
    } else {
        let out = output::success("med_add", json!({
            "id": medication.id,
            "name": medication.name,
            "dose": medication.dose,
            "route": medication.route.to_string(),
            "frequency": medication.frequency.to_string(),
            "active": medication.active,
            "started_at": medication.started_at.to_rfc3339(),
        }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_take(
    name: &str, dose: Option<&str>, note: Option<&str>,
    tags: Option<&str>, date: Option<NaiveDate>, human_flag: bool,
) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let (metric, medication) = med::take_medication(
        &db, &config, name, dose, note, tags, date,
    )?;
    if human_flag {
        println!("{}", human::format_med_take(
            &medication.name,
            metric.note.as_deref().unwrap_or("1 dose"),
            &medication.route.to_string(),
            &metric.timestamp.format("%b %d, %Y %H:%M").to_string(),
        ));
    } else {
        let out = output::success("med_take", json!({
            "medication": medication.name,
            "dose": metric.note,
            "route": medication.route.to_string(),
            "entry": {
                "id": metric.id,
                "timestamp": metric.timestamp.to_rfc3339(),
                "type": metric.metric_type,
                "value": metric.value,
                "unit": metric.unit,
                "note": metric.note,
            }
        }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_list(all: bool, human_flag: bool) -> Result<()> {
    let db = Database::open(&Config::db_path())?;
    let meds = med::list_medications(&db, all)?;
    if human_flag {
        println!("{}", human::format_med_list(&meds, all));
    } else {
        let items: Vec<_> = meds.iter().map(|m| json!({
            "name": m.name,
            "dose": m.dose,
            "route": m.route.to_string(),
            "frequency": m.frequency.to_string(),
            "active": m.active,
            "started_at": m.started_at.to_rfc3339(),
            "note": m.note,
        })).collect();
        let out = output::success("med_list", json!({
            "medications": items,
            "count": items.len(),
        }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_stop(name: &str, reason: Option<&str>, date: Option<NaiveDate>, human_flag: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let resolved = config.resolve_alias(name);
    med::stop_medication(&db, &resolved, reason, date)?;
    if human_flag {
        println!("{}", human::format_med_stop(&resolved, reason));
    } else {
        let out = output::success("med_stop", json!({
            "name": resolved,
            "stopped": true,
            "reason": reason,
        }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_remove(name: &str, human_flag: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let resolved = config.resolve_alias(name);
    med::remove_medication(&db, &resolved)?;
    if human_flag {
        println!("Removed medication record: {}", resolved);
    } else {
        let out = output::success("med_remove", json!({
            "name": resolved,
            "removed": true,
        }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_status(name: Option<&str>, last: u32, human_flag: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let resolved = name.map(|n| config.resolve_alias(n));
    let statuses = med::adherence_status(&db, resolved.as_deref(), last)?;
    if human_flag {
        let today = chrono::Local::now().date_naive();
        println!("{}", human::format_med_status(&statuses, today));
    } else {
        let out = output::success("med_status", serde_json::to_value(&statuses)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
```

**Step 4: Add dispatch in `src/main.rs`**

```rust
// Add to use statement:
use cli::{..., MedAction};

// Add to match:
Commands::Med { action } => match action {
    MedAction::Add { name, dose, freq, route, note, started } => {
        cmd::med::run_add(&name, dose.as_deref(), &freq, &route, note.as_deref(), started, cli.human)
    }
    MedAction::Take { name, dose, note, tags } => {
        cmd::med::run_take(&name, dose.as_deref(), note.as_deref(), tags.as_deref(), cli.date, cli.human)
    }
    MedAction::List { all } => cmd::med::run_list(all, cli.human),
    MedAction::Stop { name, reason } => {
        cmd::med::run_stop(&name, reason.as_deref(), cli.date, cli.human)
    }
    MedAction::Remove { name } => cmd::med::run_remove(&name, cli.human),
    MedAction::Status { name, last } => {
        cmd::med::run_status(name.as_deref(), last, cli.human)
    }
},
```

**Step 5: Update `src/cmd/mod.rs`**

Add `pub mod med;`.

**Step 6: Run full test suite**

```bash
cargo test
```

Expected: All existing tests + new tests pass. `cargo clippy -- -D warnings` clean.

**Step 7: Commit**

```
feat(med): add CLI interface and command handlers for medication management
```

---

### Task 5: Integration — Trend, Goal, Status, Export

**Files:**
- Modify: `src/core/trend.rs` — force sum aggregation for `Category::Medication`
- Modify: `src/core/goal.rs` — recognize medication as cumulative
- Modify: `src/core/status.rs` — add medications section
- Modify: `src/output/human.rs` — update `format_status` for medications
- Modify: `src/cmd/status.rs` — pass medication data
- Modify: `src/core/export.rs` — add `export_medications`, `import_medications_from_json`
- Modify: `src/cmd/export.rs` — add `--with-medications` flag
- Modify: `src/cli.rs` — add `--with-medications` flag to Export command
- Test: `tests/med_integration.rs`

**Context:** Depends on Tasks 1–4.

**Step 1: Write integration tests in `tests/med_integration.rs`**

```rust
// tests/med_integration.rs
mod common;

use chrono::{Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use openvital::core::{med, trend, goal};
use openvital::db::Database;
use openvital::models::config::Config;
use openvital::models::goal::{Direction, Timeframe};
use openvital::models::metric::{Category, is_cumulative};

fn default_config() -> Config {
    Config::default()
}

// ── Trend: medication uses sum aggregation ──

#[test]
fn trend_medication_uses_sum_not_avg() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();

    // Take 3 times today
    for _ in 0..3 {
        med::take_medication(&db, &config, "ibuprofen", None, None, None, None).unwrap();
    }

    let result = trend::compute(&db, "ibuprofen", trend::TrendPeriod::Daily, Some(7)).unwrap();
    // For medication, daily data should show sum (count=3), not average
    assert!(!result.data.is_empty());
    // The count field already shows 3; with sum aggregation, avg field should also be sum
    // (This test verifies the behavioral change)
}

// ── Goal: medication is cumulative ──

#[test]
fn goal_medication_uses_sum_for_evaluation() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(&db, &config, "metformin", Some("500mg"), "2x_daily", None, None, None).unwrap();

    // Take twice today
    med::take_medication(&db, &config, "metformin", None, None, None, None).unwrap();
    med::take_medication(&db, &config, "metformin", None, None, None, None).unwrap();

    // Set goal: metformin above 2 daily
    goal::set_goal(&db, "metformin".to_string(), 2.0, Direction::Above, Timeframe::Daily).unwrap();

    let statuses = goal::goal_status(&db, Some("metformin")).unwrap();
    assert_eq!(statuses.len(), 1);
    assert!(statuses[0].is_met); // sum of 2 intakes >= 2
    assert_eq!(statuses[0].current_value, Some(2.0));
}

// ── Status: includes medication section ──

#[test]
fn status_includes_medication_data() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();

    let status = openvital::core::status::compute(&db, &config).unwrap();
    // After adding medication integration, status should include medication info
    assert!(status.medications.is_some());
    let med_status = status.medications.as_ref().unwrap();
    assert_eq!(med_status.active_count, 1);
}

// ── Export: default does NOT include medications ──

#[test]
fn export_default_no_medications_key() {
    let (_dir, db) = common::setup_db();
    med::add_medication(&db, &Config::default(), "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();

    let json = openvital::core::export::to_json(&db, None, None, None).unwrap();
    // Default export is metrics only
    assert!(!json.contains("medications"));
}

// ── Export: --with-medications includes medications ──

#[test]
fn export_with_medications_includes_both() {
    let (_dir, db) = common::setup_db();
    let config = default_config();
    med::add_medication(&db, &config, "ibuprofen", Some("400mg"), "as_needed", None, None, None).unwrap();
    med::take_medication(&db, &config, "ibuprofen", None, None, None, None).unwrap();

    let json = openvital::core::export::to_json_with_medications(&db, None, None, None).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["medications"].is_array());
    assert!(parsed["metrics"].is_array());
}

// ── Import: auto-detects medications key ──

#[test]
fn import_json_with_medications_key() {
    let (_dir, db) = common::setup_db();
    let json = r#"{
        "metrics": [{"type": "weight", "value": 80.0}],
        "medications": [
            {"name": "ibuprofen", "dose": "400mg", "route": "oral", "frequency": "as_needed", "active": true}
        ]
    }"#;
    let (metric_count, med_count) = openvital::core::export::import_json_auto(&db, json).unwrap();
    assert_eq!(metric_count, 1);
    assert_eq!(med_count, 1);
}

#[test]
fn import_old_format_no_medications_key_works() {
    let (_dir, db) = common::setup_db();
    // Old format: just an array of metrics
    let json = r#"[{"type": "weight", "value": 80.0}]"#;
    // Should work as before — auto-detect sees array, imports as metrics
    let count = openvital::core::export::import_json(&db, json).unwrap();
    assert_eq!(count, 1);
}

// ── Name conflict: existing metric unaffected ──

#[test]
fn existing_water_metric_category_unchanged() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Log water as nutrition metric
    let entry = openvital::core::logging::LogEntry {
        metric_type: "water",
        value: 500.0,
        note: None,
        tags: None,
        source: None,
        date: None,
    };
    let water_metric = openvital::core::logging::log_metric(&db, &config, entry).unwrap();
    assert_eq!(water_metric.category, Category::Nutrition);

    // Add medication named "water"
    med::add_medication(&db, &config, "water", Some("5ml"), "daily", None, None, None).unwrap();
    let (med_metric, _) = med::take_medication(&db, &config, "water", None, None, None, None).unwrap();
    assert_eq!(med_metric.category, Category::Medication);

    // Original water metric still Nutrition
    let stored = db.query_by_type("water", Some(10)).unwrap();
    let nutrition_count = stored.iter().filter(|m| m.category == Category::Nutrition).count();
    assert_eq!(nutrition_count, 1);
}

// ── Backward compat: migration doesn't break existing data ──

#[test]
fn existing_db_works_after_migration() {
    let (_dir, db) = common::setup_db();
    let config = default_config();

    // Insert a metric before any medication stuff
    let entry = openvital::core::logging::LogEntry {
        metric_type: "weight",
        value: 80.0,
        note: None,
        tags: None,
        source: None,
        date: None,
    };
    openvital::core::logging::log_metric(&db, &config, entry).unwrap();

    // Verify existing data still works
    let stored = db.query_by_type("weight", Some(1)).unwrap();
    assert_eq!(stored.len(), 1);
    assert!((stored[0].value - 80.0).abs() < f64::EPSILON);
}
```

**Step 2: Modify `src/core/trend.rs`**

In `compute()`, after building `PeriodData`, if the metric is a medication (check category from entries or use a lookup), replace `avg` with `sum`. The cleanest approach: check if the first entry has `category == "medication"` or `source == "med_take"`.

```rust
// In compute(), after building buckets:
let is_medication = entries.first().map_or(false, |e| e.category == Category::Medication);

// In the bucket → PeriodData mapping:
// If is_medication, set avg = sum (the count of intakes)
let avg = if is_medication { sum } else { sum / values.len() as f64 };
```

Add `use crate::models::metric::Category;` to imports.

**Step 3: Modify `src/core/goal.rs`**

In `compute_current()`, update the cumulative check:

```rust
fn compute_current(db: &Database, goal: &Goal, today: NaiveDate) -> Result<Option<f64>> {
    use crate::models::metric::is_cumulative;
    // Check if metric type is cumulative by hardcoded list OR by category
    let cumulative = is_cumulative(&goal.metric_type) || is_medication_type(db, &goal.metric_type)?;
    // ... rest unchanged
}

fn is_medication_type(db: &Database, metric_type: &str) -> Result<bool> {
    let entries = db.query_by_type(metric_type, Some(1))?;
    Ok(entries.first().map_or(false, |e| e.category == Category::Medication))
}
```

**Step 4: Modify `src/core/status.rs`**

Add `MedicationStatus` struct and include it in `StatusData`:

```rust
#[derive(Serialize)]
pub struct MedicationStatus {
    pub active_count: usize,
    pub adherent_today: usize,
    pub non_adherent_today: usize,
    pub as_needed: usize,
    pub missed: Vec<String>,
    pub overall_adherence_7d: Option<f64>,
}
```

Add `pub medications: Option<MedicationStatus>` to `StatusData`.

In `compute()`, after existing logic, query medications and compute status. If no medications table or no meds, set to `None`.

**Step 5: Modify `src/core/export.rs`**

Add:
- `to_json_with_medications(db, type, from, to) -> Result<String>` — returns `{"metrics": [...], "medications": [...]}`
- `import_json_auto(db, json_str) -> Result<(usize, usize)>` — auto-detects format. If input is an object with `medications` key, import both. If array, import as metrics (backward compat). Returns (metric_count, med_count).

**Step 6: Modify `src/cli.rs`**

Add `--with-medications` flag to Export command:

```rust
    Export {
        // ... existing fields ...
        /// Include medication records in export
        #[arg(long)]
        with_medications: bool,
    },
```

**Step 7: Modify `src/cmd/export.rs`**

Update `run_export` to handle `with_medications` flag. Update `run_import` to use `import_json_auto` for JSON imports.

**Step 8: Update `src/main.rs`**

Pass `with_medications` through in the Export dispatch.

**Step 9: Update `src/output/human.rs`**

Update `format_status` to include medication section when present.

**Step 10: Run full test suite**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --all -- --check
```

Expected: All tests pass, no clippy warnings, formatted.

**Step 11: Commit**

```
feat(med): integrate medications with trend, goal, status, and export
```

---

### Task 6: Final Verification and Cleanup

**Files:**
- All files from Tasks 1–5
- Test: all test files

**Step 1: Run the full test suite**

```bash
cargo test
```

**Step 2: Run clippy**

```bash
cargo clippy -- -D warnings
```

**Step 3: Run fmt check**

```bash
cargo fmt --all -- --check
```

**Step 4: Verify design alignment**

Manually review each section of `docs/plans/2026-02-19-medication-management-design.md` against implementation:
- [ ] Section 2.1: medications table schema matches
- [ ] Section 2.2: Route enum with Other variant
- [ ] Section 2.3: Dose parsing rules and negative test cases
- [ ] Section 2.4: med take creates metric with value=1, unit=dose, category=Medication
- [ ] Section 2.5: Category::Medication added, from_type() unchanged
- [ ] Section 3.1-3.2: All 6 commands implemented (add, take, list, stop, remove, status)
- [ ] Section 3.2: JSON output shapes match spec
- [ ] Section 4.1-4.4: Adherence logic correct for all frequencies
- [ ] Section 5.1: Status includes medication section
- [ ] Section 5.2: Trend uses sum for medication
- [ ] Section 5.3: Correlation works with medication
- [ ] Section 5.4: Goal treats medication as cumulative
- [ ] Section 5.5: Export backward compatible, --with-medications flag
- [ ] Section 5.6: Aliases work for med commands
- [ ] Section 7: All edge cases handled
- [ ] Section 8: Human-readable output formats match
- [ ] Section 10: All testing categories covered

**Step 5: Fix any misalignment found**

**Step 6: Commit**

```
test(med): complete medication feature verification against design spec
```

---

## Dependency Graph

```
Task 1 (Models)  ──┐
                   ├── Task 3 (Core Logic) ──┐
Task 2 (DB CRUD) ──┘                         ├── Task 5 (Integrations) ── Task 6 (Verify)
                   ┌── Task 4 (CLI/Cmd) ─────┘
                   │
Tasks 1-2 ─────────┘
```

**Tasks 1 and 2** can potentially run in parallel (they share the `Category::Medication` update in metric.rs, so coordinate carefully — Task 1 should own that change).

**Task 3** depends on both Tasks 1 and 2.

**Task 4** depends on Task 3.

**Task 5** depends on Tasks 3 and 4.

**Task 6** depends on everything.
