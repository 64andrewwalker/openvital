use anyhow::Result;
use rusqlite::Connection;

pub fn run(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metrics (
            id         TEXT PRIMARY KEY,
            timestamp  TEXT NOT NULL,
            category   TEXT NOT NULL,
            type       TEXT NOT NULL,
            value      REAL NOT NULL,
            unit       TEXT NOT NULL,
            note       TEXT,
            tags       TEXT,
            source     TEXT NOT NULL DEFAULT 'manual'
        );
        CREATE INDEX IF NOT EXISTS idx_metrics_type_ts ON metrics(type, timestamp);
        CREATE INDEX IF NOT EXISTS idx_metrics_ts ON metrics(timestamp);",
    )?;
    Ok(())
}
