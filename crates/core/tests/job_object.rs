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

/// The probe in `tree` mode starts a `ping` and prints its PID. Started through `ProcessTree::spawn`, that `ping`
/// is in the probe's job from the first instant, so `kill` ends it with the probe.
#[test]
fn a_tree_started_in_its_job_ends_whole() {
    use marshell_core::job::ProcessTree;

    /// Ends exactly the `ping` with this PID, if the test left it running.
    struct EndPing(String);
    impl Drop for EndPing {
        fn drop(&mut self) {
            if ping_is_running(&self.0) {
                let _ = Command::new("taskkill").args(["/PID", &self.0, "/F"]).output();
            }
        }
    }

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_job-probe"));
    cmd.arg("tree").stdout(Stdio::piped());
    let (mut probe, tree) = ProcessTree::spawn(&mut cmd).unwrap();
    let mut lines = BufReader::new(probe.stdout.take().unwrap()).lines();
    let pid = lines.next().unwrap().unwrap();
    let _end = EndPing(pid.clone());
    assert!(ping_is_running(&pid), "the probe's child did not start");

    tree.kill(&mut probe);
    let _ = probe.wait();

    let deadline = Instant::now() + Duration::from_secs(3);
    while ping_is_running(&pid) {
        assert!(Instant::now() < deadline, "the child outlived the kill of its tree");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn job_failure_is_not_fatal() {
    let _ = marshell_core::job::tie_children_to_app();
    let _ = marshell_core::job::tie_children_to_app();
}
