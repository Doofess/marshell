#![cfg(windows)]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn ping_is_running(pid: &str) -> bool {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .to_ascii_uppercase()
        .contains("PING.EXE")
}

#[test]
fn child_dies_when_app_is_killed() {
    let mut probe = Command::new(env!("CARGO_BIN_EXE_job-probe"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(probe.stdout.take().unwrap()).lines();
    assert_eq!(lines.next().unwrap().unwrap(), "ok");
    let pid = lines.next().unwrap().unwrap();
    assert!(ping_is_running(&pid), "the probe's child did not start");

    probe.kill().unwrap();
    let _ = probe.wait();

    let deadline = Instant::now() + Duration::from_secs(3);
    while ping_is_running(&pid) {
        assert!(Instant::now() < deadline, "the child outlived the killed app");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn child_outlives_a_normal_exit() {
    let mut probe = Command::new(env!("CARGO_BIN_EXE_job-probe"))
        .arg("release")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(probe.stdout.take().unwrap()).lines();
    assert_eq!(lines.next().unwrap().unwrap(), "ok");
    let pid = lines.next().unwrap().unwrap();
    assert_eq!(lines.next().unwrap().unwrap(), "released");

    probe.kill().unwrap();
    let _ = probe.wait();

    std::thread::sleep(Duration::from_secs(1));
    let survived = ping_is_running(&pid);
    let _ = Command::new("taskkill").args(["/PID", &pid, "/F"]).output();
    assert!(survived, "a child died although the app released it before exiting");
}

#[test]
fn job_failure_is_not_fatal() {
    let _ = marshell_core::job::tie_children_to_app();
    let _ = marshell_core::job::tie_children_to_app();
}
