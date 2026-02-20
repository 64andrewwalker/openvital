# ADR-0006: Jules API Automation Integration

## Status
Accepted

## Context

OpenVital is a solo-maintained Rust CLI project. Repetitive development tasks — keeping docs in sync after PR merges, reviewing PRs for common issues, triaging simple bugs, and running proactive code health scans — consume disproportionate time relative to the project's scale. Google Jules (v1alpha REST API) provides an AI coding agent that can be triggered programmatically via GitHub Actions, making it possible to automate these tasks without adding human reviewers.

Key constraints that shaped the decision:

1. **Solo maintainer** — no second pair of eyes by default; automation fills the review gap
2. **Agent-first design** — OpenVital's JSON output contract and 4-layer architecture (ADR-0002) already optimize for machine consumption, making AI agent integration a natural extension
3. **Existing CI/CD maturity** — GitHub Actions CI (`ci.yml`) and release-please (`release-please.yml`) were already in place, providing a proven foundation for additional workflow automation
4. **Safety-critical requirement** — autonomous agents must never merge code without human approval, and must not create infinite feedback loops

## Decision

Integrate Jules as an always-on development agent via **four automation pipelines**, orchestrated by two GitHub Actions workflow files:

### Pipeline Architecture

| # | Pipeline | Trigger | Output | Human Gate |
|---|----------|---------|--------|------------|
| 1 | Doc Sync | PR merged to `master` | Documentation update PR | Required |
| 2 | PR Review | PR opened/updated | Review comments + approve/request changes | N/A (advisory) |
| 3 | Issue AutoFix | Issue created (non-Jules) | Fix PR or assessment comment | Required |
| 4 | Scheduled Tasks | Cron (daily/weekly) | PR or Issue | Required |

### Workflow Separation

- **`jules-events.yml`** — event-driven pipelines (1-3), triggered by `pull_request` and `issues` events
- **`jules-scheduled.yml`** — cron-based pipelines (4), with 5 active agents: Bug Hunter (daily), Security Patrol (daily), Test Coverage (weekly/Mon), Dependency Health (weekly/Wed), Doc Consistency (weekly/Fri)

### Key Design Decisions

**Prompts as external text files** (``.github/jules-prompts/*.txt``), loaded at runtime via `cat` + `jq`. This avoids JSON/shell escaping issues that arise from inlining multi-line prompts into `curl -d` payloads. Each prompt is self-contained and includes project-specific conventions (4-layer architecture, `cargo clippy -D warnings`, Conventional Commits).

**Shared helper script** (``helpers.sh``) with `jules_create_session()` function providing: API session creation, JSON payload construction via `jq`, retry logic (60s delay, single retry on 5xx/429), and structured GitHub Actions error reporting (`::warning::`, `::error::`).

**Conditional `automationMode`** — the helper's fourth parameter controls whether Jules creates PRs automatically. PR Review passes an empty string (review-only, no PR creation), while Doc Sync and Issue AutoFix pass `AUTO_CREATE_PR`.

**Advisory review model** — Jules submits GitHub reviews but is NOT a required reviewer in branch protection. This matches the Phase 1 rollout strategy: observe quality before granting gate authority.

### Self-Loop Prevention (Defense-in-Depth)

AI agent feedback loops are the primary safety concern. Three independent layers prevent Jules from triggering itself:

1. **Workflow-level** — `if` conditions filter out Jules PRs (`[jules-*]` title prefix), release PRs (`chore(release)`), and Jules-created issues (`jules-created` label + actor check)
2. **Prompt-level** — Issue AutoFix prompt includes an explicit `STOP` instruction if it detects `jules-created`/`jules-error` labels or `[jules-` title prefix
3. **Concurrency control** — per-job concurrency groups prevent overlapping runs; scheduled jobs run independently but never duplicate

### Scheduled Task Distribution

Weekly tasks are spread across Mon/Wed/Fri (not all on Monday) to distribute the PR review load for a solo maintainer. Cron times are staggered (2:00, 2:30, 3:00 UTC) to respect API rate limits.

### Error Handling

Failed API calls create a GitHub Issue tagged `jules-error` with the workflow run URL. Deduplication logic (`gh issue list` + `jq` filtering) prevents repeated failures from creating duplicate error issues.

## Alternatives Considered

| Alternative | Why Rejected |
|-------------|-------------|
| **No automation** | Review gap remains; repetitive tasks consume too much time for solo maintenance |
| **GitHub Copilot / native PR review** | Less configurable; no cron-based proactive scanning; limited prompt customization |
| **Self-hosted CI agent (e.g., custom GPT wrapper)** | Higher maintenance burden; Jules provides managed infrastructure with GitHub-native integration |
| **Auto-merge Jules PRs** | Violates safety constraint — all code changes require human approval regardless of source |

## Consequences

### Positive
- Solo maintainer gets automated PR review, documentation sync, and proactive bug/security scanning
- Prompts encode project conventions (architecture, testing, commit format), acting as executable documentation
- Template (`jules-integration-template.md`) is reusable for other projects
- Three-phase rollout (Observe → Trust → Scale) limits blast radius of misconfigured prompts

### Negative
- Jules API is alpha (`v1alpha`) — breaking changes are expected; workflow files may need updates
- Agent quality is non-deterministic — false positives in reviews and low-quality fix PRs require human triage
- Operational experience confirmed: Jules may claim to have applied fixes without actually committing them (observed in PR #20 timezone bug and PR #21 test issues), requiring careful human verification of all Jules PRs
- Additional CI costs (GitHub Actions minutes) for each pipeline trigger

### Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| Infinite feedback loops | 3-layer defense-in-depth (workflow + prompt + concurrency) |
| Low-quality PRs creating noise | Advisory-only review model; human merge gate on all PRs |
| API rate limiting | Staggered cron, per-job concurrency groups, 429 retry logic |
| Jules claiming fixes without implementing them | Always verify PR diffs against review responses; never trust claim without code evidence |
| API breaking changes | Prompts and helper script isolated in `.github/jules-prompts/`; changes localized |

## References

- Design documents: `docs/jules/jules-integration-template.md`, `docs/jules/jules-openvital.md`, `docs/jules/jules-scheduled-tasks.md`
- Implementation plan: `docs/plans/2026-02-20-jules-integration.md`
- Workflow files: `.github/workflows/jules-events.yml`, `.github/workflows/jules-scheduled.yml`
