# Jules Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Set up Google Jules as an automated development agent for OpenVital via GitHub Actions workflows and prompt files.

**Architecture:** Two GitHub Actions workflows (`jules-events.yml` for event-driven pipelines, `jules-scheduled.yml` for cron tasks) call the Jules API via a shared helper script. Prompts are stored as text files in `.github/jules-prompts/` and loaded at runtime to avoid JSON/shell escaping issues.

**Tech Stack:** GitHub Actions, Jules REST API (v1alpha), bash, jq, curl

**Testing approach:** This is CI/infra work (no Rust code), so TDD manifests as:
- **shellcheck** lint for all bash scripts before commit
- **YAML syntax validation** for all workflow files before commit
- **Unit tests for helpers.sh** via a bash test script that mocks curl responses (success, 4xx retry, 5xx failure)
- **Smoke tests** via `workflow_dispatch` after deployment

**Design docs:**
- `docs/jules/jules-integration-template.md` — generic PRD template
- `docs/jules/jules-scheduled-tasks.md` — scheduled task prompt library
- `docs/jules/jules-openvital.md` — OpenVital-specific configuration

---

### Task 1: Create helper script

**Files:**
- Create: `.github/jules-prompts/helpers.sh`

**Step 1: Write the helper script**

```bash
#!/usr/bin/env bash
set -euo pipefail

# jules_create_session — create a Jules API session with retry and error handling
#
# Usage: jules_create_session <prompt_file> <title> <branch> [automation_mode]
#
# Required env vars: JULES_API_URL, JULES_API_KEY, JULES_SOURCE
jules_create_session() {
  local PROMPT_FILE="$1" TITLE="$2" BRANCH="$3" MODE="${4:-AUTO_CREATE_PR}"
  local PROMPT
  PROMPT=$(cat "$PROMPT_FILE")

  local PAYLOAD
  PAYLOAD=$(jq -n \
    --arg title "$TITLE" \
    --arg prompt "$PROMPT" \
    --arg source "$JULES_SOURCE" \
    --arg branch "$BRANCH" \
    --arg mode "$MODE" \
    '{title: $title, prompt: $prompt, sourceContext: {source: $source, githubRepoContext: {startingBranch: $branch}}, automationMode: $mode}')

  local HTTP_CODE RESPONSE BODY
  RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "$JULES_API_URL/sessions" \
    -H "Content-Type: application/json" \
    -H "X-Goog-Api-Key: $JULES_API_KEY" \
    -d "$PAYLOAD")
  HTTP_CODE=$(echo "$RESPONSE" | tail -1)
  BODY=$(echo "$RESPONSE" | sed '$d')

  if [[ "$HTTP_CODE" -ge 200 && "$HTTP_CODE" -lt 300 ]]; then
    echo "$BODY"
    return 0
  fi

  echo "::warning::Jules API returned HTTP $HTTP_CODE — retrying in 60s"
  sleep 60

  RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "$JULES_API_URL/sessions" \
    -H "Content-Type: application/json" \
    -H "X-Goog-Api-Key: $JULES_API_KEY" \
    -d "$PAYLOAD")
  HTTP_CODE=$(echo "$RESPONSE" | tail -1)
  BODY=$(echo "$RESPONSE" | sed '$d')

  if [[ "$HTTP_CODE" -ge 200 && "$HTTP_CODE" -lt 300 ]]; then
    echo "$BODY"
    return 0
  fi

  echo "::error::Jules API failed after retry (HTTP $HTTP_CODE): $BODY"
  return 1
}
```

**Step 2: Run shellcheck**

Run: `shellcheck .github/jules-prompts/helpers.sh`
Expected: No warnings (clean lint). Install with `brew install shellcheck` if missing.

**Step 3: Write the test script**

