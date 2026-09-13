use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn version_flag_prints_package_version() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn config_schema_version_prints_1() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["config", "schema-version"])
        .assert()
        .success()
        .stdout("1\n");
}

#[test]
fn config_validate_ok() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
version = "1"
[paths]
"#,
    )
    .unwrap();

    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["config", "validate", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout("OK\n");
}

#[test]
fn config_validate_reports_error() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["config", "validate", "/no/such/shellenv-config.toml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
