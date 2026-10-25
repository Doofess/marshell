mod common;

use marshell_core::pty::{FreezeError, MAX_UNACKED, Mark, Pull, Session, SpawnSpec};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn fake(args: &[&str]) -> SpawnSpec {
    common::tie_children_to_test();
    let mut argv = vec![env!("CARGO_BIN_EXE_fake-agent").to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    SpawnSpec {
        argv,
        cwd: None,
        env: vec![],
        env_remove: vec![],
        env_strip: None,
        cols: 80,
        rows: 24,
        on_exit: None,
        on_mark: None,
        on_tool: None,
        record: None,
        classify: None,
        initial_command: None,
    }
}

#[cfg(windows)]
#[test]
fn sessions_die_when_the_test_process_exits() {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
    };
    const PROBE: &str = "MARSHELL_TEST_CHILD_PID_FILE";
    if let Some(path) = std::env::var_os(PROBE) {
        let _session = Session::spawn("test-exit".into(), fake(&["sleep"])).unwrap();
        // A child outside ConPTY also has to die; closing a console alone cannot protect it.
        #[allow(clippy::zombie_processes)] // The parent verifies job cleanup after this process exits.
        let child = std::process::Command::new(env!("CARGO_BIN_EXE_fake-agent"))
            .arg("sleep")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let path = std::path::PathBuf::from(path);
        std::fs::write(&path, child.id().to_string()).unwrap();
        assert!(wait_until(Duration::from_secs(10), || path
            .with_extension("ready")
            .exists()));
        if std::env::var_os("MARSHELL_TEST_CHILD_PANIC").is_some() {
            panic!("deliberate test panic");
        }
        return;
    }

    for panics in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args(["--exact", "sessions_die_when_the_test_process_exits"])
            .env(PROBE, &path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if panics {
            child.env("MARSHELL_TEST_CHILD_PANIC", "1");
        }
        let child = child.spawn().unwrap();
        let mut pid = None;
        assert!(wait_until(Duration::from_secs(10), || {
            pid = std::fs::read_to_string(&path).ok().and_then(|s| s.parse::<u32>().ok());
            pid.is_some()
        }));
        let pid = pid.unwrap();
        // SAFETY: this PID belongs to the child our probe just started; the handle is checked.
        let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_TERMINATE, 0, pid) };
        assert!(
            !process.is_null(),
            "could not open probe child: {}",
            std::io::Error::last_os_error()
        );
        std::fs::write(path.with_extension("ready"), b"").unwrap();
        let output = child.wait_with_output().unwrap();
        // SAFETY: the handle stays open through the wait and cleanup, then is closed exactly once.
        let gone = unsafe {
            let gone = WaitForSingleObject(process, 5000) == WAIT_OBJECT_0;
            if !gone {
                TerminateProcess(process, 1);
                WaitForSingleObject(process, 5000);
            }
            CloseHandle(process);
            gone
        };
        assert_eq!(output.status.success(), !panics, "{output:?}");
        assert!(gone, "fake-agent outlived its test process (panics={panics})");
    }
}

