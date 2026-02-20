# OpenVital

Agent-native health management CLI. Log metrics, track trends, set goals, and generate reports — all from the terminal. Designed for AI agents and power users alike.

## Why OpenVital?

- **Agent-first**: Every command outputs structured JSON by default — parseable, pipeable, composable
- **Single binary, zero dependencies**: Ship as one executable, no daemon or server needed
- **Local-first**: All data stored in SQLite. You own your data
- **Zero-friction logging**: `openvital log weight 85.5` — three words, done

## Install

```bash
# From source
cargo install openvital

# Or build locally
git clone https://github.com/punkpeye/openvital.git
cd openvital && cargo build --release
```

## Quick Start

```bash
# Initialize profile
openvital init

# Log some metrics
openvital log weight 85.5
openvital log cardio 45 --note "Morning run"
openvital log water 2000
openvital log pain 4 --tags "wrist,left"

# Check today's status
openvital status --human

# View trends
openvital trend weight --period weekly --last 8

# Set a goal
openvital goal set water --target 2000 --direction above --timeframe daily
```

## Commands

| Command | Description |
|---------|-------------|
| `init` | Interactive profile setup |
| `log <type> <value>` | Log a metric (single or `--batch`) |
| `show [type]` | Show metric history |
| `trend <type>` | Trend analysis with period bucketing |
| `trend --correlate a,b` | Pearson correlation between two metrics |
| `goal set/status/remove` | Goal management |
| `status` | Daily overview with streaks and pain alerts |
| `report` | Period reports (week/month/custom) |
| `export` | Export to CSV/JSON |
| `import` | Import from CSV/JSON |
| `config show/set` | Configuration management |
| `completions <shell>` | Shell completions (bash/zsh/fish) |

### Global Flags

- `--human` / `-H` — Human-readable output (default is JSON)
- `--quiet` / `-q` — Minimal output
- `--date <YYYY-MM-DD>` — Override entry date
- `--config <path>` — Custom config file path

## JSON Output

All commands return a standard envelope:

```json
{
  "status": "ok",
  "command": "log",
  "data": { "entry": { "id": "...", "type": "weight", "value": 85.5, "unit": "kg" } },
  "error": null
}
```

## Agent Integration

OpenVital is designed to be called by AI agents (OpenClaw, Claude CLI, etc.):

```bash
# Agent checks daily status
openvital status

# Agent logs on behalf of user
openvital log cardio 30 --source agent --note "User reported via chat"

# Agent analyzes correlations
openvital trend --correlate pain,screen_time --last 30

# Agent generates weekly report
openvital report --period week
```

## Built-in Metric Types

| Category | Types |
|----------|-------|
| Body | `weight`, `body_fat`, `waist` |
| Exercise | `cardio`, `strength`, `calories_burned` |
| Sleep | `sleep_hours`, `sleep_quality`, `bed_time`, `wake_time` |
| Nutrition | `calories_in`, `water` |
| Pain | `pain`, `soreness` |
| Habit | `standing_breaks`, `screen_time` |

Custom types are accepted — the tool does not reject unknown types. Aliases are configurable (e.g., `w` → `weight`, `p` → `pain`).

## Architecture

4-layer design: **CLI → Command → Core → DB**

- `cmd/` — Thin shells (no business logic)
- `core/` — Pure business logic, reusable by future entry points (MCP server, plugins)
- `db/` — SQLite persistence via rusqlite
- `models/` — Data types and config

See [docs/](docs/) for ADRs and development guide.

## Development

```bash
cargo build                    # Dev build
cargo test                     # Run all tests (22 integration tests)
cargo fmt --all -- --check     # Check formatting
cargo clippy -- -D warnings    # Lint
```

Enable local pre-commit checks:

```bash
git config core.hooksPath .githooks
```

The hook is a local convenience guard. Enforcement happens in GitHub via required CI and branch protection.

CI runs on Linux, macOS, and Windows via GitHub Actions.

## License

[MIT](LICENSE)
