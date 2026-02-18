# ADR-0005: Export/Import Format Design

## Status
Accepted

## Context
Users need to back up data, migrate between systems, and bulk-load historical data. The spec requires CSV and JSON export/import.

## Decision

### Export
- CSV format: `timestamp,type,value,unit,note,tags,source` header
- JSON format: array of full Metric objects (using serde serialization)
- Both support optional filtering by metric type and date range
- Output to file (`--output`) or stdout (pipe-friendly)

### Import
- JSON: array of `{type, value, timestamp?, note?, tags?, source?}` objects
- CSV: same column format as export, with header row
- Each imported entry gets a new UUID (no ID collision)
- Source defaults to "import" when not specified
- Apple Health XML import deferred to future extension

## Consequences
- Round-trip export → import preserves data faithfully
- CSV is human-editable and spreadsheet-compatible
- JSON is agent-friendly and preserves types exactly
- No dependency on external CSV parsing crate (simple comma splitting suffices for our controlled format)
