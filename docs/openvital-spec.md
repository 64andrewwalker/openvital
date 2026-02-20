# OpenVital — Agent-Native Health Management CLI

## Spec Version: 0.1.0

---

## 1. Vision & Design Philosophy

### Why CLI?

The future interaction paradigm is human ↔ agent ↔ tools. GUI is designed for human eyes and hands. CLI is designed for programmatic invocation — which is exactly what agents need. A health management tool that is CLI-first means:

- Any agent (OpenClaw, Claude CLI, etc.) can call it directly
- Data flows in and out as structured JSON — parseable, pipeable, composable
- Human can use it too, but the primary consumer is an agent acting on the human's behalf
- Zero friction logging: one command, done

### Design Principles

1. **Agent-first, human-friendly**: Every command outputs structured JSON by default, with a `--human` flag for pretty-printed output
2. **Single binary, zero dependencies**: Ship as one executable. No daemon, no server, no database server
3. **Local-first**: All data stored locally in SQLite. User owns their data
4. **Composable**: Follows Unix philosophy. Pipes work. Exit codes are meaningful
5. **Opinionated defaults, flexible overrides**: Sensible defaults, but everything configurable

---

## 2. Target Use Cases

OpenVital is designed for people who:

- Prefer terminal over GUI
- Use AI agents as daily assistants (OpenClaw, Claude CLI, etc.)
- Want health data to be agent-accessible for automated insights
- Need zero-friction logging — anything with more than one step will be abandoned
- Want to own their data locally, not in a cloud service

Key scenarios:
- Weight management and body composition tracking
- Exercise habit building (any type: gym, rhythm games, walking, etc.)
- Sleep tracking
- Pain / injury monitoring (RSI, tendinitis, chronic conditions)
- Nutrition logging
- Correlation analysis between metrics (e.g., screen time vs. pain levels)

---

## 3. Data Model

### 3.1 Metrics

All metrics are append-only time-series entries.

```
Metric {
  id:         UUID
  timestamp:  ISO 8601 (auto-generated, overridable)
  category:   enum (body, exercise, sleep, nutrition, pain, habit)
  type:       string (e.g., "weight", "steps", "sleep_hours")
  value:      float
  unit:       string (e.g., "kg", "min", "hours", "kcal")
  note:       string (optional, free text)
  tags:       string[] (optional)
  source:     string (default: "manual", could be "agent", "apple_health", etc.)
}
```

### 3.2 Built-in Metric Types

| Category | Type | Unit | Description |
|----------|------|------|-------------|
| body | weight | kg | Body weight |
| body | body_fat | % | Body fat percentage |
| body | waist | cm | Waist circumference |
| exercise | cardio | min | Cardio exercise duration |
| exercise | strength | min | Strength training duration |
| exercise | calories_burned | kcal | Estimated calories burned |
| sleep | sleep_hours | hours | Total sleep duration |
| sleep | sleep_quality | 1-5 | Subjective sleep quality |
| sleep | bed_time | HH:MM | Time went to bed |
| sleep | wake_time | HH:MM | Time woke up |
| nutrition | calories_in | kcal | Total calorie intake |
| nutrition | water | ml | Water intake |
| pain | pain | 0-10 | General pain level (use tags for location) |
| pain | soreness | 0-10 | General body soreness |
| habit | standing_breaks | count | Standing/stretching breaks taken |
| habit | screen_time | hours | Total screen time |

Users can define custom types at any time. The tool does not reject unknown types.

### 3.3 Goals

```
Goal {
  id:           UUID
  metric_type:  string
  target_value: float
  direction:    enum (above, below, equal)
  timeframe:    enum (daily, weekly, monthly)
  active:       bool
  created_at:   ISO 8601
}
```

Example goals:
- weight below 75kg (monthly check)
- cardio above 150min (weekly)
- water above 2000ml (daily)
- pain below 3 (daily)

### 3.4 Medication

```
Medication {
  id:           UUID
  name:         string
  dose:         string (optional, e.g. "400mg")
  route:        enum (oral, topical, inhaled, injection, ...)
  frequency:    enum (daily, 2x_daily, 3x_daily, weekly, as_needed)
  active:       bool
  started_at:   ISO 8601
  stopped_at:   ISO 8601 (optional)
  note:         string (optional)
}
```

Taking a medication creates a standard `Metric` entry with:
- category: `medication` (implied)
- type: `<medication_name>`
- value: `1.0` (count of doses/applications)
- unit: `"dose"` or `"application"`

---

## 4. CLI Interface

### 4.1 Command Structure

```
openvital <command> [subcommand] [args] [flags]
```

