# ADR-0003: BDD + TDD Development Workflow

## Status
Accepted

## Context
Need a consistent development methodology that ensures correctness and regression prevention, especially important for a health data tool.

## Decision
All features and bug fixes follow BDD + TDD:
1. Write integration tests in `tests/` describing expected behavior (BDD)
2. Write unit tests in `#[cfg(test)]` modules for pure logic (TDD)
3. Red → Green → Refactor cycle
4. Tests must pass before commit

## Consequences
- Every feature has test coverage from day one
- Integration tests use temp databases (no interference with user data)
- CI runs tests on 3 platforms (linux, macOS, Windows)
- Slightly slower initial development, much faster long-term iteration
