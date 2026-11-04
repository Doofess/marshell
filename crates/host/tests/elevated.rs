//! The core's `connect` against the real helper, started unelevated through `PlainLaunch`.
#![cfg(windows)]

use marshell_core::pty::dsr::DSR_REPLY;
use marshell_core::pty::elevated::{ElevatedError, HostLauncher, LaunchedHost, PlainLaunch, RemotePty, connect};
use marshell_protocol::host::SpawnRequest;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const HOST: &str = env!("CARGO_BIN_EXE_marshell-host");

fn plain() -> PlainLaunch {
    PlainLaunch {
        env: vec![("MARSHELL_HOST_ALLOW_UNELEVATED".into(), "1".into())],
    }
}

fn request(argv: &[&str], extra_env: &[(&str, &str)]) -> SpawnRequest {
    let mut env: Vec<(String, String)> = std::env::vars().collect();
    env.extend(extra_env.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    SpawnRequest {
        argv: argv.iter().map(|s| s.to_string()).collect(),
        cwd: None,
        env,
        cols: 80,
        rows: 24,
    }
}

/// What the test needs after it has played the core: the output, a writer, the rest of the pty.
struct Played {
    output: Receiver<Vec<u8>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pty: RemotePty,
}

/// Plays the core: reads the output on a thread and answers ESC[6n the way `Session` does, with
/// no other input.
fn play_core(mut pty: RemotePty) -> Played {
    let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(std::mem::replace(
        &mut pty.writer,
        Box::new(std::io::sink()),
    )));
    let mut reader = std::mem::replace(&mut pty.reader, Box::new(std::io::empty()));
    let (tx, output) = mpsc::channel();
    let answer = Arc::downgrade(&writer);
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut answered = false;
        let mut seen = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    seen.extend_from_slice(&buf[..n]);
                    if !answered && seen.windows(4).any(|w| w == b"\x1b[6n") {
                        answered = true;
                        if let Some(answer) = answer.upgrade() {
                            let mut w = answer.lock().unwrap();
                            let _ = w.write_all(DSR_REPLY).and_then(|()| w.flush());
                        }
                    }
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    Played { output, writer, pty }
}

/// Reads output until it contains `needle` or the time is up; returns everything seen.
fn wait_for(rx: &Receiver<Vec<u8>>, needle: &str, within: Duration) -> Result<String, String> {
    let deadline = Instant::now() + within;
    let mut seen = String::new();
    loop {
        if seen.contains(needle) {
            return Ok(seen);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(b) => seen.push_str(&String::from_utf8_lossy(&b)),
            Err(_) => return Err(seen),
        }
    }
}

fn start(argv: &[&str], extra_env: &[(&str, &str)]) -> Played {
    let pty =
        connect(&plain(), Path::new(HOST), request(argv, extra_env)).unwrap_or_else(|e| panic!("connect failed: {e}"));
    play_core(pty)
}

#[test]
fn first_prompt_renders_with_no_input() {
    let p = start(&["cmd.exe", "/d", "/q", "/k", "echo ready"], &[]);
    let seen = wait_for(&p.output, "ready", Duration::from_secs(5));
    assert!(seen.is_ok(), "no first prompt; saw {:?}", seen.err());
}

#[test]
fn the_shell_sees_the_core_computed_environment() {
    let p = start(
        &["cmd", "/d", "/c", "echo %MARSHELL_TAB_ID% %ZZ_PROFILE%"],
        &[("MARSHELL_TAB_ID", "t-elev"), ("ZZ_PROFILE", "1")],
    );
    let seen = wait_for(&p.output, "t-elev 1", Duration::from_secs(5));
    assert!(seen.is_ok(), "env missing; saw {:?}", seen.err());
}

#[test]
fn input_and_resize_reach_the_shell() {
    let p = start(&["cmd.exe", "/d", "/q", "/k", "echo ready"], &[]);
    wait_for(&p.output, "ready", Duration::from_secs(5)).expect("prompt");
    p.pty.control.resize(100, 40).expect("resize");
    {
        let mut w = p.writer.lock().unwrap();
        w.write_all(b"set /a 6*7\r").unwrap();
        w.write_all(b"for /f \"tokens=2\" %a in ('mode con ^| findstr Columns') do @echo W=%a\r")
            .unwrap();
        w.flush().unwrap();
    }
    let seen = wait_for(&p.output, "W=100", Duration::from_secs(8));
    let seen = seen.unwrap_or_else(|s| panic!("no resize; saw {s:?}"));
    assert!(seen.contains("42"), "input did not arrive; saw {seen:?}");
}

