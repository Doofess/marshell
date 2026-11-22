//! POST /v1/sessions/{tab}/tree: one level of the session's project folder at a time, for the Files
//! panel's tree.
//!
//! This route lists folders on a person's disk from a path the webview sends, so it holds a hard
//! line: nothing outside the project root is listed, and no folder is entered through a link (a
//! symlink or a junction), inside the root or out. `.git` is never listed or entered.

use super::AppState;
use super::files::{Root, links_refused, modified_ms, project_root, refused, row_of, strip_verbatim};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{TreeAsk, TreeDir, TreeDirState, TreeEntry, TreeEntryKind, TreeReply, TreeRequest};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::PathBuf;

/// The most folders one request may ask for.
const MAX_ASKS: usize = 64;
/// A folder lists at most this many entries; the rest are counted in `more`.
const MAX_ENTRIES: usize = 5_000;

/// Runs git with `args` in a folder, giving it `stdin`. Swapped in tests to count the runs.
type RunGit<'a> = &'a dyn Fn(&std::path::Path, &[&str], Vec<u8>) -> anyhow::Result<std::process::Output>;

/// Digits compare as numbers, everything else without regard to case. Equal in that sense, the plain
/// text order decides, so the order is total.
pub(super) fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn digits(it: &mut std::iter::Peekable<std::str::Chars>) -> String {
        let mut run = String::new();
        while let Some(c) = it.next_if(char::is_ascii_digit) {
            run.push(c);
        }
        run.trim_start_matches('0').to_string()
    }
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(p), Some(q)) if p.is_ascii_digit() && q.is_ascii_digit() => {
                let (m, n) = (digits(&mut x), digits(&mut y));
                let order = m.len().cmp(&n.len()).then_with(|| m.cmp(&n));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(p), Some(q)) => {
                let order = p.to_lowercase().cmp(q.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Whether a path component names `.git` in any way Windows would resolve to it: in any case, with
/// trailing dots or spaces.
fn is_dot_git(component: &str) -> bool {
    component.trim_end_matches(['.', ' ']).eq_ignore_ascii_case(".git")
}

/// The rules on the text of an asked folder, before the disk is touched. The parts of a clean one.
fn clean_parts(dir: &str) -> Option<Vec<&str>> {
    if dir.is_empty() {
        return Some(Vec::new());
    }
    if dir.contains(['\\', ':', '\0']) || dir.starts_with('/') || refused(dir) {
        return None;
    }
    let parts: Vec<&str> = dir.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == ".." || is_dot_git(p))
    {
        return None;
    }
    Some(parts)
}

/// The folder an ask names, walked from the root one component at a time without following a link.
enum Found {
    Dir(PathBuf),
    Gone,
    Refused,
}

fn find(root: &std::path::Path, canonical_root: &std::path::Path, dir: &str) -> Found {
    let Some(parts) = clean_parts(dir) else {
        return Found::Refused;
    };
    let mut cur = root.to_path_buf();
    for part in parts {
        cur.push(part);
        match std::fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => return Found::Refused,
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Found::Gone,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                return Found::Gone;
            }
            Err(_) => return Found::Refused,
        }
    }
    let Some(text) = cur.to_str() else {
        return Found::Refused;
    };
    if refused(text) {
        return Found::Refused;
    }
    // A short name (`GIT~1`) or a case the text check missed shows only in the real path.
    let real = crate::projects::canonical(&cur);
    let Ok(below) = real.strip_prefix(canonical_root) else {
        return Found::Refused;
    };
    if below
        .components()
        .any(|c| c.as_os_str().to_str().is_none_or(is_dot_git))
    {
        return Found::Refused;
    }
    Found::Dir(cur)
}

