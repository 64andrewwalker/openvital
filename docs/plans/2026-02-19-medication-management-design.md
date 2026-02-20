# Medication Management Feature Design

## Date: 2026-02-19
## Status: Revised after code review (R1)

---

## 1. Overview

Add medication management to OpenVital, enabling users to:
- Maintain an active medication list with structured metadata
- Record each dose taken via `med take`
- Track adherence (compliance) automatically
- Correlate medication usage with health metrics (pain, sleep, etc.)

### Design Principles

- **Reuse over rebuild**: Medication intake events are stored as metric entries (value=1, count semantics), enabling reuse of trend/correlate/goal with targeted adaptations (see Section 5)
- **Structured metadata**: A dedicated `medications` table stores drug information that doesn't fit the float-value time-series model
- **Route-aware**: Supports oral, topical, liquid, injection, and other administration routes with appropriate dose representations

---

## 2. Data Model

### 2.1 New Table: `medications`

```sql
CREATE TABLE medications (
    id          TEXT PRIMARY KEY,   -- UUID
    name        TEXT NOT NULL,      -- drug identifier (e.g., "ibuprofen", "retinol_cream")
    dose        TEXT,               -- dosage text (e.g., "400mg", "5ml", "2 drops", "thin layer")
    dose_value  REAL,               -- parsed numeric portion (400.0, 5.0, 2.0, or NULL)
    dose_unit   TEXT,               -- parsed unit portion ("mg", "ml", "drops", "application")
    route       TEXT NOT NULL DEFAULT 'oral',  -- administration route (see 2.2)
    frequency   TEXT NOT NULL,      -- daily | 2x_daily | 3x_daily | weekly | as_needed
    active      INTEGER NOT NULL DEFAULT 1,  -- 1=active, 0=stopped
    started_at  TEXT NOT NULL,      -- RFC3339 UTC
    stopped_at  TEXT,               -- RFC3339 UTC (set when stopped)
    stop_reason TEXT,               -- why the medication was stopped
    note        TEXT,               -- general notes (e.g., "take with food", "apply to affected area")
    created_at  TEXT NOT NULL       -- RFC3339 UTC
);

-- Only one active medication per name; stopped duplicates are allowed.
-- This enables stop → re-add workflow without conflict.
CREATE UNIQUE INDEX idx_medications_name_active
    ON medications(name) WHERE active = 1;

CREATE INDEX idx_medications_active ON medications(active);
```

**Uniqueness rule**: The partial unique index ensures only one *active* medication per name. A user can `med stop ibuprofen` then `med add ibuprofen --dose "200mg"` to restart with a different dose. Stopped records are preserved for historical adherence queries.

**Timestamp convention**: All timestamps are stored as RFC3339 UTC (consistent with existing metrics table). Display layer formats to local time or short date as needed.

### 2.2 Administration Routes

| Route | Description | Typical Dose Formats |
|-------|-------------|---------------------|
| `oral` | Tablets, capsules, syrups | "400mg", "5ml", "1 tablet" |
| `topical` | Creams, ointments, gels | "thin layer", "2cm", "1 application" |
| `ophthalmic` | Eye drops, ointments | "2 drops", "1 drop" |
| `injection` | Subcutaneous, intramuscular | "0.5ml", "10 units" |
| `inhaled` | Inhalers, nebulizers | "2 puffs", "1 dose" |
| `sublingual` | Under-tongue tablets | "0.5mg", "1 tablet" |
| `transdermal` | Patches | "1 patch" |
| `other` | Catch-all | Free text |

**Implementation**: `Route` is an enum with a `Other(String)` variant for extensibility:

```rust
pub enum Route {
    Oral,
    Topical,
    Ophthalmic,
    Injection,
    Inhaled,
    Sublingual,
    Transdermal,
    Other(String),  // accepts any user-provided string
}
```

`FromStr` parses known variants (case-insensitive); unrecognized strings become `Other(input)`. Stored as lowercase text in SQLite. This provides type safety for known routes while remaining open to user input.

### 2.3 Dose Parsing Rules

When a medication is added with `--dose`, the system parses it into `dose_value` + `dose_unit`:

