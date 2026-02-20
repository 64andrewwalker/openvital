#[cfg(unix)]
mod tests {
    use openvital::models::config::Config;
    use openvital::db::Database;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn test_security_permissions() {
        let dir = tempdir().unwrap();
        let home_dir = dir.path().join(".openvital");
        let config_path = home_dir.join("config.toml");
        let db_path = home_dir.join("data.db");

        // Set the environment variable so paths use our temp dir
        std::env::set_var("OPENVITAL_HOME", &home_dir);

        // Test Config save
        let config = Config::default();
        config.save().expect("Failed to save config");

        assert!(home_dir.exists());
        assert!(config_path.exists());

        let dir_mode = fs::metadata(&home_dir).expect("Failed to get dir metadata").permissions().mode();
        assert_eq!(dir_mode & 0o777, 0o700, "Directory should have 0700 permissions");

        let config_mode = fs::metadata(&config_path).expect("Failed to get config metadata").permissions().mode();
        assert_eq!(config_mode & 0o777, 0o600, "Config file should have 0600 permissions");

        // Test Database open
        Database::open(&db_path).expect("Failed to open database");

        assert!(db_path.exists());
        let db_mode = fs::metadata(&db_path).expect("Failed to get db metadata").permissions().mode();
        assert_eq!(db_mode & 0o777, 0o600, "Database file should have 0600 permissions");

        // Test hardening of existing files
        let mut perms = fs::metadata(&config_path).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&config_path, perms).unwrap();

        config.save().expect("Failed to save config second time");
        let config_mode_after = fs::metadata(&config_path).unwrap().permissions().mode();
        assert_eq!(config_mode_after & 0o777, 0o600, "Config file should be corrected to 0600");
    }
}