/// Reads everything until Exit (or the timeout), acking as it goes. Returns (bytes, exit code).
fn drain(s: &Arc<Session>, timeout: Duration) -> (Vec<u8>, Option<i32>) {
    let (generation, _) = s.attach();
    s.resume(0);
    let deadline = Instant::now() + timeout;
    let (mut cursor, mut all) = (0u64, Vec::new());
    while Instant::now() < deadline {
        match s.pull(cursor, 64 * 1024) {
            Pull::Data { seq, bytes } => {
                cursor = seq + bytes.len() as u64;
                all.extend(bytes);
                s.ack(cursor);
            }
            Pull::Reset { seq, screen } => {
                cursor = seq;
                all.extend(screen);
                s.ack(cursor);
            }
            Pull::Exit(code) => {
                s.detach(generation);
                return (all, Some(code));
            }
            Pull::Idle => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    s.detach(generation);
    (all, None)
}

#[test]
fn prints_and_exits_zero() {
    let s = Session::spawn("t1".into(), fake(&["print", "hello marshell"])).unwrap();
    let (out, code) = drain(&s, Duration::from_secs(10));
    assert!(String::from_utf8_lossy(&out).contains("hello marshell"));
    assert_eq!(code, Some(0));
}

#[test]
fn reports_nonzero_exit() {
    let s = Session::spawn("t2".into(), fake(&["exit", "3"])).unwrap();
    assert_eq!(drain(&s, Duration::from_secs(10)).1, Some(3));
}

#[test]
fn input_reaches_the_child() {
    let s = Session::spawn("t3".into(), fake(&["echo"])).unwrap();
    s.write_input(b"ping\r".to_vec());
    s.write_input(b"exit\r".to_vec());
    let (out, code) = drain(&s, Duration::from_secs(10));
    assert!(String::from_utf8_lossy(&out).contains("got:ping"));
    assert_eq!(code, Some(0));
}

#[test]
fn backpressure_bounds_unacked_output() {
    let s = Session::spawn("t4".into(), fake(&["flood", "8000000"])).unwrap();
    let (generation, _) = s.attach();
    s.resume(0);
    // Never ack. First prove output flowed, so a slow machine cannot pass vacuously.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (end, acked) = s.counters();
        if end - acked >= MAX_UNACKED / 2 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "output never reached {} bytes",
            MAX_UNACKED / 2
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(500));
    let (end, acked) = s.counters();
    assert!(
        end - acked <= MAX_UNACKED + 64 * 1024,
        "unacked {} exceeds the limit",
        end - acked
    );
    s.detach(generation);
    s.kill();
}

/// S6 raw ConPTY baseline: the same flood as `s6_throughput_50mb`, through `Session` only (no WebSocket).
/// Run with: cargo test -p marshell-core --release --test session -- --ignored --nocapture s6_raw_conpty_50mb
#[test]
#[ignore]
fn s6_raw_conpty_50mb() {
    let s = Session::spawn("s6".into(), fake(&["flood", "52428800"])).unwrap();
    let started = Instant::now();
    let (out, code) = drain(&s, Duration::from_secs(120));
    let secs = started.elapsed().as_secs_f64();
    println!(
        "S6 raw: {} bytes in {secs:.2}s = {:.1} MB/s, exit {:?}",
        out.len(),
        out.len() as f64 / secs / 1e6,
        code
    );
    assert_eq!(code, Some(0));
    assert!(out.len() >= 50 * 1024 * 1024 - 1024);
}

#[test]
fn kill_after_exit_is_a_noop() {
    let s = Session::spawn("t7".into(), fake(&["exit", "0"])).unwrap();
    assert_eq!(drain(&s, Duration::from_secs(10)).1, Some(0));
    s.kill();
    s.force_kill();
}

#[test]
fn zero_resize_is_ignored() {
    let s = Session::spawn("t5".into(), fake(&["sleep"])).unwrap();
    s.resize(0, 0).unwrap();
    s.resize(100, 30).unwrap();
    s.kill();
}

#[test]
fn kill_takes_the_tree() {
    let s = Session::spawn("t6".into(), fake(&["spawn-child"])).unwrap();
    let (generation, _) = s.attach();
    s.resume(0);
    let deadline = Instant::now() + Duration::from_secs(10);
    let (mut cursor, mut text) = (0u64, String::new());
    let child_pid: u32 = loop {
        assert!(Instant::now() < deadline, "no child pid printed");
        if let Pull::Data { seq, bytes } = s.pull(cursor, 65536) {
            cursor = seq + bytes.len() as u64;
            text.push_str(&String::from_utf8_lossy(&bytes));
            s.ack(cursor);
            if let Some(rest) = text.split("child:").nth(1) {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty() && rest.len() > digits.len() {
                    break digits.parse().unwrap();
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    s.detach(generation);
    s.kill();
    std::thread::sleep(Duration::from_millis(2500));
    assert!(!process_alive(child_pid), "grandchild {child_pid} survived kill");
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    let out = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[test]
fn on_exit_callback_gets_the_code() {
    common::tie_children_to_test();
    let argv: &[&str] = if cfg!(windows) {
        &["cmd", "/c", "exit 3"]
    } else {
        &["sh", "-c", "exit 3"]
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let spec = SpawnSpec {
        argv: argv.iter().map(|s| s.to_string()).collect(),
        cwd: None,
        env: vec![],
        env_remove: vec![],
        env_strip: None,
        cols: 80,
        rows: 24,
        on_exit: Some(Box::new(move |code| {
            let _ = tx.send(code);
        })),
        on_mark: None,
        on_tool: None,
        record: None,
        classify: None,
        initial_command: None,
    };
    let _session = Session::spawn("t-exit-cb".into(), spec).unwrap();
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(3));
}

#[test]
fn marks_reach_the_callback() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut spec = fake(&["osc133"]);
    spec.on_mark = Some(Box::new(move |mark| {
        let _ = tx.send(mark);
    }));
    let _session = Session::spawn("t-marks".into(), spec).unwrap();
    let mut got = Vec::new();
    while let Ok(mark) = rx.recv_timeout(Duration::from_secs(10)) {
        got.push(mark);
        if got.len() == 3 {
            break;
        }
    }
    assert_eq!(got, [Mark::PromptStart, Mark::Executed, Mark::Finished(Some(7))]);
}

#[test]
fn command_line_is_tracked_from_the_shell_marks() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut spec = fake(&["cmdline", "docker compose up"]);
    spec.on_mark = Some(Box::new(move |mark| {
        let _ = tx.send(mark);
    }));
    let session = Session::spawn("t-cmdline".into(), spec).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && !(session.command_running() && session.command_line().is_some()) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(session.command_running());
    assert!(session.ever_executed());
    assert_eq!(session.command_line().as_deref(), Some("docker compose up"));
    // The session updates its tracking before the callback sees the mark.
    while let Ok(mark) = rx.recv_timeout(Duration::from_secs(10)) {
        if matches!(mark, Mark::Finished(_)) {
            break;
        }
    }
    assert!(!session.command_running());
    assert!(session.ever_executed());
    assert_eq!(session.command_line(), None);
}

#[test]
fn the_command_epoch_moves_on_every_c_d_and_a_mark() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut spec = fake(&["cmdline", "git status"]);
    spec.on_mark = Some(Box::new(move |mark| {
        let _ = tx.send(mark);
    }));
    let session = Session::spawn("t-epoch".into(), spec).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && !session.command_running() {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(session.command_running());
    // A reading taken now is stale once the D mark arrives.
    let during = session.command_epoch();
    assert!(during > 0, "the C mark did not move the epoch");
    // The session updates its tracking before the callback sees the mark.
    while let Ok(mark) = rx.recv_timeout(Duration::from_secs(10)) {
        if matches!(mark, Mark::Finished(_)) {
            break;
        }
    }
    assert!(!session.command_running());
    assert_ne!(session.command_epoch(), during, "the D mark did not move the epoch");
}

#[test]
fn command_line_keeps_only_the_first_line_capped_at_200_chars() {
    let text = format!("{}\\nsecond", "x".repeat(300));
    let session = Session::spawn("t-cmdline-cap".into(), fake(&["cmdline", &text])).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && session.command_line().is_none() {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(session.command_line(), Some("x".repeat(200)));
}

/// Polls the recording until `want` shows up: the writer thread flushes about once a second.
fn wait_for_recording(path: &std::path::Path, want: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if text.contains(want) || Instant::now() > deadline {
            return text;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn session_records_its_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t-1.cast");
    let mut spec = fake(&["print", "hello"]);
    spec.record = Some((path.clone(), None));
    let s = Session::spawn("t-rec".into(), spec).unwrap();
    assert_eq!(s.recording_path(), Some(path.clone()));
    assert_eq!(drain(&s, Duration::from_secs(10)).1, Some(0));
    let text = wait_for_recording(&path, "hello");
    let hello = text.lines().skip(1).any(|line| {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        v[1] == "o" && v[2].as_str().unwrap().contains("hello")
    });
    assert!(hello, "{text}");
}

#[test]
fn session_records_prompt_marks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t-2.cast");
    let mut spec = fake(&["osc133"]);
    spec.record = Some((path.clone(), None));
    let s = Session::spawn("t-rec-marks".into(), spec).unwrap();
    drain(&s, Duration::from_secs(10));
    let text = wait_for_recording(&path, "\"m\",\"prompt\"");
    assert!(text.contains("\"m\",\"prompt\""), "{text}");
}

#[test]
fn session_without_a_recordable_path_still_runs() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("plain-file");
    std::fs::write(&file, b"x").unwrap();
    let mut spec = fake(&["print", "still here"]);
    spec.record = Some((file.join("under").join("t-3.cast"), None));
    let s = Session::spawn("t-rec-bad".into(), spec).unwrap();
    assert_eq!(s.recording_path(), None);
    let (out, code) = drain(&s, Duration::from_secs(10));
    assert!(String::from_utf8_lossy(&out).contains("still here"));
    assert_eq!(code, Some(0));
}

fn printer(nested: bool) -> SpawnSpec {
    common::tie_children_to_test();
    let argv: Vec<String> = match (cfg!(windows), nested) {
        (true, false) => vec!["ping".into(), "-t".into(), "127.0.0.1".into()],
        (true, true) => vec!["cmd".into(), "/c".into(), "cmd /c ping -t 127.0.0.1".into()],
        (false, false) => vec!["sh".into(), "-c".into(), "while :; do echo x; sleep 0.1; done".into()],
        (false, true) => vec![
            "sh".into(),
            "-c".into(),
            "sh -c 'while :; do echo x; sleep 0.1; done'".into(),
        ],
    };
    SpawnSpec {
        argv,
        cwd: None,
        env: vec![],
        env_remove: vec![],
        env_strip: None,
        cols: 80,
        rows: 24,
        on_exit: None,
        on_mark: None,
        on_tool: None,
        record: None,
        classify: None,
        initial_command: None,
    }
}

fn wait_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    done()
}

fn freeze_then_thaw_a_printer(nested: bool) {
    let s = Session::spawn("t-freeze".into(), printer(nested)).unwrap();
    assert!(wait_until(Duration::from_secs(10), || s.counters().0 > 0), "no output");
    s.freeze().unwrap();
    assert!(s.is_frozen());
    std::thread::sleep(Duration::from_millis(300));
    let frozen_at = s.counters().0;
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(s.counters().0, frozen_at, "output grew while frozen");
    s.thaw();
    assert!(!s.is_frozen());
    assert!(
        wait_until(Duration::from_secs(3), || s.counters().0 > frozen_at),
        "no output after thaw"
    );
    s.force_kill();
}

#[test]
fn freeze_stops_output() {
    freeze_then_thaw_a_printer(false);
}

#[test]
fn freeze_stops_a_grandchild() {
    freeze_then_thaw_a_printer(true);
}

#[test]
fn kill_ends_a_frozen_session() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut spec = printer(false);
    spec.on_exit = Some(Box::new(move |code| {
        let _ = tx.send(code);
    }));
    let s = Session::spawn("t-freeze-kill".into(), spec).unwrap();
    assert!(wait_until(Duration::from_secs(10), || s.counters().0 > 0), "no output");
    s.freeze().unwrap();
    s.kill();
    assert!(
        rx.recv_timeout(Duration::from_secs(5)).is_ok(),
        "frozen session did not end"
    );
}

#[test]
fn freeze_after_exit_is_refused() {
    let s = Session::spawn("t-freeze-exited".into(), fake(&["exit", "0"])).unwrap();
    assert!(wait_until(Duration::from_secs(10), || s.has_exited()));
    assert_eq!(s.freeze(), Err(FreezeError::NotRunning));
    assert!(!s.is_frozen());
}

/// Windows only: killing just the root of a frozen tree must not leave its descendants suspended.
#[cfg(windows)]
#[test]
fn a_root_that_exits_while_paused_does_not_leave_descendants_suspended() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.txt");
    let mut spec = printer(true);
    // A payload size no other test uses, so the grandchild can be found by its command line.
    // `start` gives the grandchild a console of its own, so closing the session's pseudoconsole
    // does not end it; only a resume can unfreeze it. The root itself waits on a second ping.
    std::fs::write(dir.path().join("inner.cmd"), "ping -t -l 1234 127.0.0.1 > out.txt\r\n").unwrap();
    spec.argv = vec![
        "cmd".into(),
        "/c".into(),
        "start /min inner.cmd & ping -n 1000 -l 1235 127.0.0.1 > nul".into(),
    ];
    spec.cwd = Some(dir.path().to_path_buf());
    let s = Session::spawn("t-freeze-root-exit".into(), spec).unwrap();
    let size = || std::fs::metadata(&out).map_or(0, |m| m.len());
    assert!(
        wait_until(Duration::from_secs(10), || size() > 0),
        "grandchild wrote nothing"
    );
    s.freeze().unwrap();
    let root = s.pid().unwrap();
    // Kill the root only, not its tree.
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &root.to_string(), "/F"])
        .output()
        .unwrap();
    assert!(status.status.success(), "taskkill failed");
    assert!(
        wait_until(Duration::from_secs(10), || s.has_exited()),
        "root did not exit"
    );
    let at_exit = size();
    // Closing the pseudoconsole may end the grandchild, which is fine. What must not happen is
    // that it lingers suspended: it either keeps writing or is gone.
    let find = |pat: &str| {
        format!(
            "Get-CimInstance Win32_Process -Filter \"Name='ping.exe'\" | Where-Object {{ $_.CommandLine -like '{pat}' }}"
        )
    };
    let alive = || {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("@({}).Count", find("*-l 1234*"))])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim() != "0"
    };
    let settled = wait_until(Duration::from_secs(8), || size() > at_exit || !alive());
    // Clean up the grandchild whatever happened.
    let _ = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "{} | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}",
                find("*-l 123*")
            ),
        ])
        .output();
    assert!(settled, "the grandchild lingered suspended after the root exited");
}

