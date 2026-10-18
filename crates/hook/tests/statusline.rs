mod common;

use std::io::Cursor;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{Reply, argv, env_of, refused_port, serve, write_endpoint};
use marshell_hook::run;

const WAIT: Duration = Duration::from_secs(2);
const STDIN: &[u8] = b"{\"model\":{\"display_name\":\"Opus\"}}\n";

fn write_chain(home: &Path, command: &str) {
    let dir = home.join("agents").join("claude");
    std::fs::create_dir_all(&dir).unwrap();
    let chain = serde_json::json!({"version":1,"statusLine":{"type":"command","command":command}});
    std::fs::write(dir.join("statusline-chain.json"), chain.to_string()).unwrap();
}

/// The real PATH is passed on, so the resolver finds Git Bash the way it does for a user.
fn go(home: &Path) -> Vec<u8> {
    let path = std::env::var("PATH").unwrap_or_default();
    let extra = [("PATH", path.as_str())];
    let env = env_of(home, &extra);
    run(&argv("statusline"), &mut Cursor::new(STDIN.to_vec()), &env)
}

fn log_lines(home: &Path) -> Vec<String> {
    std::fs::read_to_string(home.join("logs").join("bridge.log"))
        .map(|t| t.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

#[test]
fn prints_the_original_output_byte_for_byte() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    write_chain(home.path(), "cat");
    let out = go(home.path());
    assert_eq!(out, STDIN);
    let c = fake.captured(WAIT).expect("no request");
    assert_eq!(c.headers["x-marshell-role"], "statusline");
    assert_eq!(c.headers["x-marshell-agent"], "claude");
    assert_eq!(c.body, STDIN);
    assert!(log_lines(home.path()).is_empty());
}

#[test]
fn still_prints_when_the_app_is_closed() {
    let home = tempfile::tempdir().unwrap();
    write_chain(home.path(), "cat");
    assert_eq!(go(home.path()), STDIN);
    // A stale endpoint file does the same.
    write_endpoint(home.path(), refused_port(), "t");
    assert_eq!(go(home.path()), STDIN);
}

#[test]
fn still_prints_when_the_core_answers_an_error() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(401, "no"));
    write_endpoint(home.path(), fake.port, "secret-token");
    write_chain(home.path(), "cat");
    assert_eq!(go(home.path()), STDIN);
    assert!(fake.captured(WAIT).is_some());
}

#[test]
fn failing_original_prints_nothing_of_ours() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    write_chain(home.path(), "printf x; exit 3");
    let out = go(home.path());
    assert!(out.is_empty(), "{:?}", String::from_utf8_lossy(&out));
    let lines = log_lines(home.path());
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains(" statusline "), "{}", lines[0]);
    assert!(!lines[0].contains("Opus"), "the body is not logged: {}", lines[0]);
}

#[test]
fn missing_chain_file_prints_nothing_and_logs() {
    let home = tempfile::tempdir().unwrap();
    let out = go(home.path());
    assert!(out.is_empty());
    let lines = log_lines(home.path());
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains(" statusline "), "{}", lines[0]);
}

#[test]
fn broken_chain_file_prints_nothing_and_logs() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("agents").join("claude");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("statusline-chain.json"), "{not json").unwrap();
    assert!(go(home.path()).is_empty());
    assert_eq!(log_lines(home.path()).len(), 1);
    std::fs::write(dir.join("statusline-chain.json"), r#"{"version":1,"statusLine":{}}"#).unwrap();
    assert!(go(home.path()).is_empty());
    assert_eq!(log_lines(home.path()).len(), 2);
}

#[test]
fn columns_and_lines_reach_the_original() {
    let home = tempfile::tempdir().unwrap();
    write_chain(home.path(), "printf '%s,%s' \"$COLUMNS\" \"$LINES\"");
    let path = std::env::var("PATH").unwrap_or_default();
    let extra = [("PATH", path.as_str()), ("COLUMNS", "97"), ("LINES", "31")];
    let env = env_of(home.path(), &extra);
    let out = run(&argv("statusline"), &mut Cursor::new(STDIN.to_vec()), &env);
    assert_eq!(out, b"97,31");
}

#[test]
fn slow_core_does_not_delay_output() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, "").delayed(Duration::from_secs(2)));
    write_endpoint(home.path(), fake.port, "t");
    write_chain(home.path(), "cat");
    let start = Instant::now();
    let out = go(home.path());
    let took = start.elapsed();
    eprintln!("slow_core_does_not_delay_output took {took:?}");
    assert_eq!(out, STDIN);
    assert!(took < Duration::from_secs(1), "took {took:?}");
}
