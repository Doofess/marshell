use marshell_core::pty::{MAX_UNACKED, Pull, Session, SpawnSpec};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn fake(args: &[&str]) -> SpawnSpec {
    let mut argv = vec![env!("CARGO_BIN_EXE_fake-agent").to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    SpawnSpec {
        argv,
        cwd: None,
        env: vec![],
        cols: 80,
        rows: 24,
    }
}

/// Reads everything until Exit (or the timeout), acking as it goes. Returns (bytes, exit code).
fn drain(s: &Arc<Session>, timeout: Duration) -> (Vec<u8>, Option<i32>) {
    let generation = s.attach();
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
    let generation = s.attach();
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
    let generation = s.attach();
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
