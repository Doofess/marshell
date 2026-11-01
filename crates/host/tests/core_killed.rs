//! The helper's shell, and what the shell started, end with the helper however the core goes away:
//! killed by PID, or closing the session. PowerShell 7 from the Microsoft Store is the hard case:
//! a packaged program started in a ConPTY from the bundled OpenConsole is not in its creator's
//! job, so the helper's own job does not reach it (live check, 2026-11-01).
#![cfg(windows)]

use marshell_core::pty::elevated::{PlainLaunch, RemotePty, connect};
use marshell_protocol::host::SpawnRequest;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const HOST: &str = env!("CARGO_BIN_EXE_marshell-host");
const PROBE_SWITCH: &str = "MARSHELL_TEST_CORE_PROBE";

/// The Store PowerShell's app execution alias, when this machine has it.
fn store_pwsh() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(r"Microsoft\WindowsApps\pwsh.exe");
    p.exists().then_some(p)
}

/// A shell that starts a child a second later; the child prints `KID=<pid>` and waits.
fn request(pwsh: &Path) -> SpawnRequest {
    let mut env: Vec<(String, String)> = std::env::vars().filter(|(k, _)| k != PROBE_SWITCH).collect();
    env.push(("MARSHELL_HOST_ALLOW_UNELEVATED".into(), "1".into()));
    SpawnRequest {
        argv: vec![
            pwsh.display().to_string(),
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-Command".into(),
            "Start-Sleep 1; pwsh -NoProfile -c 'Write-Output KID=$PID; Start-Sleep 900'".into(),
        ],
        cwd: None,
        env,
        cols: 80,
        rows: 24,
    }
}

fn plain() -> PlainLaunch {
    PlainLaunch {
        env: vec![("MARSHELL_HOST_ALLOW_UNELEVATED".into(), "1".into())],
    }
}

/// Reads the pty on a thread and sends the child's pid once it prints `KID=<pid>`.
fn watch_for_kid(mut pty_reader: Box<dyn Read + Send>) -> Receiver<u32> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut seen = String::new();
        let mut buf = [0u8; 4096];
        let mut sent = false;
        while let Ok(n) = pty_reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if sent {
                continue;
            }
            seen.push_str(&String::from_utf8_lossy(&buf[..n]));
            if let Some(at) = seen.find("KID=") {
                let digits: String = seen[at + 4..].chars().take_while(char::is_ascii_digit).collect();
                let rest = &seen[at + 4 + digits.len()..];
                if !digits.is_empty() && !rest.is_empty() {
                    sent = tx.send(digits.parse().unwrap()).is_ok();
                }
            }
        }
    });
    rx
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

fn stop(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
    // SAFETY: the handle is checked, used once and closed.
    unsafe {
        let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if !h.is_null() {
            TerminateProcess(h, 1);
            CloseHandle(h);
        }
    }
}

/// Waits up to 3 s for every pid to end; stops the ones that did not and names them.
fn all_end_within_3s(pids: &[(&str, u32)]) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while pids.iter().any(|(_, p)| !gone(*p)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let left: Vec<String> = pids
        .iter()
        .filter(|(_, p)| !gone(*p))
        .map(|(what, p)| format!("{what} {p}"))
        .collect();
    for (_, p) in pids {
        stop(*p);
    }
    if left.is_empty() { Ok(()) } else { Err(left.join(", ")) }
}

/// The probe core: runs only when `a_killed_core_ends_a_store_shell_and_its_child` starts this
/// binary with the switch. Ties itself to a job as `main` does, starts the shell through the
/// helper, prints the pids and waits to be killed.
#[test]
#[ignore = "run by a_killed_core_ends_a_store_shell_and_its_child as its probe"]
fn core_probe() {
    if std::env::var(PROBE_SWITCH).as_deref() != Ok("1") {
        return;
    }
    let pwsh = store_pwsh().expect("the parent test checked for it");
    marshell_core::job::tie_children_to_app().expect("job");
    let mut pty = connect(&plain(), Path::new(HOST), request(&pwsh)).expect("connect");
    let reader = std::mem::replace(&mut pty.reader, Box::new(std::io::empty()));
    let kid = watch_for_kid(reader)
        .recv_timeout(Duration::from_secs(30))
        .expect("the child never printed its pid");
    println!();
    println!("PROBE shell={} kid={kid}", pty.pid);
    std::thread::sleep(Duration::from_secs(600));
}

#[test]
fn a_killed_core_ends_a_store_shell_and_its_child() {
    let Some(_) = store_pwsh() else {
        eprintln!("SKIPPED: no Microsoft Store PowerShell 7 on this machine");
        return;
    };
    let mut probe = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "core_probe", "--ignored", "--nocapture", "--test-threads=1"])
        .env(PROBE_SWITCH, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = probe.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some((_, rest)) = line.split_once("PROBE shell=")
                && let Some((shell, kid)) = rest.split_once(" kid=")
            {
                let _ = tx.send((shell.trim().parse::<u32>().unwrap(), kid.trim().parse::<u32>().unwrap()));
                return;
            }
        }
    });
    let Ok((shell, kid)) = rx.recv_timeout(Duration::from_secs(60)) else {
        let _ = probe.kill();
        panic!("the probe never reported its shell");
    };
    assert!(!gone(shell) && !gone(kid), "the shell and its child should be running");

    probe.kill().unwrap();
    let _ = probe.wait();

    if let Err(left) = all_end_within_3s(&[("shell", shell), ("child", kid)]) {
        panic!("still running 3 s after the core was killed: {left}");
    }
}

#[test]
fn closing_ends_a_store_shell_and_its_child() {
    let Some(pwsh) = store_pwsh() else {
        eprintln!("SKIPPED: no Microsoft Store PowerShell 7 on this machine");
        return;
    };
    let mut pty: RemotePty = connect(&plain(), Path::new(HOST), request(&pwsh)).expect("connect");
    let reader = std::mem::replace(&mut pty.reader, Box::new(std::io::empty()));
    let kid = watch_for_kid(reader)
        .recv_timeout(Duration::from_secs(30))
        .expect("the child never printed its pid");
    let shell = pty.pid;
    assert!(!gone(shell) && !gone(kid), "the shell and its child should be running");

    pty.control.close();

    if let Err(left) = all_end_within_3s(&[("shell", shell), ("child", kid)]) {
        panic!("still running 3 s after the session was closed: {left}");
    }
}
