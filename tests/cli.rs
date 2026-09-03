use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_process-diagnostics"))
}

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "process-diagnostics-cli-{label}-{}",
        std::process::id()
    ))
}

#[test]
fn version_and_help_are_available() {
    let version = Command::new(binary()).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("process-diagnostics"));
    let help = Command::new(binary()).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("replay"));
}

#[test]
fn run_json_records_exact_command() {
    let output = Command::new(binary())
        .args(["run", "--", "/bin/sh", "-c", "printf cli"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["exit"]["outcome"], "success");
    assert_eq!(report["spec"]["program"], "/bin/sh");
    assert_eq!(report["spec"]["args"][0], "-c");
    assert_eq!(report["stdout"], "Y2xp");
}

#[test]
fn run_nonzero_returns_one_and_text_explains() {
    let output = Command::new(binary())
        .args(["run", "--format", "text", "--", "/bin/sh", "-c", "exit 9"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("outcome: nonzero"));
}

#[test]
fn replay_reads_transcript_without_running_command() {
    let path = temp_path("replay.json");
    let output = Command::new(binary())
        .args(["run", "--", "/bin/sh", "-c", "printf replay"])
        .output()
        .unwrap();
    assert!(output.status.success());
    fs::write(&path, &output.stdout).unwrap();
    let replay = Command::new(binary())
        .args(["replay", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(replay.status.success());
    assert!(String::from_utf8_lossy(&replay.stdout).contains("outcome: success"));
    fs::remove_file(path).unwrap();
}

#[test]
fn explicit_environment_and_timeout_options_work() {
    let output = Command::new(binary())
        .args([
            "run",
            "--clear-env",
            "--env",
            "PD_CLI=seen",
            "--",
            "/bin/sh",
            "-c",
            "printf %s \"$PD_CLI\"",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["stdout"], "c2Vlbg==");

    let timeout = Command::new(binary())
        .args([
            "run",
            "--timeout-ms",
            "20",
            "--",
            "/bin/sh",
            "-c",
            "sleep 1",
        ])
        .output()
        .unwrap();
    assert_eq!(timeout.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&timeout.stdout).unwrap();
    assert_eq!(report["exit"]["outcome"], "timeout");
}
