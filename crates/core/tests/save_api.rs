//! A text tab's save: `PUT /v1/files/content` and `POST /v1/files/unsave`.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use marshell_core::config::hash::hash_bytes;
use marshell_core::server::{AppState, router};
use marshell_core::userfiles::history::History;
use std::path::{Path, PathBuf};
use tower::ServiceExt;

const TOKEN: &str = "test-token";
const PORT: u16 = 5555;
const ORIGIN: &str = "http://tauri.localhost";
const TEXT_CAP: usize = 2 * 1024 * 1024;

/// Marshell's home and a project folder, each in its own temp folder, with the app's router.
struct Setup {
    home: tempfile::TempDir,
    _proj: tempfile::TempDir,
    proj: PathBuf,
    app: axum::Router,
}

fn setup() -> Setup {
    let home = tempfile::tempdir().unwrap();
    let proj_dir = tempfile::tempdir().unwrap();
    let proj = canon(proj_dir.path());
    let app = router(AppState::new(
        TOKEN.into(),
        PORT,
        vec![ORIGIN.into()],
        home.path().to_path_buf(),
        Vec::new(),
    ));
    Setup {
        home,
        _proj: proj_dir,
        proj,
        app,
    }
}

/// The path as the core spells it: canonical, without the `\\?\` prefix.
fn canon(path: &Path) -> PathBuf {
    let real = std::fs::canonicalize(path).unwrap();
    let text = real.to_string_lossy().into_owned();
    PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
}

fn enc(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn request(method: &str, uri: &str, body: Vec<u8>, bearer: bool) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, format!("127.0.0.1:{PORT}"));
    if bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    builder.body(Body::from(body)).unwrap()
}

fn save_uri(path: &str, expect: &str) -> String {
    format!("/v1/files/content?path={}&expect={expect}", enc(path))
}

async fn save(s: &Setup, path: &Path, expect: &str, body: &[u8]) -> (StatusCode, Vec<u8>) {
    save_text(s, &path.to_string_lossy(), expect, body).await
}

async fn save_text(s: &Setup, path: &str, expect: &str, body: &[u8]) -> (StatusCode, Vec<u8>) {
    let res = s
        .app
        .clone()
        .oneshot(request("PUT", &save_uri(path, expect), body.to_vec(), true))
        .await
        .unwrap();
    let status = res.status();
    (
        status,
        axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
}

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap_or_else(|_| panic!("not JSON: {}", String::from_utf8_lossy(bytes)))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Whether the file history holds nothing.
fn no_snapshots(s: &Setup) -> bool {
    let dir = s.home.path().join("file-history");
    !dir.exists()
        || std::fs::read_dir(dir)
            .unwrap()
            .all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".bin"))
}

#[tokio::test]
async fn save_is_refused_without_the_bearer() {
    let s = setup();
    let file = s.proj.join("a.txt");
    std::fs::write(&file, "old").unwrap();
    let uri = save_uri(&file.to_string_lossy(), &hash_bytes(b"old"));
    let res = s
        .app
        .clone()
        .oneshot(request("PUT", &uri, b"new".to_vec(), false))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(std::fs::read(&file).unwrap(), b"old");
}

#[tokio::test]
async fn content_sends_the_hash_of_its_bytes() {
    let s = setup();
    let file = s.proj.join("a.txt");
    std::fs::write(&file, "hello").unwrap();
    let uri = format!("/v1/files/content?path={}&kind=text", enc(&file.to_string_lossy()));
    let req = Request::builder()
        .uri(&uri)
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::ORIGIN, ORIGIN)
        .body(Body::empty())
        .unwrap();
    let res = s.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get("x-content-hash").unwrap(),
        hash_bytes(b"hello").as_str()
    );
    // The webview is another origin: it reads the header only when the reply exposes it.
    let exposed = res
        .headers()
        .get(header::ACCESS_CONTROL_EXPOSE_HEADERS)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        exposed.contains("x-content-hash") && exposed.contains("x-modified-ms"),
        "{exposed}"
    );
}

