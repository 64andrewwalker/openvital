# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
cargo build                        # Dev build
cargo build --release              # Release build
cargo test                         # Run all tests
cargo fmt --all                    # Auto-format
cargo fmt --all -- --check         # Check formatting (CI)
cargo clippy -- -D warnings        # Lint (warnings = errors in CI)
```

CI enforces: `check`, `fmt --check`, `clippy -D warnings`, and `test` on Linux/macOS/Windows.

## Architecture

4-layer design: **CLI → Command → Core → DB**

```
src/
├── cli.rs          # clap definitions (Cli, Commands, ConfigAction)
├── main.rs         # Parse CLI → dispatch to cmd/ → handle errors
├── cmd/            # Thin shells: open db + call core + format output
├── core/           # Pure business logic, no CLI/IO dependency
│   ├── logging.rs  # log_metric(LogEntry), log_batch()
│   ├── query.rs    # show() → ShowResult enum
│   └── status.rs   # compute() → StatusData struct
├── db/
│   ├── mod.rs      # Database struct (rusqlite Connection wrapper)
│   ├── migrate.rs  # Schema creation + indexes
│   └── metrics.rs  # insert_metric, query_by_type, query_by_date
├── models/
│   ├── metric.rs   # Metric, Category, default_unit()
│   └── config.rs   # Config, Profile, Units, Alerts + load/save/aliases
└── output/
    ├── mod.rs      # JSON envelope: success(), error()
    └── human.rs    # --human mode formatting
```

**Key rule**: `cmd/` never contains business logic — it delegates to `core/`. This allows future entry points (MCP server, plugin system) to reuse `core/` directly.

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

## Development Workflow: BDD + TDD

When developing features or fixing bugs, follow **BDD (Behavior-Driven Development)** combined with **TDD (Test-Driven Development)**:

1. **Define behavior first** — Write acceptance-level tests (integration tests in `tests/`) that describe the expected behavior from the user/agent perspective. Use the CLI binary or `core/` public API as the test surface.
2. **Red** — Run the tests, confirm they fail.
3. **Green** — Implement the minimum code in `core/` and `db/` to make the tests pass.
4. **Refactor** — Clean up while keeping tests green.

Test organization:
- **Unit tests** — `#[cfg(test)] mod tests` inside each source file, for pure logic in `core/` and `models/`
- **Integration tests** — `tests/` directory, exercising the full pipeline (db → core → output) with a temp database

Example cycle for a new command `openvital trend`:
1. Write `tests/trend.rs` asserting JSON output shape and edge cases
2. Add `core/trend.rs` with failing stubs
3. Implement until tests pass
4. Wire up `cmd/trend.rs` + `cli.rs`

## Conventions

- `anyhow::Result<T>` for all fallible functions
- Clippy with `-D warnings` — no exceptions, no `#[allow]` unless structurally necessary
- Params structs (e.g., `LogEntry`) instead of functions with >7 arguments
- New commands: add variant to `Commands` enum in `cli.rs`, handler in `cmd/`, logic in `core/`
- Release via [Conventional Commits](https://www.conventionalcommits.org/) → release-please automates versioning

## Spec Reference

`openvital-spec.md` contains the full product specification. Phase 1 (MVP) is implemented. Phase 2 (trend, goal, report, streak) and Phase 3 (export/import, correlation) are planned — directory structure already accommodates them.
