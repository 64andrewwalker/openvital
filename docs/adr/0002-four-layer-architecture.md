# ADR-0002: Four-Layer Architecture (CLI → Command → Core → DB)

## Status
Accepted

## Context
Business logic must be reusable by future entry points (MCP server, plugin system) without depending on CLI parsing.

## Decision
Separate into 4 layers:
1. **cli.rs** — clap definitions only
2. **cmd/** — thin shells that parse args, call core, format output
3. **core/** — pure business logic, no CLI/IO dependency
4. **db/** — SQLite persistence layer

## Consequences
- `core/` functions can be called by any entry point (CLI, MCP, tests)
- Commands are easy to test at the core level without invoking the binary
- Adding a new command requires touching 3 files: `cli.rs`, `cmd/x.rs`, `core/x.rs`