/// The entries of a folder, folders and links first, each group in natural order, cut at the cap.
fn entries_of(folder: &std::path::Path) -> Option<(Vec<TreeEntry>, u32)> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(folder).ok()?.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if is_dot_git(&name) {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        let kind = if kind.is_symlink() {
            if links_refused(&path) {
                continue;
            }
            match std::fs::metadata(&path) {
                Ok(m) if m.is_dir() => TreeEntryKind::Link,
                Ok(m) if m.is_file() => TreeEntryKind::File,
                _ => continue,
            }
        } else if kind.is_dir() {
            TreeEntryKind::Folder
        } else {
            TreeEntryKind::File
        };
        entries.push(TreeEntry {
            name,
            kind,
            ignored: false,
        });
    }
    entries.sort_by(|a, b| {
        let is_file = |e: &TreeEntry| e.kind == TreeEntryKind::File;
        is_file(a).cmp(&is_file(b)).then_with(|| natural_cmp(&a.name, &b.name))
    });
    let more = entries.len().saturating_sub(MAX_ENTRIES);
    entries.truncate(MAX_ENTRIES);
    Some((entries, more as u32))
}

fn reply(dir: &str, state: TreeDirState, stamp: u64) -> TreeDir {
    TreeDir {
        dir: dir.to_string(),
        state,
        stamp,
        entries: Vec::new(),
        more: 0,
    }
}