/// Polls `foreground_chain` until it equals `want` or 3 s pass. Returns the last chain read.
#[cfg(windows)]
fn chain_settles(s: &Session, want: &[&str]) -> Option<Vec<String>> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let chain = s.foreground_chain();
        if chain.as_deref().is_some_and(|c| c == want) || Instant::now() >= deadline {
            return chain;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(windows)]
#[test]
fn foreground_chain_names_the_running_child() {
    let mut spec = fake(&[]);
    spec.argv = vec![
        "cmd.exe".into(),
        "/d".into(),
        "/c".into(),
        format!("{} sleep", env!("CARGO_BIN_EXE_fake-agent")),
    ];
    let s = Session::spawn("t-chain".into(), spec).unwrap();
    assert_eq!(chain_settles(&s, &["fake-agent"]), Some(vec!["fake-agent".to_string()]));
    s.kill();
    std::thread::sleep(Duration::from_millis(500));
    assert!(s.foreground_chain().is_none_or(|c| c.is_empty()));
}

#[cfg(windows)]
#[test]
fn a_renamed_agent_binary_is_found() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_fake-agent"),
        dir.path().join("codex-marshell-test.exe"),
    )
    .unwrap();
    let mut spec = fake(&[]);
    spec.argv = vec![
        "cmd.exe".into(),
        "/d".into(),
        "/c".into(),
        // A path, not a bare name: with NoDefaultCurrentDirectoryInExePath set, cmd would find a
        // real `codex` on PATH instead. The name is one no real program uses, so nothing else is hit.
        r".\codex-marshell-test.exe sleep".into(),
    ];
    spec.cwd = Some(dir.path().to_path_buf());
    let s = Session::spawn("t-renamed".into(), spec).unwrap();
    let chain = chain_settles(&s, &["codex-marshell-test"]).unwrap_or_default();
    s.kill();
    assert_eq!(
        marshell_core::profiles::detect::agent_in(&marshell_core::agents::manifest::Agents::shipped(), &chain, None)
            .as_deref(),
        Some("codex"),
        "chain was {chain:?}"
    );
    std::thread::sleep(Duration::from_millis(500));
    std::fs::remove_dir_all(dir.path()).ok();
}