#[tokio::test]
async fn stat_names_why_a_file_is_read_only() {
    let s = setup();
    let stat = async |path: &Path| {
        let uri = format!("/v1/files/stat?path={}", enc(&path.to_string_lossy()));
        let res = s
            .app
            .clone()
            .oneshot(request("GET", &uri, Vec::new(), true))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        json(&axum::body::to_bytes(res.into_body(), 4096).await.unwrap())["read_only"].clone()
    };
    let plain = s.proj.join("a.txt");
    std::fs::write(&plain, "x").unwrap();
    assert!(stat(&plain).await.is_null());

    let own = canon(s.home.path()).join("settings.json");
    std::fs::write(&own, "{}").unwrap();
    assert_eq!(stat(&own).await, "own");

    let managed = s.proj.join("managed-settings.json");
    std::fs::write(&managed, "{}").unwrap();
    assert_eq!(stat(&managed).await, "managed");

    let locked = s.proj.join("locked.txt");
    std::fs::write(&locked, "x").unwrap();
    set_read_only(&locked, true);
    assert_eq!(stat(&locked).await, "attribute");
    set_read_only(&locked, false);
}

fn set_read_only(path: &Path, on: bool) {
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(on);
    std::fs::set_permissions(path, perms).unwrap();
}

#[tokio::test]
async fn save_writes_the_bytes_and_answers_the_new_stat() {
    let s = setup();
    let file = s.proj.join("a.txt");
    std::fs::write(&file, "old").unwrap();
    let (status, body) = save(&s, &file, &hash_bytes(b"old"), b"new text").await;
    assert_eq!(status, StatusCode::OK, "{}", text(&body));
    assert_eq!(std::fs::read(&file).unwrap(), b"new text");
    let saved = json(&body);
    assert_eq!(saved["hash"], hash_bytes(b"new text"));
    assert_eq!(saved["size"], 8);
    assert!(saved["modified_ms"].as_u64().unwrap() > 0);
    let snapshot = saved["snapshot"].as_str().unwrap();
    let kept = History::new(s.home.path()).get(snapshot).unwrap().unwrap();
    assert_eq!(kept.before, b"old");
    assert_eq!(kept.after_hash, hash_bytes(b"new text"));
}

#[tokio::test]
async fn save_creates_a_deleted_file_with_expect_none() {
    let s = setup();
    let file = s.proj.join("gone.txt");
    let (status, body) = save(&s, &file, "none", b"back").await;
    assert_eq!(status, StatusCode::OK, "{}", text(&body));
    assert_eq!(std::fs::read(&file).unwrap(), b"back");
    assert!(json(&body)["snapshot"].is_null());
    assert!(no_snapshots(&s));
}

#[tokio::test]
async fn save_of_the_same_bytes_writes_nothing() {
    let s = setup();
    let file = s.proj.join("a.txt");
    std::fs::write(&file, "same").unwrap();
    let before = std::fs::metadata(&file).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    let (status, body) = save(&s, &file, &hash_bytes(b"same"), b"same").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(std::fs::metadata(&file).unwrap().modified().unwrap(), before);
    let saved = json(&body);
    assert!(saved["snapshot"].is_null());
    assert_eq!(saved["hash"], hash_bytes(b"same"));
    assert!(no_snapshots(&s));
}

#[tokio::test]
async fn save_conflicts_when_the_disk_changed() {
    let s = setup();
    let file = s.proj.join("a.txt");
    std::fs::write(&file, "an agent wrote this").unwrap();
    let (status, body) = save(&s, &file, &hash_bytes(b"what the tab loaded"), b"mine").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["hash"], hash_bytes(b"an agent wrote this"));
    assert_eq!(std::fs::read(&file).unwrap(), b"an agent wrote this");
    assert!(no_snapshots(&s));
}

#[tokio::test]
async fn save_conflicts_when_the_file_is_gone() {
    let s = setup();
    let file = s.proj.join("gone.txt");
    let (status, body) = save(&s, &file, &hash_bytes(b"old"), b"mine").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(json(&body)["hash"].is_null());
    assert!(!file.exists());
}

#[tokio::test]
async fn save_conflicts_when_expect_none_finds_a_file() {
    let s = setup();
    let file = s.proj.join("a.txt");
    std::fs::write(&file, "it is back").unwrap();
    let (status, body) = save(&s, &file, "none", b"mine").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["hash"], hash_bytes(b"it is back"));
    assert_eq!(std::fs::read(&file).unwrap(), b"it is back");
}