| Input | dose_value | dose_unit |
|-------|-----------|-----------|
| `"400mg"` | 400.0 | "mg" |
| `"5ml"` | 5.0 | "ml" |
| `"2 drops"` | 2.0 | "drops" |
| `"0.5mg"` | 0.5 | "mg" |
| `"10 units"` | 10.0 | "units" |
| `"1 tablet"` | 1.0 | "tablet" |
| `"thin layer"` | NULL | "application" |
| `"1 patch"` | 1.0 | "patch" |
| (none provided) | 1.0 | "dose" |

**Parsing strategy**: regex `^(\d*\.?\d+)\s*(.+)$` — matches leading decimal (including `.5mg`), then unit text. Fallback chain:

| Input | Parsed as | Rationale |
|-------|-----------|-----------|
| `".5mg"` | 0.5, "mg" | Leading dot accepted |
| `"1/2 tablet"` | 0.5, "tablet" | Fraction `a/b` converted to float |
| `"½ tablet"` | 0.5, "tablet" | Unicode fraction mapped |
| `"thin layer"` | NULL, "application" | No numeric prefix → fallback |
| `""` (empty/omitted) | 1.0, "dose" | Default when `--dose` not provided |

The parser tries in order: (1) decimal float, (2) fraction `a/b`, (3) fallback to NULL. On parse failure, the raw text is preserved in `dose` column and `dose_value` is NULL — the system never rejects dose input, only falls back gracefully.

**Negative test cases** (to verify):
- `"mg400"` → NULL, "application" (unit-first rejected, no numeric prefix)
- `"0/0 tablet"` → error: division by zero in fraction → fallback to NULL, "application"
- `"-5mg"` → NULL, "application" (negative values rejected for dose)

### 2.4 Metric Entry for `med take`

When `med take <name>` is called, a metric entry is inserted into the existing `metrics` table:

| Field | Value |
|-------|-------|
| `metric_type` | medication name (e.g., `"ibuprofen"`) |
| `value` | **Always `1.0`** (count semantics — one intake event) |
| `unit` | `"dose"` |
| `category` | `Medication` (new enum variant) |
| `source` | `"med_take"` |
| `note` | Optional. Actual dose text is appended here (e.g., `"400mg"`, `"200mg (override)"`) |
| `tags` | Optional, from `--tags` flag |

**Why value=1 (count), not the dose amount?**

Storing dose amounts (e.g., 400.0 for 400mg ibuprofen) would produce incorrect semantics in existing infrastructure:
- `trend` computes avg/min/max — averaging dose amounts is meaningless for adherence
- `goal set metformin above 2 daily` would check if sum-of-dose >= 2, not intake-count >= 2
- `correlate` would correlate dose magnitude with pain, not dose frequency

With value=1, all existing features produce correct semantics:
- `trend ibuprofen` → weekly count of intakes, direction (increasing/decreasing usage)
- `goal set metformin above 2 daily` → correctly checks 2+ intakes per day
- `correlate ibuprofen,pain` → correlates days-with-medication vs pain level
- `is_cumulative()` will include medication types → daily sum = intake count

The actual dose text is preserved in the `note` field for display and audit purposes. The `--dose` flag on `med take` overrides the note text (e.g., `"200mg (override)"`) but value remains 1.0.

### 2.5 Category Enum Update

```rust
pub enum Category {
    Body,
    Exercise,
    Sleep,
    Nutrition,
    Pain,
    Habit,
    Medication,  // NEW
    Custom,
}
```

`Category::from_type()` remains a **pure function** — it does not query the database. The `Medication` variant is not added to the match arms in `from_type()`. Instead, `core::med::take_medication()` explicitly sets `category = Category::Medication` when constructing the Metric entry. For existing metric types that happen to share a name with a medication, `from_type()` returns the original category (e.g., if someone names a medication "water", existing "water" metrics keep `Category::Nutrition`). This preserves the layering rule: models never depend on DB.

---

## 3. CLI Interface

### 3.1 Command Structure

```
openvital med <action>

Actions:
  add <name>       Add a medication to the active list
  take <name>      Record a dose taken
  list             List medications (active by default)
  stop <name>      Mark a medication as stopped
  remove <name>    Delete a medication and its metadata
  status [name]    View adherence status
```

### 3.2 Detailed Command Specification

#### `med add`