/// What `set` (or `env`) prints for a session spawned from `spec`.
fn environment_of(id: &str, mut spec: SpawnSpec) -> String {
    spec.argv = if cfg!(windows) {
        vec!["cmd".into(), "/d".into(), "/c".into(), "set".into()]
    } else {
        vec!["env".into()]
    };
    let s = Session::spawn(id.into(), spec).unwrap();
    let (bytes, _) = drain(&s, Duration::from_secs(10));
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn env_remove_drops_an_inherited_variable() {
    // Names no other test uses, so setting the process environment cannot race one.
    unsafe {
        std::env::set_var("MARSHELL_TEST_INHERITED_GONE", "1");
        std::env::set_var("MARSHELL_TEST_INHERITED_KEPT", "1");
    }
    let mut spec = fake(&[]);
    spec.env_remove = vec!["MARSHELL_TEST_INHERITED_GONE".into()];
    let out = environment_of("t-env-remove", spec);
    assert!(!out.contains("MARSHELL_TEST_INHERITED_GONE="), "{out}");
    assert!(out.contains("MARSHELL_TEST_INHERITED_KEPT=1"), "{out}");
}

#[test]
fn a_name_in_env_and_env_remove_keeps_the_env_value() {
    unsafe { std::env::set_var("MARSHELL_TEST_INHERITED_BOTH", "inherited") };
    let mut spec = fake(&[]);
    spec.env = vec![("MARSHELL_TEST_INHERITED_BOTH".into(), "on-purpose".into())];
    spec.env_remove = vec!["MARSHELL_TEST_INHERITED_BOTH".into()];
    let out = environment_of("t-env-both", spec);
    assert!(out.contains("MARSHELL_TEST_INHERITED_BOTH=on-purpose"), "{out}");
}

fn classify_with_shipped_tools() -> marshell_core::pty::Classifier {
    let tools = marshell_core::tools::Tools::shipped();
    Arc::new(move |line| marshell_core::tools::watch_for(&tools, line))
}

#[test]
fn a_secret_command_hides_its_output_from_the_recording() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t-secret.cast");
    let mut spec = fake(&["secret-run", "gh auth token", "gho_SECRET"]);
    spec.record = Some((path.clone(), None));
    spec.classify = Some(classify_with_shipped_tools());
    let s = Session::spawn("t-rec-secret".into(), spec).unwrap();
    let (out, code) = drain(&s, Duration::from_secs(10));
    assert_eq!(code, Some(0));
    // The terminal gets the output; the recording does not.
    assert!(String::from_utf8_lossy(&out).contains("gho_SECRET"));
    let text = wait_for_recording(&path, "Output recorded again");
    assert!(text.contains("Output hidden: gh auth token … prints secrets"), "{text}");
    assert!(text.contains("Output recorded again"), "{text}");
    assert!(!text.contains("gho_SECRET"), "{text}");
    assert!(!text.contains("]633;"), "{text}");
    let text = wait_for_recording(&path, "visible after");
    assert!(text.contains("visible after"), "{text}");
    assert_eq!(text.matches("Output hidden").count(), 1, "{text}");
    assert_eq!(text.matches("Output recorded again").count(), 1, "{text}");
}

