//! The `marshell` program against a real in-process core on a temp home. The program is run as a child with
//! `MARSHELL_HOME` set on that child only, so nothing reads the real `~/.marshell` or this test's own environment.

use marshell_core::server::{CoreConfig, RunningCore, start};
use std::path::Path;
use std::process::Command;
use std::sync::Once;
#[cfg(windows)]
use std::time::{Duration, Instant};

struct Output {
    code: i32,
    out: String,
    err: String,
}

/// Runs `marshell <args>` with `MARSHELL_HOME` set to `home`.
fn marshell(home: &Path, args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_marshell"))
        .args(args)
        .env("MARSHELL_HOME", home)
        .output()
        .unwrap();
    Output {
        code: output.status.code().unwrap(),
        out: String::from_utf8_lossy(&output.stdout).into_owned(),
        err: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

async fn core(home: &Path) -> RunningCore {
    core_with_focus(home, None).await
}

/// A core whose focus route hands the tab to `focus`, as the app shell does.
async fn core_with_focus(home: &Path, focus: Option<marshell_core::server::FocusSink>) -> RunningCore {
    static TIE: Once = Once::new();
    TIE.call_once(|| marshell_core::job::tie_children_to_app().expect("could not tie children to the test process"));
    start(CoreConfig {
        home: home.to_path_buf(),
        allowed_origins: vec![],
        programs_dir: None,
        notify: None,
        focus,
        notify_paused: Default::default(),
        window_hidden: Default::default(),
        import: None,
    })
    .await
    .unwrap()
}

/// Runs the program without holding up the core's runtime.
async fn run(home: &Path, args: &[&str]) -> Output {
    tokio::task::block_in_place(|| marshell(home, args))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closed_app_says_so() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        &["list"][..],
        &["list", "--json"],
        &["send", "1", "hi"],
        &["new"],
        &["new", "--agent", "claude", "--prompt", "hi"],
        &["focus", "1"],
    ] {
        let o = run(home.path(), args).await;
        assert_eq!((o.code, o.out.as_str()), (1, ""), "{args:?}");
        assert_eq!(o.err.trim_end(), "Marshell isn't running.", "{args:?}");
    }
    // A file left behind by a crash, naming a port nobody listens on, is the same.
    std::fs::create_dir_all(home.path().join("run")).unwrap();
    std::fs::write(
        home.path().join("run").join("endpoint.json"),
        r#"{"version":1,"port":1,"token":"never-printed-token","platform":"windows"}"#,
    )
    .unwrap();
    let o = run(home.path(), &["list"]).await;
    assert_eq!(o.code, 1);
    assert_eq!(o.err.trim_end(), "Marshell isn't running.");
    assert!(!o.out.contains("never-printed-token") && !o.err.contains("never-printed-token"));
}

/// `--start` with no record of the app, or a record naming an exe that is gone, starts nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn start_without_an_app_says_what_is_missing() {
    let home = tempfile::tempdir().unwrap();
    for args in [&["focus", "1", "--start"][..], &["new", "--start"]] {
        let o = run(home.path(), args).await;
        assert_eq!((o.code, o.out.as_str()), (1, ""), "{args:?}");
        assert_eq!(
            o.err.trim_end(),
            "Couldn't start Marshell: open it once from the Start menu first.",
            "{args:?}"
        );
    }
    let gone = home.path().join("gone").join("marshell-app.exe");
    std::fs::create_dir_all(home.path().join("bin")).unwrap();
    std::fs::write(
        home.path().join("bin").join("app.json"),
        serde_json::json!({ "exe": gone }).to_string(),
    )
    .unwrap();
    let o = run(home.path(), &["new", "--start"]).await;
    assert_eq!(o.code, 1);
    assert_eq!(
        o.err.trim_end(),
        format!("Couldn't start Marshell: {} isn't there.", gone.display())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn usage_errors_exit_2() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        &["send", "6"][..],
        &["list", "--bogus"],
        &["frobnicate"],
        &["new", "--mode", "sideways"],
        &["new", "--agent"],
        &["focus"],
    ] {
        let o = run(home.path(), args).await;
        assert_eq!(o.code, 2, "{args:?}");
        assert!(o.err.contains("Usage:"), "{}", o.err);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_with_no_sessions() {
    let home = tempfile::tempdir().unwrap();
    let core = core(home.path()).await;
    let o = run(home.path(), &["list"]).await;
    assert_eq!((o.code, o.out.trim_end(), o.err.as_str()), (0, "No sessions.", ""));
    let o = run(home.path(), &["list", "--json"]).await;
    assert_eq!(o.code, 0);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&o.out).unwrap(),
        serde_json::json!([])
    );
    // No session by that number or tab id.
    let o = run(home.path(), &["send", "6", "hi"]).await;
    assert_eq!((o.code, o.err.trim_end()), (1, "No session 6."));
    core.shutdown();
}

