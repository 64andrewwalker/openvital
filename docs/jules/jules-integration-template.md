# Jules Integration PRD Template

> **Version:** 1.0
> **Status:** Template — copy and customize for your project
> **API Status:** Jules API is in alpha; specifications may change

This is a project-agnostic template for integrating [Google Jules](https://developers.google.com/jules/api) into a development workflow. It defines four automation pipelines that leverage Jules as an AI coding agent to accelerate documentation, code review, issue resolution, and proactive code health.

## Table of Contents

1. [Overview](#1-overview)
2. [Prerequisites](#2-prerequisites)
3. [Pipeline 1: Documentation Sync](#3-pipeline-1-documentation-sync)
4. [Pipeline 2: PR Review](#4-pipeline-2-pr-review)
5. [Pipeline 3: Issue AutoFix](#5-pipeline-3-issue-autofix)
6. [Pipeline 4: Scheduled Tasks](#6-pipeline-4-scheduled-tasks)
7. [Global Rules](#7-global-rules)
8. [GitHub Actions Reference](#8-github-actions-reference)
9. [Rollout Strategy](#9-rollout-strategy)
10. [Customization Checklist](#10-customization-checklist)

---

## 1. Overview

### Problem

Development teams spend significant time on repetitive tasks: keeping documentation in sync with code, reviewing boilerplate PRs, triaging and fixing simple issues, and running proactive code health checks. These tasks are well-suited for AI agent automation.

### Solution

Integrate Jules as an always-on development agent via four pipelines:

| # | Pipeline | Trigger | Output | Human Gate |
|---|----------|---------|--------|------------|
| 1 | Doc Sync | PR merged to main | Documentation update PR | Required |
| 2 | PR Review | PR opened | Review comments + approve/request changes | N/A (see note below) |
| 3 | Issue AutoFix | Issue created | Fix PR or assessment comment | Required |
| 4 | Scheduled Tasks | Cron | PR or Issue | Required |

### Non-Goals

- Replacing human code review for complex changes
- Autonomous merging without human approval
- Handling issues that require architectural decisions
- Managing secrets, deployments, or infrastructure

---

## 2. Prerequisites

- GitHub repository connected to Jules at [jules.google.com](https://jules.google.com)
- Jules API key stored as GitHub Actions secret (`JULES_API_KEY`)
- GitHub Actions enabled on the repository
- CI pipeline that runs tests on PRs (Jules PRs must pass CI before human review)

---

## 3. Pipeline 1: Documentation Sync

### Trigger

`pull_request` event with type `closed` and `merged == true` on the main branch.

### Behavior

1. Jules receives the merged PR diff
2. Analyzes which documentation files may be affected (README, API docs, spec, changelog, config comments, etc.)
3. If updates are needed, creates a follow-up PR with documentation changes
4. If no updates are needed, takes no action (silent)

### Prompt Template

```
You are a documentation maintainer for this repository.

A PR was just merged. Here is the context:
- PR title: {{PR_TITLE}}
- PR body: {{PR_BODY}}
- Changed files: {{CHANGED_FILES}}

Analyze the changes and determine if any documentation needs updating. Check:
1. README — feature lists, usage examples, installation instructions
2. API/spec documentation — parameter changes, new endpoints, behavior changes
3. Configuration docs — new options, changed defaults
4. CHANGELOG — ensure the change is captured (if not auto-generated)
5. Code comments — inline docs that reference changed behavior

Rules:
- Only create a PR if documentation is factually out of date
- Do NOT rewrite documentation style — only fix factual inaccuracies
- Do NOT add documentation for undocumented features unless the merged PR explicitly adds them
- PR title format: [jules-docs] Update docs for {{PR_TITLE}}
- If no documentation updates are needed, do nothing
```

### Output

- PR with title `[jules-docs] Update docs for <original PR title>`
- Human must review and merge

---

## 4. Pipeline 2: PR Review

### Trigger

`pull_request` event with type `opened` or `synchronize` on PRs targeting main branch.

### Behavior

1. Jules receives the PR diff and description
2. Reviews for: bugs, logic errors, security issues, test coverage gaps, style violations
3. Posts a review comment summarizing findings
4. Submits a GitHub review: `APPROVE` if no issues, `REQUEST_CHANGES` if blocking issues found

### Prompt Template

```
You are a code reviewer for this repository.

Review the following pull request:
- Title: {{PR_TITLE}}
- Description: {{PR_BODY}}
- Diff: (provided via source context)

Review criteria (in priority order):
1. **Correctness** — Logic errors, off-by-one, null/undefined handling, race conditions
2. **Security** — Injection risks, hardcoded secrets, unsafe deserialization
3. **Test coverage** — Are new code paths tested? Are edge cases covered?
4. **Error handling** — Swallowed errors, missing fallbacks, unclear error messages
5. **API contract** — Breaking changes without version bumps, missing migrations

Rules:
- Focus on substantive issues, not style (linters handle style)
- For each issue, explain WHY it's a problem and suggest a fix
- Categorize issues as: 🔴 Blocking / 🟡 Suggestion / 🟢 Nit
- If no blocking issues: approve the PR
- If blocking issues found: request changes
- Do NOT comment on code you don't understand — ask a question instead
- Keep comments concise — one issue per comment, max 3-4 sentences
```

### Output

- PR review comments on specific lines
- Overall review status: APPROVE or REQUEST_CHANGES
- Summary comment with issue counts by severity

### Review Authority

Jules submits GitHub reviews with `APPROVE` or `REQUEST_CHANGES` status. Choose one model based on your branch protection setup:

| Model | When to use | Branch protection setting |
|-------|-------------|--------------------------|
| **Advisory** | Early rollout or small teams | Do NOT add Jules bot as a required reviewer; its reviews are informational only |
| **Gate** | High-trust, established teams | Add Jules bot as a required reviewer; its `REQUEST_CHANGES` blocks merge |

**Recommendation:** Start with Advisory during Phase 1 (see Rollout Strategy). Only promote to Gate after validating false positive rates are acceptable (< 20%).

### Exclusions

Skip review for PRs with these title prefixes:
- `[jules-*]` (avoid self-review loops)
- `release-please` or `chore(release)` (auto-generated release PRs)

---

## 5. Pipeline 3: Issue AutoFix

### Trigger

`issues` event with type `opened`, **excluding issues created by Jules itself** (to prevent self-triggering loops with Pipeline 4).

### Behavior

1. Jules receives the new issue title and body
2. Evaluates whether it can fix the issue autonomously:
   - **Can fix:** Creates a PR with the fix, references the issue
   - **Cannot fix:** Leaves a comment explaining the assessment and why it needs human intervention
   - **Unclear:** Asks clarifying questions in a comment
3. Assessment criteria for "can fix":
   - Clear reproduction steps or error description
   - Isolated to a single module/file
   - Does not require architectural changes
   - Has existing test coverage to validate the fix

### Prompt Template

```
You are a developer triaging a new issue for this repository.

Issue:
- Title: {{ISSUE_TITLE}}
- Body: {{ISSUE_BODY}}
- Labels: {{ISSUE_LABELS}}

Step 1: Assess whether you can fix this issue autonomously.

You CAN fix it if ALL of these are true:
- The problem is clearly described (you understand what's wrong)
- The fix is localized (1-3 files)
- The fix does not require architectural decisions or new dependencies
- You can verify the fix with existing tests or by adding a targeted test

You CANNOT fix it if ANY of these are true:
- The issue describes a feature request requiring design decisions
- The fix requires changes across many modules
- You're not confident you understand the root cause
- The fix could have unintended side effects you can't test for

Step 2: Take action based on your assessment.

If you CAN fix it:
- Create a PR with the fix
- PR title format: [jules-fix] <concise description> (fixes #{{ISSUE_NUMBER}})
- Include a test that reproduces the bug and validates the fix
- PR description must explain: root cause, fix approach, how to verify

If you CANNOT fix it:
- Leave a comment on the issue with your assessment:
  - What you think the root cause might be
  - Why automated fixing isn't appropriate
  - Suggested approach for a human developer
- Do NOT create a PR for partial or uncertain fixes

If the issue is UNCLEAR:
- Leave a comment asking specific clarifying questions
- Do NOT attempt a fix without understanding the problem
```

### Output

- Fix PR with title `[jules-fix] <description> (fixes #<issue>)`, OR
- Assessment comment on the issue

---

## 6. Pipeline 4: Scheduled Tasks

### Trigger

Cron schedule via GitHub Actions `schedule` trigger, calling Jules API to create sessions.

### Behavior

Each scheduled task runs independently in a fresh environment (no shared state between runs). Tasks form an "agent pod" — multiple agents each focused on a specific code health dimension.

Refer to the companion document **jules-scheduled-tasks.md** for the full prompt library.

### Recommended Agent Pod

| Agent | Frequency | PR Prefix | Focus |
|-------|-----------|-----------|-------|
| Bug Hunter | Daily | `[jules-bug]` | Logic errors, error handling, resource leaks |
| Dependency Health | Weekly | `[jules-deps]` | Vulnerabilities, outdated packages |
| Test Coverage | Weekly | `[jules-test]` | Missing tests for core modules |
| Security Patrol | Daily | `[jules-security]` | Injection, auth, data exposure |
| Performance Scout | Weekly | `[jules-perf]` | Algorithm complexity, N+1 queries |
| Doc Consistency | Weekly | `[jules-docs]` | Stale docs, wrong examples |
| Dead Code Cleanup | Monthly | `[jules-cleanup]` | Unused exports, commented-out code |

### Output

- PR for issues that can be safely auto-fixed
- Issue for problems requiring human judgment
- No output if nothing found (silent success)

---

## 7. Global Rules

These rules apply across ALL pipelines:

### Safety

- **All Jules PRs require human review and merge** — no auto-merge
- **Jules never force-pushes** — always creates new branches
- **Jules PRs must pass CI** before human review
- **Jules never commits secrets** — API keys, tokens, credentials

### PR Conventions

- Title prefix: `[jules-<type>]` where type is `docs`, `fix`, `bug`, `deps`, `test`, `security`, `perf`, `cleanup`
- PR description must include: what changed, why, how to verify
- One issue per PR — no mega-PRs
- Branch naming: `jules/<type>/<short-description>`

### Self-Loop Prevention

- Jules does NOT review its own PRs (skip PRs with `[jules-*]` prefix)
- Jules does NOT create follow-up issues from its own PRs
- Scheduled tasks do NOT trigger event-driven pipelines
- **Issue AutoFix skips issues created by Jules** — use **both** label check (`jules-created`) and actor check (`github.actor`) in workflow conditions for defense-in-depth
- The actor name (`jules-bot` in examples) must match the actual GitHub actor used by Jules sessions — **verify this after connecting your repo** and update the workflow `if` condition accordingly
- All Jules-created Issues must include the `jules-created` label
- Scheduled task prompts must instruct Jules to apply the `jules-created` label to any Issues it creates

### Rate Limiting

- Max concurrent Jules sessions: configurable (default: 3)
- Daily PR limit: configurable (default: 5)
- Avoid scheduling all cron tasks at the same time — stagger them

**Implementation:** Use GitHub Actions `concurrency` groups to enforce limits:

```yaml
# In jules-events.yml — each pipeline gets its own concurrency group
concurrency:
  group: jules-events-${{ github.event_name }}-${{ github.event.number || github.event.issue.number }}
  cancel-in-progress: false

# In jules-scheduled.yml — limit total concurrent scheduled sessions
concurrency:
  group: jules-scheduled
  cancel-in-progress: false
```

The `cancel-in-progress: false` ensures queued sessions wait rather than being cancelled. The scheduled tasks concurrency group ensures only one scheduled job runs at a time, preventing API rate limit errors.

### Failure Handling

- If Jules API returns an error, the GitHub Action should retry once after 60s
- If the retry fails, create a GitHub Issue tagged `jules-error` with the error details
- Never silently swallow errors

---

## 8. GitHub Actions Reference

### Prompt Management

**Do NOT inline prompts into `curl -d` JSON strings.** Multi-line prompts with quotes, newlines, and special characters will break JSON encoding or shell escaping.

Instead, store prompts as separate files and load them at runtime:

```
.github/jules-prompts/
├── doc-sync.txt
├── pr-review.txt
├── issue-autofix.txt
├── bug-hunter.txt
├── security-patrol.txt
├── test-coverage.txt
├── dependency-health.txt
├── performance-scout.txt
├── doc-consistency.txt
└── dead-code-cleanup.txt
```

Use `jq` to safely construct JSON payloads, and **always check HTTP status codes** (see Failure Handling in Section 7):

```bash
# Helper function — include in all workflows via a shared script or inline
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

  local HTTP_CODE RESPONSE
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

Usage in workflow steps:

```bash
source .github/jules-prompts/helpers.sh  # or inline the function
jules_create_session .github/jules-prompts/doc-sync.txt "[jules-docs] Sync docs" "main"
```

### Workflow: Event-Driven Pipelines

```yaml
# .github/workflows/jules-events.yml
name: Jules Event Pipelines

on:
  pull_request:
    types: [opened, synchronize, closed]
    branches: [{{MAIN_BRANCH}}]
  issues:
    types: [opened]

env:
  JULES_API_URL: https://jules.googleapis.com/v1alpha
  JULES_SOURCE: sources/github/{{OWNER}}/{{REPO}}

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
            "{{MAIN_BRANCH}}"

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
            "{{MAIN_BRANCH}}"
```

### Workflow: Scheduled Tasks

Each task is a **separate job** (not a matrix) so that `workflow_dispatch` can run a single task without triggering the entire group.

```yaml
# .github/workflows/jules-scheduled.yml
name: Jules Scheduled Tasks

on:
  schedule:
    # Stagger schedules to avoid concurrent session limits
    - cron: '0 2 * * *'   # Daily at 2:00 UTC
    - cron: '30 2 * * *'  # Daily at 2:30 UTC (staggered)
    - cron: '0 3 * * 1'   # Weekly on Monday at 3:00 UTC
    - cron: '30 3 * * 1'  # Weekly on Monday at 3:30 UTC
    - cron: '0 4 * * 1'   # Weekly on Monday at 4:00 UTC
    - cron: '30 4 * * 1'  # Weekly on Monday at 4:30 UTC
    - cron: '0 4 1 * *'   # Monthly on 1st at 4:00 UTC
  workflow_dispatch:
    inputs:
      task:
        description: 'Task to run manually'
        required: true
        type: choice
        options:
          - bug-hunter
          - dependency-health
          - test-coverage
          - security-patrol
          - performance-scout
          - doc-consistency
          - dead-code-cleanup

env:
  JULES_API_URL: https://jules.googleapis.com/v1alpha
  JULES_SOURCE: sources/github/{{OWNER}}/{{REPO}}

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
          jules_create_session .github/jules-prompts/bug-hunter.txt "[jules-bug] Scheduled bug hunt" "{{MAIN_BRANCH}}"

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
          jules_create_session .github/jules-prompts/security-patrol.txt "[jules-security] Scheduled security patrol" "{{MAIN_BRANCH}}"

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
          jules_create_session .github/jules-prompts/test-coverage.txt "[jules-test] Scheduled test coverage" "{{MAIN_BRANCH}}"

  dependency-health:
    if: github.event.schedule == '30 3 * * 1' || github.event.inputs.task == 'dependency-health'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run dependency-health
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session .github/jules-prompts/dependency-health.txt "[jules-deps] Scheduled dependency health check" "{{MAIN_BRANCH}}"

  performance-scout:
    if: github.event.schedule == '0 4 * * 1' || github.event.inputs.task == 'performance-scout'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run performance-scout
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session .github/jules-prompts/performance-scout.txt "[jules-perf] Scheduled performance scout" "{{MAIN_BRANCH}}"

  doc-consistency:
    if: github.event.schedule == '30 4 * * 1' || github.event.inputs.task == 'doc-consistency'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run doc-consistency
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session .github/jules-prompts/doc-consistency.txt "[jules-docs] Scheduled doc consistency check" "{{MAIN_BRANCH}}"

  dead-code-cleanup:
    if: github.event.schedule == '0 4 1 * *' || github.event.inputs.task == 'dead-code-cleanup'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run dead-code-cleanup
        env:
          JULES_API_KEY: ${{ secrets.JULES_API_KEY }}
        run: |
          source .github/jules-prompts/helpers.sh
          jules_create_session .github/jules-prompts/dead-code-cleanup.txt "[jules-cleanup] Scheduled dead code cleanup" "{{MAIN_BRANCH}}"
```

---

## 9. Rollout Strategy

### Phase 1: Observe (Week 1-2)

- Enable Pipeline 2 (PR Review) only
- Jules reviews PRs but team treats it as advisory
- Evaluate review quality, false positive rate

### Phase 2: Trust (Week 3-4)

- Enable Pipeline 1 (Doc Sync) and Pipeline 3 (Issue AutoFix)
- Start with a few manually-created test issues
- Monitor PR quality and CI pass rate

### Phase 3: Scale (Month 2+)

- Enable Pipeline 4 (Scheduled Tasks), starting with 2-3 agents
- Gradually add more agents based on value vs noise ratio
- Tune cron schedules based on team capacity to review Jules PRs

### Success Metrics

| Metric | Target |
|--------|--------|
| Jules PR CI pass rate | > 90% |
| Jules PR merge rate (after human review) | > 70% |
| False positive rate in reviews | < 20% |
| Average time from issue to Jules PR | < 30 min |
| Documentation staleness (measured by doc-consistency agent) | Decreasing trend |

---

## 10. Customization Checklist

When adopting this template for a new project, replace these placeholders:

| Placeholder | Description | Example |
|-------------|-------------|---------|
| `{{MAIN_BRANCH}}` | Main/default branch name | `main`, `master` |
| `{{OWNER}}` | GitHub org or user | `myorg` |
| `{{REPO}}` | Repository name | `my-project` |
Additional customization points:

- [ ] Set `JULES_API_KEY` as a GitHub Actions secret
- [ ] Connect repository at jules.google.com
- [ ] Create `.github/jules-prompts/` directory with prompt text files (see Section 8: Prompt Management)
- [ ] Populate each prompt file from Section 3-5 templates + project-specific context
- [ ] Populate scheduled task prompt files from jules-scheduled-tasks.md
- [ ] Adjust cron schedules for team timezone (override template defaults in project-specific doc)
- [ ] Configure rate limits (max concurrent sessions, daily PR limit)
- [ ] Create GitHub label `jules-created` for self-loop prevention
- [ ] Create GitHub label `jules-error` for failure tracking
- [ ] Select which scheduled task agents to activate
