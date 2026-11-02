//! `Session::spawn_elevated` against the real helper, started unelevated through `PlainLaunch`.
#![cfg(windows)]

use marshell_core::pty::elevated::PlainLaunch;
use marshell_core::pty::{FreezeError, Session, SpawnSpec};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const HOST: &str = env!("CARGO_BIN_EXE_marshell-host");

fn plain() -> PlainLaunch {
    PlainLaunch {
        env: vec![("MARSHELL_HOST_ALLOW_UNELEVATED".into(), "1".into())],
    }
}

fn spec(record: Option<PathBuf>) -> SpawnSpec {
    SpawnSpec {
        argv: ["cmd", "/d", "/q", "/k", "echo ready"].map(String::from).to_vec(),
        cwd: None,
        env: Vec::new(),
        env_remove: Vec::new(),
        env_strip: None,
        cols: 80,
        rows: 24,
        on_exit: None,
        on_mark: None,
        on_tool: None,
        record: record.map(|p| (p, None)),
        classify: None,
        initial_command: None,
    }
}

fn start(record: Option<PathBuf>) -> Arc<Session> {
    Session::spawn_elevated("t-elevated".into(), spec(record), &plain(), Path::new(HOST)).expect("spawn_elevated")
}

fn until(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ok() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("marshell-elevated-session-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// True when process `pid` has ended within `within`.
fn ended(pid: u32, within: Duration) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};
    const SYNCHRONIZE: u32 = 0x0010_0000;
    unsafe {
        let handle = OpenProcess(SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return true;
        }
        let result = WaitForSingleObject(handle, within.as_millis() as u32);
        CloseHandle(handle);
        result == WAIT_OBJECT_0
    }
}

#[test]
fn an_elevated_session_answers_the_startup_dsr_and_records() {
    let dir = scratch("dsr");
    let cast = dir.join("t.cast");
    let session = start(Some(cast.clone()));
    assert!(session.is_elevated());
    until("ready on the screen", || {
        session.peek(24).iter().any(|l| l.contains("ready"))
    });
    until("the recording to hold the output", || {
        std::fs::read_to_string(&cast).is_ok_and(|t| t.contains("ready"))
    });
    session.kill();
    until("the session to end", || session.has_exited());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn killing_an_elevated_session_ends_its_shell() {
    let session = start(None);
    let pid = session.pid().expect("the shell's pid");
    until("ready on the screen", || {
        session.peek(24).iter().any(|l| l.contains("ready"))
    });
    session.kill();
    assert!(ended(pid, Duration::from_secs(3)), "the shell {pid} outlived the kill");
    until("the session to end", || session.has_exited());
}

#[test]
fn pause_on_an_elevated_session_is_refused_with_the_copy() {
    let session = start(None);
    until("ready on the screen", || {
        session.peek(24).iter().any(|l| l.contains("ready"))
    });
    let err = session.freeze().unwrap_err();
    assert_eq!(err, FreezeError::Elevated);
    assert_eq!(
        err.to_string(),
        "Pause isn't available for a session running as administrator."
    );
    assert!(!session.is_frozen());
    session.kill();
    until("the session to end", || session.has_exited());
}
