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

        #[cfg(unix)]
        {
            use std::fs::{self, OpenOptions};
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            if !path.exists() {
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .mode(0o600)
                    .open(&path)?;
            } else {
                let mut perms = fs::metadata(&path)?.permissions();
                if perms.mode() & 0o777 != 0o600 {
                    perms.set_mode(0o600);
                    fs::set_permissions(&path, perms)?;
                }
            }
        }

        let conn = Connection::open(path)?;
        let db = Self { conn };
        migrate::run(&db.conn)?;
        Ok(db)
    }
}
