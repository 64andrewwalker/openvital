# OpenVital v0.2: Bug Fixes & UX Improvements

**Date:** 2026-02-18
**Status:** Approved
**Scope:** 4 bug fixes + 5 UX improvements

## Bug Fixes

### BUG-1: Goal aggregation wrong for snapshot metrics (P1)

**Problem:** `core/goal.rs` uses `sum` for all `Direction::Above` goals. Sleep 7.5h logged twice becomes 15, falsely meeting an 8h goal.

**Root cause:** Line 84 in `core/goal.rs` — `Direction::Above` always sums, regardless of metric type.

**Fix:**
- Add `is_cumulative(metric_type: &str) -> bool` in `models/metric.rs`
- Cumulative types: `water`, `steps`, `calories_in`, `calories_burned`, `standing_breaks`
- All other types are snapshot (use last value)
- In `core/goal.rs`, use `is_cumulative()` to decide sum vs last-value for daily/weekly aggregation

### BUG-2: Blood pressure `120/80` cannot be logged (P1)

**Problem:** VALUE is parsed as `f64`, compound values fail.

**Fix:**
- Change CLI VALUE argument from `f64` to `String` in `cli.rs`
- In `cmd/log.rs`, detect `N/N` pattern for `blood_pressure` type
- Auto-split into two metrics: `bp_systolic` (first number) + `bp_diastolic` (second number)
- Add `bp_systolic` and `bp_diastolic` to `default_unit()` → `mmHg`
- Single log call returns both entries in response

### BUG-3: `--batch` ignores `--human` flag (P2)

**Problem:** `run_batch()` in `cmd/log.rs` has no `human_flag` parameter, always outputs JSON.

**Fix:**
- Add `human_flag: bool` parameter to `run_batch()`
- When true, iterate entries and print via `human::format_metric()`
- When false, output JSON as before

### BUG-4: 30-day projection produces unreasonable values (P2)

**Problem:** Weight 72.5kg projects to 2.0kg. Linear extrapolation with no bounds.

**Fix:**
- Clamp projection to `[last_avg * 0.5, last_avg * 1.5]` range
- Ensure projection is never negative for physical metrics
- This prevents absurd extrapolation while still showing meaningful trends

## UX Improvements

### UX-1: `show <type>` default from 1 to 10

**Current:** `show weight` returns only the most recent entry (default `--last 1`).
**Change:** Default to `--last 10` in `core/query.rs`.

### UX-2: Default units for common metrics

**Current:** `sleep`, `steps`, `mood`, `heart_rate` show no unit.
**Change:** Add to `default_unit()`:

| Type | Unit |
|------|------|
| sleep | hours |
| steps | steps |
| mood | 1-10 |
| heart_rate | bpm |
| bp_systolic | mmHg |
| bp_diastolic | mmHg |

### UX-3: `goal set` positional arguments

**Current:** `goal set weight --target 70 --direction below --timeframe daily` (verbose).
**Change:** Support `goal set weight 70 below daily` (positional) alongside existing named syntax. Implementation: make `--target`, `--direction`, `--timeframe` optional in clap, add positional `[TARGET] [DIRECTION] [TIMEFRAME]` args, prefer positional when present.

### UX-4: `status` deduplicates "Logged today"

**Current:** `Logged today: water, water, weight, sleep, steps, heart_rate, mood, water, weight`
**Change:** `Logged today: weight(2), water(3), sleep(1), steps(1), heart_rate(1), mood(1)`

### UX-5: Batch simple format

**Current:** Only JSON array: `--batch '[{"type":"weight","value":72.5}]'`
**Change:** Also accept simple format: `--batch "weight:72.5,sleep:7.5,mood:8"`
- Detection: if input starts with `[`, parse as JSON; otherwise parse as `key:value` pairs
- Simple format does not support tags/notes/source (use JSON for those)

## Files to Modify

| File | Changes |
|------|---------|
| `src/models/metric.rs` | Add `is_cumulative()`, extend `default_unit()` |
| `src/cli.rs` | VALUE: f64→String, goal set positional args |
| `src/cmd/log.rs` | BP split logic, batch human_flag, simple batch format |
| `src/core/logging.rs` | Handle BP compound value |
| `src/core/goal.rs` | Use `is_cumulative()` for aggregation |
| `src/core/query.rs` | Default --last 10 |
| `src/core/trend.rs` | Clamp projection bounds |
| `src/core/status.rs` | Deduplicate "Logged today" display |
| `src/output/human.rs` | Format BP pair output |

## Testing Strategy (BDD + TDD)

For each change:
1. Write failing integration test in `tests/` describing expected behavior
2. Write failing unit test if pure logic is involved
3. Implement minimum code to pass
4. Refactor, verify all tests green
