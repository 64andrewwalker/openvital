use assert_cmd::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_init_infinite_loop_prevention() {
    let dir = TempDir::new().unwrap();

    // Simulate invalid input repeatedly
    // 10 retries allowed, so we send 15 invalid inputs
    let input = "invalid\n".repeat(15);

    let mut cmd = cargo_bin_cmd!("openvital");
    cmd.env("OPENVITAL_HOME", dir.path())
        .arg("init")
        .write_stdin(input)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Maximum retry limit exceeded"));
}
