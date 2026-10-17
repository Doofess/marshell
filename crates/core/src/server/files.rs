//! The files API behind the preview pane, clickable paths and quick open.
//!
//! These routes read any file the person can read, and `resolve` runs on mouse hover over text a
//! program printed, so every path goes through `refused` before the first filesystem call. Touching
//! a UNC path would send the person's NTLM hash to whoever the server name points at.

use super::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderName, StatusCode, header};
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{FileList, FileStat, ResolveRequest, ResolveResponse, SessionInfo};
use serde::Deserialize;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, Instant, UNIX_EPOCH};

/// The most bytes `content` serves for a text file.
pub const TEXT_CAP: u64 = 2 * 1024 * 1024;
/// The most bytes `content` serves for an image.
pub const IMAGE_CAP: u64 = 20 * 1024 * 1024;
/// The most candidates one `resolve` call takes.
pub const MAX_CANDIDATES: usize = 64;
/// The longest candidate `resolve` takes, in characters.
pub const MAX_CANDIDATE_LEN: usize = 1024;
/// The folder list stops here and sets `truncated`.
pub const MAX_LIST_FILES: usize = 20_000;
/// The folder list also stops after this long, so a huge folder can't hang the palette.
const LIST_BUDGET: Duration = Duration::from_secs(2);
/// Folders the walk outside a repo does not enter.
const SKIPPED_DIRS: [&str; 5] = ["node_modules", ".git", "target", "dist", "build"];

const MODIFIED_MS: HeaderName = HeaderName::from_static("x-modified-ms");

/// True for a path that must not reach the filesystem: a UNC or device path, a NUL character, and
/// on Windows a reserved device name in any component or a drive-relative form. It reads the
/// string only; the UNC and device rules hold on every platform, since the string may come from a
/// shell on another one.
pub fn refused(path: &str) -> bool {
    if path.contains('\0') {
        return true;
    }
    let sep = |c: char| c == '/' || c == '\\';
    // `\\server\share`, `//server/share`, `\\?\`, `\\.\`, `//?/`, `//./`, and the NT form `\??\`.
    let mut chars = path.chars();
    if let (Some(a), Some(b)) = (chars.next(), chars.next())
        && sep(a)
        && sep(b)
    {
        return true;
    }
    if path.starts_with(r"\??\") {
        return true;
    }
    #[cfg(windows)]
    if windows_refused(path) {
        return true;
    }
    false
}

#[cfg(windows)]
fn windows_refused(path: &str) -> bool {
    let bytes = path.as_bytes();
    // `C:foo` is relative to the drive's current folder, and a colon past the drive letter names
    // an alternate data stream. `C:` and `C:\x` are the only forms with a colon.
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if drive && bytes.get(2).is_some_and(|b| *b != b'/' && *b != b'\\') {
        return true;
    }
    if drive && bytes.len() == 2 {
        return true;
    }
    if path[if drive { 2 } else { 0 }..].contains(':') {
        return true;
    }
    path.split(['/', '\\']).any(reserved_name)
}

/// Whether a path component is a DOS device name: `CON`, `NUL.txt`, `com1 `, `LPT¹`, `CONIN$`.
#[cfg(windows)]
fn reserved_name(component: &str) -> bool {
    // Windows ignores trailing dots and spaces, and everything from the first dot is an extension.
    let trimmed = component.trim_end_matches(['.', ' ']);
    let stem = trimmed.split('.').next().unwrap_or("").trim_end_matches(' ');
    let upper = stem.to_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") {
        return true;
    }
    let mut rest = ["COM", "LPT"].iter().filter_map(|p| upper.strip_prefix(p));
    rest.any(|digit| {
        let mut it = digit.chars();
        matches!((it.next(), it.next()), (Some('0'..='9' | '¹' | '²' | '³'), None))
    })
}

