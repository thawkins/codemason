//! Inline tests for `crash_dump.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn record_round_trips_and_reports_running() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_record(dir.path(), "running").expect("write");

    let path = record_path(dir.path());
    let content = std::fs::read_to_string(&path).expect("read");
    let record: CrashRecord = serde_json::from_str(&content).expect("parse");
    assert_eq!(record.pid, std::process::id());
    assert_eq!(record.status, "running");
    assert!(record.is_unclean());
    assert_ne!(record.core_dump_hint, "");
}

#[test]
fn clean_exit_is_not_reported_as_unclean() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_record(dir.path(), "clean exit").expect("write");

    assert!(previous_unclean_exit(dir.path()).expect("scan").is_none());
}

#[test]
fn live_pid_is_not_reported_as_unclean() {
    let dir = tempfile::tempdir().expect("tempdir");
    // The record names this very process, which is obviously still alive.
    write_record(dir.path(), "running").expect("write");

    assert!(previous_unclean_exit(dir.path()).expect("scan").is_none());
}

#[test]
fn missing_marker_reports_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(previous_unclean_exit(dir.path()).expect("scan").is_none());
}

#[test]
fn exited_pid_is_not_alive() {
    // A child that has already exited and been reaped: its pid names no live
    // process (pid reuse inside this window is not a realistic concern).
    let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .arg("--list")
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn child");
    let pid = child.id();
    child.wait().expect("child exits");
    assert!(!process_alive(pid), "an exited pid must not read as alive");
}

#[test]
fn own_pid_is_alive() {
    assert!(process_alive(std::process::id()));
}
