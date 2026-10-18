mod common;

use std::io::Cursor;
use std::time::{Duration, Instant};

use common::{Reply, argv, env_of, refused_port, serve, write_endpoint};
use marshell_hook::run;

const WAIT: Duration = Duration::from_secs(2);

fn go(home: &std::path::Path, extra: &[(&str, &str)], args: Vec<String>, stdin: &[u8]) -> Vec<u8> {
    let env = env_of(home, extra);
    run(&args, &mut Cursor::new(stdin.to_vec()), &env)
}

fn log_lines(home: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(home.join("logs").join("bridge.log"))
        .map(|t| t.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

#[test]
fn event_posts_stdin_with_headers_and_prints_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    let out = go(
        home.path(),
        &[("MARSHELL_TAB_ID", "tab1")],
        argv("event"),
        br#"{"a":1}"#,
    );
    assert!(out.is_empty());
    let c = fake.captured(WAIT).expect("no request");
    assert!(c.request_line.starts_with("POST /v1/hook "), "{}", c.request_line);
    assert_eq!(c.headers["authorization"], "Bearer t");
    assert_eq!(c.headers["x-marshell-role"], "event");
    assert_eq!(c.headers["x-marshell-agent"], "claude");
    assert_eq!(c.headers["x-marshell-tab"], "tab1");
    assert!(!c.headers["x-marshell-version"].is_empty());
    assert_eq!(c.body, br#"{"a":1}"#);
    assert!(log_lines(home.path()).is_empty());
}

#[test]
fn no_tab_env_sends_no_tab_header() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    go(home.path(), &[], argv("event"), b"{}");
    let c = fake.captured(WAIT).expect("no request");
    assert!(!c.headers.contains_key("x-marshell-tab"));
}

#[test]
fn missing_endpoint_file_prints_nothing_and_logs_nothing() {
    let home = tempfile::tempdir().unwrap();
    let out = go(home.path(), &[], argv("event"), b"{}");
    assert!(out.is_empty());
    assert!(!home.path().join("logs").join("bridge.log").exists());
}

#[test]
fn refused_connection_prints_nothing_quickly() {
    let home = tempfile::tempdir().unwrap();
    write_endpoint(home.path(), refused_port(), "t");
    let start = Instant::now();
    let out = go(home.path(), &[], argv("event"), b"{}");
    assert!(out.is_empty());
    assert!(start.elapsed() < Duration::from_millis(500), "{:?}", start.elapsed());
    assert!(log_lines(home.path()).is_empty(), "a closed app is not logged");
}

#[test]
fn non_2xx_answer_prints_nothing_and_logs_one_line() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(401, "nope"));
    // Not `t`: a one-letter token would match any log line.
    write_endpoint(home.path(), fake.port, "sekrit-token-9f3a");
    let out = go(home.path(), &[], argv("event"), br#"{"body":"stdin-body-marker"}"#);
    assert!(out.is_empty());
    let lines = log_lines(home.path());
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains(" event "), "{}", lines[0]);
    assert!(lines[0].contains("401"), "{}", lines[0]);
    assert!(!lines[0].contains("sekrit-token-9f3a"));
    assert!(!lines[0].contains("Bearer"));
    assert!(!lines[0].contains("stdin-body-marker"));
}

#[test]
fn permission_prints_a_200_body_verbatim() {
    let home = tempfile::tempdir().unwrap();
    let body = r#"{"hookSpecificOutput":{"decision":{"behavior":"allow"}}}"#;
    let fake = serve(Reply::new(200, body));
    write_endpoint(home.path(), fake.port, "t");
    let out = go(home.path(), &[], argv("permission"), b"{}");
    assert_eq!(out, body.as_bytes());
    let c = fake.captured(WAIT).unwrap();
    assert_eq!(c.headers["x-marshell-role"], "permission");
}

#[test]
fn permission_204_prints_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    assert!(go(home.path(), &[], argv("permission"), b"{}").is_empty());
    assert!(log_lines(home.path()).is_empty());
}

#[test]
fn context_prints_a_200_body_and_nothing_on_204() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, "extra context"));
    write_endpoint(home.path(), fake.port, "t");
    assert_eq!(go(home.path(), &[], argv("context"), b"{}"), b"extra context");

    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    assert!(go(home.path(), &[], argv("context"), b"{}").is_empty());
}

#[test]
fn slow_event_answer_gives_up_within_budget() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, "").delayed(Duration::from_secs(2)));
    write_endpoint(home.path(), fake.port, "t");
    let start = Instant::now();
    let out = go(home.path(), &[], argv("event"), b"{}");
    assert!(out.is_empty());
    assert!(start.elapsed() < Duration::from_secs(1), "{:?}", start.elapsed());
    assert_eq!(log_lines(home.path()).len(), 1);
}

#[test]
fn stdin_over_1_mib_posts_nothing_and_logs() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    let big = vec![b' '; 1_048_576 + 1];
    assert!(go(home.path(), &[], argv("event"), &big).is_empty());
    assert!(fake.captured(Duration::from_millis(300)).is_none());
    assert_eq!(log_lines(home.path()).len(), 1);
}

#[test]
fn stdin_of_exactly_1_mib_is_posted() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    let body = vec![b' '; 1_048_576];
    go(home.path(), &[], argv("event"), &body);
    assert_eq!(fake.captured(WAIT).unwrap().body.len(), 1_048_576);
}

#[test]
fn record_saves_stdin_and_still_posts() {
    let home = tempfile::tempdir().unwrap();
    let rec = tempfile::tempdir().unwrap();
    let rec_dir = rec.path().to_string_lossy().into_owned();
    let extra = [("MARSHELL_RECORD_DIR", rec_dir.as_str())];
    let stdin = br#"{"hook_event_name":"Stop"}"#;

    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    assert!(go(home.path(), &extra, argv("record"), stdin).is_empty());
    assert_eq!(std::fs::read(rec.path().join("Stop.json")).unwrap(), stdin);
    assert_eq!(fake.captured(WAIT).unwrap().headers["x-marshell-role"], "record");

    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    go(home.path(), &extra, argv("record"), stdin);
    assert_eq!(std::fs::read(rec.path().join("Stop-2.json")).unwrap(), stdin);
}

#[test]
fn record_without_a_record_dir_uses_fixtures_under_home() {
    let home = tempfile::tempdir().unwrap();
    go(home.path(), &[], argv("record"), b"not json");
    let saved = home.path().join("fixtures").join("claude").join("record.json");
    assert_eq!(std::fs::read(saved).unwrap(), b"not json");
}

#[test]
fn bad_args_print_nothing_and_log_one_line() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    let args = vec!["bogus".to_string(), "--agent".into(), "claude".into()];
    assert!(go(home.path(), &[], args, b"{}").is_empty());
    assert_eq!(log_lines(home.path()).len(), 1);
    assert!(fake.captured(Duration::from_millis(300)).is_none());
}

#[test]
fn big_log_rotates_to_dot_1() {
    let home = tempfile::tempdir().unwrap();
    let logs = home.path().join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::write(logs.join("bridge.log"), vec![b'x'; 1_048_576 + 1]).unwrap();
    // A failure to log: bad args.
    go(home.path(), &[], vec!["bogus".into()], b"");
    assert_eq!(std::fs::metadata(logs.join("bridge.log.1")).unwrap().len(), 1_048_577);
    assert_eq!(log_lines(home.path()).len(), 1);
}
