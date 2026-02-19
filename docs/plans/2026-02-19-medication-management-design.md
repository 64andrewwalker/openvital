# Medication Management Feature Design

## Date: 2026-02-19
## Status: Draft - Pending Review

---

## 1. Overview

Add medication management to OpenVital, enabling users to:
- Maintain an active medication list with structured metadata
- Record each dose taken via `med take`
- Track adherence (compliance) automatically
- Correlate medication usage with health metrics (pain, sleep, etc.)

### Design Principles

- **Reuse over rebuild**: Medication intake events are stored as metric entries, so existing trend, correlate, goal, and export infrastructure works with zero modification
- **Structured metadata**: A dedicated `medications` table stores drug information that doesn't fit the float-value time-series model
- **Route-aware**: Supports oral, topical, liquid, injection, and other administration routes with appropriate dose representations

---

## 2. Data Model

### 2.1 New Table: `medications`

```sql
CREATE TABLE medications (
    id          TEXT PRIMARY KEY,   -- UUID
    name        TEXT NOT NULL UNIQUE, -- drug identifier (e.g., "ibuprofen", "retinol_cream")
    dose        TEXT,               -- dosage text (e.g., "400mg", "5ml", "2 drops", "thin layer")
    dose_value  REAL,               -- parsed numeric portion (400.0, 5.0, 2.0, or NULL)
    dose_unit   TEXT,               -- parsed unit portion ("mg", "ml", "drops", "application")
    route       TEXT NOT NULL DEFAULT 'oral',  -- administration route
    frequency   TEXT NOT NULL,      -- daily | 2x_daily | 3x_daily | weekly | as_needed
    active      INTEGER NOT NULL DEFAULT 1,  -- 1=active, 0=stopped
    started_at  TEXT NOT NULL,      -- RFC3339
    stopped_at  TEXT,               -- RFC3339 (set when stopped)
    stop_reason TEXT,               -- why the medication was stopped
    note        TEXT,               -- general notes (e.g., "take with food", "apply to affected area")
    created_at  TEXT NOT NULL       -- RFC3339
);

CREATE INDEX idx_medications_active ON medications(active);
CREATE INDEX idx_medications_name ON medications(name);
```

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

Routes are stored as lowercase strings. The system does not reject unknown routes - users can enter any string, these are the documented defaults.

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

Parsing strategy: regex `^(\d+\.?\d*)\s*(.+)$`. If no numeric prefix, `dose_value = NULL`, `dose_unit = "application"`.

### 2.4 Metric Entry for `med take`

When `med take <name>` is called, a metric entry is inserted into the existing `metrics` table:

| Field | Value |
|-------|-------|
| `metric_type` | medication name (e.g., `"ibuprofen"`) |
| `value` | `dose_value` from medications table (or 1.0 if NULL) |
| `unit` | `dose_unit` from medications table (or "dose") |
| `category` | `Medication` (new enum variant) |
| `source` | `"med_take"` |
| `note` | Optional, from `--note` flag |
| `tags` | Optional, from `--tags` flag |

The `--dose` flag on `med take` can override the default dose for that specific intake (e.g., taking half a dose).

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

`Category::from_type()` will check the medications table: if a metric_type matches an active (or inactive) medication name, it returns `Category::Medication`. Otherwise falls through to existing logic.

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
    "started_at": "2026-02-19"
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
      "value": 400.0,
      "unit": "mg"
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
        "started_at": "2026-01-15",
        "note": null
      },
      {
        "name": "retinol",
        "dose": "thin layer",
        "route": "topical",
        "frequency": "daily",
        "active": true,
        "started_at": "2026-02-01",
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

`adherence_7d` = (number of adherent days in last 7) / 7. Same logic for 30d. Only applies to non-`as_needed` medications. Only counts days within the `started_at` to `stopped_at` (or today) window.

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

Since medication intakes are stored as metric entries, all existing trend commands work:

```bash
# How often am I taking ibuprofen?
openvital trend ibuprofen --period weekly --last 8

# Output: weekly count of ibuprofen intakes, trend direction
```

### 5.3 Correlation Analysis

```bash
# Does ibuprofen correlate with pain reduction?
openvital trend --correlate ibuprofen,pain --last 30

# Does sleep quality change with medication?
openvital trend --correlate metformin,sleep_quality --last 30
```

### 5.4 Goal System

```bash
# Ensure I take metformin at least 2x daily
openvital goal set metformin above 2 --timeframe daily
```

### 5.5 Export / Import

- `openvital export` includes a `medications` key with the full medication list
- `openvital import` accepts a `medications` array to restore medication metadata
- Metric entries for `med take` are already exported as regular metrics

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
| `src/models/med.rs` | Models | `Medication`, `Frequency`, `Route` structs + enums |
| `src/db/meds.rs` | DB | CRUD for medications table |
| `src/core/med.rs` | Core | Business logic: add, take, stop, adherence calculation |
| `src/cmd/med.rs` | Command | CLI handler (thin shell) |

### 6.2 Modified Files

| File | Change |
|------|--------|
| `src/cli.rs` | Add `Med(MedAction)` variant to `Commands`, `MedAction` enum |
| `src/main.rs` | Add dispatch for `Commands::Med` |
| `src/models/metric.rs` | Add `Medication` to `Category` enum, update `from_type()` |
| `src/db/migrate.rs` | Add `CREATE TABLE medications` migration |
| `src/output/human.rs` | Add human-readable formatting for med commands |
| `src/lib.rs` | Re-export `core::med` and `models::med` |
| `src/core/status.rs` | Include medication adherence in status output |

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
| `med add` duplicate name | Error: "Medication 'X' already exists. Use `med stop` + `med add` to update." |
| Medication name conflicts with existing metric type | Allowed. `Category::from_type()` checks medications table first. Existing metric data is unaffected. |
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
  Recorded at 2026-02-19 08:30 UTC
```

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
   - `trend <medication>` works
   - `trend --correlate <medication>,pain` works
   - `status` includes medication section
   - `goal set <medication>` works
   - `export` includes medications