/// Starts a Command Prompt session in `cwd` and returns its tab id.
#[cfg(windows)]
async fn start_cmd(core: &RunningCore, cwd: &Path) -> String {
    let body = serde_json::json!({ "cols": 100, "rows": 24, "profile_id": "cmd", "cwd": cwd.to_string_lossy() });
    let res = tokio::task::block_in_place(|| {
        ureq::Agent::new_with_defaults()
            .post(format!("http://127.0.0.1:{}/v1/sessions", core.endpoint.port))
            .header("Authorization", format!("Bearer {}", core.endpoint.token))
            .header("Content-Type", "application/json")
            .send(body.to_string().as_bytes())
            .unwrap()
            .body_mut()
            .read_to_string()
            .unwrap()
    });
    serde_json::from_str::<serde_json::Value>(&res).unwrap()["tab_id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[cfg(windows)]
fn rows_of(core: &RunningCore, tab: &str) -> Vec<String> {
    core.state.session(tab).unwrap().peek(24)
}

#[cfg(windows)]
async fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ok() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_and_send_fill_without_enter() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let core = core(home.path()).await;
    let tab = start_cmd(&core, work.path()).await;
    wait_for("the prompt", || {
        rows_of(&core, &tab).iter().any(|r| r.trim_end().ends_with('>'))
    })
    .await;

    // The table and the JSON name the session.
    let o = run(home.path(), &["list"]).await;
    assert_eq!((o.code, o.err.as_str()), (0, ""));
    let lines: Vec<&str> = o.out.lines().collect();
    assert_eq!(lines.len(), 2, "{}", o.out);
    assert!(lines[1].starts_with('1') && lines[1].contains("running"), "{}", o.out);
    let o = run(home.path(), &["list", "--json"]).await;
    let json: serde_json::Value = serde_json::from_str(&o.out).unwrap();
    assert_eq!(json[0]["tab_id"], tab.as_str());
    assert_eq!(json[0]["number"], 1);
    assert_eq!(json[0]["state"], "running");
    assert!(json[0]["agent"].is_null());
    assert!(json[0]["project"].is_string());
    assert!(!o.out.contains(&core.endpoint.token));

    // By number and by tab id: the text reaches the prompt line, and nothing presses Enter.
    let o = run(home.path(), &["send", "1", "echo ZQX9"]).await;
    assert_eq!((o.code, o.err.as_str()), (0, ""));
    assert_eq!(o.out.trim_end(), "Filled session 1's prompt line.");
    wait_for("the text", || {
        rows_of(&core, &tab).iter().any(|r| r.contains("echo ZQX9"))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    // Had Enter been pressed, the command would have printed its argument on a row of its own.
    assert!(
        !rows_of(&core, &tab).iter().any(|r| r.trim() == "ZQX9"),
        "{:?}",
        rows_of(&core, &tab)
    );
    let o = run(home.path(), &["send", &tab, "x"]).await;
    assert_eq!((o.code, o.out.trim_end()), (0, "Filled session 1's prompt line."));

    core.shutdown();
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_refuses_controls_and_ended() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let core = core(home.path()).await;
    let tab = start_cmd(&core, work.path()).await;
    wait_for("the prompt", || {
        rows_of(&core, &tab).iter().any(|r| r.trim_end().ends_with('>'))
    })
    .await;

    // The core's own line, as it is, and nothing reaches the session.
    let o = run(home.path(), &["send", "1", "a\nb"]).await;
    assert_eq!((o.code, o.out.as_str()), (1, ""));
    assert_eq!(
        o.err.trim_end(),
        "The text can't hold line breaks or other control characters: Marshell never presses Enter."
    );
    let o = run(home.path(), &["send", "1", &"x".repeat(4001)]).await;
    assert_eq!((o.code, o.err.trim_end()), (1, "The text is over 4000 characters."));
    assert!(!rows_of(&core, &tab).iter().any(|r| r.contains("xxxx")));

    // The shell exits: the row ends, and the CLI refuses before it asks the core.
    core.state.session(&tab).unwrap().write_input(b"exit\r".to_vec());
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let o = run(home.path(), &["list", "--json"]).await;
        let json: serde_json::Value = serde_json::from_str(&o.out).unwrap();
        if json[0]["state"] != "running" {
            break;
        }
        assert!(Instant::now() < deadline, "the shell never exited");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let o = run(home.path(), &["send", "1", "hi"]).await;
    assert_eq!((o.code, o.out.as_str()), (1, ""));
    assert_eq!(o.err.trim_end(), "Session 1 has ended, so it can't take text.");

    core.shutdown();
}

/// The core refuses the text before anything is created: no session, and no question about starting the app.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_checks_the_prompt_before_it_creates_anything() {
    let home = tempfile::tempdir().unwrap();
    let o = run(
        home.path(),
        &[
            "new", "--prompt", "a
b",
        ],
    )
    .await;
    assert_eq!((o.code, o.out.as_str()), (1, ""));
    assert_eq!(
        o.err.trim_end(),
        "The text can't hold line breaks or other control characters: Marshell never presses Enter."
    );
    let core = core(home.path()).await;
    let o = run(home.path(), &["new", "--prompt", &"x".repeat(4001)]).await;
    assert_eq!((o.code, o.err.trim_end()), (1, "The text is over 4000 characters."));
    let o = run(home.path(), &["list"]).await;
    assert_eq!(o.out.trim_end(), "No sessions.");
    // An agent the core doesn't know is its own refusal, and creates nothing either.
    let o = run(home.path(), &["new", "--agent", "bogus"]).await;
    assert_eq!((o.code, o.out.as_str()), (1, ""));
    assert_eq!(o.err.trim_end(), "Unknown agent: bogus.");
    let o = run(home.path(), &["list"]).await;
    assert_eq!(o.out.trim_end(), "No sessions.");
    core.shutdown();
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_creates_then_fills() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let core = core(home.path()).await;

    // A plain shell stands in for the agent: `new --prompt` starts it, waits for it to be ready, and fills its line.
    let folder = work.path().to_string_lossy().into_owned();
    let o = run(
        home.path(),
        &["new", "--cwd", &folder, "--name", "build", "--prompt", "echo ZQX9"],
    )
    .await;
    assert_eq!((o.code, o.err.as_str()), (0, ""), "{}", o.out);
    assert_eq!(
        o.out,
        "Started session 1.
Filled session 1's prompt line.
"
    );
    let o = run(home.path(), &["list", "--json"]).await;
    let json: serde_json::Value = serde_json::from_str(&o.out).unwrap();
    assert_eq!(
        (json[0]["number"].clone(), json[0]["name"].clone()),
        (1.into(), "build".into())
    );
    assert_eq!(json[0]["state"], "running");
    let tab = json[0]["tab_id"].as_str().unwrap().to_string();
    wait_for("the text", || {
        rows_of(&core, &tab).iter().any(|r| r.contains("echo ZQX9"))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    // Nothing pressed Enter: the command never printed its argument on a row of its own.
    assert!(
        !rows_of(&core, &tab).iter().any(|r| r.trim() == "ZQX9"),
        "{:?}",
        rows_of(&core, &tab)
    );

    // Without --cwd the session starts in the CLI's own folder, and without --prompt nothing is filled.
    let o = run(home.path(), &["new"]).await;
    assert_eq!(
        (o.code, o.out.as_str(), o.err.as_str()),
        (
            0,
            "Started session 2.
",
            ""
        )
    );
    let o = run(home.path(), &["list", "--json"]).await;
    let json: serde_json::Value = serde_json::from_str(&o.out).unwrap();
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
        std::path::Path::new(json[1]["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        cwd.canonicalize().unwrap()
    );
    core.shutdown();
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn focus_posts_the_tab() {
    let home = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let focused = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = {
        let focused = focused.clone();
        std::sync::Arc::new(move |tab: String| focused.lock().unwrap().push(tab)) as marshell_core::server::FocusSink
    };
    let windowed = core_with_focus(home.path(), Some(sink)).await;
    let tab = start_cmd(&windowed, work.path()).await;

    // A number resolves to its tab, and a tab id is taken as it is.
    let o = run(home.path(), &["focus", "1"]).await;
    assert_eq!((o.code, o.out.as_str(), o.err.as_str()), (0, "", ""));
    let o = run(home.path(), &["focus", &tab]).await;
    assert_eq!((o.code, o.out.as_str(), o.err.as_str()), (0, "", ""));
    assert_eq!(*focused.lock().unwrap(), vec![tab.clone(), tab.clone()]);

    let o = run(home.path(), &["focus", "9"]).await;
    assert_eq!((o.code, o.out.as_str(), o.err.trim_end()), (1, "", "No session 9."));
    assert_eq!(focused.lock().unwrap().len(), 2);
    windowed.shutdown();

    // A core without a window says so.
    let home = tempfile::tempdir().unwrap();
    let bare = core(home.path()).await;
    start_cmd(&bare, work.path()).await;
    let o = run(home.path(), &["focus", "1"]).await;
    assert_eq!(
        (o.code, o.err.trim_end()),
        (1, "Marshell's window isn't there to focus.")
    );
    bare.shutdown();
}