fn gone(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: the handle is checked, used for one query and closed.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return true;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        ok != 0 && code != 259
    }
}

#[test]
fn dropping_the_core_side_ends_the_shell() {
    let p = start(&["cmd.exe", "/d", "/q", "/k", "echo ready"], &[]);
    wait_for(&p.output, "ready", Duration::from_secs(5)).expect("prompt");
    let pid = p.pty.pid;
    assert!(!gone(pid), "the shell should be running");
    drop(p);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !gone(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(gone(pid), "the shell outlived the core side");
}

#[test]
fn closing_the_control_ends_the_shell_and_wait_returns() {
    let p = start(&["cmd.exe", "/d", "/q", "/k", "echo ready"], &[]);
    wait_for(&p.output, "ready", Duration::from_secs(5)).expect("prompt");
    let pid = p.pty.pid;
    p.pty.control.close();
    let wait = p.pty.wait;
    let started = Instant::now();
    let _ = wait();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(gone(pid));
}

#[test]
fn a_shell_that_ends_reports_its_exit_code() {
    let p = start(&["cmd.exe", "/d", "/c", "exit 7"], &[]);
    assert_eq!((p.pty.wait)(), 7);
}

/// Starts the real helper with arguments it refuses.
struct BadArgs;

impl HostLauncher for BadArgs {
    fn launch(&self, exe: &Path, _args: &str) -> Result<LaunchedHost, ElevatedError> {
        plain().launch(exe, "--bogus")
    }
}

#[test]
fn a_helper_that_exits_before_connecting_fails_fast() {
    let started = Instant::now();
    let err = connect(&BadArgs, Path::new(HOST), request(&["cmd.exe"], &[]))
        .err()
        .expect("an error");
    assert!(matches!(err, ElevatedError::ExitedEarly(2)), "got {err:?}");
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_missing_cwd_comes_back_as_the_helpers_words() {
    let mut req = request(&["cmd.exe"], &[]);
    req.cwd = Some(r"Z:\no-such-marshell-folder".into());
    let err = connect(&plain(), Path::new(HOST), req).err().expect("an error");
    match err {
        ElevatedError::Failed(t) => assert!(t.starts_with("The folder Z:"), "got {t}"),
        other => panic!("got {other:?}"),
    }
}

/// The helper's own token check, so this test knows which side of it it runs on.
#[allow(dead_code)]
#[path = "../src/win.rs"]
mod win;

#[test]
fn the_helper_refuses_unelevated_without_the_switch() {
    let status = std::process::Command::new(HOST)
        .args(["--pipe", "marshell-host-none", "--core-pid", "1"])
        .env_remove("MARSHELL_HOST_ALLOW_UNELEVATED")
        .status()
        .expect("the helper starts");
    if win::is_elevated() {
        // CI's Windows runners run elevated: the check passes, and the helper fails later, at the pipe.
        assert_ne!(status.code(), Some(3));
    } else {
        assert_eq!(status.code(), Some(3));
    }
}

#[test]
fn the_helper_drops_variables_the_request_does_not_carry() {
    // `request()` copies this process's variables, so the env is built here without the helper's switch.
    let req = SpawnRequest {
        argv: ["cmd", "/d", "/c", "echo [%MARSHELL_HOST_ALLOW_UNELEVATED%] [%ZZ_KEPT%]"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        cwd: None,
        env: std::env::vars()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("MARSHELL_HOST_ALLOW_UNELEVATED"))
            .chain([("ZZ_KEPT".to_string(), "1".to_string())])
            .collect(),
        cols: 80,
        rows: 24,
    };
    let pty = connect(&plain(), Path::new(HOST), req).unwrap_or_else(|e| panic!("connect failed: {e}"));
    let p = play_core(pty);
    let seen = wait_for(
        &p.output,
        "[%MARSHELL_HOST_ALLOW_UNELEVATED%] [1]",
        Duration::from_secs(5),
    );
    assert!(seen.is_ok(), "the helper's own variable leaked; saw {:?}", seen.err());
}
