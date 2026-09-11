use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("box").expect("binary 'box' should exist");
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.2.0"));
}

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("box").expect("binary 'box' should exist");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Autonomous Application Packaging"))
        .stdout(predicate::str::contains("build"))
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("volume"));
}

#[test]
fn test_cli_volume_lifecycle() {
    let temp_home = tempdir().expect("Failed to create tempdir");
    let home_str = temp_home.path().to_string_lossy().to_string();

    // 1. Create volume
    let mut cmd_create = Command::cargo_bin("box").unwrap();
    cmd_create
        .env("BOX_HOME", &home_str)
        .args(["volume", "create", "test_cli_vol"])
        .assert()
        .success();

    // 2. List volume
    let mut cmd_list = Command::cargo_bin("box").unwrap();
    cmd_list
        .env("BOX_HOME", &home_str)
        .args(["volume", "ls"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test_cli_vol"));

    // 3. Remove volume
    let mut cmd_rm = Command::cargo_bin("box").unwrap();
    cmd_rm
        .env("BOX_HOME", &home_str)
        .args(["volume", "rm", "test_cli_vol"])
        .assert()
        .success();
}

#[test]
fn test_cli_build_and_info_e2e() {
    let temp_proj = tempdir().expect("Failed to create temp project");
    let proj_dir = temp_proj.path();

    // Create minimal python app
    let boxconfig_content = r#"
name: cli-test-app
version: 1.0.0
description: CLI E2E test project
category: UTILITIES
protect: false
entrypoint: app.py
runtime:
  type: python
  version: "3.12"
"#;
    fs::write(proj_dir.join("boxconfig.yml"), boxconfig_content).unwrap();
    fs::write(proj_dir.join("app.py"), "print('cli test')\n").unwrap();

    // Run box build inside the project directory
    let mut cmd_build = Command::cargo_bin("box").unwrap();
    cmd_build
        .current_dir(proj_dir)
        .arg("build")
        .assert()
        .success();

    let output_box = proj_dir.join("cli-test-app.box");
    assert!(output_box.exists(), "Box output archive must exist");

    // Run box info --json
    let mut cmd_info = Command::cargo_bin("box").unwrap();
    cmd_info
        .args(["info", output_box.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cli-test-app"))
        .stdout(predicate::str::contains("payload_sha256"))
        .stdout(predicate::str::contains("Plain Source"));
}