#[tokio::test]
async fn save_refuses_a_bad_path() {
    let s = setup();
    let file = s.proj.join("a.txt");
    std::fs::write(&file, "old").unwrap();
    let old = hash_bytes(b"old");
    let folder = s.proj.join("folder");
    std::fs::create_dir(&folder).unwrap();
    let dotted = s.proj.join("folder").join("..").join("a.txt");
    let mut bad: Vec<(String, &str)> = vec![
        (r"\\server\share\a.txt".into(), old.as_str()),
        ("//server/share/a.txt".into(), old.as_str()),
        (format!(r"\\?\{}", file.display()), old.as_str()),
        ("a.txt".into(), old.as_str()),
        (dotted.to_string_lossy().into_owned(), old.as_str()),
        (folder.to_string_lossy().into_owned(), old.as_str()),
        (folder.to_string_lossy().into_owned(), "none"),
        (
            s.proj.join("missing").join("a.txt").to_string_lossy().into_owned(),
            "none",
        ),
        // The expect is neither a hash nor "none".
        (file.to_string_lossy().into_owned(), "abc"),
        (file.to_string_lossy().into_owned(), ""),
    ];
    if cfg!(windows) {
        bad.push((s.proj.join("NUL").to_string_lossy().into_owned(), "none"));
        bad.push((format!("{}:stream", file.display()), "none"));
        bad.push((s.proj.join("con.txt").to_string_lossy().into_owned(), "none"));
    }
    for (path, expect) in bad {
        let (status, body) = save_text(&s, &path, expect, b"mine").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path} {expect}");
        assert_eq!(text(&body), "That path can't be saved.", "{path} {expect}");
    }
    // No expect at all.
    let uri = format!("/v1/files/content?path={}", enc(&file.to_string_lossy()));
    let res = s
        .app
        .clone()
        .oneshot(request("PUT", &uri, b"mine".to_vec(), true))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    assert_eq!(std::fs::read(&file).unwrap(), b"old");
    assert!(!s.proj.join("missing").exists());
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
    assert!(no_snapshots(&s));
}

#[tokio::test]
async fn save_refuses_marshells_own_folder() {
    let s = setup();
    let home = canon(s.home.path());
    let own = home.join("settings.json");
    std::fs::write(&own, "{}").unwrap();
    std::fs::create_dir_all(home.join("agents")).unwrap();
    let agent = home.join("agents").join("x.md");
    std::fs::write(&agent, "{}").unwrap();
    for (path, expect) in [
        (own, hash_bytes(b"{}")),
        (agent, hash_bytes(b"{}")),
        (home.join("new.txt"), "none".into()),
    ] {
        let existed = path.exists();
        let (status, body) = save(&s, &path, &expect, b"mine").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path:?}");
        assert_eq!(text(&body), "Marshell doesn't save files in its own folder.");
        assert_eq!(path.exists(), existed);
        if existed {
            assert_eq!(std::fs::read(&path).unwrap(), b"{}");
        }
    }
    assert!(no_snapshots(&s));
}

#[tokio::test]
async fn save_refuses_a_managed_name() {
    let s = setup();
    let name = if cfg!(windows) {
        "Managed-Settings.JSON"
    } else {
        "managed-settings.json"
    };
    let file = s.proj.join(name);
    std::fs::write(&file, "{}").unwrap();
    let (status, body) = save(&s, &file, &hash_bytes(b"{}"), b"mine").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        text(&body),
        "This file is managed by your organization, so Marshell doesn't save it."
    );
    assert_eq!(std::fs::read(&file).unwrap(), b"{}");
    assert!(no_snapshots(&s));
}

#[tokio::test]
async fn save_refuses_a_read_only_file() {
    let s = setup();
    let file = s.proj.join("locked.txt");
    std::fs::write(&file, "old").unwrap();
    set_read_only(&file, true);
    let (status, body) = save(&s, &file, &hash_bytes(b"old"), b"mine").await;
    set_read_only(&file, false);
    // A root on Unix writes a read-only file anyway; the answer is checked where the OS refuses.
    if std::fs::read(&file).unwrap() == b"old" {
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(text(&body), "The file is read-only or in use by another program.");
    }
}

