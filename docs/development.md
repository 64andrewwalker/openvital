# Development Guide

## Prerequisites
- Rust stable (edition 2024)
- Git

## Quick Start
```bash
cargo build
cargo test
cargo run -- init --skip
cargo run -- log weight 85.5
cargo run -- -H status
```

## Testing
```bash
cargo test                    # All tests
cargo test test_name          # Single test
cargo test --test integration # Integration tests only
```

Integration tests use temp databases via `tempfile`. No cleanup needed.

## Adding a New Command

1. Define behavior in `tests/<command>.rs`
2. Add `core/<command>.rs` with business logic
3. Add `cmd/<command>.rs` as thin shell
4. Add variant to `Commands` enum in `cli.rs`
5. Wire dispatch in `main.rs`
6. Update `cmd/mod.rs` and `core/mod.rs`

## Architecture Decision Records
See `docs/adr/` for key decisions and their rationale.
