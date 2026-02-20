#[cfg(unix)]
mod tests {
    use openvital::db::Database;
    use openvital::models::config::Config;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{LazyLock, Mutex};
    use tempfile::TempDir;

    // Serialize tests that mutate OPENVITAL_HOME to avoid races with
    // models_test.rs (which uses the same lock name but in a separate
    // test binary — Rust runs each test file in a separate process,
    // so we only need intra-binary serialization here).
    static CONFIG_ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct OpenVitalHomeGuard {
        previous: Option<OsString>,
    }

    impl OpenVitalHomeGuard {
        fn set(path: &std::path::Path) -> Self {
            let previous = std::env::var_os("OPENVITAL_HOME");
            // SAFETY: tests that touch OPENVITAL_HOME are serialized by CONFIG_ENV_LOCK.
            unsafe { std::env::set_var("OPENVITAL_HOME", path) };
            Self { previous }
        }
    }

    impl Drop for OpenVitalHomeGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => {
                    // SAFETY: serialized by CONFIG_ENV_LOCK.
                    unsafe { std::env::set_var("OPENVITAL_HOME", value) };
                }
                None => {
                    // SAFETY: serialized by CONFIG_ENV_LOCK.
                    unsafe { std::env::remove_var("OPENVITAL_HOME") };
                }
            }
        }
    }

    #[test]
    fn test_config_and_db_created_with_restricted_permissions() {
        let _lock = CONFIG_ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let home_dir = dir.path().join(".openvital");
        let _guard = OpenVitalHomeGuard::set(&home_dir);

        let config_path = home_dir.join("config.toml");
        let db_path = home_dir.join("data.db");

        // Save config — creates directory + config file
        let config = Config::default();
        config.save().expect("Failed to save config");

        assert!(home_dir.exists());
        assert!(config_path.exists());

        let dir_mode = fs::metadata(&home_dir).unwrap().permissions().mode();
        assert_eq!(
            dir_mode & 0o777,
            0o700,
            "Directory should have 0700 permissions"
        );

        let config_mode = fs::metadata(&config_path).unwrap().permissions().mode();
        assert_eq!(
            config_mode & 0o777,
            0o600,
            "Config file should have 0600 permissions"
        );

        // Open database — creates db file
        Database::open(&db_path).expect("Failed to open database");
        assert!(db_path.exists());

        let db_mode = fs::metadata(&db_path).unwrap().permissions().mode();
        assert_eq!(
            db_mode & 0o777,
            0o600,
            "Database file should have 0600 permissions"
        );
    }

    #[test]
    fn test_existing_loose_permissions_are_corrected() {
        let _lock = CONFIG_ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let home_dir = dir.path().join(".openvital");
        let _guard = OpenVitalHomeGuard::set(&home_dir);

        let config_path = home_dir.join("config.toml");

        // First save to create the file
        let config = Config::default();
        config.save().unwrap();

        // Loosen permissions to simulate pre-fix state
        let mut perms = fs::metadata(&config_path).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&config_path, perms).unwrap();

        // Re-save should correct permissions
        config.save().unwrap();

        let mode = fs::metadata(&config_path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "Config file should be corrected to 0600 after re-save"
        );
    }
}