#[tokio::test]
async fn save_refuses_a_body_over_the_cap() {
    let s = setup();
    let file = s.proj.join("a.txt");
    std::fs::write(&file, "old").unwrap();
    let (status, _) = save(&s, &file, &hash_bytes(b"old"), &vec![b'x'; TEXT_CAP + 1]).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(std::fs::read(&file).unwrap(), b"old");
    // Exactly the cap is fine.
    let (status, _) = save(&s, &file, &hash_bytes(b"old"), &vec![b'x'; TEXT_CAP]).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_saves_of_one_file_run_one_after_the_other() {
    let s = setup();
    let file = s.proj.join("a.txt");
    std::fs::write(&file, "old").unwrap();
    let old = hash_bytes(b"old");
    let (one, two) = tokio::join!(save(&s, &file, &old, b"first"), save(&s, &file, &old, b"second"));
    let mut statuses = [one.0, two.0];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
    let on_disk = std::fs::read(&file).unwrap();
    assert!(on_disk == b"first" || on_disk == b"second");
}

#[tokio::test]
async fn a_save_through_a_link_into_marshells_home_is_refused() {
    let s = setup();
    let home = canon(s.home.path());
    std::fs::write(home.join("settings.json"), "{}").unwrap();
    let link = s.proj.join("inside");
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&home, &link).is_ok();
    #[cfg(windows)]
    let made = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&home)
        .output()
        .is_ok_and(|o| o.status.success());
    // A junction needs no privilege, so on Windows the link is always made.
    assert!(made || !cfg!(windows));
    if !made {
        return;
    }
    let (status, _) = save(&s, &link.join("settings.json"), &hash_bytes(b"{}"), b"mine").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(std::fs::read(home.join("settings.json")).unwrap(), b"{}");
}
async fn unsave_as(s: &Setup, path: &str, snapshot: &str, bearer: bool) -> (StatusCode, Vec<u8>) {
    let body = serde_json::json!({ "path": path, "snapshot": snapshot }).to_string();
    let mut req = request("POST", "/v1/files/unsave", body.into_bytes(), bearer);
    req.headers_mut()
        .insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    let res = s.app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (
        status,
        axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
}

async fn unsave(s: &Setup, path: &Path, snapshot: &str) -> (StatusCode, Vec<u8>) {
    unsave_as(s, &path.to_string_lossy(), snapshot, true).await
}

/// `name` in the project holding "old", saved over with "new". The file and the save's snapshot.
async fn saved_file(s: &Setup, name: &str) -> (PathBuf, String) {
    let file = s.proj.join(name);
    std::fs::write(&file, "old").unwrap();
    let (status, body) = save(s, &file, &hash_bytes(b"old"), b"new").await;
    assert_eq!(status, StatusCode::OK, "{}", text(&body));
    (file, json(&body)["snapshot"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn unsave_is_refused_without_the_bearer() {
    let s = setup();
    let (file, snapshot) = saved_file(&s, "a.txt").await;
    let (status, _) = unsave_as(&s, &file.to_string_lossy(), &snapshot, false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(std::fs::read(&file).unwrap(), b"new");
}

#[tokio::test]
async fn unsave_puts_back_the_bytes_before_the_save() {
    let s = setup();
    let (file, snapshot) = saved_file(&s, "a.txt").await;
    let (status, body) = unsave(&s, &file, &snapshot).await;
    assert_eq!(status, StatusCode::OK, "{}", text(&body));
    assert_eq!(std::fs::read(&file).unwrap(), b"old");
    let undone = json(&body);
    assert_eq!(undone["hash"], hash_bytes(b"old"));
    assert_eq!(undone["size"], 3);
}

#[tokio::test]
async fn unsave_keeps_what_it_replaced() {
    let s = setup();
    let (file, snapshot) = saved_file(&s, "a.txt").await;
    let (_, body) = unsave(&s, &file, &snapshot).await;
    let undone = json(&body);
    let kept = undone["snapshot"].as_str().unwrap();
    assert_ne!(kept, snapshot);
    let kept = History::new(s.home.path()).get(kept).unwrap().unwrap();
    assert_eq!(kept.before, b"new");
    assert_eq!(kept.after_hash, hash_bytes(b"old"));
    // So the undo can be undone the same way.
    let (status, _) = unsave(&s, &file, undone["snapshot"].as_str().unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(std::fs::read(&file).unwrap(), b"new");
}

#[tokio::test]
async fn unsave_conflicts_when_the_file_changed_since() {
    let s = setup();
    let (file, snapshot) = saved_file(&s, "a.txt").await;
    std::fs::write(&file, "an agent wrote this").unwrap();
    let (status, body) = unsave(&s, &file, &snapshot).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["hash"], hash_bytes(b"an agent wrote this"));
    assert_eq!(std::fs::read(&file).unwrap(), b"an agent wrote this");
}

#[tokio::test]
async fn unsave_conflicts_when_the_file_is_gone() {
    let s = setup();
    let (file, snapshot) = saved_file(&s, "a.txt").await;
    std::fs::remove_file(&file).unwrap();
    let (status, body) = unsave(&s, &file, &snapshot).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(json(&body)["hash"].is_null());
    assert!(!file.exists());
}

#[tokio::test]
async fn unsave_of_another_files_snapshot_is_a_404() {
    let s = setup();
    let (_, snapshot) = saved_file(&s, "a.txt").await;
    // The other file holds what the save wrote, so only the path tells them apart.
    let other = s.proj.join("b.txt");
    std::fs::write(&other, "new").unwrap();
    let (status, _) = unsave(&s, &other, &snapshot).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(std::fs::read(&other).unwrap(), b"new");
}

#[tokio::test]
async fn unsave_of_a_malformed_id_is_a_404() {
    let s = setup();
    let (file, _) = saved_file(&s, "a.txt").await;
    for id in ["../x", "", "f000000000000", "index"] {
        let (status, _) = unsave(&s, &file, id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id:?}");
    }
    assert_eq!(std::fs::read(&file).unwrap(), b"new");
}

#[tokio::test]
async fn unsave_refuses_marshells_own_folder() {
    let s = setup();
    let (_, snapshot) = saved_file(&s, "a.txt").await;
    let own = canon(s.home.path()).join("settings.json");
    std::fs::write(&own, "new").unwrap();
    let (status, body) = unsave(&s, &own, &snapshot).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(text(&body), "Marshell doesn't save files in its own folder.");
    assert_eq!(std::fs::read(&own).unwrap(), b"new");
}

#[tokio::test]
async fn unsave_refuses_a_bad_path() {
    let s = setup();
    let (file, snapshot) = saved_file(&s, "a.txt").await;
    let dotted = s.proj.join("x").join("..").join("a.txt");
    for path in [
        r"\\server\share\a.txt".to_string(),
        "a.txt".into(),
        dotted.to_string_lossy().into_owned(),
    ] {
        let (status, body) = unsave_as(&s, &path, &snapshot, true).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(text(&body), "That path can't be saved.");
    }
    assert_eq!(std::fs::read(&file).unwrap(), b"new");
}

#[tokio::test]
async fn unsave_refuses_a_read_only_file() {
    let s = setup();
    let (file, snapshot) = saved_file(&s, "a.txt").await;
    set_read_only(&file, true);
    let (status, body) = unsave(&s, &file, &snapshot).await;
    set_read_only(&file, false);
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(text(&body), "The file is read-only or in use by another program.");
    assert_eq!(std::fs::read(&file).unwrap(), b"new");
}

#[tokio::test]
async fn a_pdf_save_may_pass_the_text_cap() {
    let s = setup();
    let file = s.proj.join("x.PDF");
    let body = vec![b'p'; 3 * 1024 * 1024];
    let (status, _) = save(&s, &file, "none", &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(std::fs::read(&file).unwrap(), body);
    // A file that big is replaced under its hash, too.
    let newer = vec![b'q'; 3 * 1024 * 1024 + 5];
    let (status, _) = save(&s, &file, &hash_bytes(&body), &newer).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(std::fs::read(&file).unwrap(), newer);
}

#[tokio::test]
async fn a_text_save_still_stops_at_the_text_cap() {
    let s = setup();
    let file = s.proj.join("x.txt");
    let (status, _) = save(&s, &file, "none", &vec![b't'; 3 * 1024 * 1024]).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!file.exists());
    let (status, _) = save(&s, &file, "none", &vec![b't'; TEXT_CAP]).await;
    assert_eq!(status, StatusCode::OK);
}
