use std::sync::Once;

/// Keep test children in a kill-on-close job, including when a test panics.
/// On other platforms the app's job helper does nothing.
pub fn tie_children_to_test() {
    static INIT: Once = Once::new();
    INIT.call_once(|| marshell_core::job::tie_children_to_app().expect("could not tie children to the test process"));
}

#[allow(dead_code)]
pub const TOKEN: &str = "test-token";
#[allow(dead_code)]
pub const PORT: u16 = 5555;
#[allow(dead_code)]
pub const ORIGIN: &str = "http://tauri.localhost";

/// A profile of the shell `family` running `command`.
#[allow(dead_code)]
pub fn family_profile(
    id: &str,
    command: Vec<String>,
    family: marshell_protocol::api::ShellFamily,
) -> marshell_protocol::api::Profile {
    tie_children_to_test();
    marshell_protocol::api::Profile {
        id: id.into(),
        name: id.into(),
        command,
        family,
        cwd: None,
        env: Vec::new(),
        shell_integration: false,
        terminal_theme: None,
        detected: true,
    }
}

/// A session whose program turns bracketed paste on and appends every byte typed to it to a file, started in
/// `cwd` with `home` as the Marshell home and a profile of the shell `family`. Returns the router, the tab and
/// that file (`typed.log` in `cwd`).
#[allow(dead_code)]
pub async fn capturing_in(
    cwd: &std::path::Path,
    home: &std::path::Path,
    family: marshell_protocol::api::ShellFamily,
) -> (axum::Router, String, std::path::PathBuf) {
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use tower::ServiceExt;

    let log = cwd.join("typed.log");
    let fake = env!("CARGO_BIN_EXE_fake-agent");
    let command = vec![fake.into(), "capture-input".into(), log.to_string_lossy().into_owned()];
    let state = marshell_core::server::AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.to_path_buf(),
        vec![family_profile("capture", command, family)],
    );
    let app = marshell_core::server::router(state.clone());
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": "capture", "cwd": cwd.to_string_lossy(), "agent": null,
    });
    let req = Request::builder()
        .method("POST")
        .uri("/v1/sessions")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let tab = json["tab_id"].as_str().unwrap().to_string();
    let session = state.session(&tab).unwrap();
    // The program sets raw input before it asks for bracketed paste, so this also means it is ready.
    for _ in 0..200 {
        if session.bracketed_paste() {
            return (app, tab, log);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the program never turned bracketed paste on");
}

/// What the program of `capturing_in` was typed, once at least `at_least` bytes came or the wait ran out.
#[allow(dead_code)]
pub async fn typed(log: &std::path::Path, at_least: usize) -> Vec<u8> {
    for _ in 0..200 {
        let bytes = std::fs::read(log).unwrap_or_default();
        if bytes.len() >= at_least {
            // Give a stray extra write the chance to show up.
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            return std::fs::read(log).unwrap_or_default();
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    std::fs::read(log).unwrap_or_default()
}