Global flags:
- `--json` (default): Output as JSON
- `--human` / `-h`: Pretty-printed human-readable output
- `--quiet` / `-q`: Minimal output (just confirmation or error)
- `--date <YYYY-MM-DD>`: Override date (default: today)
- `--config <path>`: Custom config file path

### 4.2 Core Commands

#### `openvital log <type> <value> [flags]`

Log a metric entry.

```bash
# Basic usage
openvital log weight 85.5
openvital log cardio 45 --note "Morning run"
openvital log pain 4 --tags "wrist,left" --note "After long coding session"
openvital log water 500
openvital log sleep_hours 7.5

# With tags
openvital log cardio 30 --tags "hiit,gym"

# Override timestamp
openvital log weight 86 --date 2026-02-15

# Multiple entries at once (agent-friendly)
openvital log --batch '[{"type":"weight","value":85.5},{"type":"water","value":2000},{"type":"sleep_hours","value":7}]'
```

Output (JSON):
```json
{
  "status": "ok",
  "entry": {
    "id": "...",
    "timestamp": "2026-02-17T10:30:00+08:00",
    "type": "weight",
    "value": 85.5,
    "unit": "kg"
  }
}
```

#### `openvital show <type> [flags]`

Show metric history.

```bash
# Latest value
openvital show weight

# Last N entries
openvital show weight --last 7

# Date range
openvital show weight --from 2026-01-01 --to 2026-02-17

# All types for a day
openvital show --date 2026-02-17

# Today's summary (agent will call this frequently)
openvital show today
```

#### `openvital trend <type> [flags]`

Analyze trends and generate insights.

```bash
# Weight trend (weekly averages)
openvital trend weight --period weekly --last 12

# Exercise compliance
openvital trend cardio --period weekly --last 4

# Correlation analysis (agent-friendly)
openvital trend --correlate pain,screen_time --last 30
```

Output (JSON):
```json
{
  "type": "weight",
  "period": "weekly",
  "data": [
    {"week": "2026-W01", "avg": 86.2, "min": 85.8, "max": 86.5, "count": 5},
    {"week": "2026-W02", "avg": 85.8, "min": 85.5, "max": 86.1, "count": 6}
  ],
  "trend": {
    "direction": "decreasing",
    "rate": -0.4,
    "rate_unit": "kg/week",
    "projected_30d": 84.2
  }
}
```

#### `openvital goal <subcommand>`

Manage goals.

```bash
# Set a goal
openvital goal set weight --below 75 --timeframe monthly
openvital goal set cardio --above 150 --timeframe weekly
openvital goal set water --above 2000 --timeframe daily

# Check goal status
openvital goal status
openvital goal status weight

# Remove a goal
openvital goal remove <goal_id>
```

#### `openvital med <subcommand>`

Manage medications and adherence.

```bash
# Add a new medication
openvital med add ibuprofen --dose "400mg" --freq as_needed
openvital med add vitamin_d --dose "1 tablet" --freq daily --route oral

# Record taking a dose
openvital med take ibuprofen
openvital med take vitamin_d --note "With breakfast"

# Check adherence status
openvital med status
openvital med status vitamin_d --last 30

# List medications
openvital med list
openvital med list --all  # include stopped

# Stop a medication
openvital med stop ibuprofen --reason "No longer needed"
```

#### `openvital status`

Quick overview — the primary command an agent will call to assess current state.

```bash
openvital status
```

Output (JSON):
```json
{
  "date": "2026-02-17",
  "profile": {
    "height_cm": 175,
    "latest_weight_kg": 85.5,
    "bmi": 27.9,
    "bmi_category": "overweight"
  },
  "today": {
    "logged": ["weight", "water"],
    "missing": ["sleep_hours", "cardio"],
    "pain_alerts": [
      {"type": "pain", "value": 4, "tags": ["wrist", "left"]}
    ]
  },
  "goals": {
    "on_track": ["water"],
    "behind": ["cardio"],
    "achieved": [],
    "weight_progress": {
      "current": 85.5,
      "target": 75,
      "remaining": 10.5,
      "weekly_rate": -0.4,
      "eta_weeks": 26
    }
  },
  "streaks": {
    "logging_days": 12,
    "exercise_this_week": 2,
    "water_goal_streak": 5
  }
}
```

#### `openvital report [flags]`

Generate a report for a time period.

```bash
# Weekly report (default: last 7 days)
openvital report --period week

# Monthly report
openvital report --period month --month 2026-01

# Custom range
openvital report --from 2026-01-01 --to 2026-02-17
```

The report outputs a comprehensive JSON blob with all metrics, trends, goal progress, and correlations. This is the primary input for an agent to generate health advice.