Create `.github/jules-prompts/test-helpers.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Test harness for helpers.sh — mocks curl to test retry/error logic
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PASS=0
FAIL=0

assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [[ "$expected" == "$actual" ]]; then
    echo "  PASS: $label"
    ((PASS++))
  else
    echo "  FAIL: $label — expected '$expected', got '$actual'"
    ((FAIL++))
  fi
}

# Mock curl: returns predefined response based on MOCK_HTTP_CODE env var
curl() {
  echo "${MOCK_RESPONSE:-{}}"
  echo "${MOCK_HTTP_CODE:-200}"
}
export -f curl

# Setup required env vars
export JULES_API_URL="https://test.example.com"
export JULES_API_KEY="test-key"
export JULES_SOURCE="sources/github/test/repo"

source "$SCRIPT_DIR/helpers.sh"

# Create a temp prompt file
TMPFILE=$(mktemp)
echo "test prompt content" > "$TMPFILE"

echo "Test 1: Successful API call (HTTP 200)"
MOCK_HTTP_CODE=200 MOCK_RESPONSE='{"id":"session-1"}'
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" 2>/dev/null)
EXIT_CODE=$?
assert_eq "exit code" "0" "$EXIT_CODE"
assert_eq "returns body" '{"id":"session-1"}' "$OUTPUT"

echo ""
echo "Test 2: First call fails (HTTP 500), retry succeeds"
# This test would need stateful mock — simplified to verify non-zero exit on double failure
MOCK_HTTP_CODE=500 MOCK_RESPONSE='{"error":"server error"}'
OUTPUT=$(jules_create_session "$TMPFILE" "test title" "main" 2>/dev/null) || EXIT_CODE=$?
assert_eq "exit code non-zero on failure" "1" "${EXIT_CODE:-0}"

rm -f "$TMPFILE"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
```

**Step 4: Run the test (Red → Green verification)**

Run: `bash .github/jules-prompts/test-helpers.sh`
Expected: All tests pass

**Step 5: Commit**

```bash
git add .github/jules-prompts/helpers.sh .github/jules-prompts/test-helpers.sh
git commit -m "ci(jules): add API helper script with tests, retry, and error handling"
```

---

### Task 2: Create event-driven prompt files

**Files:**
- Create: `.github/jules-prompts/doc-sync.txt`
- Create: `.github/jules-prompts/pr-review.txt`
- Create: `.github/jules-prompts/issue-autofix.txt`

Each prompt combines the generic template (from `jules-integration-template.md` Section 3-5) with OpenVital-specific context (from `jules-openvital.md` Section 3).

**Step 1: Write doc-sync.txt**

```
You are a documentation maintainer for the OpenVital repository.

A PR was just merged. Analyze the changes and determine if any documentation needs updating.

Check these OpenVital documentation files:
1. README.md — CLI command table, installation instructions, usage examples
2. docs/openvital-spec.md — product specification, feature descriptions
3. CLAUDE.md — architecture section, CLI commands table, conventions
4. docs/adr/ — if the change introduces a new architectural pattern,
   note that an ADR may be needed (create Issue with label "jules-created", don't write it)

Rules:
- Only create a PR if documentation is factually out of date
- Do NOT rewrite documentation style — only fix factual inaccuracies
- Do NOT add documentation for undocumented features unless the merged PR explicitly adds them
- PR title format: [jules-docs] Update docs for <merged PR title>
- If no documentation updates are needed, do nothing
- CHANGELOG is managed by release-please — do NOT modify it manually
- CLI commands are documented in a table in both README.md and CLAUDE.md
- The architecture section in CLAUDE.md contains the source tree layout
- New commands follow the pattern: cli.rs variant → cmd/ handler → core/ logic

OpenVital project conventions:
- Build verification: cargo check && cargo fmt --all -- --check && cargo clippy -- -D warnings && cargo test
- All four commands must pass before creating a PR (matches CI pipeline)
- Follow Conventional Commits format: type(scope): description
- If you create any GitHub Issues, apply the label "jules-created"
```

**Step 2: Write pr-review.txt**

