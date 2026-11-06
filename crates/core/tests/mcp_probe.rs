//! The MCP probe against `fake-agent mcp <mode>`: what it lists, how each misbehaving server ends, and that the
//! whole process tree is gone afterwards. No real MCP server or CLI is started.
use marshell_core::doctor::probe::{Failure, Ready, probe};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn fake() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-agent"))
}

/// The two variables a program needs to start on Windows; everything else is what the test gives.
fn base_env() -> Vec<(String, String)> {
    ["SYSTEMROOT", "PATH"]
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|v| (name.to_string(), v)))
        .collect()
}

fn ready(mode: &str, extra_env: &[(&str, String)], timeout_secs: u64) -> Ready {
    let mut env = base_env();
    env.extend(extra_env.iter().map(|(k, v)| (k.to_string(), v.clone())));
    Ready {
        program: fake(),
        args: vec!["mcp".into(), mode.into()],
        env,
        cwd: None,
        timeout: Duration::from_secs(timeout_secs),
    }
}

#[test]
fn ok_lists_tools_over_pages() {
    let listed = probe(&ready("ok", &[], 10)).unwrap();
    assert_eq!(listed.instructions, "Use it.");
    let names: Vec<&str> = listed.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["alpha", "beta"]);
    let alpha = &listed.tools[0];
    assert_eq!(alpha.description, "First tool.\nMore.");
    assert_eq!(alpha.meta_true, ["anthropic/alwaysLoad"]);
    assert!(alpha.schema_chars > 0);
    assert!(listed.tools[1].meta_true.is_empty());
}

#[test]
fn no_tools_capability_lists_none() {
    let listed = probe(&ready("notools", &[], 10)).unwrap();
    assert_eq!(listed.instructions, "Use it.");
    assert!(listed.tools.is_empty());
}

#[test]
fn silent_times_out() {
    let started = Instant::now();
    assert_eq!(probe(&ready("silent", &[], 2)), Err(Failure::NoAnswer(2)));
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn exit_code_is_reported() {
    assert_eq!(probe(&ready("exit3", &[], 10)), Err(Failure::Exited(Some(3))));
}

#[test]
fn garbage_is_not_mcp() {
    assert_eq!(probe(&ready("garbage", &[], 10)), Err(Failure::NotMcp));
}

#[test]
fn refused_initialize() {
    assert_eq!(probe(&ready("refuse", &[], 10)), Err(Failure::Refused));
}

#[test]
fn flood_is_not_mcp_and_bounded() {
    let started = Instant::now();
    assert_eq!(probe(&ready("flood", &[], 10)), Err(Failure::NotMcp));
    assert!(
        started.elapsed() < Duration::from_secs(12),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn missing_program_cant_start() {
    let mut r = ready("ok", &[], 10);
    r.program = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("no-such-program.exe");
    assert_eq!(probe(&r), Err(Failure::CantStart));
}

#[test]
fn env_is_exactly_ready_env() {
    let listed = probe(&ready("ok", &[("FAKE_MCP_ECHO", "1".into())], 10)).unwrap();
    let mut seen: Vec<String> = listed.instructions.split(',').map(|n| n.to_ascii_uppercase()).collect();
    seen.sort();
    // Windows 11 adds no names of its own to a process started with an explicit environment (seen by running this
    // test with nothing removed), so the sets must be equal.
    let mut want: Vec<String> = base_env().into_iter().map(|(k, _)| k.to_ascii_uppercase()).collect();
    want.push("FAKE_MCP_ECHO".into());
    want.sort();
    assert_eq!(seen, want);
}

#[cfg(windows)]
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

#[cfg(windows)]
#[test]
fn timeout_ends_the_grandchild() {
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("grandchild.pid");
    let r = ready("grandchild", &[("FAKE_MCP_PIDFILE", pidfile.display().to_string())], 2);
    assert_eq!(probe(&r), Err(Failure::NoAnswer(2)));
    let pid: u32 = std::fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !gone(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(gone(pid), "the grandchild {pid} outlived the probe");
}
