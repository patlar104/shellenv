use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;

fn fixture_config() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/configs/basic-exec.toml")
}

fn bash_available() -> bool {
    std::process::Command::new("bash")
        .args(["--noprofile", "--norc", "-c", "exit 0"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_with_fixture() -> Command {
    let mut cmd = Command::cargo_bin("shellenv").unwrap();
    cmd.args([
        "run",
        "--config",
        fixture_config().to_str().unwrap(),
        "--profile",
        "default",
        "--shell",
        "bash",
    ]);
    cmd
}

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
fn config_schema_prints_json_schema() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["config", "schema"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"$schema\""))
        .stdout(predicate::str::contains("\"title\": \"Config\""));
}

#[test]
fn committed_schema_v1_matches_command() {
    let output = Command::cargo_bin("shellenv")
        .unwrap()
        .args(["config", "schema"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let committed = fs::read_to_string("config/schema-v1.json").unwrap();
    assert_eq!(stdout, committed);
}

#[test]
fn example_config_validates() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["config", "validate", "config/config.example.toml"])
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

#[test]
fn config_validate_uses_default_path_when_omitted() {
    let home = tempfile::tempdir().unwrap();
    let config_dir = home.path().join(".config").join("shellenv");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "version = \"1\"\n[paths]\n").unwrap();

    Command::cargo_bin("shellenv")
        .unwrap()
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .args(["config", "validate"])
        .assert()
        .success()
        .stdout("OK\n");
}

#[test]
fn config_validate_accepts_path_flag() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "version = \"1\"\n[paths]\n").unwrap();

    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["config", "validate", "--path", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout("OK\n");
}

#[test]
fn config_validate_default_path_missing_reports_error() {
    let home = tempfile::tempdir().unwrap();

    Command::cargo_bin("shellenv")
        .unwrap()
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .args(["config", "validate"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"))
        .stderr(predicate::str::contains(".config"))
        .stderr(predicate::str::contains("shellenv"));
}

#[test]
fn run_echo_hello_succeeds() {
    if !bash_available() {
        return;
    }
    run_with_fixture()
        .args(["--", "echo hello"])
        .assert()
        .success()
        .stdout("hello\n")
        .stderr("");
}

#[test]
fn run_applies_profile_environment_overrides() {
    if !bash_available() {
        return;
    }
    Command::cargo_bin("shellenv")
        .unwrap()
        .args([
            "run",
            "--config",
            fixture_config().to_str().unwrap(),
            "--profile",
            "test_exec",
            "--shell",
            "bash",
            "--",
            "echo MY_VAR=$MY_VAR",
        ])
        .assert()
        .success()
        .stdout("MY_VAR=from_profile\n");
}

#[test]
fn run_propagates_false_exit_status() {
    if !bash_available() {
        return;
    }
    run_with_fixture()
        .args(["--", "false"])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn run_propagates_explicit_exit_status() {
    if !bash_available() {
        return;
    }
    run_with_fixture()
        .args(["--", "exit 7"])
        .assert()
        .failure()
        .code(7);
}

#[test]
fn run_missing_profile_is_a_cli_error() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args([
            "run",
            "--config",
            fixture_config().to_str().unwrap(),
            "--profile",
            "definitely-does-not-exist",
            "--shell",
            "bash",
            "--",
            "echo hello",
        ])
        .assert()
        .failure()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("definitely-does-not-exist"));
}

#[test]
fn run_invalid_shell_is_a_cli_error() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args([
            "run",
            "--config",
            fixture_config().to_str().unwrap(),
            "--profile",
            "default",
            "--shell",
            "definitely-not-a-shell",
            "--",
            "echo hello",
        ])
        .assert()
        .failure()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("definitely-not-a-shell"))
        .stderr(predicate::str::contains("supported shells"));
}

#[test]
fn run_requires_a_command() {
    run_with_fixture()
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "no command supplied; place the command after --",
        ));
}

#[test]
fn run_forwards_stderr() {
    if !bash_available() {
        return;
    }
    run_with_fixture()
        .args(["--", "echo out; echo err >&2"])
        .assert()
        .success()
        .stdout("out\n")
        .stderr("err\n");
}

#[test]
fn run_timeout_is_a_cli_error() {
    if !bash_available() {
        return;
    }
    run_with_fixture()
        .args(["--timeout-ms", "100", "--", "sleep 5"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("timed out"));
}

#[test]
fn run_cwd_changes_working_directory() {
    if !bash_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let output = run_with_fixture()
        .args(["--cwd", dir.path().to_str().unwrap(), "--", "pwd"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let expected = dir.path().canonicalize().unwrap();
    let actual = PathBuf::from(stdout.trim()).canonicalize().unwrap();
    assert_eq!(actual, expected);
}

fn assert_init_shell(shell_arg: &str, literal_shell: &str) {
    let version = env!("CARGO_PKG_VERSION");
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["init", shell_arg])
        .assert()
        .success()
        .stdout(predicate::str::contains("shellenv_run"))
        .stdout(predicate::str::contains("shellenv_exec_agent"))
        .stdout(predicate::str::contains(format!(
            "# Generated by shellenv {version}"
        )))
        .stdout(predicate::str::contains(format!("--shell {literal_shell}")))
        .stdout(predicate::str::contains("--timeout-ms"))
        .stdout(predicate::str::contains("30000"));
}

#[test]
fn init_bash_prints_code() {
    assert_init_shell("bash", "bash");
}

#[test]
fn init_zsh_prints_code() {
    assert_init_shell("zsh", "zsh");
}

#[test]
fn init_fish_prints_code() {
    assert_init_shell("fish", "fish");
}

#[test]
fn init_pwsh_prints_code() {
    assert_init_shell("pwsh", "pwsh");
}

#[test]
fn init_bash_case_insensitive() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["init", "BASH"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--shell bash"));
}

#[test]
fn init_unknown_shell_errors() {
    Command::cargo_bin("shellenv")
        .unwrap()
        .args(["init", "ksh"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported shell"));
}
