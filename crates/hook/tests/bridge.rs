mod common;

use std::io::Cursor;
use std::time::{Duration, Instant};

use common::{Reply, argv, env_of, refused_port, serve, write_endpoint};
use marshell_hook::{run, run_with};

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

const TAB: &[(&str, &str)] = &[("MARSHELL_TAB_ID", "tab1")];
const SUGGESTED: &str = r#"{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"node *"}],"behavior":"allow","destination":"localSettings"}"#;

fn permission_request() -> String {
    format!(r#"{{"tool_name":"Bash","permission_suggestions":[{SUGGESTED}]}}"#)
}

fn decision(decision: &str) -> String {
    format!(r#"{{"hookSpecificOutput":{{"hookEventName":"PermissionRequest","decision":{decision}}}}}"#)
}

/// What the bridge prints for a permission request when the core answers 200 with `body`.
fn permission_answer(body: &str) -> (Vec<u8>, Vec<String>) {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, body));
    write_endpoint(home.path(), fake.port, "t");
    let out = go(home.path(), TAB, argv("permission"), permission_request().as_bytes());
    (out, log_lines(home.path()))
}

#[test]
fn permission_prints_an_allow_or_a_deny_verbatim() {
    for body in [
        decision(r#"{"behavior":"allow"}"#),
        decision(r#"{"behavior":"deny","message":"Denied in Marshell."}"#),
        decision(&format!(r#"{{"behavior":"allow","updatedPermissions":[{SUGGESTED}]}}"#)),
        decision(&format!(
            r#"{{"behavior":"allow","updatedPermissions":[{}]}}"#,
            SUGGESTED.replace("localSettings", "session")
        )),
    ] {
        let (out, log) = permission_answer(&body);
        assert_eq!(String::from_utf8(out).unwrap(), body);
        assert!(log.is_empty(), "{log:?}");
    }
}

#[test]
fn permission_without_a_tab_asks_nothing() {
    // The core answers 204 at once without a tab; a listener on a stale port must not get the chance to answer.
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, &decision(r#"{"behavior":"allow"}"#)));
    write_endpoint(home.path(), fake.port, "t");
    assert!(go(home.path(), &[], argv("permission"), permission_request().as_bytes()).is_empty());
    assert!(
        fake.captured(Duration::from_millis(300)).is_none(),
        "the bridge posted without a tab"
    );
}

#[test]
fn a_codex_permission_without_tab_prints_nothing() {
    // Codex run outside Marshell: no tab, so no connection is tried and Codex asks the person itself.
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, &decision(r#"{"behavior":"allow"}"#)));
    write_endpoint(home.path(), fake.port, "t");
    let args: Vec<String> = ["permission", "--agent", "codex", "--tag", "mshl1"]
        .map(String::from)
        .to_vec();
    let started = Instant::now();
    let request = br#"{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"dir"}}"#;
    assert!(go(home.path(), &[], args, request).is_empty());
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "{:?}",
        started.elapsed()
    );
    assert!(
        fake.captured(Duration::from_millis(300)).is_none(),
        "the bridge posted without a tab"
    );
    assert!(log_lines(home.path()).is_empty());
}

const DAEMON_CHAIN: &[&str] = &[
    r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
    r"C:\Users\me\.codex\packages\app-server-daemon\releases\0.163.0\bin\codex.exe",
    r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe",
];
const TUI_CHAIN: &[&str] = &[
    r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
    r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe",
];

/// `go` with a tab id and the given process chain above the bridge, parent first.
fn go_under(home: &std::path::Path, chain: &[&str], args: Vec<String>, stdin: &[u8]) -> Vec<u8> {
    let env = env_of(home, TAB);
    let chain: Vec<String> = chain.iter().map(|p| (*p).to_owned()).collect();
    run_with(&args, &mut Cursor::new(stdin.to_vec()), &env, &|| chain.clone())
}

fn codex(role: &str) -> Vec<String> {
    [role, "--agent", "codex", "--tag", "mshl1"].map(String::from).to_vec()
}

#[test]
fn a_codex_event_from_the_shared_daemon_posts_with_no_tab() {
    // The daemon keeps the tab id of the Codex that started it, so the hook may be another tab's.
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    assert!(go_under(home.path(), DAEMON_CHAIN, codex("event"), b"{}").is_empty());
    let c = fake.captured(WAIT).expect("no request");
    assert_eq!(c.headers["x-marshell-agent"], "codex");
    assert!(!c.headers.contains_key("x-marshell-tab"));
}

#[test]
fn a_codex_permission_from_the_shared_daemon_prints_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, &decision(r#"{"behavior":"allow"}"#)));
    write_endpoint(home.path(), fake.port, "t");
    let request = br#"{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"dir"}}"#;
    assert!(go_under(home.path(), DAEMON_CHAIN, codex("permission"), request).is_empty());
    assert!(
        fake.captured(Duration::from_millis(300)).is_none(),
        "the bridge posted for another tab"
    );
    assert!(log_lines(home.path()).is_empty());
}

#[test]
fn the_tab_stands_for_codex_without_the_daemon_and_for_claude() {
    for (chain, args) in [(TUI_CHAIN, codex("event")), (DAEMON_CHAIN, argv("event"))] {
        let home = tempfile::tempdir().unwrap();
        let fake = serve(Reply::new(204, ""));
        write_endpoint(home.path(), fake.port, "t");
        go_under(home.path(), chain, args, b"{}");
        assert_eq!(
            fake.captured(WAIT).expect("no request").headers["x-marshell-tab"],
            "tab1"
        );
    }
}

#[test]
fn permission_prints_nothing_for_an_answer_marshell_never_gives() {
    // Whatever answers on the port might not be the core (ADR 0001, reviewed for 3d): only the core's own shapes pass.
    let other_rule = SUGGESTED.replace("node *", "*");
    for body in [
        decision(r#"{"behavior":"allow","updatedInput":{"command":"calc"}}"#),
        decision(&format!(r#"{{"behavior":"allow","updatedPermissions":[{other_rule}]}}"#)),
        decision(&format!(r#"{{"behavior":"allow","updatedPermissions":[{}]}}"#, SUGGESTED.replace("localSettings", "userSettings"))),
        decision(r#"{"behavior":"allow","updatedPermissions":[{"type":"setMode","mode":"bypassPermissions","destination":"session"}]}"#),
        decision(r#"{"behavior":"deny","message":"no","interrupt":true}"#),
        decision(r#"{"behavior":"ask"}"#),
        r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}},"systemMessage":"hi"}"#.to_string(),
        r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","decision":{"behavior":"allow"}}}"#.to_string(),
        r#"{"continue":false}"#.to_string(),
        "not json".to_string(),
    ] {
        let (out, log) = permission_answer(&body);
        assert!(out.is_empty(), "printed {body}");
        assert_eq!(log.len(), 1, "{body}: {log:?}");
        assert!(log[0].contains("answer"), "{}", log[0]);
    }
}

#[test]
fn permission_204_prints_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    assert!(go(home.path(), TAB, argv("permission"), b"{}").is_empty());
    assert!(fake.captured(WAIT).is_some());
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

/// What the bridge prints for a Claude Code event when the core answers 200 with `body`.
fn event_answer(body: &str) -> (Vec<u8>, Vec<String>) {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, body));
    write_endpoint(home.path(), fake.port, "t");
    let out = go(
        home.path(),
        TAB,
        argv("event"),
        br#"{"hook_event_name":"UserPromptSubmit"}"#,
    );
    (out, log_lines(home.path()))
}

#[test]
fn an_event_answer_is_printed_only_when_it_is_context() {
    let context = r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"References: 'a' = C:/r/a."}}"#;
    let (out, log) = event_answer(context);
    assert_eq!(String::from_utf8(out).unwrap(), context);
    assert!(log.is_empty(), "{log:?}");

    // Whatever answers on the port might not be the core: only the core's own shape reaches the agent.
    for body in [
        r#"{"decision":"block"}"#,
        r#"{"decision":"block","reason":"no"}"#,
        r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"x"},"decision":"block"}"#,
        r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"x","updatedInput":{}}}"#,
        r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"x"}}"#,
        r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":1}}"#,
        r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit"}}"#,
        r#"{"continue":false}"#,
        "not json",
    ] {
        let (out, log) = event_answer(body);
        assert!(out.is_empty(), "printed {body}");
        assert_eq!(log.len(), 1, "{body}: {log:?}");
    }
}