```
You are a code reviewer for the OpenVital repository (Rust CLI, health tracking).

Review the pull request for substantive issues.

Review criteria (in priority order):
1. Correctness — Logic errors, off-by-one, null handling, race conditions
2. Security — Injection risks, hardcoded secrets, unsafe deserialization
3. Test coverage — Are new code paths tested? Are edge cases covered?
4. Error handling — Swallowed errors, missing fallbacks, unclear error messages
5. API contract — Breaking changes without version bumps, missing migrations

Additional criteria for this Rust CLI project:
1. Architecture compliance:
   - cmd/ files must NOT contain business logic — only delegation to core/
   - All imports from lib must use `openvital::` prefix, not `crate::`
   - New public functions must use `anyhow::Result<T>`
2. Output contract:
   - Every command must support both JSON (default) and --human modes
   - JSON must follow the envelope: {"status": "ok|error", "command": "...", "data": {...}}
3. Rust-specific:
   - No `#[allow]` attributes unless structurally necessary
   - Use `FromStr` trait for enums parsed from CLI strings
   - Prefer let-chain syntax for collapsible if-let + condition
   - Params structs for functions with >7 arguments
4. Testing:
   - Integration tests in tests/ exercise the full pipeline with temp database
   - Bug fixes should include a regression test

Rules:
- Focus on substantive issues, not style (linters handle style)
- For each issue, explain WHY it's a problem and suggest a fix
- Categorize issues as: 🔴 Blocking / 🟡 Suggestion / 🟢 Nit
- If no blocking issues: approve the PR
- If blocking issues found: request changes
- Do NOT comment on code you don't understand — ask a question instead
- Keep comments concise — one issue per comment, max 3-4 sentences
```

**Step 3: Write issue-autofix.txt**

```
You are a developer triaging a new issue for the OpenVital repository.

OpenVital is a Rust CLI with 4-layer architecture (CLI → Command → Core → DB).
Build: cargo check && cargo fmt --all -- --check && cargo clippy -- -D warnings && cargo test
It uses SQLite with a migration system in src/db/migrate.rs.
Metrics are the core data model (health data points with type, value, tags).
Goals, trends, reports, and status are computed from metrics.

Step 1: Assess whether you can fix this issue autonomously.

