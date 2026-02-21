# AI Health Context & Anomaly Detection Design

**Date:** 2026-02-21
**Status:** Approved (autonomous decision based on research)

## Problem Statement

OpenVital has comprehensive health tracking (metrics, goals, medications, trends, correlations) but lacks two capabilities that AI agents need to serve as effective health assistants:

1. **No single "memory read" endpoint** — An AI agent must call 5+ commands (status, trend, goal status, med status, show) and stitch results together to understand the user's health state. This burns tokens and increases error risk.

2. **No anomaly detection** — The system can compute trends and correlations, but cannot flag unusual readings against personal baselines. AI agents cannot proactively identify health concerns without this capability.

## Research Basis

Cross-validated across:
- Apple Health MCP Server (Momentum) — 7-tool pattern, summary-first design
- Spike API MCP integration — pre-aggregated data for LLMs
- ICLR 2026 MemAgents workshop — context-aware memory systems
- Nature Scientific Reports (2025) — personalized health monitoring with explainable AI
- Open Wearables API — unified wearable data patterns

Key insight: **The tool should do the computation, not the LLM.** Pre-aggregate, summarize in natural language, and flag anomalies before the data reaches the AI.

## Feature 1: `openvital context`

### Purpose
Single command returning complete health state as structured JSON with natural language summaries. The AI agent's "external memory read."

### CLI
```
openvital context [--days N] [--types t1,t2,...] [--human]
```
- `--days N` — lookback window (default: 7)
- `--types` — filter to specific metric types (default: all)

### Output Schema
```json
{
  "status": "ok",
  "command": "context",
  "data": {
    "generated_at": "2026-02-21T10:00:00Z",
    "period": { "start": "2026-02-14", "end": "2026-02-21", "days": 7 },
    "summary": "Weight declining steadily (-0.3 kg/week). Exercise streak at 12 days...",
    "profile": { "height": 180, "birth_year": 1990, "gender": "male" },
    "metrics": {
      "<type>": {
        "latest": { "value": 83.2, "unit": "kg", "timestamp": "..." },
        "trend": { "direction": "decreasing", "rate": -0.3, "rate_unit": "kg/week" },
        "stats": { "min": 82.8, "max": 84.1, "avg": 83.4, "count": 5 },
        "summary": "Weight declining 0.3 kg/week, on track for goal"
      }
    },
    "goals": [
      {
        "metric_type": "weight", "target": 80.0, "direction": "below",
        "current": 83.2, "is_met": false, "progress": 0.65,
        "summary": "65% toward weight goal (83.2 → 80.0 kg)"
      }
    ],
    "medications": {
      "active_count": 2,
      "adherence_today": 1.0,
      "adherence_7d": 0.85,
      "medications": [...],
      "summary": "2 active medications. All taken today. 85% adherence (7d)."
    },
    "streaks": { "logging": 15, "exercise": 12 },
    "alerts": [...],
    "anomalies": [...]
  }
}
```

### Architecture
- `cli.rs`: Add `Context` variant to `Commands` enum
- `cmd/context.rs`: Thin shell — open db, call `core::context::compute()`, format
- `core/context.rs`: Composes existing functions:
  - `core::status::compute()` for today's snapshot
  - `core::trend::compute()` for per-metric trends
  - `core::goal::goal_status()` for goals
  - `core::med::adherence_status()` for medications
  - `core::anomaly::detect()` for anomalies
  - New: natural language summary generation
- `output/human.rs`: Context section formatting

### No New DB Tables
Purely a read/compute layer composing existing queries.

## Feature 2: `openvital anomaly`

### Purpose
Detect unusual readings by comparing against personal rolling baselines using IQR-based statistical methods.

### CLI
```
openvital anomaly [type] [--days N] [--threshold moderate|strict|relaxed] [--human]
```
- No type = scan all tracked metrics
- `--days N` — baseline window (default: 30)
- `--threshold` — sensitivity control (default: moderate)

### Detection Method: IQR
Chosen over z-score/standard deviation because health data is often skewed:
```
Q1 = 25th percentile, Q3 = 75th percentile
IQR = Q3 - Q1
Lower = Q1 - factor * IQR, Upper = Q3 + factor * IQR
Factors: relaxed=2.0, moderate=1.5, strict=1.0
```

### Severity Levels
- **info**: 1.0-1.5x IQR from bounds
- **warning**: 1.5-2.0x IQR from bounds
- **alert**: >2.0x IQR from bounds

### Output Schema
```json
{
  "status": "ok",
  "command": "anomaly",
  "data": {
    "period": { "baseline_start": "...", "baseline_end": "...", "days": 30 },
    "threshold": "moderate",
    "anomalies": [
      {
        "metric_type": "heart_rate",
        "value": 92.0,
        "timestamp": "...",
        "baseline": { "q1": 68.0, "median": 72.0, "q3": 76.0, "iqr": 8.0 },
        "bounds": { "lower": 56.0, "upper": 88.0 },
        "deviation": "above",
        "severity": "warning",
        "summary": "Heart rate 92 bpm above normal range (56-88 bpm)"
      }
    ],
    "scanned_types": [...],
    "clean_types": [...],
    "summary": "1 anomaly detected across 4 metric types."
  }
}
```

### Architecture
- `cli.rs`: Add `Anomaly` variant to `Commands` enum
- `cmd/anomaly.rs`: Thin shell
- `core/anomaly.rs`: New module
  - `compute_baseline(db, type, days) -> Baseline`
  - `detect(db, type?, days, threshold) -> AnomalyResult`
- `models/anomaly.rs`: `Anomaly`, `Baseline`, `Severity`, `Threshold` structs
- `output/human.rs`: Anomaly formatting
- No new DB tables — computes from existing metrics

### Minimum Data Requirement
At least 7 data points needed for meaningful baseline. Returns empty with info message otherwise.

## Integration Points

1. `context` command includes anomalies automatically
2. `status` command gains an optional `--anomalies` flag
3. Both commands maintain the standard JSON envelope
4. Both support `--human` formatting

## What We're NOT Building (YAGNI)

- MCP server mode (separate project, separate timeline)
- Wearable sync (import path already exists)
- TUI dashboard (low value for AI agents)
- Cloud sync (out of scope, local-first by design)
- Predictive models (trend projection already exists, that's sufficient)
- Medicine interaction checking (liability concerns, out of scope)

## Testing Strategy

Per BDD+TDD mandate:
1. Integration tests in `tests/context_test.rs` and `tests/anomaly_test.rs`
2. Unit tests for IQR computation, summary generation, baseline calculation
3. Edge cases: no data, single data point, all identical values, negative values
4. Variant tests: different threshold levels, different time windows