/// What the bridge prints for an Antigravity `PreInvocation` when the core answers 200 with `body`.
fn agy_answer(body: &str) -> (Vec<u8>, Vec<String>) {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, body));
    write_endpoint(home.path(), fake.port, "t");
    let argv = [
        "event",
        "--agent",
        "antigravity",
        "--event",
        "PreInvocation",
        "--tag",
        "mshl1",
    ]
    .map(String::from)
    .to_vec();
    let out = go(home.path(), TAB, argv, br#"{"invocationNum":0}"#);
    (out, log_lines(home.path()))
}

#[test]
fn an_antigravity_answer_is_printed_only_when_it_is_its_context_shape() {
    let context = r#"{"injectSteps":[{"ephemeralMessage":"References: 'a' = C:/r/a."}]}"#;
    let (out, log) = agy_answer(context);
    assert_eq!(String::from_utf8(out).unwrap(), context);
    assert!(log.is_empty(), "{log:?}");

    for body in [
        r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"x"}}"#,
        r#"{"injectSteps":[{"ephemeralMessage":"x"}],"decision":"block"}"#,
        r#"{"injectSteps":[{"ephemeralMessage":"x"},{"ephemeralMessage":"y"}]}"#,
        r#"{"injectSteps":[{"userMessage":"x"}]}"#,
        r#"{"injectSteps":[{"ephemeralMessage":1}]}"#,
        "not json",
    ] {
        let (out, log) = agy_answer(body);
        assert!(out.is_empty(), "printed {body}");
        assert_eq!(log.len(), 1, "{body}: {log:?}");
    }
}