```bash
openvital med add <name> [flags]

Flags:
  --dose <text>       Dosage (e.g., "400mg", "5ml", "2 drops", "thin layer")
  --freq <frequency>  Frequency: daily, 2x_daily, 3x_daily, weekly, as_needed (required)
  --route <route>     Administration route (default: oral)
  --note <text>       Notes (e.g., "take with food", "apply to left knee")
  --started <date>    Start date (default: today)
```

Examples:
```bash
# Oral tablet
openvital med add ibuprofen --dose "400mg" --freq as_needed --note "take with food"

# Daily oral medication
openvital med add metformin --dose "500mg" --freq 2x_daily

# Topical cream
openvital med add retinol --dose "thin layer" --freq daily --route topical --note "apply to face at night"

# Eye drops
openvital med add latanoprost --dose "1 drop" --freq daily --route ophthalmic --note "left eye only"

# Liquid medication
openvital med add amoxicillin --dose "5ml" --freq 3x_daily --route oral

# Injection
openvital med add insulin --dose "10 units" --freq 2x_daily --route injection

# Transdermal patch
openvital med add nicotine_patch --dose "1 patch" --freq daily --route transdermal
```

JSON output:
```json
{
  "status": "ok",
  "command": "med_add",
  "data": {
    "id": "uuid...",
    "name": "ibuprofen",
    "dose": "400mg",
    "route": "oral",
    "frequency": "as_needed",
    "active": true,
    "started_at": "2026-02-19T00:00:00Z"
  }
}
```

#### `med take`

```bash
openvital med take <name> [flags]

Flags:
  --dose <text>    Override dose for this intake
  --note <text>    Note for this intake
  --tags <tags>    Comma-separated tags
  --date <date>    Override date (default: now)
```

Examples:
```bash
# Standard dose (uses dose from med add)
openvital med take ibuprofen

# Override dose
openvital med take ibuprofen --dose "200mg"

# With note
openvital med take metformin --note "after breakfast"

# Applied topical
openvital med take retinol --note "face and neck"

# Backfill yesterday
openvital med take metformin --date 2026-02-18
```

JSON output:
```json
{
  "status": "ok",
  "command": "med_take",
  "data": {
    "medication": "ibuprofen",
    "dose": "400mg",
    "route": "oral",
    "entry": {
      "id": "uuid...",
      "timestamp": "2026-02-19T08:30:00Z",
      "type": "ibuprofen",
      "value": 1.0,
      "unit": "dose",
      "note": "400mg"
    }
  }
}
```

#### `med list`

```bash
openvital med list [flags]

Flags:
  --all    Include stopped medications
```

JSON output:
```json
{
  "status": "ok",
  "command": "med_list",
  "data": {
    "medications": [
      {
        "name": "metformin",
        "dose": "500mg",
        "route": "oral",
        "frequency": "2x_daily",
        "active": true,
        "started_at": "2026-01-15T00:00:00Z",
        "note": null
      },
      {
        "name": "retinol",
        "dose": "thin layer",
        "route": "topical",
        "frequency": "daily",
        "active": true,
        "started_at": "2026-02-01T00:00:00Z",
        "note": "apply to face at night"
      }
    ],
    "count": 2
  }
}
```

#### `med stop`

```bash
openvital med stop <name> [flags]

Flags:
  --reason <text>   Reason for stopping
  --date <date>     Stop date (default: today)
```

Sets `active = 0`, records `stopped_at` and `stop_reason`. Does not delete data.

#### `med remove`

```bash
openvital med remove <name>
```

Permanently deletes the medication record from the `medications` table. Metric entries in the `metrics` table are **not** deleted (they remain as historical data). Requires confirmation in interactive mode.

#### `med status`

```bash
openvital med status [name] [flags]

Flags:
  --last <n>    Show adherence for last N days (default: 7)
```

**All medications:**
```json
{
  "status": "ok",
  "command": "med_status",
  "data": {
    "date": "2026-02-19",
    "medications": [
      {
        "name": "metformin",
        "dose": "500mg",
        "route": "oral",
        "frequency": "2x_daily",
        "required_today": 2,
        "taken_today": 1,
        "adherent_today": false,
        "streak_days": 12,
        "adherence_7d": 0.86
      },
      {
        "name": "retinol",
        "dose": "thin layer",
        "route": "topical",
        "frequency": "daily",
        "required_today": 1,
        "taken_today": 1,
        "adherent_today": true,
        "streak_days": 18,
        "adherence_7d": 1.0
      },
      {
        "name": "ibuprofen",
        "dose": "400mg",
        "route": "oral",
        "frequency": "as_needed",
        "taken_today": 0,
        "adherent_today": null,
        "streak_days": null,
        "adherence_7d": null
      }
    ],
    "overall_adherence_7d": 0.93
  }
}
```

