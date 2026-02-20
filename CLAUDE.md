# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
cargo build                        # Dev build
cargo build --release              # Release build
cargo test                         # Run all tests
cargo test test_name               # Run single test
cargo fmt --all                    # Auto-format
cargo fmt --all -- --check         # Check formatting (CI)
cargo clippy -- -D warnings        # Lint (warnings = errors in CI)
```

CI enforces: `check`, `fmt --check`, `clippy -D warnings`, and `test` on Linux/macOS/Windows.

Pre-commit hook runs fmt + clippy + test automatically. Setup: `git config core.hooksPath .githooks`

## Architecture

4-layer design: **CLI → Command → Core → DB**

```
src/
├── cli.rs          # clap definitions (Cli, Commands, GoalAction, ConfigAction)
├── main.rs         # Parse CLI → dispatch to cmd/ → handle errors
├── lib.rs          # Public API: re-exports core, db, models, output
├── cmd/            # Thin shells: open db + call core + format output
│   ├── config.rs   # config show/set
│   ├── export.rs   # export (csv/json) and import (csv/json)
│   ├── goal.rs     # goal set/status/remove
│   ├── init.rs     # init profile
│   ├── log.rs      # log single + batch
│   ├── med.rs      # medication add/take/list/stop/remove/status
│   ├── report.rs   # period reports (week/month/custom)
│   ├── show.rs     # show entries
│   ├── status.rs   # daily status overview
│   └── trend.rs    # trend analysis + correlation
├── core/           # Pure business logic, no CLI/IO dependency
│   ├── export.rs   # to_csv, to_json, import_json, import_csv
│   ├── goal.rs     # set_goal, remove_goal, goal_status
│   ├── logging.rs  # log_metric(LogEntry), log_batch()
│   ├── med.rs      # add_medication, take_medication, adherence_status
│   ├── query.rs    # show() → ShowResult enum
│   ├── report.rs   # generate() → ReportResult
│   ├── status.rs   # compute(), compute_streaks(), check_consecutive_pain()
│   └── trend.rs    # compute() → TrendResult, correlate() → CorrelationResult
├── db/
│   ├── mod.rs      # Database struct (rusqlite Connection wrapper)
│   ├── migrate.rs  # Schema creation + indexes (metrics + goals + meds tables)
│   ├── metrics.rs  # insert, query_by_type/date/range/all, distinct_entry_dates
│   ├── goals.rs    # insert/list/get/remove goals
│   └── meds.rs     # insert/list/get/remove/stop medications
├── models/
│   ├── metric.rs   # Metric, Category, default_unit()
│   ├── goal.rs     # Goal, Direction, Timeframe with FromStr traits
│   ├── med.rs      # Medication, Route, Frequency
│   └── config.rs   # Config, Profile, Units, Alerts + load/save/aliases
└── output/
    ├── mod.rs      # JSON envelope: success(), error()
    └── human.rs    # --human mode formatting
```

**Key rule**: `cmd/` never contains business logic — it delegates to `core/`. This allows future entry points (MCP server, plugin system) to reuse `core/` directly.

**Crate structure**: `lib.rs` exposes public modules for integration tests. Binary crate (`main.rs`) uses `openvital::` path for all imports from lib. `cmd/` files use `openvital::` prefix, not `crate::`.

## Output Contract

All commands default to JSON with a standard envelope:

```json
{"status": "ok|error", "command": "...", "data": {...}, "error": null}
```

`--human` flag switches to human-readable text. Every command must support both modes.

## Data Model

- **Storage**: SQLite at `~/.openvital/data.db`, config at `~/.openvital/config.toml`
- **Metric creation**: `Metric::new()` auto-generates UUID, sets UTC timestamp, infers category and unit from type string
- **Alias resolution**: Config aliases (e.g., `w`→`weight`) are resolved in `core/` before any DB operation
- **Tags**: comma-separated on input, stored as JSON array in SQLite
- **Timestamps**: stored as RFC3339 (UTC), queried by date range for day-level queries
- **Goals**: stored in goals table with direction (above/below/equal) and timeframe (daily/weekly/monthly)

## CLI Commands

| Command | Description |
|---------|-------------|
| `init` | Profile setup |
| `log <type> <value>` | Log metric entry (single or `--batch`) |
| `show [type]` | Show metric history |
| `trend <type>` | Trend analysis with period bucketing |
| `trend --correlate a,b` | Pearson correlation between two metrics |
| `goal set/status/remove` | Goal management |
| `med add/take/list/status` | Medication management |
| `status` | Daily overview with streaks, pain alerts |
| `report` | Period reports (week/month/custom range) |
| `export` | Export to CSV/JSON |
| `import` | Import from CSV/JSON |
| `config show/set` | Configuration management |
| `completions <shell>` | Shell completions (bash/zsh/fish) |

Global flags: `--human/-H`, `--quiet/-q`, `--date`, `--config`

## Development Workflow: BDD + TDD (MANDATORY)

**No production code without a failing test first.** This is non-negotiable for all code changes — features, bug fixes, refactors. Code submitted without corresponding tests will be rejected.

When developing features or fixing bugs, follow **BDD (Behavior-Driven Development)** combined with **TDD (Test-Driven Development)**:

1. **Define behavior first** — Write acceptance-level tests (integration tests in `tests/`) that describe the expected behavior from the user/agent perspective. Use the CLI binary or `core/` public API as the test surface.
2. **Red** — Run the tests, confirm they fail for the expected reason (missing feature, not a typo).
3. **Green** — Implement the minimum code in `core/` and `db/` to make the tests pass. Nothing more.
4. **Refactor** — Clean up while keeping tests green. Do not add behavior during refactor.

**Rules:**
- Every new function/method must have a test
- Bug fixes must include a regression test that reproduces the bug
- Watch each test fail before implementing — if a test passes immediately, it tests nothing useful
- Write minimal code to pass — do not anticipate future requirements
- Shell scripts (`.sh`) must have test scripts (`test-*.sh`) exercising key behaviors

Test organization:
- **Integration tests** — `tests/` directory, exercising the full pipeline (db → core → output) with a temp database
- **Unit tests** — `#[cfg(test)] mod tests` inside source files for pure logic
- **Shell tests** — `test-*.sh` scripts alongside shell scripts, mocking external dependencies

## Conventions

- `anyhow::Result<T>` for all fallible functions
- Clippy with `-D warnings` — no exceptions, no `#[allow]` unless structurally necessary
- Params structs (e.g., `LogEntry`) instead of functions with >7 arguments
- Use let-chain syntax for collapsible `if let` + condition (Rust 2024 edition)
- `FromStr` trait for enums parsed from CLI strings (Direction, Timeframe, TrendPeriod)
- New commands: add variant to `Commands` enum in `cli.rs`, handler in `cmd/`, logic in `core/`
- Release via [Conventional Commits](https://www.conventionalcommits.org/) → release-please automates versioning
- **Never push directly to master** — always create a feature branch and open a PR. CI must pass before merging.

## Commit Format

`type(scope): description` — e.g., `feat(trend): add moving average support`

## Spec Reference

`docs/openvital-spec.md` contains the full product specification. All Phase 1-3 features are implemented.