/// `\\?\C:\x` as `C:\x`. None for `\\?\UNC\...`, which is a network path.
fn strip_verbatim(path: &std::path::Path) -> Option<String> {
    let text = path.to_str()?;
    if text.starts_with(r"\\?\UNC\") {
        return None;
    }
    Some(text.strip_prefix(r"\\?\").unwrap_or(text).to_string())
}

fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// `~`, `~/x` and `~\x` against the home folder. Anything else is returned as it is.
fn expand_home(candidate: &str) -> Option<String> {
    let Some(rest) = candidate.strip_prefix('~') else {
        return Some(candidate.to_string());
    };
    if !(rest.is_empty() || rest.starts_with(['/', '\\'])) {
        return Some(candidate.to_string());
    }
    let home = home_dir()?;
    let rest = rest.trim_start_matches(['/', '\\']);
    Some(home.join(rest).to_string_lossy().into_owned())
}

/// The canonical path of `path` when it is a regular file (after following symlinks).
fn existing_file(path: &str) -> Option<String> {
    if refused(path) {
        return None;
    }
    let canonical = std::fs::canonicalize(path).ok()?;
    let text = strip_verbatim(&canonical)?;
    // A symlink may lead somewhere the string did not show.
    if refused(&text) || !std::fs::metadata(&canonical).ok()?.is_file() {
        return None;
    }
    Some(text)
}

/// One candidate against the bases in order. The first base where the file exists wins.
fn resolve_one(candidate: &str, bases: &[PathBuf]) -> Option<String> {
    if candidate.is_empty() || refused(candidate) {
        return None;
    }
    let expanded = expand_home(candidate)?;
    if refused(&expanded) {
        return None;
    }
    if std::path::Path::new(&expanded).is_absolute() {
        return existing_file(&expanded);
    }
    bases.iter().find_map(|base| {
        let joined = base.join(&expanded).to_string_lossy().into_owned();
        // The base is whatever the shell reported, so the joined path is checked again.
        existing_file(&joined)
    })
}

/// The folders a session's relative paths are tried against, most specific first: the folder the
/// shell last reported, the launch folder, the project root.
fn bases_of(row: Option<&SessionInfo>, reported: Option<PathBuf>) -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = reported.into_iter().collect();
    if let Some(row) = row {
        bases.extend(
            [&row.cwd, &row.project]
                .into_iter()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
        );
    }
    bases
}

fn row_of(st: &AppState, tab: &str) -> Option<SessionInfo> {
    st.registry().snapshot().sessions.into_iter().find(|s| s.tab_id == tab)
}

/// POST /v1/sessions/{tab}/resolve
pub async fn resolve(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<ResolveRequest>) -> Response {
    let row = row_of(&st, &tab);
    let session = st.session(&tab);
    // The shell panel's shell has a session but no row.
    if row.is_none() && session.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    if req.candidates.len() > MAX_CANDIDATES || req.candidates.iter().any(|c| c.chars().count() > MAX_CANDIDATE_LEN) {
        return (
            StatusCode::BAD_REQUEST,
            format!("At most {MAX_CANDIDATES} paths of {MAX_CANDIDATE_LEN} characters."),
        )
            .into_response();
    }
    let bases = bases_of(row.as_ref(), session.and_then(|s| s.current_cwd()));
    let done = tokio::task::spawn_blocking(move || {
        req.candidates
            .iter()
            .map(|c| resolve_one(c, &bases))
            .collect::<Vec<_>>()
    })
    .await;
    match done {
        Ok(paths) => Json(ResolveResponse { paths }).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct FileQuery {
    path: String,
    kind: Option<String>,
}

/// The path as a `PathBuf` when it may be read: absolute and not refused.
fn checked(path: &str) -> Result<PathBuf, (StatusCode, &'static str)> {
    if refused(path) {
        return Err((StatusCode::BAD_REQUEST, "That path can't be read."));
    }
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err((StatusCode::BAD_REQUEST, "The path must be absolute."));
    }
    Ok(path)
}

fn modified_ms(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

/// Opens a regular file. None for a missing path, a folder or a device. On Unix the open does not
/// block, so a FIFO swapped in after the check can't hang the thread.
fn open_regular(path: &std::path::Path) -> Option<(std::fs::File, std::fs::Metadata)> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path).ok()?;
    let meta = file.metadata().ok()?;
    meta.is_file().then_some((file, meta))
}

/// GET /v1/files/stat
pub async fn stat(Query(q): Query<FileQuery>) -> Response {
    let path = match checked(&q.path) {
        Ok(p) => p,
        Err(refusal) => return refusal.into_response(),
    };
    let meta = tokio::task::spawn_blocking(move || open_regular(&path).map(|(_, meta)| meta)).await;
    match meta {
        Ok(Some(meta)) => Json(FileStat {
            size: meta.len(),
            modified_ms: modified_ms(&meta),
        })
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The Content-Type for a file name's extension. Text kinds, HTML included, are `text/plain`: the
/// frontend renders them itself, and the core never serves anything a webview would run.
fn content_type(path: &std::path::Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "txt" | "text" | "md" | "markdown" | "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "json" | "jsonc"
        | "toml" | "yaml" | "yml" | "css" | "scss" | "html" | "htm" | "xml" | "csv" | "log" | "ini" | "cfg"
        | "conf" | "env" | "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd" | "py" | "rb" | "go" | "java" | "kt" | "c"
        | "h" | "cpp" | "hpp" | "cs" | "swift" | "sql" | "lock" | "gitignore" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// What reading a file ended in.
enum Outcome {
    Bytes(Vec<u8>, u64),
    TooLarge { size: u64 },
    Missing,
}

/// Reads at most `cap` bytes. It reads `cap + 1` at most, so a file that grows can't pass the cap.
fn read_capped(path: &std::path::Path, cap: u64) -> Outcome {
    let Some((file, meta)) = open_regular(path) else {
        return Outcome::Missing;
    };
    if meta.len() > cap {
        return Outcome::TooLarge { size: meta.len() };
    }
    let mut bytes = Vec::new();
    if file.take(cap + 1).read_to_end(&mut bytes).is_err() {
        return Outcome::Missing;
    }
    if bytes.len() as u64 > cap {
        // It grew after the check: the size is what it is now.
        let size = open_regular(path).map_or(0, |(_, m)| m.len()).max(bytes.len() as u64);
        return Outcome::TooLarge { size };
    }
    Outcome::Bytes(bytes, modified_ms(&meta))
}

/// GET /v1/files/content
pub async fn content(Query(q): Query<FileQuery>) -> Response {
    let path = match checked(&q.path) {
        Ok(p) => p,
        Err(refusal) => return refusal.into_response(),
    };
    let cap = match q.kind.as_deref() {
        Some("text") => TEXT_CAP,
        Some("image") => IMAGE_CAP,
        _ => return (StatusCode::BAD_REQUEST, "The kind is text or image.").into_response(),
    };
    let ctype = content_type(&path);
    let read = tokio::task::spawn_blocking(move || read_capped(&path, cap)).await;
    match read {
        Ok(Outcome::Bytes(bytes, modified)) => (
            [
                (header::CONTENT_TYPE, ctype.to_string()),
                (MODIFIED_MS, modified.to_string()),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            ],
            bytes,
        )
            .into_response(),
        Ok(Outcome::TooLarge { size }) => (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({ "size": size, "limit": cap })),
        )
            .into_response(),
        Ok(Outcome::Missing) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `path` relative to `root` with `/` separators. None when it is not under `root` or not UTF-8.
fn relative(root: &std::path::Path, path: &std::path::Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rel.components().map(|c| c.as_os_str().to_str()).collect();
    Some(parts?.join("/"))
}

/// Tracked and untracked files, minus the ignored ones. None when `root` is not in a repo or git
/// is not installed. Files the index lists but the disk lacks, and submodule folders, are left out.
fn git_files(root: &std::path::Path) -> Option<(Vec<String>, bool)> {
    let out = crate::git::run(root, &["ls-files", "--cached", "--others", "--exclude-standard", "-z"]).ok()?;
    if !out.ok {
        return None;
    }
    let started = Instant::now();
    let mut seen = std::collections::HashSet::new();
    let mut files = Vec::new();
    for name in out.stdout.split('\0').filter(|n| !n.is_empty()) {
        if files.len() >= MAX_LIST_FILES || started.elapsed() > LIST_BUDGET {
            return Some((files, true));
        }
        if seen.insert(name) && std::fs::metadata(root.join(name)).is_ok_and(|m| m.is_file()) {
            files.push(name.to_string());
        }
    }
    Some((files, false))
}

/// The files under `root` outside a repo. Symlinked and junctioned folders are not entered.
fn walk_files(root: &std::path::Path) -> (Vec<String>, bool) {
    let started = Instant::now();
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            if files.len() >= MAX_LIST_FILES || started.elapsed() > LIST_BUDGET {
                files.sort();
                return (files, true);
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                let skipped = entry.file_name().to_str().is_some_and(|n| SKIPPED_DIRS.contains(&n));
                if !skipped {
                    folders.push(path);
                }
            } else if kind.is_file() || (kind.is_symlink() && std::fs::metadata(&path).is_ok_and(|m| m.is_file())) {
                files.extend(relative(root, &path));
            }
        }
    }
    files.sort();
    (files, false)
}

/// The first of these that is a folder the core may touch, spelled as the disk spells it.
fn first_folder(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates
        .into_iter()
        .find(|p| p.to_str().is_some_and(|s| !s.is_empty() && !refused(s)) && p.is_dir())
        .map(|p| crate::projects::canonical(&p))
        // A symlink may have led somewhere the string did not show.
        .filter(|p| p.to_str().is_some_and(|s| !refused(s)))
}

/// GET /v1/sessions/{tab}/files
pub async fn list(State(st): State<AppState>, Path(tab): Path<String>) -> Response {
    let row = row_of(&st, &tab);
    let session = st.session(&tab);
    if row.is_none() && session.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let reported = session.and_then(|s| s.current_cwd());
    let done = tokio::task::spawn_blocking(move || {
        // In a repo the root is the project. `project` is the launch folder itself outside git, so
        // the repo check looks for a `.git`, as `projects` does, instead of trusting the field.
        let in_repo = row
            .as_ref()
            .is_some_and(|r| crate::projects::repo_root(std::path::Path::new(&r.cwd)).is_some());
        if in_repo && let Some(root) = first_folder(row.iter().map(|r| PathBuf::from(&r.project))) {
            // Without git on PATH the walk still gives a list, though it lists ignored files.
            let (files, truncated) = git_files(&root).unwrap_or_else(|| walk_files(&root));
            return Some((root, files, truncated));
        }
        let launch = row.iter().map(|r| PathBuf::from(&r.cwd));
        let root = first_folder(reported.into_iter().chain(launch))?;
        let (files, truncated) = walk_files(&root);
        Some((root, files, truncated))
    })
    .await;
    match done {
        Ok(Some((root, files, truncated))) => Json(FileList {
            root: strip_verbatim(&root).unwrap_or_default(),
            files,
            truncated,
        })
        .into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "This session has no folder.").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_unc_device_and_nul_paths_everywhere() {
        for p in [
            r"\\srv\share\a",
            "//srv/share/a",
            r"\\?\C:\a",
            r"\\.\pipe\x",
            "//?/C:/a",
            "//./pipe/x",
            r"\??\C:\a",
            "/home/u/a\0b",
        ] {
            assert!(refused(p), "{p:?} should be refused");
        }
    }

    #[test]
    fn allows_ordinary_paths() {
        for p in [
            r"C:\a\b.rs",
            "C:/a/b.rs",
            "/home/u/a",
            "src/a.rs",
            "a/b//c.rs",
            "~/x",
            "my.con.rs",
        ] {
            assert!(!refused(p), "{p:?} should be allowed");
        }
    }

    #[cfg(windows)]
    #[test]
    fn refuses_reserved_names_and_drive_relative_paths_on_windows() {
        for p in [
            r"C:\dir\NUL",
            "con.txt",
            "COM1",
            r"a\Com9.log\b",
            "src/lpt3",
            "PRN.",
            "aux .txt",
            "CONIN$",
            r"C:\x\conout$",
            "COM\u{b9}",
            "COM0",
            r"C:foo",
            "C:",
            r"C:\a\b.txt:stream",
            "nul:x",
        ] {
            assert!(refused(p), "{p:?} should be refused");
        }
        for p in ["console.rs", "COM10", "com.rs", "nulls", r"C:\a\b.rs"] {
            assert!(!refused(p), "{p:?} should be allowed");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn reserved_names_are_ordinary_files_off_windows() {
        assert!(!refused("con.txt"));
        assert!(!refused("a:b"));
    }

    #[test]
    fn strips_the_verbatim_prefix_and_refuses_unc_results() {
        let p = |s: &str| strip_verbatim(std::path::Path::new(s));
        assert_eq!(p(r"\\?\C:\a\b").as_deref(), Some(r"C:\a\b"));
        assert_eq!(p(r"C:\a\b").as_deref(), Some(r"C:\a\b"));
        assert_eq!(p(r"\\?\UNC\srv\share\a"), None);
    }

    #[test]
    fn content_types_come_from_the_extension() {
        let ct = |s: &str| content_type(std::path::Path::new(s));
        assert_eq!(ct("a.PNG"), "image/png");
        assert_eq!(ct("a.svg"), "image/svg+xml");
        assert_eq!(ct("a.rs"), "text/plain; charset=utf-8");
        // Never text/html: the frontend renders HTML itself.
        assert_eq!(ct("index.html"), "text/plain; charset=utf-8");
        assert_eq!(ct("a.exe"), "application/octet-stream");
        assert_eq!(ct("noext"), "application/octet-stream");
    }

    #[test]
    fn relative_paths_use_forward_slashes() {
        let root = std::path::Path::new("root");
        assert_eq!(relative(root, &root.join("a").join("b.rs")).as_deref(), Some("a/b.rs"));
        assert_eq!(relative(root, std::path::Path::new("other/x")), None);
    }
}
