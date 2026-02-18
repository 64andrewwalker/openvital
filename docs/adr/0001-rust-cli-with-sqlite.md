# ADR-0001: Rust CLI with Bundled SQLite

## Status
Accepted

## Context
OpenVital needs to be a single-binary, zero-dependency CLI tool for health metric tracking. Primary consumers are AI agents; secondary consumers are humans.

## Decision
Use Rust with `clap` (derive) for CLI, `rusqlite` (bundled) for SQLite, `serde_json` for JSON output. Ship as a single binary with no runtime dependencies.

## Consequences
- Fast startup (<50ms), critical for agent workflows
- Cross-platform builds via cargo + CI matrix (linux, macOS x86/arm, Windows)
- SQLite bundled — no system library dependency
- Trade-off: longer compile times, steeper learning curve for contributors