#### `openvital config <subcommand>`

Manage configuration.

```bash
# Set profile
openvital config set height 175
openvital config set birth_year 1995
openvital config set gender male
openvital config set conditions "adhd,tendinitis"

# View config
openvital config show

# Set reminder preferences (used by agent integration)
openvital config set reminder.weight "09:00"
openvital config set reminder.exercise "18:00"
```

#### `openvital export [flags]`

Export data for backup or analysis.

```bash
openvital export --format csv --output health_data.csv
openvital export --format json --output health_data.json
openvital export --format csv --type weight --from 2026-01-01
```

#### `openvital import [flags]`

Import data from external sources.

```bash
# From Apple Health export
openvital import --source apple_health --file export.xml

# From CSV
openvital import --source csv --file data.csv

# From JSON (agent can use this for bulk operations)
openvital import --source json --file data.json
```

---

## 5. Agent Integration Design

### 5.1 OpenClaw Skill Interface

The tool should be packagable as an OpenClaw skill:

```yaml
# SKILL.md for OpenClaw
---
name: openvital
description: "Personal health and fitness tracking CLI. Log weight, exercise, sleep, nutrition, and pain metrics. Query trends, check goals, and generate reports."
tools:
  - openvital log
  - openvital show
  - openvital trend
  - openvital goal
  - openvital status
  - openvital report
---
```

### 5.2 Agent Workflow Examples

**Daily check-in (agent-initiated):**
```bash
# Agent calls this at configured time
openvital status --json
# Agent interprets the output, generates natural language advice
# Agent sends advice to user via messaging channel
```

**User says "I just exercised for 40 minutes":**
```bash
# Agent translates natural language to CLI call
openvital log cardio 40 --tags "running" --note "User reported via chat" --source agent
```

**User asks "How's my weight trend?":**
```bash
openvital trend weight --period weekly --last 8 --json
# Agent formats the response conversationally
```

**Weekly report (agent-scheduled via cron):**
```bash
openvital report --period week --json
# Agent generates a summary and sends to user
```

### 5.3 Structured Output Contract

Every command returns JSON with a consistent envelope:

```json
{
  "status": "ok" | "error",
  "command": "log",
  "data": { ... },
  "error": null | { "code": "...", "message": "..." }
}
```

Exit codes:
- 0: Success
- 1: General error
- 2: Invalid arguments
- 3: Data not found
- 4: Validation error

---

## 6. UX Optimizations

### 6.1 Zero-Friction Logging

- **Minimum viable input**: `openvital log weight 85.5` — three words, done
- **Batch logging**: `--batch` mode for backfilling multiple entries at once
- **No mandatory fields**: Only `type` and `value` are required. Everything else is optional
- **Forgiving input**: Aliases work everywhere (see 6.3). Typo-tolerant where possible
- **Streak tracking**: Positive reinforcement, not guilt. Show streaks in `status`, never punish gaps

### 6.2 Pain & Injury Monitoring

- Pain entries above a configurable threshold for N consecutive days trigger a `pain_alerts` flag in `openvital status`
- Correlation tracking between pain metrics and other metrics (e.g., `screen_time`, `exercise`) is a first-class feature via `openvital trend --correlate`
- Agents can use this data to suggest rest days or ergonomic interventions

### 6.3 Metric Aliases

Configurable in `config.toml`. Defaults:

| Alias | Expands to |
|-------|-----------|
| w | weight |
| bf | body_fat |
| c | cardio |
| s | strength |
| sl | sleep_hours |
| sq | sleep_quality |
| wa | water |
| p | pain |
| so | soreness |
| cal | calories_in |
| st | screen_time |

Users can add custom aliases in config.

---

## 7. Technical Requirements

### 7.1 Language & Runtime

- **Language**: Rust
  - Rationale: Single binary, fast startup, no runtime dependency, excellent cross-compilation
  - Key crates:
    - `clap` — CLI argument parsing (derive mode)
    - `rusqlite` — SQLite with bundled feature (no system dependency)
    - `serde` + `serde_json` — JSON serialization
    - `chrono` — Date/time handling
    - `uuid` — Entry IDs
    - `toml` — Config file parsing
    - `colored` / `comfy-table` — Human-readable output formatting
    - `dirs` — Cross-platform home directory resolution

### 7.2 Storage

- **SQLite** via embedded driver (rusqlite or go-sqlite3)
- Database location: `~/.openvital/data.db`
- Config location: `~/.openvital/config.toml`
- All timestamps stored in UTC, displayed in local timezone

### 7.3 Installation

