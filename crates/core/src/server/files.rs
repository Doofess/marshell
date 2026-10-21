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
/// The largest PDF `GET /v1/files/content?kind=pdf` serves.
pub const PDF_CAP: u64 = 50 * 1024 * 1024;
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

/// The most links one path may pass through before it is refused as a loop.
const MAX_LINK_HOPS: usize = 40;

/// A link target as text for `refused`. Windows writes a junction's or a symlink's local target as
/// `\??\C:\x` or `\\?\C:\x`, which are not network paths, so those lose the prefix. The UNC forms
/// keep theirs and stay refused.
fn link_target_text(target: &std::path::Path) -> Option<String> {
    let text = target.to_str()?;
    for prefix in [r"\??\", r"\\?\"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            let b = rest.as_bytes();
            let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/');
            if drive {
                return Some(rest.to_string());
            }
        }
    }
    Some(text.to_string())
}

/// Whether following `path` would pass through a link whose target is refused, or through more than
/// `MAX_LINK_HOPS` links. It walks the components from the root and asks `read` about each folder
/// or file reached: `Some(target)` for a link, `None` for anything else. A missing path is not a
/// link; any other error refuses. Nothing is asked past a refused target, and `read` must not follow
/// links itself.
fn links_refused_with(
    path: &std::path::Path,
    read: &impl Fn(&std::path::Path) -> std::io::Result<Option<PathBuf>>,
) -> bool {
    use std::collections::VecDeque;
    use std::ffi::OsString;
    use std::path::Component;

    /// A path component that owns its text, so a link's target can be put in front of the rest.
    enum Part {
        Prefix(OsString),
        Root,
        Current,
        Parent,
        Name(OsString),
    }
    fn parts(path: &std::path::Path) -> impl Iterator<Item = Part> + '_ {
        path.components().map(|c| match c {
            Component::Prefix(p) => Part::Prefix(p.as_os_str().to_os_string()),
            Component::RootDir => Part::Root,
            Component::CurDir => Part::Current,
            Component::ParentDir => Part::Parent,
            Component::Normal(n) => Part::Name(n.to_os_string()),
        })
    }

    let mut rest: VecDeque<Part> = parts(path).collect();
    let mut current = PathBuf::new();
    let mut hops = 0;
    while let Some(part) = rest.pop_front() {
        match part {
            Part::Prefix(p) => current = PathBuf::from(p),
            Part::Root => current.push(Component::RootDir.as_os_str()),
            Part::Current => {}
            Part::Parent => {
                current.pop();
            }
            Part::Name(name) => {
                current.push(name);
                let target = match read(&current) {
                    Ok(Some(target)) => target,
                    Ok(None) => continue,
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                        ) =>
                    {
                        continue;
                    }
                    Err(_) => return true,
                };
                hops += 1;
                if hops > MAX_LINK_HOPS || link_target_text(&target).is_none_or(|t| refused(&t)) {
                    return true;
                }
                // The target replaces the link: a relative one starts in the folder holding it.
                current.pop();
                for part in parts(&target).collect::<Vec<_>>().into_iter().rev() {
                    rest.push_front(part);
                }
            }
        }
    }
    false
}

/// `links_refused_with` against the disk: `symlink_metadata` never follows, and a Windows junction
/// counts as a link. A link `read_link` can't read is an error, so the path is refused.
fn links_refused(path: &std::path::Path) -> bool {
    links_refused_with(path, &|p| {
        if std::fs::symlink_metadata(p)?.file_type().is_symlink() {
            std::fs::read_link(p).map(Some)
        } else {
            Ok(None)
        }
    })
}