#[test]
fn a_profiled_tools_prompt_reaches_on_tool_through_a_pty() {
    use marshell_core::tools::ToolEvent;
    let (tx, rx) = std::sync::mpsc::channel();
    let mut spec = fake(&["tool-prompt", "vercel login", "Waiting for authentication..."]);
    spec.classify = Some(classify_with_shipped_tools());
    spec.on_tool = Some(Box::new(move |event| {
        let _ = tx.send(event);
    }));
    let s = Session::spawn("t-tool-prompt".into(), spec).unwrap();
    let first = rx.recv_timeout(Duration::from_secs(10)).expect("a needs-you event");
    assert_eq!(first, ToolEvent::NeedsYou("Waiting for sign-in in the browser".into()));
    // Three rows of other text push the prompt out of the matcher's tail; that, and not the D mark, ends it.
    let second = rx.recv_timeout(Duration::from_secs(10)).expect("a cleared event");
    assert_eq!(second, ToolEvent::Cleared);
    assert_eq!(drain(&s, Duration::from_secs(10)).1, Some(0));
    assert!(rx.try_recv().is_err(), "an event came after the prompt cleared");
}

#[test]
fn a_secret_flag_in_the_command_line_never_reaches_the_cast_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t-flag.cast");
    let mut spec = fake(&["secret-run", "docker login -u me -p hunter2", "Login Succeeded"]);
    spec.record = Some((path.clone(), None));
    spec.classify = Some(classify_with_shipped_tools());
    let s = Session::spawn("t-rec-flag".into(), spec).unwrap();
    assert_eq!(drain(&s, Duration::from_secs(10)).1, Some(0));
    let text = wait_for_recording(&path, "visible after");
    assert!(text.contains("Login Succeeded"), "{text}");
    assert!(!text.contains("hunter2"), "{text}");
    assert!(!text.contains("]633;"), "{text}");
    assert!(!text.contains("Output hidden"), "{text}");
}