```bash
# Preferred: Homebrew
brew install openvital

# Or: cargo
cargo install openvital

# Or: npm (if Node.js implementation)
npm install -g openvital

# Or: direct download
curl -sSL https://github.com/<org>/openvital/releases/latest/download/openvital-$(uname -s)-$(uname -m) -o /usr/local/bin/openvital
```

### 7.4 First Run

```bash
openvital init
```

Interactive setup that asks:
- Height (cm)
- Current weight (kg)
- Birth year
- Known conditions (free text, comma separated)
- Primary exercise type
- Preferred units (metric / imperial)

Stores in `~/.openvital/config.toml`. Skippable with `openvital init --skip`.

---

## 8. Future Extensions (Not in v0.1)

These are documented for direction, not implementation:

1. **Apple Health / Google Fit sync**: Import from health data exports
2. **Wearable integration**: Garmin, Fitbit API sync via agent
3. **AI-powered insights**: Agent periodically calls `openvital report`, feeds to LLM, gets personalized advice
4. **Meal photo logging**: Agent receives food photo → vision model estimates calories → `openvital log cal <value>`
5. **OpenClaw skill package**: Published to OpenClaw skill registry
6. **MCP server mode**: Expose as MCP tool server for direct LLM integration
7. **TUI dashboard**: Optional `openvital dashboard` command with terminal UI (sparklines, gauges)
8. **Reminder daemon**: Lightweight background process that sends reminders via system notifications

---

## 9. Success Criteria

The tool is successful if:

1. Logging any metric takes < 2 seconds end-to-end (command to confirmation)
2. An agent can call `openvital status` and get a complete picture in one call
3. A user with low patience actually uses it for > 30 days consecutively
4. Weight trend is visible and motivating within 2 weeks of use
5. Pain correlation with behavioral patterns becomes visible within 1 month

---

## 10. Implementation Priority

### Phase 1 (MVP)
- [ ] `openvital init` — Profile setup
- [ ] `openvital log` — Core logging (single + batch)
- [ ] `openvital show` — Query entries
- [ ] `openvital status` — Daily overview
- [ ] `openvital config` — Profile management
- [ ] Metric aliases
- [ ] SQLite storage
- [ ] JSON output (default) + human-readable flag

### Phase 2
- [ ] `openvital trend` — Trend analysis with projections
- [ ] `openvital goal` — Goal setting and tracking
- [ ] `openvital report` — Period reports
- [ ] Streak tracking
- [ ] Pain alerts

### Phase 3
- [ ] `openvital export` / `openvital import`
- [ ] Correlation analysis (`--correlate`)
- [ ] OpenClaw skill packaging
- [ ] Shell completions (bash, zsh, fish)

---

## Appendix A: Example Agent Session

```
User (via chat): Did I exercise today?

Agent:
  1. Calls `openvital show today --json`
  2. Checks if cardio was logged
  3. If not, reminds user

Agent: No exercise logged today. Planning to work out later?

User: Yeah, did 30 minutes of cardio

Agent:
  1. Calls `openvital log cardio 30 --source agent`
  2. Calls `openvital goal status cardio --json`
  3. Formats response

Agent: Logged! You're at 90/150 minutes for the week. Two more
       sessions and you'll hit your goal.

User: My wrist hurts today

Agent:
  1. Asks: On a scale of 1-10?

User: 5

Agent:
  1. Calls `openvital log pain 5 --tags "wrist" --source agent`
  2. Calls `openvital trend --correlate pain,screen_time --last 7 --json`
  3. Analyzes correlation

Agent: Logged at 5/10. Looking at the past week, your screen time
       has been 12+ hours on the days when pain spikes. Consider
       taking more breaks tomorrow and skipping wrist-heavy exercise.
```

---

## Appendix B: Config File Example

```toml
# ~/.openvital/config.toml

[profile]
height_cm = 175
birth_year = 1995
gender = "male"
conditions = ["tendinitis"]
primary_exercise = "running"

[units]
weight = "kg"
height = "cm"
water = "ml"
temperature = "celsius"

[aliases]
w = "weight"
bf = "body_fat"
c = "cardio"
s = "strength"
sl = "sleep_hours"
sq = "sleep_quality"
wa = "water"
p = "pain"
so = "soreness"
cal = "calories_in"
st = "screen_time"

[goals]
weight = { target = 75, direction = "below", timeframe = "monthly" }
cardio = { target = 150, direction = "above", timeframe = "weekly" }
water = { target = 2000, direction = "above", timeframe = "daily" }

[alerts]
pain_threshold = 5
pain_consecutive_days = 3

[agent]
default_source = "manual"
status_include_streaks = true
```