pub(super) fn home_dir() -> Option<PathBuf> {
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
pub(crate) fn existing_file(path: &str) -> Option<String> {
    // A link on the way may lead to a network path, so the links are read before anything follows them.
    if refused(path) || links_refused(std::path::Path::new(path)) {
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

/// The checkout the session was launched in: the nearest folder above the launch folder (or it
/// itself) with a `.git` folder or file. A linked worktree has a `.git` file, so this is the
/// worktree, where `projects::repo_root` gives the main checkout. None outside git.
fn checkout_of(row: &SessionInfo) -> Option<PathBuf> {
    if row.cwd.is_empty() || refused(&row.cwd) || links_refused(std::path::Path::new(&row.cwd)) {
        return None;
    }
    let root = std::path::Path::new(&row.cwd)
        .ancestors()
        .find(|d| d.join(".git").exists())?;
    Some(crate::projects::canonical(root))
}

/// The folders a session's relative paths are tried against, most specific first: the folder the
/// shell last reported, the launch folder, the launch folder's checkout (the project root when
/// that finds nothing).
fn bases_of(row: Option<&SessionInfo>, reported: Option<PathBuf>) -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = reported.into_iter().collect();
    if let Some(row) = row {
        let top = checkout_of(row).unwrap_or_else(|| PathBuf::from(&row.project));
        bases.extend(
            [PathBuf::from(&row.cwd), top]
                .into_iter()
                .filter(|p| !p.as_os_str().is_empty()),
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
    let reported = session.and_then(|s| s.current_cwd());
    let done = tokio::task::spawn_blocking(move || {
        let bases = bases_of(row.as_ref(), reported);
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

/// `checked`, and the links on the way are read too, off the async threads.
async fn checked_links(path: &str) -> Result<PathBuf, (StatusCode, &'static str)> {
    let path = checked(path)?;
    let probe = path.clone();
    match tokio::task::spawn_blocking(move || links_refused(&probe)).await {
        Ok(false) => Ok(path),
        Ok(true) => Err((StatusCode::BAD_REQUEST, "That path can't be read.")),
        Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, "The path could not be checked.")),
    }
}

/// GET /v1/files/stat
pub async fn stat(Query(q): Query<FileQuery>) -> Response {
    let path = match checked_links(&q.path).await {
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
        "pdf" => "application/pdf",
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
    let path = match checked_links(&q.path).await {
        Ok(p) => p,
        Err(refusal) => return refusal.into_response(),
    };
    let cap = match q.kind.as_deref() {
        Some("text") => TEXT_CAP,
        Some("image") => IMAGE_CAP,
        Some("pdf") => PDF_CAP,
        _ => return (StatusCode::BAD_REQUEST, "The kind is text, image or pdf.").into_response(),
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

/// Whether `path` is a regular file the list may show. A link is followed only after its target is
/// checked, and the folders above the file are checked once each (`safe_folders` remembers them).
fn is_listable(path: &std::path::Path, safe_folders: &mut std::collections::HashSet<PathBuf>) -> bool {
    if let Some(folder) = path.parent()
        && !safe_folders.contains(folder)
    {
        if links_refused(folder) {
            return false;
        }
        safe_folders.insert(folder.to_path_buf());
    }
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if meta.file_type().is_symlink() {
        return !links_refused(path) && std::fs::metadata(path).is_ok_and(|m| m.is_file());
    }
    meta.is_file()
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
    // Folders already read link by link, so a big repo costs one check per folder.
    let mut safe_folders = std::collections::HashSet::new();
    let mut files = Vec::new();
    for name in out.stdout.split('\0').filter(|n| !n.is_empty()) {
        if files.len() >= MAX_LIST_FILES || started.elapsed() > LIST_BUDGET {
            return Some((files, true));
        }
        if seen.insert(name) && is_listable(&root.join(name), &mut safe_folders) {
            files.push(name.to_string());
        }
    }
    Some((files, false))
}

/// The files under `root` outside a repo. Symlinked and junctioned folders are not entered, and a
/// linked file is listed only when its target passes `links_refused` and is a file.
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
            } else if kind.is_file()
                || (kind.is_symlink() && !links_refused(&path) && std::fs::metadata(&path).is_ok_and(|m| m.is_file()))
            {
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
        .find(|p| p.to_str().is_some_and(|s| !s.is_empty() && !refused(s)) && !links_refused(p) && p.is_dir())
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
        // In a repo the root is the checkout the session was launched in, which for a task is its
        // worktree. `project` is the launch folder itself outside git, so it is not used here.
        if let Some(root) = row.as_ref().and_then(checkout_of).and_then(|c| first_folder([c])) {
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

    /// A link reader over a table: Some(target) for a listed link, None for anything else. It logs
    /// every path it is asked about.
    struct Links {
        table: Vec<(PathBuf, PathBuf)>,
        asked: std::cell::RefCell<Vec<PathBuf>>,
    }

    impl Links {
        fn new(table: &[(&PathBuf, &str)]) -> Self {
            Links {
                table: table.iter().map(|(l, t)| ((*l).clone(), PathBuf::from(t))).collect(),
                asked: Default::default(),
            }
        }

        fn refused(&self, path: &std::path::Path) -> bool {
            links_refused_with(path, &|p: &std::path::Path| {
                self.asked.borrow_mut().push(p.to_path_buf());
                Ok(self.table.iter().find(|(l, _)| l == p).map(|(_, t)| t.clone()))
            })
        }
    }

    fn base() -> PathBuf {
        PathBuf::from(if cfg!(windows) { r"C:\" } else { "/" })
    }

    #[test]
    fn a_link_to_a_unc_share_refuses_the_path_and_nothing_past_it_is_asked() {
        let link = base().join("proj").join("link");
        let links = Links::new(&[(&link, r"\\192.0.2.1\share")]);
        assert!(links.refused(&link.join("deep").join("x.txt")));
        let asked = links.asked.borrow();
        assert_eq!(asked.last(), Some(&link), "asked {asked:?}");
        assert!(
            !asked.iter().any(|p| p.starts_with(&link) && p != &link),
            "asked {asked:?}"
        );
    }

    #[test]
    fn a_link_whose_target_is_a_device_or_nt_path_is_refused() {
        let link = base().join("link");
        for target in [
            r"\\?\UNC\192.0.2.1\share",
            r"\??\UNC\192.0.2.1\share",
            "//192.0.2.1/share",
        ] {
            let links = Links::new(&[(&link, target)]);
            assert!(links.refused(&link), "{target}");
        }
    }

    #[test]
    fn a_link_to_a_local_folder_is_followed_and_the_rest_is_checked_there() {
        let (link, real) = (base().join("a").join("link"), base().join("b").join("real"));
        let links = Links::new(&[(&link, real.to_str().unwrap())]);
        assert!(!links.refused(&link.join("x.txt")));
        assert!(links.asked.borrow().contains(&real.join("x.txt")));
        // A relative target is read against the folder holding the link.
        let rel = Links::new(&[(&link, "sibling")]);
        assert!(!rel.refused(&link.join("x.txt")));
        assert!(
            rel.asked
                .borrow()
                .contains(&base().join("a").join("sibling").join("x.txt"))
        );
        // A link further along the way is found too.
        let (inner, bad) = (real.join("inner"), r"\\192.0.2.1\share");
        let chain = Links::new(&[(&link, real.to_str().unwrap()), (&inner, bad)]);
        assert!(chain.refused(&link.join("inner").join("x.txt")));
    }

    #[test]
    fn a_junction_style_nt_target_to_a_drive_is_not_a_network_path() {
        let link = base().join("link");
        for target in [r"\??\C:\work\x", r"\\?\C:\work\x"] {
            let links = Links::new(&[(&link, target)]);
            assert!(!links.refused(&link), "{target}");
        }
    }

    #[test]
    fn link_loops_and_long_chains_are_refused() {
        let (a, b) = (base().join("a"), base().join("b"));
        let looped = Links::new(&[(&a, b.to_str().unwrap()), (&b, a.to_str().unwrap())]);
        assert!(looped.refused(&a.join("x")));
        let itself = Links::new(&[(&a, a.to_str().unwrap())]);
        assert!(itself.refused(&a));
        // 41 links in a row is one too many; 40 is fine.
        let names: Vec<PathBuf> = (0..=41).map(|i| base().join(format!("l{i}"))).collect();
        let table: Vec<(&PathBuf, &str)> = (0..41).map(|i| (&names[i], names[i + 1].to_str().unwrap())).collect();
        assert!(Links::new(&table).refused(&names[0]));
        assert!(!Links::new(&table[1..]).refused(&names[1]));
    }

    #[test]
    fn a_reader_error_other_than_missing_refuses_and_missing_is_fine() {
        let p = base().join("a").join("b");
        let missing = |_: &std::path::Path| Err(std::io::ErrorKind::NotFound.into());
        assert!(!links_refused_with(&p, &missing));
        let denied = |_: &std::path::Path| Err(std::io::ErrorKind::PermissionDenied.into());
        assert!(links_refused_with(&p, &denied));
    }

    #[test]
    fn content_types_come_from_the_extension() {
        let ct = |s: &str| content_type(std::path::Path::new(s));
        assert_eq!(ct("a.PNG"), "image/png");
        assert_eq!(ct("a.svg"), "image/svg+xml");
        assert_eq!(ct("a.PDF"), "application/pdf");
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