**Single medication with history:**
```json
{
  "status": "ok",
  "command": "med_status",
  "data": {
    "name": "metformin",
    "dose": "500mg",
    "route": "oral",
    "frequency": "2x_daily",
    "adherence_history": [
      {"date": "2026-02-19", "required": 2, "taken": 1, "adherent": false},
      {"date": "2026-02-18", "required": 2, "taken": 2, "adherent": true},
      {"date": "2026-02-17", "required": 2, "taken": 2, "adherent": true}
    ],
    "streak_days": 12,
    "adherence_7d": 0.86,
    "adherence_30d": 0.92
  }
}
```

---

## 4. Adherence Logic

### 4.1 Daily Adherence Calculation

| Frequency | Required per Day | Adherent if |
|-----------|-----------------|-------------|
| `daily` | 1 | taken_today >= 1 |
| `2x_daily` | 2 | taken_today >= 2 |
| `3x_daily` | 3 | taken_today >= 3 |
| `weekly` | (1 per week) | taken_this_week >= 1 |
| `as_needed` | N/A | Always `null` (not tracked) |

### 4.2 Streak Calculation

A streak counts consecutive days where the medication was adherent. For `weekly` frequency, streaks count consecutive weeks. Streaks reset to 0 on the first missed day/week.

### 4.3 Period Adherence

`adherence_7d` = (number of adherent days in period) / (number of **eligible** days in period).

**Eligible days** = days within both the requested period AND the medication's active window (`started_at` to `stopped_at` or today). This avoids penalizing medications started mid-period.

**Example**: metformin started on Feb 16. On Feb 19, `adherence_7d` denominator is 4 (Feb 16-19), not 7.

Same logic for 30d. Only applies to non-`as_needed` medications.

### 4.4 Overall Adherence

`overall_adherence_7d` = average of all individual medication adherence_7d values (excluding `as_needed`).

---

## 5. Integration with Existing Systems

### 5.1 `openvital status`

Add a `medications` section to the status output:

```json
{
  "medications": {
    "active_count": 3,
    "adherent_today": 1,
    "non_adherent_today": 1,
    "as_needed": 1,
    "missed": ["metformin (1/2 taken)"],
    "overall_adherence_7d": 0.93
  }
}
```

### 5.2 Trend Analysis

Since medication intakes use value=1 (count semantics), trend analysis requires one adaptation:

**Required change to `src/models/metric.rs`**: Add medication types to `is_cumulative()`. Since medication names are dynamic (not hardcoded), `is_cumulative()` must accept a lookup mechanism or the caller must set the cumulative flag. Recommended approach: `core::trend::compute()` checks if `category == Medication` and forces sum aggregation (not avg/min/max).

```bash
# How often am I taking ibuprofen?
openvital trend ibuprofen --period weekly --last 8

# Output: weekly sum (count) of intakes, trend direction
```

### 5.3 Correlation Analysis

Correlation works correctly with value=1: daily sum = intake count, which correlates meaningfully with pain levels.

```bash
# Does ibuprofen usage correlate with pain levels?
openvital trend --correlate ibuprofen,pain --last 30

# Does sleep quality change with medication?
openvital trend --correlate metformin,sleep_quality --last 30
```

### 5.4 Goal System

**Required change to `src/core/goal.rs`**: The `is_cumulative` check at goal evaluation must recognize `Category::Medication` types as cumulative. Since goal evaluation already queries metrics from DB (which includes the category field), this is a check on `metric.category == "medication"` → use sum, not latest.

```bash
# Ensure I take metformin at least 2x daily
openvital goal set metformin above 2 --timeframe daily
# Works because: sum of value=1 entries = intake count
```

### 5.5 Export / Import

**Backward compatibility is critical.** The default export format must not change.

