# Imperial Units Support — Design Document

**Date:** 2026-02-18
**Status:** Approved

## Principle

- DB always stores metric (kg/cm/ml/C)
- `--human` output converts per user's config
- JSON output stays metric, adds `unit_system` field
- User input parsed in their unit system, converted to metric before storage
- Export/import always metric (data interchange format)

## Config Changes

```toml
[units]
system = "imperial"  # metric (default) | imperial
weight = "lbs"       # auto-set from system
height = "ft"
water = "fl_oz"
temperature = "fahrenheit"
```

`Units.system` field added. Individual unit fields auto-derived from system.
`init --units imperial` or `config set units.system imperial` to switch.

## Conversion Table

| Metric Type | Storage (metric) | Imperial Display | to_display | from_input |
|-------------|-----------------|-----------------|------------|------------|
| weight | kg | lbs | x 2.20462 | / 2.20462 |
| waist | cm | in | / 2.54 | x 2.54 |
| water | ml | fl oz | / 29.5735 | x 29.5735 |
| temperature | C | F | x1.8 + 32 | (v-32) / 1.8 |
| height (profile) | cm | ft'in" | special | special |

No conversion needed: calories, steps, sleep, mood, heart_rate, bp, pain, body_fat(%), etc.

## New Module: `src/core/units.rs`

```rust
pub fn to_display(value: f64, metric_type: &str, units: &Units) -> (f64, String)
pub fn from_input(value: f64, metric_type: &str, units: &Units) -> f64
pub fn is_imperial(units: &Units) -> bool
```

## Integration Points

1. **Input** `cmd/log.rs:run()` — after parsing value, call `from_input()` before storage
2. **Display** `output/human.rs:format_metric()` — call `to_display()` for human output
3. **Goal set** `cmd/goal.rs` — convert target value via `from_input()` before storage
4. **Goal status** — convert current/target values via `to_display()` for human output
5. **Trend/report** human output — convert displayed values
6. **init** — `--units imperial` flag sets system preference
7. **config set** — `units.system imperial` updates and auto-derives unit strings

## Unchanged

- JSON output: metric values, no conversion
- DB schema: no change
- Export/import: metric, no conversion
- Unitless metrics: no conversion
