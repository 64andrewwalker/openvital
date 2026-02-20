mod goals;
pub mod meds;
mod metrics;
mod migrate;

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    pub(crate) conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let db = Self { conn };
        migrate::run(&db.conn)?;
        Ok(db)
    }
}