/// The path of an entry as git is asked about it: relative to the root, with `/`.
fn rel_of(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// The lines to ask git about for one listed folder: the folder itself (unless it is the root) and
/// every entry, folders with a trailing `/`.
fn lines_of(d: &TreeDir) -> Vec<String> {
    let mut lines = Vec::with_capacity(d.entries.len() + 1);
    if !d.dir.is_empty() {
        lines.push(format!("{}/", d.dir));
    }
    for e in &d.entries {
        let rel = rel_of(&d.dir, &e.name);
        lines.push(if e.kind == TreeEntryKind::Folder {
            format!("{rel}/")
        } else {
            rel
        });
    }
    lines
}

enum Checked {
    /// Git answered: the ignored paths, without a trailing `/`.
    Set(HashSet<String>),
    /// Git ran and failed, as it does for a path inside a submodule.
    Failed,
    /// Git could not be started.
    NoGit,
}

fn check_ignore(root: &std::path::Path, lines: &[String], run: RunGit) -> Checked {
    let mut stdin = Vec::new();
    for line in lines {
        stdin.extend_from_slice(line.as_bytes());
        stdin.push(0);
    }
    let Ok(out) = run(root, &["check-ignore", "-z", "--stdin"], stdin) else {
        return Checked::NoGit;
    };
    match out.status.code() {
        // 1: none of them is ignored.
        Some(1) => Checked::Set(HashSet::new()),
        Some(0) => Checked::Set(
            out.stdout
                .split(|b| *b == 0)
                .filter(|p| !p.is_empty())
                .map(|p| String::from_utf8_lossy(p).trim_end_matches('/').to_string())
                .collect(),
        ),
        _ => Checked::Failed,
    }
}

fn apply_marks(d: &mut TreeDir, ignored: &HashSet<String>) {
    let folder_ignored = !d.dir.is_empty() && ignored.contains(&d.dir);
    for e in &mut d.entries {
        e.ignored = folder_ignored || ignored.contains(&rel_of(&d.dir, &e.name));
    }
}

/// Marks the ignored entries of the listed folders with one git run, or one per folder when git
/// refuses the whole list.
fn mark_ignored(root: &std::path::Path, dirs: &mut [TreeDir], run: RunGit) {
    let all: Vec<String> = dirs
        .iter()
        .filter(|d| d.state == TreeDirState::Listed)
        .flat_map(lines_of)
        .collect();
    match check_ignore(root, &all, run) {
        Checked::Set(set) => dirs
            .iter_mut()
            .filter(|d| d.state == TreeDirState::Listed)
            .for_each(|d| apply_marks(d, &set)),
        Checked::NoGit => {}
        Checked::Failed => {
            for d in dirs.iter_mut().filter(|d| d.state == TreeDirState::Listed) {
                match check_ignore(root, &lines_of(d), run) {
                    Checked::Set(set) => apply_marks(d, &set),
                    Checked::Failed => {}
                    Checked::NoGit => return,
                }
            }
        }
    }
}

/// The answers to the asks. Blocking.
fn tree_with(root: &std::path::Path, is_git: bool, asks: &[TreeAsk], run: RunGit) -> Vec<TreeDir> {
    let canonical_root = crate::projects::canonical(root);
    let mut dirs: Vec<TreeDir> = asks
        .iter()
        .map(|ask| {
            let folder = match find(root, &canonical_root, &ask.dir) {
                Found::Dir(folder) => folder,
                Found::Gone => return reply(&ask.dir, TreeDirState::Gone, 0),
                Found::Refused => return reply(&ask.dir, TreeDirState::Refused, 0),
            };
            let Ok(meta) = std::fs::metadata(&folder) else {
                return reply(&ask.dir, TreeDirState::Gone, 0);
            };
            let stamp = modified_ms(&meta);
            if stamp != 0 && ask.stamp == Some(stamp) {
                return reply(&ask.dir, TreeDirState::Same, stamp);
            }
            match entries_of(&folder) {
                Some((entries, more)) => TreeDir {
                    entries,
                    more,
                    ..reply(&ask.dir, TreeDirState::Listed, stamp)
                },
                None => reply(&ask.dir, TreeDirState::Gone, 0),
            }
        })
        .collect();
    if is_git && dirs.iter().any(|d| d.state == TreeDirState::Listed) {
        mark_ignored(root, &mut dirs, run);
    }
    dirs
}

/// POST /v1/sessions/{tab}/tree
pub async fn tree(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<TreeRequest>) -> Response {
    if req.dirs.is_empty() || req.dirs.len() > MAX_ASKS {
        return (StatusCode::BAD_REQUEST, "Ask for 1 to 64 folders.").into_response();
    }
    let row = row_of(&st, &tab);
    let session = st.session(&tab);
    let reported = session.and_then(|s| s.current_cwd());
    let done = tokio::task::spawn_blocking(move || {
        let found = project_root(row.as_ref(), reported)?;
        let is_git = matches!(found, Root::Checkout(_));
        let root = found.path().to_path_buf();
        let dirs = tree_with(&root, is_git, &req.dirs, &crate::git::run_input);
        Some(TreeReply {
            root: strip_verbatim(&root).unwrap_or_default(),
            dirs,
        })
    })
    .await;
    match done {
        Ok(Some(reply)) => Json(reply).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "This session has no folder.").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AppState, router};
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, header};
    use std::cell::Cell;
    use std::path::Path as FsPath;
    use tower::ServiceExt;

    const TOKEN: &str = "test-token";
    const PORT: u16 = 5562;

    fn ask(dir: &str) -> TreeAsk {
        TreeAsk {
            dir: dir.into(),
            stamp: None,
        }
    }

    /// A temp folder (its canonical path, as the core's root is) and the guard that removes it.
    fn temp() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(tmp.path());
        (tmp, root)
    }

    fn git(root: &FsPath, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn touch(path: &FsPath) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x").unwrap();
    }

    fn list(root: &FsPath, is_git: bool, dirs: &[&str]) -> Vec<TreeDir> {
        let asks: Vec<TreeAsk> = dirs.iter().map(|d| ask(d)).collect();
        tree_with(root, is_git, &asks, &crate::git::run_input)
    }

    fn names(d: &TreeDir) -> Vec<&str> {
        d.entries.iter().map(|e| e.name.as_str()).collect()
    }

    fn ignored(d: &TreeDir) -> Vec<&str> {
        d.entries
            .iter()
            .filter(|e| e.ignored)
            .map(|e| e.name.as_str())
            .collect()
    }

    fn kind_of(d: &TreeDir, name: &str) -> TreeEntryKind {
        d.entries.iter().find(|e| e.name == name).unwrap().kind
    }

    /// Makes a symlink; false where the OS refuses, so the test can skip itself.
    fn try_symlink(target: &FsPath, link: &FsPath, folder: bool) -> bool {
        #[cfg(windows)]
        let made = if folder {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        };
        #[cfg(unix)]
        let made = {
            let _ = folder;
            std::os::unix::fs::symlink(target, link)
        };
        made.is_ok()
    }

    #[cfg(windows)]
    fn junction(target: &FsPath, link: &FsPath) {
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    }

    #[test]
    fn lists_folders_first_in_natural_order_ignoring_case() {
        let (_tmp, root) = temp();
        for dir in ["b", "A"] {
            std::fs::create_dir(root.join(dir)).unwrap();
        }
        for file in ["file10.txt", "File2.txt", "a.txt"] {
            touch(&root.join(file));
        }
        let dirs = list(&root, false, &[""]);
        assert_eq!(dirs[0].state, TreeDirState::Listed);
        assert_eq!(names(&dirs[0]), ["A", "b", "a.txt", "File2.txt", "file10.txt"]);
        assert_eq!(kind_of(&dirs[0], "A"), TreeEntryKind::Folder);
        assert_eq!(kind_of(&dirs[0], "a.txt"), TreeEntryKind::File);
    }

    #[test]
    fn never_lists_dot_git() {
        let (_tmp, root) = temp();
        git(&root, &["init", "-q"]);
        touch(&root.join("a.txt"));
        let dirs = list(&root, true, &[""]);
        assert_eq!(names(&dirs[0]), ["a.txt"]);
    }

    #[test]
    fn marks_ignored_entries_and_everything_in_an_ignored_folder() {
        let (_tmp, root) = temp();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join(".gitignore"), "dist/\n*.log").unwrap();
        touch(&root.join("dist/a.txt"));
        touch(&root.join("dist/b/c.txt"));
        touch(&root.join("x.log"));
        touch(&root.join("tracked.log"));
        touch(&root.join("src/main.rs"));
        git(&root, &["add", "-f", "tracked.log"]);
        let dirs = list(&root, true, &["", "dist"]);
        assert_eq!(ignored(&dirs[0]), ["dist", "x.log"]);
        assert_eq!(names(&dirs[1]), ["b", "a.txt"]);
        assert_eq!(ignored(&dirs[1]), ["b", "a.txt"]);
        // A folder under an ignored one is ignored as well.
        let dirs = list(&root, true, &["dist/b"]);
        assert_eq!(ignored(&dirs[0]), ["c.txt"]);
    }

    #[test]
    fn outside_git_nothing_is_ignored() {
        let (_tmp, root) = temp();
        std::fs::write(root.join(".gitignore"), "*.log").unwrap();
        touch(&root.join("x.log"));
        let dirs = list(&root, false, &[""]);
        assert!(ignored(&dirs[0]).is_empty());
    }

    #[test]
    fn a_folder_unchanged_since_its_stamp_comes_back_same_with_no_entries() {
        let (_tmp, root) = temp();
        touch(&root.join("a.txt"));
        let first = list(&root, false, &[""]);
        assert!(first[0].stamp > 0);
        let again = tree_with(
            &root,
            false,
            &[TreeAsk {
                dir: String::new(),
                stamp: Some(first[0].stamp),
            }],
            &crate::git::run_input,
        );
        assert_eq!(again[0].state, TreeDirState::Same);
        assert_eq!(again[0].stamp, first[0].stamp);
        assert!(again[0].entries.is_empty());
        // A different stamp lists again.
        let other = tree_with(
            &root,
            false,
            &[TreeAsk {
                dir: String::new(),
                stamp: Some(first[0].stamp + 1),
            }],
            &crate::git::run_input,
        );
        assert_eq!(other[0].state, TreeDirState::Listed);
    }

    #[test]
    fn refuses_parent_steps_absolute_backslash_colon_and_dot_git_dirs() {
        let (_tmp, root) = temp();
        std::fs::create_dir(root.join("a")).unwrap();
        for dir in ["..", "a/../..", "/etc", "C:/x", "a\\b", "a:b", ".git", "x/.git"] {
            let dirs = list(&root, false, &[dir, "a"]);
            assert_eq!(dirs[0].state, TreeDirState::Refused, "{dir:?}");
            assert!(dirs[0].entries.is_empty(), "{dir:?}");
            assert_eq!(dirs[0].dir, dir);
            // The other ask of the same request is still answered.
            assert_eq!(dirs[1].state, TreeDirState::Listed, "{dir:?}");
        }
    }

    #[test]
    fn dot_git_in_any_case_is_refused() {
        let (_tmp, root) = temp();
        git(&root, &["init", "-q"]);
        std::fs::create_dir(root.join("x")).unwrap();
        for dir in [".GIT", "x/.Git", ".git.", ".git "] {
            let dirs = list(&root, false, &[dir]);
            assert_eq!(dirs[0].state, TreeDirState::Refused, "{dir:?}");
        }
    }

    #[test]
    fn a_missing_folder_is_gone() {
        let (_tmp, root) = temp();
        touch(&root.join("file.txt"));
        let dirs = list(&root, false, &["nope", "nope/deeper", "file.txt", "file.txt/x"]);
        for d in &dirs {
            assert_eq!(d.state, TreeDirState::Gone, "{:?}", d.dir);
        }
    }

    #[test]
    fn cuts_a_folder_at_5000_and_counts_the_rest() {
        let (_tmp, root) = temp();
        for i in 0..5_003 {
            std::fs::write(root.join(format!("f{i:04}.txt")), "").unwrap();
        }
        let dirs = list(&root, false, &[""]);
        assert_eq!(dirs[0].entries.len(), 5_000);
        assert_eq!(dirs[0].more, 3);
    }

    #[test]
    fn a_symlink_to_a_folder_is_a_link_and_asking_through_it_is_refused() {
        let (_tmp, root) = temp();
        let (_other_tmp, other) = temp();
        touch(&other.join("secret.txt"));
        if !try_symlink(&other, &root.join("link"), true) {
            return;
        }
        let dirs = list(&root, false, &["", "link", "link/sub"]);
        assert_eq!(kind_of(&dirs[0], "link"), TreeEntryKind::Link);
        assert_eq!(dirs[1].state, TreeDirState::Refused);
        assert_eq!(dirs[2].state, TreeDirState::Refused);
        assert!(dirs[1].entries.is_empty());
    }

    #[test]
    fn a_link_to_a_file_is_a_file() {
        let (_tmp, root) = temp();
        touch(&root.join("real.txt"));
        if !try_symlink(&root.join("real.txt"), &root.join("alias.txt"), false) {
            return;
        }
        let dirs = list(&root, false, &[""]);
        assert_eq!(kind_of(&dirs[0], "alias.txt"), TreeEntryKind::File);
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_to_outside_the_root_is_a_link_and_refused_when_asked() {
        let (_tmp, root) = temp();
        let (_other_tmp, other) = temp();
        touch(&other.join("secret.txt"));
        junction(&other, &root.join("out"));
        let dirs = list(&root, false, &["", "out", "out/x"]);
        assert_eq!(kind_of(&dirs[0], "out"), TreeEntryKind::Link);
        assert_eq!(dirs[1].state, TreeDirState::Refused);
        assert!(dirs[1].entries.is_empty());
        assert_eq!(dirs[2].state, TreeDirState::Refused);
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_inside_the_root_is_never_entered() {
        let (_tmp, root) = temp();
        touch(&root.join("b/x/y.txt"));
        junction(&root.join("b"), &root.join("a"));
        let dirs = list(&root, false, &["", "a", "a/x", "b"]);
        assert_eq!(kind_of(&dirs[0], "a"), TreeEntryKind::Link);
        assert_eq!(dirs[1].state, TreeDirState::Refused);
        assert_eq!(dirs[2].state, TreeDirState::Refused);
        assert_eq!(dirs[3].state, TreeDirState::Listed);
        assert_eq!(names(&dirs[3]), ["x"]);
    }

    #[test]
    fn a_folder_of_5000_ignored_files_answers() {
        let (_tmp, root) = temp();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join(".gitignore"), "*.log\n.gitignore").unwrap();
        // Long names, so the list and the answer are far bigger than a pipe's buffer.
        for i in 0..5_000 {
            std::fs::write(
                root.join(format!("a_rather_long_file_name_to_fill_the_pipe_{i:05}.log")),
                "",
            )
            .unwrap();
        }
        let dirs = list(&root, true, &[""]);
        // The 5,000 logs and the .gitignore (which ignores itself): one over the cap.
        assert_eq!(dirs[0].entries.len(), 5_000);
        assert_eq!(dirs[0].more, 1);
        assert!(dirs[0].entries.iter().all(|e| e.ignored));
    }

    #[test]
    fn git_runs_only_when_a_folder_was_listed() {
        let (_tmp, root) = temp();
        git(&root, &["init", "-q"]);
        touch(&root.join("a.txt"));
        let runs = Cell::new(0);
        let counted = |dir: &FsPath, args: &[&str], stdin: Vec<u8>| {
            runs.set(runs.get() + 1);
            crate::git::run_input(dir, args, stdin)
        };
        let first = tree_with(&root, true, &[ask("")], &counted);
        assert_eq!(runs.get(), 1);
        let same = TreeAsk {
            dir: String::new(),
            stamp: Some(first[0].stamp),
        };
        let again = tree_with(&root, true, &[same.clone(), ask("nope"), ask("..")], &counted);
        assert_eq!(again[0].state, TreeDirState::Same);
        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn a_path_git_refuses_falls_back_to_one_run_per_folder() {
        let (_tmp, root) = temp();
        git(&root, &["init", "-q"]);
        std::fs::write(root.join(".gitignore"), "*.log").unwrap();
        touch(&root.join("a.log"));
        touch(&root.join("sub/b.log"));
        let runs = Cell::new(0);
        // The first run, over every folder, fails as one in a submodule does; the ones per folder work.
        let flaky = |dir: &FsPath, args: &[&str], stdin: Vec<u8>| {
            runs.set(runs.get() + 1);
            if runs.get() == 1 {
                let mut out = crate::git::run_input(dir, &["check-ignore", "--bogus-flag"], Vec::new())?;
                assert_eq!(out.status.code(), Some(129));
                // Report it as the fatal 128.
                out.status = failing_status(128);
                return Ok(out);
            }
            crate::git::run_input(dir, args, stdin)
        };
        let dirs = tree_with(&root, true, &[ask(""), ask("sub")], &flaky);
        assert_eq!(runs.get(), 3);
        assert_eq!(ignored(&dirs[0]), ["a.log"]);
        assert_eq!(ignored(&dirs[1]), ["b.log"]);
    }

    fn failing_status(code: i32) -> std::process::ExitStatus {
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(code as u32)
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(code << 8)
        }
    }

    #[test]
    fn natural_cmp_orders_digits_as_numbers() {
        use std::cmp::Ordering::*;
        assert_eq!(natural_cmp("file2", "file10"), Less);
        assert_eq!(natural_cmp("file10", "file2"), Greater);
        assert_eq!(natural_cmp("A", "b"), Less);
        assert_eq!(natural_cmp("File2.txt", "file10.txt"), Less);
        assert_eq!(natural_cmp("a1", "a01"), Greater);
        assert_eq!(natural_cmp("a", "a"), Equal);
        assert_eq!(natural_cmp("a", "ab"), Less);
    }

    async fn post(dirs: usize, tab: &str, bearer: bool) -> (StatusCode, Vec<u8>) {
        let home = tempfile::tempdir().unwrap().keep();
        let st = AppState::new(TOKEN.into(), PORT, Vec::new(), home, Vec::new());
        let body = serde_json::json!({ "dirs": (0..dirs).map(|_| serde_json::json!({"dir": "", "stamp": null})).collect::<Vec<_>>() });
        let mut req = Request::builder()
            .method("POST")
            .uri(format!("/v1/sessions/{tab}/tree"))
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::CONTENT_TYPE, "application/json");
        if bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
        }
        let res = router(st)
            .oneshot(req.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, bytes.to_vec())
    }

    #[tokio::test]
    async fn more_than_64_asks_is_a_bad_request() {
        assert_eq!(post(65, "nope", true).await.0, StatusCode::BAD_REQUEST);
        assert_eq!(post(0, "nope", true).await.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn a_session_with_no_folder_is_not_found() {
        let (status, body) = post(1, "nope", true).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(String::from_utf8_lossy(&body), "This session has no folder.");
    }

    #[tokio::test]
    async fn the_tree_is_refused_without_the_bearer() {
        assert_eq!(post(1, "nope", false).await.0, StatusCode::UNAUTHORIZED);
    }
}
