# Review of PR: optimize-pain-alert-query

**Disposition: Request Changes**

## [BLOCKING] Correctness: Potential buffer overflow/truncation in `check_consecutive_pain`

**File:** `src/core/status.rs`

The function `check_consecutive_pain` uses a fixed-size buffer `[None; 30]` and queries a range of 30 days (`today - Duration::days(30)`). However, `config.alerts.pain_consecutive_days` is a `u8` (0-255). If a user configures `pain_consecutive_days` > 30 (e.g., 35), the logic will fail to detect consecutive pain because the buffer and query range are too small.

**Suggestion:** Use `max(30, alerts.pain_consecutive_days)` to dynamically determine the lookback window and buffer size, or document/enforce a maximum limit for `pain_consecutive_days`.

## [SUGGESTION] Test Coverage: Verify timezone fix with boundary conditions

**File:** `tests/status_enhanced.rs`

The new test `test_pain_consecutive_uses_local_date_bucketing` uses `12:00:00` (noon) for timestamps. Noon is typically safe from date shifts in common timezones. The fix is intended to address "entries logged near midnight". Please add a test case with a timestamp near midnight (e.g., `23:00` Local) to ensure it works correctly even when the UTC date differs from the Local date. Note that in CI (UTC), this test might pass vacuously, but it's important for verification.

## [SUGGESTION] Correctness: Similar issue likely exists in `compute_streaks`

**File:** `src/core/status.rs`

`compute_streaks` (in the same file) relies on `db.distinct_entry_dates`, which uses SQLite's `date()` function on UTC timestamps. This likely causes the same "N+1 off-by-one" bug for streaks when users log late at night in non-UTC timezones. While technically out of scope for this PR (which focuses on pain alerts), it is highly recommended to fix `compute_streaks` as well or file a follow-up issue, as they share the same root cause.

## [NIT] Architecture: Import formatting

**File:** `src/core/status.rs`

`src/core/status.rs` uses `crate::db::Database`. While acceptable within the crate, consider standardizing imports if feasible.

---

**Note:** A reproduction test case `tests/status_repro.rs` was temporarily created to verify the buffer overflow issue and subsequently removed.
