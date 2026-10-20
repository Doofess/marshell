//! The files API: resolve paths against a session's folders, stat and read a file, list a folder.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::server::{AppState, router};
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";
const TEXT_CAP: usize = 2 * 1024 * 1024;

fn profile(id: &str, command: Vec<String>) -> Profile {
    Profile {
        id: id.into(),
        name: id.into(),
        command,
        family: ShellFamily::Posix,
        cwd: None,
        env: Vec::new(),
        shell_integration: false,
        terminal_theme: None,
        detected: true,
    }
}

/// `sleeper` reports no folder. `reporter` reports `reported` as its folder, as a shell does.
fn state_in(home: &Path, reported: &Path) -> AppState {
    common::tie_children_to_test();
    let fake = env!("CARGO_BIN_EXE_fake-agent");
    let profiles = vec![
        profile("sleeper", vec![fake.into(), "sleep".into()]),
        profile(
            "reporter",
            vec![fake.into(), "folder".into(), reported.to_string_lossy().into_owned()],
        ),
    ];
    AppState::new(TOKEN.into(), PORT, vec![ORIGIN.into()], home.to_path_buf(), profiles)
}

fn request(method: &str, path: &str, body: Option<serde_json::Value>, token: bool) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{PORT}"));
    if token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn call(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> axum::response::Response {
    app.clone().oneshot(request(method, path, body, true)).await.unwrap()
}

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), 4 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Percent-encodes everything but letters, digits and `-._~`.
fn enc(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The path as the core spells it: canonical, without the `\\?\` prefix.
fn canon(path: &Path) -> String {
    let real = std::fs::canonicalize(path).unwrap();
    let text = real.to_string_lossy().into_owned();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A session of `profile_id` started in `cwd`.
async fn session(app: &axum::Router, profile_id: &str, cwd: &Path) -> String {
    let body = serde_json::json!({
        "cols": 80, "rows": 24, "profile_id": profile_id, "cwd": cwd.to_string_lossy(), "agent": null,
    });
    let res = call(app, "POST", "/v1/sessions", Some(body)).await;
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["tab_id"].as_str().unwrap().to_string()
}

async fn resolve(app: &axum::Router, tab: &str, candidates: &[&str]) -> Vec<Option<String>> {
    let res = call(
        app,
        "POST",
        &format!("/v1/sessions/{tab}/resolve"),
        Some(serde_json::json!({ "candidates": candidates })),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().map(String::from))
        .collect()
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

struct Setup {
    _home: tempfile::TempDir,
    dir: tempfile::TempDir,
    app: axum::Router,
}

/// A core with a temp home and one working folder; `reporter` reports `reported_name` inside it.
fn setup(reported_name: &str) -> Setup {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let reported = dir.path().join(reported_name);
    std::fs::create_dir_all(&reported).unwrap();
    let app = router(state_in(home.path(), &reported));
    Setup { _home: home, dir, app }
}

#[tokio::test]
async fn resolve_finds_files_and_nothing_else() {
    let s = setup("reported");
    write(&s.dir.path().join("src/a.rs"), "fn a() {}");
    let tab = session(&s.app, "sleeper", s.dir.path()).await;
    let abs = canon(&s.dir.path().join("src/a.rs"));
    let got = resolve(&s.app, &tab, &["src/a.rs", "missing.rs", "src", "", &abs]).await;
    assert_eq!(got, vec![Some(abs.clone()), None, None, None, Some(abs.clone())]);
    // `..` and `./` forms land on the same canonical file.
    let got = resolve(&s.app, &tab, &["./src/../src/a.rs"]).await;
    assert_eq!(got, vec![Some(abs)]);
}

#[tokio::test]
async fn resolve_expands_the_home_folder() {
    let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let home = PathBuf::from(std::env::var_os(home_var).expect("a home folder"));
    let file = tempfile::Builder::new()
        .prefix(".marshell-files-test-")
        .tempfile_in(&home)
        .unwrap();
    let name = file.path().file_name().unwrap().to_string_lossy().into_owned();
    let s = setup("reported");
    let tab = session(&s.app, "sleeper", s.dir.path()).await;
    let expected = Some(canon(file.path()));
    let slash = format!("~/{name}");
    let back = format!("~\\{name}");
    let got = resolve(&s.app, &tab, &[&slash, &back, "~", "~other/x"]).await;
    assert_eq!(got, vec![expected.clone(), expected, None, None]);
}

#[tokio::test]
async fn resolve_takes_at_most_64_candidates_of_1024_chars() {
    let s = setup("reported");
    let tab = session(&s.app, "sleeper", s.dir.path()).await;
    let many: Vec<String> = (0..65).map(|i| format!("f{i}.rs")).collect();
    let uri = format!("/v1/sessions/{tab}/resolve");
    let res = call(&s.app, "POST", &uri, Some(serde_json::json!({ "candidates": many }))).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let exactly: Vec<String> = (0..64).map(|i| format!("f{i}.rs")).collect();
    let res = call(&s.app, "POST", &uri, Some(serde_json::json!({ "candidates": exactly }))).await;
    assert_eq!(res.status(), StatusCode::OK);
    let long = "a".repeat(1025);
    let res = call(&s.app, "POST", &uri, Some(serde_json::json!({ "candidates": [long] }))).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let res = call(
        &s.app,
        "POST",
        "/v1/sessions/nope/resolve",
        Some(serde_json::json!({ "candidates": [] })),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn resolve_prefers_the_reported_folder() {
    let s = setup("reported");
    write(&s.dir.path().join("a.rs"), "launch");
    write(&s.dir.path().join("reported/a.rs"), "reported");
    let tab = session(&s.app, "reporter", s.dir.path()).await;
    let want = Some(canon(&s.dir.path().join("reported/a.rs")));
    // The mark arrives some time after the process starts.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let got = resolve(&s.app, &tab, &["a.rs"]).await;
        if got[0] == want {
            break;
        }
        assert!(Instant::now() < deadline, "still {got:?}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn resolve_tries_the_launch_folder_then_the_project_root() {
    let s = setup("reported");
    let root = s.dir.path();
    git(root, &["init", "-q"]);
    write(&root.join("top.rs"), "root");
    write(&root.join("sub/both.rs"), "sub");
    write(&root.join("both.rs"), "root");
    let tab = session(&s.app, "sleeper", &root.join("sub")).await;
    let got = resolve(&s.app, &tab, &["both.rs", "top.rs"]).await;
    assert_eq!(
        got,
        vec![
            Some(canon(&root.join("sub/both.rs"))),
            Some(canon(&root.join("top.rs")))
        ]
    );
}

#[tokio::test]
async fn refused_paths_are_null_in_resolve_and_400_in_stat_and_content() {
    let s = setup("reported");
    let tab = session(&s.app, "sleeper", s.dir.path()).await;
    let unc = r"\\127.0.0.1\c$\x";
    let slashes = "//127.0.0.1/c$/x";
    let device = r"\\?\C:\Windows\win.ini";
    assert_eq!(
        resolve(&s.app, &tab, &[unc, slashes, device]).await,
        vec![None, None, None]
    );
    for path in [unc, slashes, device, "bad\0name"] {
        for route in ["stat", "content"] {
            let uri = format!("/v1/files/{route}?path={}&kind=text", enc(path));
            assert_eq!(
                call(&s.app, "GET", &uri, None).await.status(),
                StatusCode::BAD_REQUEST,
                "{route} {path:?}"
            );
        }
    }
    // A relative path is not a path to read.
    let uri = format!("/v1/files/stat?path={}", enc("src/a.rs"));
    assert_eq!(call(&s.app, "GET", &uri, None).await.status(), StatusCode::BAD_REQUEST);
}

/// Tries a real link to `target`. False when the machine won't make one (Windows without developer
/// mode or elevation).
fn make_link(target: &str, link: &Path, dir: bool) -> bool {
    #[cfg(windows)]
    let made = if dir {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    };
    #[cfg(unix)]
    let made = {
        let _ = dir;
        std::os::unix::fs::symlink(target, link)
    };
    made.is_ok()
}

/// Reading through a link to a network share is refused like the share's own path, and fast: the
/// core never opens the connection that would leak the person's NTLM hash.
#[tokio::test]
async fn links_to_a_network_share_are_refused_without_touching_the_network() {
    let s = setup("reported");
    let root = s.dir.path();
    let file_link = root.join("file-link.txt");
    let dir_link = root.join("dir-link");
    let share = r"\\192.0.2.1\share";
    if !make_link(&format!(r"{share}\x.txt"), &file_link, false) || !make_link(share, &dir_link, true) {
        eprintln!("skipped: this machine can't create symlinks");
        return;
    }
    write(&root.join("plain.txt"), "p");
    let tab = session(&s.app, "sleeper", root).await;
    let started = Instant::now();
    let real_root = PathBuf::from(canon(root));
    let file_link = real_root.join("file-link.txt").to_string_lossy().into_owned();
    let through = real_root.join("dir-link").join("x.txt").to_string_lossy().into_owned();
    for path in [&file_link, &through] {
        for route in ["stat", "content"] {
            let uri = format!("/v1/files/{route}?path={}&kind=text", enc(path));
            assert_eq!(
                call(&s.app, "GET", &uri, None).await.status(),
                StatusCode::BAD_REQUEST,
                "{route} {path}"
            );
        }
    }
    let got = resolve(
        &s.app,
        &tab,
        &["file-link.txt", "dir-link/x.txt", &file_link, &through, "plain.txt"],
    )
    .await;
    assert_eq!(got[..4], [None, None, None, None]);
    assert_eq!(got[4], Some(canon(&root.join("plain.txt"))));
    let list = names(&files_of(&s.app, &tab).await);
    assert!(list.contains(&"plain.txt".to_string()), "{list:?}");
    assert!(!list.iter().any(|f| f.contains("link")), "{list:?}");
    // The same folder as a repo: the git listing checks each file too.
    git(root, &["init", "-q"]);
    let list = names(&files_of(&s.app, &tab).await);
    assert!(list.contains(&"plain.txt".to_string()), "{list:?}");
    assert!(!list.iter().any(|f| f.contains("link")), "{list:?}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "took {:?}",
        started.elapsed()
    );
}

#[cfg(windows)]
#[tokio::test]
async fn windows_device_names_are_refused() {
    let s = setup("reported");
    let tab = session(&s.app, "sleeper", s.dir.path()).await;
    assert_eq!(
        resolve(&s.app, &tab, &["NUL", "con.txt", r"C:\dir\COM1"]).await,
        vec![None, None, None]
    );
    for path in [r"C:\dir\NUL", r"C:\x\con.txt", "C:foo"] {
        let uri = format!("/v1/files/stat?path={}", enc(path));
        assert_eq!(
            call(&s.app, "GET", &uri, None).await.status(),
            StatusCode::BAD_REQUEST,
            "{path}"
        );
    }
}

#[tokio::test]
async fn stat_and_content_agree_and_respect_the_caps() {
    let s = setup("reported");
    let file = s.dir.path().join("note.md");
    write(&file, "# hello\n");
    let abs = canon(&file);

    let res = call(&s.app, "GET", &format!("/v1/files/stat?path={}", enc(&abs)), None).await;
    assert_eq!(res.status(), StatusCode::OK);
    let stat = json_of(res).await;
    assert_eq!(stat["size"], 8);

    let res = call(
        &s.app,
        "GET",
        &format!("/v1/files/content?path={}&kind=text", enc(&abs)),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "text/plain; charset=utf-8");
    assert_eq!(res.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(
        res.headers()["x-modified-ms"].to_str().unwrap(),
        stat["modified_ms"].to_string()
    );
    let bytes = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    assert_eq!(&bytes[..], b"# hello\n");

    // HTML is never served as HTML, and an unknown extension is opaque.
    write(&s.dir.path().join("page.html"), "<script>alert(1)</script>");
    write(&s.dir.path().join("blob.zzz"), "x");
    for (name, want) in [
        ("page.html", "text/plain; charset=utf-8"),
        ("blob.zzz", "application/octet-stream"),
    ] {
        let path = canon(&s.dir.path().join(name));
        let res = call(
            &s.app,
            "GET",
            &format!("/v1/files/content?path={}&kind=text", enc(&path)),
            None,
        )
        .await;
        assert_eq!(res.headers()[header::CONTENT_TYPE], want, "{name}");
    }

    // A folder and a missing file are not files.
    for path in [canon(s.dir.path()), abs.replace("note.md", "gone.md")] {
        for route in ["stat", "content"] {
            let uri = format!("/v1/files/{route}?path={}&kind=text", enc(&path));
            assert_eq!(
                call(&s.app, "GET", &uri, None).await.status(),
                StatusCode::NOT_FOUND,
                "{route} {path}"
            );
        }
    }

    // One byte over the text cap is refused; an image may be that big.
    let big = s.dir.path().join("big.png");
    std::fs::write(&big, vec![b'x'; TEXT_CAP + 1]).unwrap();
    let path = canon(&big);
    let res = call(
        &s.app,
        "GET",
        &format!("/v1/files/content?path={}&kind=text", enc(&path)),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body = json_of(res).await;
    assert_eq!(
        (body["size"].as_u64(), body["limit"].as_u64()),
        (Some(TEXT_CAP as u64 + 1), Some(TEXT_CAP as u64))
    );
    let res = call(
        &s.app,
        "GET",
        &format!("/v1/files/content?path={}&kind=image", enc(&path)),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "image/png");

    // The kind is required.
    let res = call(&s.app, "GET", &format!("/v1/files/content?path={}", enc(&path)), None).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_pdf_is_read_up_to_its_own_cap() {
    let s = setup("reported");
    let big = s.dir.path().join("big.pdf");
    std::fs::write(&big, vec![b'x'; 3 * 1024 * 1024]).unwrap();
    let path = canon(&big);
    let res = call(
        &s.app,
        "GET",
        &format!("/v1/files/content?path={}&kind=pdf", enc(&path)),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/pdf");
    let res = call(
        &s.app,
        "GET",
        &format!("/v1/files/content?path={}&kind=text", enc(&path)),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(json_of(res).await["limit"], 2_097_152);
}

async fn files_of(app: &axum::Router, tab: &str) -> serde_json::Value {
    let res = call(app, "GET", &format!("/v1/sessions/{tab}/files"), None).await;
    assert_eq!(res.status(), StatusCode::OK);
    json_of(res).await
}

fn names(list: &serde_json::Value) -> Vec<String> {
    list["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn the_file_list_in_a_repo_follows_gitignore() {
    let s = setup("reported");
    let root = s.dir.path();
    git(root, &["init", "-q"]);
    write(&root.join(".gitignore"), "ignored/\n");
    write(&root.join("src/tracked.rs"), "t");
    write(&root.join("untracked.rs"), "u");
    write(&root.join("ignored/x.rs"), "i");
    git(root, &["add", "src/tracked.rs"]);
    let tab = session(&s.app, "sleeper", &root.join("src")).await;
    let list = files_of(&s.app, &tab).await;
    let files = names(&list);
    assert!(files.contains(&"src/tracked.rs".to_string()), "{files:?}");
    assert!(files.contains(&"untracked.rs".to_string()), "{files:?}");
    assert!(files.contains(&".gitignore".to_string()), "{files:?}");
    assert!(
        !files
            .iter()
            .any(|f| f.starts_with("ignored/") || f.starts_with(".git/")),
        "{files:?}"
    );
    assert_eq!(list["truncated"], false);
    // The root is the project, not the folder the session started in.
    assert_eq!(list["root"], canon(root));
}

#[tokio::test]
async fn the_file_list_outside_a_repo_skips_build_folders() {
    let s = setup("reported");
    let root = s.dir.path();
    for skipped in ["node_modules", ".git", "target", "dist", "build"] {
        write(&root.join(skipped).join("x.txt"), "x");
        write(&root.join("src").join(skipped).join("y.txt"), "y");
    }
    write(&root.join("keep/deep/z.txt"), "z");
    write(&root.join("top.txt"), "t");
    let tab = session(&s.app, "sleeper", root).await;
    let list = files_of(&s.app, &tab).await;
    assert_eq!(names(&list), vec!["keep/deep/z.txt", "top.txt"]);
    assert_eq!(list["root"], canon(root));
}

#[tokio::test]
async fn the_file_list_root_is_the_launch_folder_when_nothing_else_is_known() {
    let s = setup("reported");
    write(&s.dir.path().join("only.txt"), "x");
    let tab = session(&s.app, "sleeper", s.dir.path()).await;
    let list = files_of(&s.app, &tab).await;
    assert_eq!(list["root"], canon(s.dir.path()));
    assert_eq!(names(&list), vec!["only.txt"]);
    let res = call(&s.app, "GET", "/v1/sessions/nope/files", None).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_file_list_root_is_the_reported_folder_outside_a_repo() {
    let s = setup("reported");
    write(&s.dir.path().join("reported/in.txt"), "x");
    write(&s.dir.path().join("out.txt"), "x");
    let tab = session(&s.app, "reporter", s.dir.path()).await;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let list = files_of(&s.app, &tab).await;
        if list["root"] == canon(&s.dir.path().join("reported")) {
            assert_eq!(names(&list), vec!["in.txt"]);
            break;
        }
        assert!(Instant::now() < deadline, "root is still {}", list["root"]);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn every_route_needs_the_token() {
    let s = setup("reported");
    let tab = session(&s.app, "sleeper", s.dir.path()).await;
    let abs = enc(&canon(s.dir.path()));
    let routes = [
        (
            "POST",
            format!("/v1/sessions/{tab}/resolve"),
            Some(serde_json::json!({ "candidates": [] })),
        ),
        ("GET", format!("/v1/files/stat?path={abs}"), None),
        ("GET", format!("/v1/files/content?path={abs}&kind=text"), None),
        ("GET", format!("/v1/sessions/{tab}/files"), None),
    ];
    for (method, uri, body) in routes {
        let res = s.app.clone().oneshot(request(method, &uri, body, false)).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
}

#[tokio::test]
async fn a_worktree_session_lists_and_resolves_against_its_own_worktree() {
    let s = setup("reported");
    let main = s.dir.path();
    git(main, &["init", "-q"]);
    write(&main.join("shared.rs"), "s");
    write(&main.join("both.rs"), "main");
    git(main, &["add", "."]);
    git(
        main,
        &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "x"],
    );
    write(&main.join("main-only.rs"), "m");
    let linked = tempfile::tempdir().unwrap();
    let wt = linked.path().join("wt");
    git(main, &["worktree", "add", "-q", &wt.to_string_lossy(), "-b", "wt"]);
    write(&wt.join("wt-only.rs"), "w");
    write(&wt.join("both.rs"), "worktree");
    std::fs::create_dir_all(wt.join("sub")).unwrap();

    let tab = session(&s.app, "sleeper", &wt.join("sub")).await;
    let list = files_of(&s.app, &tab).await;
    let files = names(&list);
    assert_eq!(list["root"], canon(&wt));
    assert!(files.contains(&"shared.rs".to_string()), "{files:?}");
    assert!(files.contains(&"wt-only.rs".to_string()), "{files:?}");
    assert!(!files.contains(&"main-only.rs".to_string()), "{files:?}");
    let got = resolve(&s.app, &tab, &["wt-only.rs", "both.rs", "main-only.rs"]).await;
    assert_eq!(
        got,
        vec![
            Some(canon(&wt.join("wt-only.rs"))),
            Some(canon(&wt.join("both.rs"))),
            None
        ]
    );
}