- Default `openvital export` continues to export only metrics (no change to existing format)
- New flag `openvital export --with-medications` includes a `medications` key alongside existing data
- `openvital import` auto-detects: if the JSON contains a `medications` key, import those records into the medications table; otherwise behave as before (metrics only)
- Metric entries for `med take` are exported as regular metrics in both modes (they're just metrics with `category: "medication"`)
- Old exports can be imported into new versions without issue (no medications key → skip)
- New exports with `--with-medications` fail gracefully on old versions (unknown key ignored by old import)

### 5.6 Aliases

Users can add medication aliases in config:

```toml
[aliases]
met = "metformin"
ibu = "ibuprofen"
```

Works for both `med take met` and `openvital trend met`.

---

## 6. Architecture

### 6.1 New Files

| File | Layer | Purpose |
|------|-------|---------|
| `src/models/med.rs` | Models | `Medication`, `Frequency`, `Route` (with `Other(String)`) structs + enums |
| `src/db/meds.rs` | DB | CRUD for medications table |
| `src/core/med.rs` | Core | Business logic: add, take, stop, adherence calculation |
| `src/cmd/med.rs` | Command | CLI handler (thin shell) |

### 6.2 Modified Files

| File | Change |
|------|--------|
| `src/cli.rs` | Add `Med(MedAction)` variant to `Commands`, `MedAction` enum |
| `src/main.rs` | Add dispatch for `Commands::Med` |
| `src/models/metric.rs` | Add `Medication` to `Category` enum (`from_type()` unchanged — stays pure) |
| `src/db/migrate.rs` | Add `CREATE TABLE medications` migration |
| `src/output/human.rs` | Add human-readable formatting for med commands |
| `src/lib.rs` | Re-export `core::med` and `models::med` |
| `src/core/status.rs` | Include medication adherence in status output |
| `src/core/trend.rs` | Force sum aggregation for `Category::Medication` types |
| `src/core/goal.rs` | Recognize medication types as cumulative for goal evaluation |
| `src/core/export.rs` | Support `--with-medications` flag for export; auto-detect on import |

### 6.3 Data Flow

```
med add ibuprofen --dose 400mg --freq as_needed --route oral
  → cli.rs: parse MedAction::Add
  → cmd/med.rs: open db, call core::med::add_medication()
  → core/med.rs: validate, parse dose, build Medication struct
  → db/meds.rs: insert into medications table
  → output: JSON envelope with medication record

med take ibuprofen
  → cli.rs: parse MedAction::Take
  → cmd/med.rs: open db, call core::med::take_medication()
  → core/med.rs: lookup medication, build Metric entry with parsed dose
  → db/metrics.rs: insert into metrics table (reuses existing insert)
  → output: JSON envelope with entry + medication context

med status
  → cli.rs: parse MedAction::Status
  → cmd/med.rs: open db, call core::med::adherence_status()
  → core/med.rs: query active meds, query today's metrics, compute adherence
  → db/meds.rs: list active medications
  → db/metrics.rs: query by type + date range (reuses existing query)
  → output: JSON envelope with adherence data
```

---

## 7. Edge Cases

| Case | Handling |
|------|----------|
| `med take` for unknown medication | Error: "Medication 'X' not found. Use `med add` first." |
| `med take` for stopped medication | Warning + allow: "Medication 'X' is stopped. Recording anyway." |
| `med add` duplicate active name | Error: "Medication 'X' is already active. Use `med stop` first, then `med add` to restart with new settings." |
| `med add` after `med stop` (same name) | Allowed. Partial unique index only constrains active=1. Creates a new record; stopped record preserved for history. |
| Medication name conflicts with existing metric type | Allowed. `Category::from_type()` remains unchanged (pure function). Only `med take` entries get `Category::Medication` — set explicitly by `core::med`. Existing metrics with the same type name keep their original category. |
| Dose with no numeric part ("thin layer") | `dose_value = NULL`, recorded as `value = 1.0, unit = "application"` in metrics |
| `med remove` with existing metric entries | Medication metadata deleted, metric entries preserved as historical data |
| `med status` when medication started mid-period | Adherence only calculated from `started_at`, not before |

---

## 8. Human-Readable Output Examples

### `med list --human`

```
Active Medications
==================
  metformin     500mg oral     2x daily   since Jan 15
  retinol       thin layer topical  daily      since Feb 01  "apply to face at night"
  ibuprofen     400mg oral     as needed  since Feb 10  "take with food"
```

### `med status --human`

```
Medication Adherence — Feb 19, 2026
====================================
  metformin     1/2 taken today    MISSED     streak: 12 days   7d: 86%
  retinol       1/1 taken today    OK         streak: 18 days   7d: 100%
  ibuprofen     0 taken today      (as needed)

Overall 7-day adherence: 93%
```

### `med take ibuprofen --human`

```
Took ibuprofen 400mg (oral)
  Recorded at Feb 19, 2026 08:30
```

Note: Display shows dose from medication metadata (or `--dose` override), not the stored value (which is always 1.0). Timestamp displayed in local timezone, stored as UTC.

---

## 9. Future Considerations (Not in Initial Implementation)

- **Reminders**: Time-based reminders for scheduled medications (requires daemon/notification system)
- **Interaction warnings**: Flag potential drug interactions (requires external database)
- **Refill tracking**: Track remaining supply and alert when running low
- **Dose schedule times**: Associate specific times with each dose (e.g., 08:00 and 20:00 for 2x_daily)
- **Prescriber/pharmacy info**: Extended metadata fields

---

## 10. Testing Strategy

Following BDD + TDD per project conventions:

1. **Integration tests** (`tests/cli_integration.rs` or `tests/med_integration.rs`):
   - `med add` → verify medications table and JSON output
   - `med take` → verify metric entry created with correct category/value/unit
   - `med take` with dose override → verify override value used
   - `med list` → verify active/all filtering
   - `med stop` → verify active flag and stopped_at
   - `med status` → verify adherence calculation for each frequency type
   - Topical/liquid/injection route handling
   - Error cases: unknown med, duplicate add, stopped med take

2. **Unit tests** (in source files):
   - Dose parsing: "400mg" → (400.0, "mg"), "thin layer" → (NULL, "application")
   - Adherence calculation logic
   - Frequency/Route enum FromStr parsing

3. **Cross-feature tests**:
   - `trend <medication>` uses sum aggregation (count), not avg
   - `trend --correlate <medication>,pain` produces valid correlation
   - `goal set <medication> above 2 daily` checks intake count, not dose value
   - `status` includes medication section
   - `export --with-medications` includes medications key
   - Default `export` (without flag) does NOT include medications key

4. **Backward compatibility tests**:
   - Old export format (pre-medication) imports successfully on new version
   - New export with `--with-medications` round-trips correctly
   - Existing metrics/goals data is unchanged after DB migration adds medications table

5. **Migration tests**:
   - Fresh DB creates medications table with correct schema
   - Existing DB (with metrics + goals data) upgrades without data loss
   - Partial unique index enforced: two active meds with same name rejected, active + stopped allowed

6. **Name conflict tests**:
   - Medication named "water" does not change category of existing water metrics
   - `trend water` (existing nutrition metric) unaffected by medication named "water"
   - `med take water` creates entry with `Category::Medication`, not `Category::Nutrition`
   - `from_type("water")` still returns `Category::Nutrition` (pure function unchanged)

---

## Appendix: Review Changelog

### R1 (2026-02-19) — 8 issues addressed

| # | Severity | Issue | Resolution |
|---|----------|-------|------------|
| 1 | High | `name UNIQUE` blocks stop→re-add workflow | Changed to partial unique index `WHERE active = 1` |
| 2 | High | Trend avg/goal hardcoded cumulative incompatible with dose values | Changed to value=1 count semantics; dose in note; trend/goal need targeted changes for `Category::Medication` |
| 3 | High | Export format break with added medications key | Default export unchanged; new `--with-medications` flag; import auto-detects |
| 4 | High | `Category::from_type()` querying DB breaks layering | `from_type()` stays pure; `core::med::take_medication()` sets category explicitly |
| 5 | Medium | Adherence denominator 7 vs active window contradiction | Clarified: denominator = eligible days within active window, with example |
| 6 | Medium | Route free string vs enum inconsistency | Unified as enum with `Other(String)` variant |
| 7 | Medium | Dose parser too narrow for `.5mg`, `1/2 tablet` | Extended regex, added fraction support, documented fallback chain and negative test cases |
| 8 | Medium | Timestamps inconsistent (RFC3339 vs date-only) | Unified: store RFC3339 UTC everywhere, display layer formats |

**Testing gaps added**: backward compatibility (Section 10.4), migration (10.5), name conflict (10.6)