#[test]
fn a_204_prints_nothing() {
    for role in ["event", "permission"] {
        let home = tempfile::tempdir().unwrap();
        let fake = serve(Reply::new(204, ""));
        write_endpoint(home.path(), fake.port, "t");
        assert!(go(home.path(), TAB, argv(role), b"{}").is_empty(), "{role}");
        assert!(log_lines(home.path()).is_empty());
    }
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

#[test]
fn grok_hook_env_makes_the_claude_bridge_exit_quietly() {
    let extra = [("MARSHELL_TAB_ID", "tab1"), ("GROK_HOOK_EVENT", "pre_tool_use")];
    for role in ["permission", "event"] {
        let home = tempfile::tempdir().unwrap();
        let fake = serve(Reply::new(200, &decision(r#"{"behavior":"allow"}"#)));
        write_endpoint(home.path(), fake.port, "t");
        assert!(go(home.path(), &extra, argv(role), permission_request().as_bytes()).is_empty());
        assert!(
            fake.captured(Duration::from_millis(300)).is_none(),
            "{role}: the bridge posted for Grok"
        );
        assert!(log_lines(home.path()).is_empty());
    }
    // Codex is not affected by the variable.
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    go(home.path(), &extra, codex("event"), b"{}");
    assert!(fake.captured(WAIT).is_some());
}

fn agy(role: &str, event: Option<&str>) -> Vec<String> {
    let mut v: Vec<String> = [role, "--agent", "antigravity"].iter().map(|s| s.to_string()).collect();
    if let Some(event) = event {
        v.extend(["--event".to_string(), event.to_string()]);
    }
    v.extend(["--tag".to_string(), "mshl1".to_string()]);
    v
}

#[test]
fn the_event_is_written_into_the_body() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    assert!(
        go(
            home.path(),
            TAB,
            agy("event", Some("Stop")),
            br#"{"conversationId":"c"}"#
        )
        .is_empty()
    );
    let body: serde_json::Value = serde_json::from_slice(&fake.captured(WAIT).unwrap().body).unwrap();
    assert_eq!(body, serde_json::json!({"conversationId":"c","hook_event_name":"Stop"}));
}

#[test]
fn an_existing_event_name_is_kept() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    let stdin = br#"{"hook_event_name":"PostToolUse","conversationId":"c"}"#;
    go(home.path(), TAB, agy("event", Some("Stop")), stdin);
    assert_eq!(fake.captured(WAIT).unwrap().body, stdin);
}

#[test]
fn a_body_that_is_not_an_object_is_posted_as_it_is() {
    for stdin in [&b"[1,2]"[..], b"\"Stop\"", b"not json", b""] {
        let home = tempfile::tempdir().unwrap();
        let fake = serve(Reply::new(204, ""));
        write_endpoint(home.path(), fake.port, "t");
        go(home.path(), TAB, agy("event", Some("Stop")), stdin);
        assert_eq!(fake.captured(WAIT).unwrap().body, stdin);
    }
}

#[test]
fn a_body_without_an_event_flag_is_posted_as_it_is() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    go(home.path(), TAB, agy("event", None), br#"{"conversationId":"c"}"#);
    assert_eq!(fake.captured(WAIT).unwrap().body, br#"{"conversationId":"c"}"#);
}

#[test]
fn record_saves_the_body_as_posted_under_the_event_name() {
    let home = tempfile::tempdir().unwrap();
    let rec = tempfile::tempdir().unwrap();
    let rec_dir = rec.path().to_string_lossy().into_owned();
    let extra = [("MARSHELL_RECORD_DIR", rec_dir.as_str())];
    let fake = serve(Reply::new(204, ""));
    write_endpoint(home.path(), fake.port, "t");
    go(
        home.path(),
        &extra,
        agy("record", Some("PreInvocation")),
        br#"{"conversationId":"c"}"#,
    );
    let posted = fake.captured(WAIT).unwrap().body;
    assert_eq!(std::fs::read(rec.path().join("PreInvocation.json")).unwrap(), posted);
    assert!(
        String::from_utf8(posted)
            .unwrap()
            .contains(r#""hook_event_name":"PreInvocation""#)
    );
    // A body that is not an object has no name of its own, so the file carries the event.
    go(home.path(), &extra, agy("record", Some("Stop")), b"not json");
    assert_eq!(std::fs::read(rec.path().join("Stop.json")).unwrap(), b"not json");
}

/// Runs the real bridge with only `MARSHELL_HOME` and `extra` in its environment, and returns what it
/// printed and its exit code.
fn run_binary(home: &std::path::Path, extra: &[(&str, &str)], args: &[String], stdin: &[u8]) -> (Vec<u8>, Option<i32>) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_marshell-hook"));
    cmd.args(args)
        .env_clear()
        .env("MARSHELL_HOME", home)
        .envs(extra.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    let mut child = cmd.spawn().unwrap();
    let mut pipe = child.stdin.take().unwrap();
    let _ = pipe.write_all(stdin);
    drop(pipe);
    let out = child.wait_with_output().unwrap();
    (out.stdout, out.status.code())
}

const AGY_REQUEST: &[u8] = br#"{"toolName":"run_command","toolInput":{"CommandLine":"echo hi"}}"#;

#[test]
fn antigravity_outside_a_tab_prints_the_neutral_answer() {
    // A stale endpoint with an eager listener must not get the chance to answer.
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, r#"{"decision":"allow"}"#));
    write_endpoint(home.path(), fake.port, "t");
    let (out, code) = run_binary(home.path(), &[], &agy("permission", Some("PreToolUse")), AGY_REQUEST);
    assert!(out.is_empty());
    assert_eq!(code, Some(0));
    assert!(fake.captured(Duration::from_millis(300)).is_none());
}

#[test]
fn antigravity_with_no_core_prints_the_neutral_answer() {
    let home = tempfile::tempdir().unwrap();
    let (out, code) = run_binary(home.path(), TAB, &agy("permission", Some("PreToolUse")), AGY_REQUEST);
    assert!(out.is_empty());
    assert_eq!(code, Some(0));
    write_endpoint(home.path(), refused_port(), "t");
    let (out, code) = run_binary(home.path(), TAB, &agy("permission", Some("PreToolUse")), AGY_REQUEST);
    assert!(out.is_empty());
    assert_eq!(code, Some(0));
}

#[test]
fn antigravity_with_a_server_error_prints_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(500, "boom"));
    write_endpoint(home.path(), fake.port, "t");
    let (out, code) = run_binary(home.path(), TAB, &agy("permission", Some("PreToolUse")), AGY_REQUEST);
    assert!(out.is_empty());
    assert_eq!(code, Some(0));
    assert!(fake.captured(WAIT).is_some());
}

#[test]
fn antigravity_prints_a_deny_the_core_gives_and_nothing_for_other_answers() {
    for (body, printed) in [
        (r#"{"decision":"deny","reason":"Denied in Marshell."}"#, true),
        (r#"{}"#, false),
        (r#"{"decision":"allow","overwrite":{}}"#, false),
        (
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}"#,
            false,
        ),
    ] {
        let home = tempfile::tempdir().unwrap();
        let fake = serve(Reply::new(200, body));
        write_endpoint(home.path(), fake.port, "t");
        let (out, code) = run_binary(home.path(), TAB, &agy("permission", Some("PreToolUse")), AGY_REQUEST);
        assert_eq!(out, if printed { body.as_bytes() } else { b"" }, "{body}");
        assert_eq!(code, Some(0));
    }
}

#[test]
fn an_antigravity_event_prints_nothing() {
    for (status, body) in [(204, ""), (200, r#"{"decision":"deny"}"#)] {
        let home = tempfile::tempdir().unwrap();
        let fake = serve(Reply::new(status, body));
        write_endpoint(home.path(), fake.port, "t");
        let (out, code) = run_binary(home.path(), TAB, &agy("event", Some("PostToolUse")), AGY_REQUEST);
        assert!(out.is_empty(), "{status}");
        assert_eq!(code, Some(0));
    }
}

#[test]
fn an_unknown_flag_on_antigravity_prints_nothing() {
    // An older bridge meeting a newer flag must not block agy.
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, r#"{"decision":"allow"}"#));
    write_endpoint(home.path(), fake.port, "t");
    let mut args = agy("permission", Some("PreToolUse"));
    args.extend(["--newer-flag".to_string(), "x".to_string()]);
    let (out, code) = run_binary(home.path(), TAB, &args, AGY_REQUEST);
    assert!(out.is_empty());
    assert_eq!(code, Some(0));
    assert!(fake.captured(Duration::from_millis(300)).is_none());
}

#[test]
fn garbage_stdin_on_antigravity_prints_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = serve(Reply::new(200, r#"{"decision":"allow"}"#));
    write_endpoint(home.path(), fake.port, "t");
    let (out, code) = run_binary(
        home.path(),
        TAB,
        &agy("permission", Some("PreToolUse")),
        b"\xff\xfe not json",
    );
    assert!(out.is_empty());
    assert_eq!(code, Some(0));
}

#[test]
fn claude_and_codex_outside_a_tab_print_nothing() {
    for agent in ["claude", "codex"] {
        let home = tempfile::tempdir().unwrap();
        let args: Vec<String> = ["permission", "--agent", agent].iter().map(|s| s.to_string()).collect();
        let (out, code) = run_binary(home.path(), &[], &args, permission_request().as_bytes());
        assert!(out.is_empty(), "{agent}");
        assert_eq!(code, Some(0));
    }
}
