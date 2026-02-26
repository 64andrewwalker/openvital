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
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Securely create the file if it doesn't exist, with 0600 permissions.
            // This prevents a race condition where the file is created with default
            // permissions (e.g. 0644) and then restricted.
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .mode(0o600)
                .open(path)?;
        }

        let conn = Connection::open(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)?.permissions();
            if perms.mode() & 0o777 != 0o600 {
                perms.set_mode(0o600);
                std::fs::set_permissions(path, perms)?;
            }
        }
        let db = Self { conn };
        migrate::run(&db.conn)?;
        Ok(db)
    }
}