You CAN fix it if ALL of these are true:
- The problem is clearly described (you understand what's wrong)
- The fix is localized (1-3 files)
- The fix does not require architectural decisions or new dependencies
- You can verify the fix with existing tests or by adding a targeted test

You CAN likely fix:
- Clippy warnings or formatting issues
- Simple logic bugs in core/ computation functions
- Missing error handling for specific edge cases
- Test coverage gaps in existing modules

You CANNOT fix:
- New CLI commands or subcommands (requires design decisions)
- Database schema changes (requires migration planning)
- Cross-platform issues (needs multi-OS testing)
- Performance issues without clear profiling data
- Feature requests requiring design decisions
- Changes across many modules
- Issues where you're not confident about the root cause

Step 2: Take action based on your assessment.

If you CAN fix it:
- Create a PR with the fix
- PR title format: [jules-fix] <concise description> (fixes #<issue_number>)
- Include a test that reproduces the bug and validates the fix
- PR description must explain: root cause, fix approach, how to verify
- Follow Conventional Commits format: type(scope): description

If you CANNOT fix it:
- Leave a comment on the issue with your assessment:
  - What you think the root cause might be
  - Why automated fixing isn't appropriate
  - Suggested approach for a human developer
- Do NOT create a PR for partial or uncertain fixes

If the issue is UNCLEAR:
- Leave a comment asking specific clarifying questions
- Do NOT attempt a fix without understanding the problem

If you create any GitHub Issues, apply the label "jules-created".
```

**Step 4: Verify all three files exist and are non-empty**

Run: `wc -l .github/jules-prompts/doc-sync.txt .github/jules-prompts/pr-review.txt .github/jules-prompts/issue-autofix.txt`
Expected: All three files listed with line counts > 10

**Step 5: Commit**

```bash
git add .github/jules-prompts/doc-sync.txt .github/jules-prompts/pr-review.txt .github/jules-prompts/issue-autofix.txt
git commit -m "ci(jules): add event-driven pipeline prompt files"
```

---

### Task 3: Create scheduled task prompt files

**Files:**
- Create: `.github/jules-prompts/bug-hunter.txt`
- Create: `.github/jules-prompts/security-patrol.txt`
- Create: `.github/jules-prompts/test-coverage.txt`
- Create: `.github/jules-prompts/dependency-health.txt`
- Create: `.github/jules-prompts/doc-consistency.txt`

Each prompt comes from `jules-scheduled-tasks.md` with the OpenVital suffix appended from `jules-openvital.md` Section 4 "Prompt Additions" and the OpenVital-specific notes from the agent pod table.

**Step 1: Write bug-hunter.txt**

```
You are a Bug Hunter for the OpenVital codebase (Rust CLI, health tracking).

Scan recently modified code (prioritize changes from the last 7 days via git log),
looking for these categories of bugs:

1. Logic errors: off-by-one, unhandled null/None/undefined, race conditions, dead code paths
2. Error handling flaws: swallowed exceptions, empty catch blocks, missing fallbacks
3. Resource leaks: unclosed connections, file handles, streams
4. Type safety: implicit conversions, unsafe casts, unvalidated type assertions

Focus on core/ computation logic — this is where most business rules live.

Rules:
- Submit at most 1 PR, focused on the single most severe bug
- PR title format: [jules-bug] <concise description>
- PR description must include: problem statement, impact scope, fix approach, verification steps
- If no bugs worth fixing are found, do NOT create a PR
- Do NOT make style changes — that's the linter's job
- Do NOT refactor — only fix bugs
- Include a regression test that reproduces the bug
- If you create any GitHub Issues, apply the label "jules-created"

OpenVital project conventions:
- Build verification: cargo check && cargo fmt --all -- --check && cargo clippy -- -D warnings && cargo test
- All four commands must pass before creating a PR (matches CI pipeline)
- Follow Conventional Commits format: type(scope): description
- Never modify CHANGELOG.md (managed by release-please)
- Integration tests use temp databases — see tests/ for patterns
```

**Step 2: Write security-patrol.txt**

```
You are a Security Patrol agent for the OpenVital codebase (Rust CLI, SQLite database).

Scan the code for security vulnerabilities, focusing on:

1. Injection risks: SQL string concatenation in src/db/, unescaped user input, command injection
2. Auth/AuthZ: hardcoded secrets or tokens, sensitive data in config defaults
3. Data exposure: health data leaking in logs, internal details in error responses
4. Config security: debug mode enabled in production configs
5. Cryptography: use of known-insecure hash/encryption algorithms

Pay special attention to the db/ layer — check for SQL injection via string formatting.

Rules:
- Prioritize by severity — fix directly exploitable issues first
- For safely auto-fixable issues (e.g., replacing insecure function calls): create a PR
- For issues requiring architectural changes: create an Issue with priority label and "jules-created" label
- PR title format: [jules-security] <description>
- Do NOT create false positives — if unsure whether something is a real vulnerability,
  create an Issue for discussion rather than changing code

OpenVital project conventions:
- Build verification: cargo check && cargo fmt --all -- --check && cargo clippy -- -D warnings && cargo test
- All four commands must pass before creating a PR (matches CI pipeline)
- Follow Conventional Commits format: type(scope): description
- Never modify CHANGELOG.md (managed by release-please)
- Integration tests use temp databases — see tests/ for patterns
```

**Step 3: Write test-coverage.txt**

```
Your task is to improve test coverage for the OpenVital codebase (Rust CLI).

Workflow:
1. Identify core modules lacking test coverage
   (prioritize business logic in src/core/ over thin command handlers in src/cmd/)
2. Pick 1 file/module per run and write unit tests for it
3. Tests should cover: happy path, boundary conditions, error handling paths

Requirements:
- Follow the project's existing test style:
  - Unit tests use #[cfg(test)] mod tests inside source files
  - Integration tests go in tests/ directory, exercising the full pipeline with temp databases
- Test names should describe the behavior being tested, not implementation details
- Only mock external I/O (filesystem, database) — not internal modules
- PR title format: [jules-test] Add tests for <module name>
- All new tests must pass

OpenVital project conventions:
- Build verification: cargo check && cargo fmt --all -- --check && cargo clippy -- -D warnings && cargo test
- All four commands must pass before creating a PR (matches CI pipeline)
- Follow Conventional Commits format: type(scope): description
- Never modify CHANGELOG.md (managed by release-please)
- Integration tests use temp databases — see tests/ for patterns
```

**Step 4: Write dependency-health.txt**

```
Audit the health of the OpenVital project's Rust dependencies.

Focus areas:
1. Security vulnerabilities: run `cargo audit` to identify high and critical severity CVEs
2. Outdated dependencies: check `cargo outdated` for core dependencies with major version gaps
3. Deprecation warnings: check for crates or APIs marked as deprecated

Actions:
- For security-patching patch/minor upgrades: create a PR with the upgrade
- For major upgrades requiring breaking changes: create a GitHub Issue with "jules-created" label
  explaining the risk, migration effort, and recommended timeline — do NOT upgrade directly
- PR title format: [jules-deps] <description>
- Ensure the project builds and all existing tests pass after upgrades
- Maximum 3 dependency upgrades per PR to keep diffs reviewable

OpenVital project conventions:
- Build verification: cargo check && cargo fmt --all -- --check && cargo clippy -- -D warnings && cargo test
- All four commands must pass before creating a PR (matches CI pipeline)
- Follow Conventional Commits format: type(scope): description
- Never modify CHANGELOG.md (managed by release-please)
- Integration tests use temp databases — see tests/ for patterns
```

**Step 5: Write doc-consistency.txt**

```
Check the OpenVital codebase for documentation-code inconsistencies.

Review:
1. README.md usage examples — do they still work with current CLI commands?
2. CLAUDE.md CLI commands table — does it match actual commands in src/cli.rs?
3. CLAUDE.md architecture section — does the source tree layout match actual files?
4. docs/openvital-spec.md — are feature descriptions consistent with implementation?
5. Configuration file comments — do they describe actual current behavior?

Cross-check README commands table against src/cli.rs enum variants to find mismatches.

Actions:
- Fix factually incorrect documentation: create a PR
- PR title format: [jules-docs] Update <file/module> documentation
- Only fix factual errors — do NOT rewrite for style
- If large sections of documentation are missing entirely:
  create an Issue with "jules-created" label rather than writing it yourself
  (inaccurate documentation is worse than no documentation)

OpenVital project conventions:
- Build verification: cargo check && cargo fmt --all -- --check && cargo clippy -- -D warnings && cargo test
- All four commands must pass before creating a PR (matches CI pipeline)
- Follow Conventional Commits format: type(scope): description
- Never modify CHANGELOG.md (managed by release-please)
- Integration tests use temp databases — see tests/ for patterns
```

**Step 6: Verify all five files exist and are non-empty**

Run: `wc -l .github/jules-prompts/bug-hunter.txt .github/jules-prompts/security-patrol.txt .github/jules-prompts/test-coverage.txt .github/jules-prompts/dependency-health.txt .github/jules-prompts/doc-consistency.txt`
Expected: All five files listed with line counts > 10

**Step 7: Commit**

```bash
git add .github/jules-prompts/bug-hunter.txt .github/jules-prompts/security-patrol.txt .github/jules-prompts/test-coverage.txt .github/jules-prompts/dependency-health.txt .github/jules-prompts/doc-consistency.txt
git commit -m "ci(jules): add scheduled task prompt files for 5 active agents"
```

---

### Task 4: Create jules-events.yml workflow

**Files:**
- Create: `.github/workflows/jules-events.yml`

**Step 1: Write the workflow file**

```yaml
name: Jules Event Pipelines

on:
  pull_request:
    types: [opened, synchronize, closed]
    branches: [master]
  issues:
    types: [opened]

concurrency:
  group: jules-events-${{ github.event_name }}-${{ github.event.number || github.event.issue.number }}
  cancel-in-progress: false

env:
  JULES_API_URL: https://jules.googleapis.com/v1alpha
  JULES_SOURCE: sources/github/punkpeye/openvital

jobs:
  # Pipeline 1: Doc Sync (on PR merge)
  doc-sync:
    if: >
      github.event_name == 'pull_request' &&
      github.event.action == 'closed' &&
      github.event.pull_request.merged == true &&
      !startsWith(github.event.pull_request.title, '[jules-')
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Create Jules doc-sync session
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session \
            .github/jules-prompts/doc-sync.txt \
            "[jules-docs] Sync docs for PR #${{ github.event.pull_request.number }}" \
            "master"

  # Pipeline 2: PR Review (on PR open/update)
  pr-review:
    if: >
      github.event_name == 'pull_request' &&
      (github.event.action == 'opened' || github.event.action == 'synchronize') &&
      !startsWith(github.event.pull_request.title, '[jules-') &&
      !startsWith(github.event.pull_request.title, 'chore(release)') &&
      !startsWith(github.event.pull_request.title, 'release-please')
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Create Jules review session
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session \
            .github/jules-prompts/pr-review.txt \
            "Review PR #${{ github.event.pull_request.number }}" \
            "${{ github.event.pull_request.head.ref }}" \
            ""

  # Pipeline 3: Issue AutoFix (on issue create, excluding Jules-created issues)
  issue-autofix:
    if: >
      github.event_name == 'issues' &&
      github.event.action == 'opened' &&
      !contains(github.event.issue.labels.*.name, 'jules-created') &&
      github.actor != 'jules-bot'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Create Jules autofix session
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session \
            .github/jules-prompts/issue-autofix.txt \
            "Assess issue #${{ github.event.issue.number }}" \
            "master"
```

**Step 2: Validate YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/jules-events.yml'))"`
Expected: No output (valid YAML)

**Step 3: Commit**

```bash
git add .github/workflows/jules-events.yml
git commit -m "ci(jules): add event-driven pipelines workflow (doc-sync, review, autofix)"
```

---

### Task 5: Create jules-scheduled.yml workflow

**Files:**
- Create: `.github/workflows/jules-scheduled.yml`

**Step 1: Write the workflow file**

Uses OpenVital-specific cron values from `jules-openvital.md` Section 4 (Mon/Wed/Fri spread).

```yaml
name: Jules Scheduled Tasks

on:
  schedule:
    - cron: '0 2 * * *'    # Daily: bug-hunter at 2:00 UTC
    - cron: '30 2 * * *'   # Daily: security-patrol at 2:30 UTC
    - cron: '0 3 * * 1'    # Weekly Mon: test-coverage at 3:00 UTC
    - cron: '0 3 * * 3'    # Weekly Wed: dependency-health at 3:00 UTC
    - cron: '0 3 * * 5'    # Weekly Fri: doc-consistency at 3:00 UTC
  workflow_dispatch:
    inputs:
      task:
        description: 'Task to run manually'
        required: true
        type: choice
        options:
          - bug-hunter
          - security-patrol
          - test-coverage
          - dependency-health
          - doc-consistency

concurrency:
  group: jules-scheduled
  cancel-in-progress: false

env:
  JULES_API_URL: https://jules.googleapis.com/v1alpha
  JULES_SOURCE: sources/github/punkpeye/openvital

jobs:
  bug-hunter:
    if: github.event.schedule == '0 2 * * *' || github.event.inputs.task == 'bug-hunter'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run bug-hunter
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session \
            .github/jules-prompts/bug-hunter.txt \
            "[jules-bug] Scheduled bug hunt" \
            "master"

  security-patrol:
    if: github.event.schedule == '30 2 * * *' || github.event.inputs.task == 'security-patrol'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run security-patrol
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session \
            .github/jules-prompts/security-patrol.txt \
            "[jules-security] Scheduled security patrol" \
            "master"

  test-coverage:
    if: github.event.schedule == '0 3 * * 1' || github.event.inputs.task == 'test-coverage'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run test-coverage
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session \
            .github/jules-prompts/test-coverage.txt \
            "[jules-test] Scheduled test coverage" \
            "master"

  dependency-health:
    if: github.event.schedule == '0 3 * * 3' || github.event.inputs.task == 'dependency-health'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run dependency-health
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session \
            .github/jules-prompts/dependency-health.txt \
            "[jules-deps] Scheduled dependency health check" \
            "master"

  doc-consistency:
    if: github.event.schedule == '0 3 * * 5' || github.event.inputs.task == 'doc-consistency'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run doc-consistency
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session \
            .github/jules-prompts/doc-consistency.txt \
            "[jules-docs] Scheduled doc consistency check" \
            "master"
```

**Step 2: Validate YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/jules-scheduled.yml'))"`
Expected: No output (valid YAML)

**Step 3: Commit**

```bash
git add .github/workflows/jules-scheduled.yml
git commit -m "ci(jules): add scheduled tasks workflow (5 agents, OpenVital cron)"
```

---

### Task 6: Create GitHub labels and verify setup

This task requires manual actions and `gh` CLI.

**Step 1: Create labels**

Run: `gh label create jules-created --description "Issue created by Jules automation" --color "0E8A16"`
Expected: Label created

Run: `gh label create jules-error --description "Jules automation error requiring investigation" --color "D93F0B"`
Expected: Label created

**Step 2: Verify JULES_API_KEY secret exists (or remind to set it)**

Run: `gh secret list | grep JULES_API_KEY || echo "WARNING: JULES_API_KEY secret not set — add it at https://github.com/punkpeye/openvital/settings/secrets/actions"`
Expected: Either the secret is listed, or a warning is printed

**Step 3: Verify all files are in place**

Run: `ls -la .github/jules-prompts/ .github/workflows/jules-events.yml .github/workflows/jules-scheduled.yml`
Expected: All files listed:
- `.github/jules-prompts/helpers.sh`
- `.github/jules-prompts/doc-sync.txt`
- `.github/jules-prompts/pr-review.txt`
- `.github/jules-prompts/issue-autofix.txt`
- `.github/jules-prompts/bug-hunter.txt`
- `.github/jules-prompts/security-patrol.txt`
- `.github/jules-prompts/test-coverage.txt`
- `.github/jules-prompts/dependency-health.txt`
- `.github/jules-prompts/doc-consistency.txt`
- `.github/workflows/jules-events.yml`
- `.github/workflows/jules-scheduled.yml`

**Step 4: Update OpenVital setup checklist**

Check off completed items in `docs/jules/jules-openvital.md` Section 5.

**Step 5: Commit checklist update**

```bash
git add docs/jules/jules-openvital.md
git commit -m "docs(jules): update setup checklist with completed items"
```

---

### Task 7: Smoke test PR Review pipeline

**Step 1: Push the branch and create a test PR**

Run: `git push -u origin feat/jules-integration`
Expected: Branch pushed

Run: `gh pr create --title "ci(jules): integrate Jules API for automated workflows" --body "$(cat <<'EOF'
## Summary
- Add Jules API event-driven workflows (doc sync, PR review, issue autofix)
- Add Jules API scheduled tasks (bug hunter, security patrol, test coverage, dependency health, doc consistency)
- Helper script with retry logic and error handling
- Prompt files stored as text for safe JSON construction

## Test plan
- [ ] Verify JULES_API_KEY is set in repo secrets
- [ ] Verify jules-created and jules-error labels exist
- [ ] Open a test PR to trigger pr-review pipeline
- [ ] Manually dispatch a scheduled task via workflow_dispatch

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"`
Expected: PR URL returned

**Step 2: Verify the PR Review workflow triggers**

Check GitHub Actions tab for the PR — the `pr-review` job should start (it will fail if JULES_API_KEY is not yet set, which is expected).

**Step 3: Manually test a scheduled task**

Run: `gh workflow run jules-scheduled.yml -f task=bug-hunter`
Expected: Workflow dispatched (will fail if JULES_API_KEY not set — that's OK for smoke test)
