//! An HTML file an agent's file tool wrote, queued to open in that session's side pane as `marshell open` would.
//!
//! The adapters say which files an event wrote (`agents::written`); this module knows nothing of a tool's name. It
//! never logs a path.

use super::{AppState, opens};
use crate::agents;
use std::path::PathBuf;

fn is_sep(c: char) -> bool {
    c == '/' || c == '\\'
}

/// `C:\x` or `C:/x`.
fn drive_absolute(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\')
}

/// `C:` in front of anything else (`C:a`, `C:`), a form that names no one file.
fn drive_relative(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && !drive_absolute(path)
}

/// The path's components with `.` and empty ones dropped and `..` folded into the one before it.
fn folded(text: &str, sep: impl Fn(char) -> bool) -> Vec<&str> {
    let mut parts: Vec<&str> = Vec::new();
    for part in text.split(sep) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts
}

/// A drive-absolute Windows path, with backslashes and no dot segments.
fn fold_windows(path: &str) -> String {
    format!("{}\\{}", &path[..2], folded(&path[2..], is_sep).join("\\"))
}

/// An absolute Linux path, with no dot segments.
fn fold_linux(path: &str) -> String {
    format!("/{}", folded(path, |c| c == '/').join("/"))
}

/// One reported path as an absolute Windows path, or None when this side can't tell where it is.
fn target_of(path: &str, cwd: Option<&str>, distro: Option<&str>) -> Option<PathBuf> {
    let path = path.trim();
    if path.is_empty() || path.contains('\0') {
        return None;
    }
    // A share path is passed on as it is: the checks that follow refuse it.
    if path.starts_with(r"\\") || (distro.is_none() && path.starts_with("//")) {
        return Some(PathBuf::from(path));
    }
    if drive_absolute(path) {
        return Some(PathBuf::from(fold_windows(path)));
    }
    if drive_relative(path) {
        return None;
    }
    let cwd = cwd.map(str::trim).filter(|cwd| !cwd.is_empty());
    match distro {
        Some(distro) => {
            let linux = if path.starts_with('/') {
                fold_linux(path)
            } else {
                let cwd = cwd?;
                if drive_absolute(cwd) {
                    return Some(PathBuf::from(fold_windows(&format!("{cwd}\\{path}"))));
                }
                if !cwd.starts_with('/') {
                    return None;
                }
                fold_linux(&format!("{cwd}/{path}"))
            };
            crate::wsl::to_windows(distro, &linux)
        }
        None => {
            if path.starts_with(is_sep) {
                return None;
            }
            let cwd = cwd.filter(|cwd| drive_absolute(cwd))?;
            Some(PathBuf::from(fold_windows(&format!("{cwd}\\{path}"))))
        }
    }
}

/// Whether the last component of the path ends in `.html` or `.htm`, in any case.
fn is_html(path: &str) -> bool {
    let name = path.rsplit(is_sep).next().unwrap_or_default();
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty() && (extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm"))
    })
}

/// The HTML files among `written`, as absolute Windows paths, each once, in order. A relative path is joined to `cwd`
/// first (the Linux way with a `distro`), and one with no `cwd` is dropped.
pub(crate) fn html_targets(written: &[String], cwd: Option<&str>, distro: Option<&str>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for path in written {
        let Some(target) = target_of(path, cwd, distro) else {
            continue;
        };
        let text = target.to_string_lossy();
        if !is_html(&text)
            || out
                .iter()
                .any(|seen| seen.to_string_lossy().eq_ignore_ascii_case(&text))
        {
            continue;
        }
        drop(text);
        out.push(target);
    }
    out
}

/// What one hook event asks the queue to open: the paths its file tool wrote and the folder the event reports. Only
/// these are moved into the spawned task, never the event's body.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Pending {
    written: Vec<String>,
    cwd: Option<String>,
}

/// The work an event leaves for `queue`, or None when there is none: the setting is off, or the event wrote no file.
/// It only borrows the body, so an event that opens nothing costs no copy of it and no task.
pub(crate) fn pending(open_agent_html: bool, agent: &str, body: &serde_json::Value) -> Option<Pending> {
    if !open_agent_html {
        return None;
    }
    let written = agents::written(agent, body);
    if written.is_empty() {
        return None;
    }
    let cwd = body
        .pointer("/cwd")
        .and_then(serde_json::Value::as_str)
        .filter(|cwd| !cwd.trim().is_empty())
        .map(str::to_string);
    Some(Pending { written, cwd })
}

/// Queues the opens for one hook event. Spawned, never awaited by the hook's answer.
pub(crate) async fn queue(st: AppState, agent: String, tab: String, pending: Pending) {
    let Pending { written, cwd } = pending;
    let cwd = cwd.or_else(|| st.registry().cwd_of_tab(&tab));
    // Reading the distro, and every check of a file, touches the system: off the async threads. An answer that fails
    // to arrive (a panic) opens nothing.
    let _ = tokio::task::spawn_blocking(move || {
        let distro = st.wsl_distro_of_tab(&tab);
        for target in html_targets(&written, cwd.as_deref(), distro.as_deref()) {
            let path = target.to_string_lossy().into_owned();
            // The checks `marshell open` makes. A path the file guard refuses ends here.
            if opens::classify(&path, cfg!(windows)) != Ok("file") || opens::file_refusal(&path).is_some() {
                continue;
            }
            // The receiver is dropped: nobody waits for the app's answer. A full queue drops the rest.
            if st.opens().push(&tab, "file", path, agent.clone(), true).is_err() {
                break;
            }
        }
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_queues_nothing_with_the_setting_off_or_when_it_writes_no_file() {
        let write = serde_json::json!({
            "hook_event_name": "PostToolUse", "tool_name": "Write", "cwd": r"C:\proj",
            "tool_input": {"file_path": r"C:\proj\a.html", "content": "<p>x</p>"},
            "tool_response": {"filePath": r"C:\proj\a.html"}
        });
        assert_eq!(pending(false, "claude", &write), None);
        let read = serde_json::json!({
            "hook_event_name": "PostToolUse", "tool_name": "Read", "cwd": r"C:\proj",
            "tool_input": {"file_path": r"C:\proj\a.html"}
        });
        assert_eq!(pending(true, "claude", &read), None);
        assert_eq!(pending(true, "claude", &serde_json::json!({})), None);
        assert_eq!(pending(true, "unknown", &write), None);
        let job = pending(true, "claude", &write).expect("a write queues");
        assert_eq!(job.written, [r"C:\proj\a.html"]);
        assert_eq!(job.cwd.as_deref(), Some(r"C:\proj"));
    }

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn keeps_html_files_only_whatever_the_case() {
        let t = html_targets(
            &s(&[
                "C:\\p\\a.html",
                "C:\\p\\B.HTM",
                "C:\\p\\c.htmlx",
                "C:\\p\\d.md",
                "C:\\p\\html",
            ]),
            None,
            None,
        );
        assert_eq!(t, [PathBuf::from("C:\\p\\a.html"), PathBuf::from("C:\\p\\B.HTM")]);
    }

    #[test]
    fn a_relative_path_is_read_from_the_hooks_folder_and_dropped_without_one() {
        assert_eq!(
            html_targets(&s(&["out/report.html"]), Some("C:\\proj"), None),
            [PathBuf::from("C:\\proj\\out\\report.html")]
        );
        assert_eq!(
            html_targets(&s(&["out/report.html"]), None, None),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn dot_segments_are_folded_and_a_file_is_listed_once() {
        assert_eq!(
            html_targets(&s(&["C:\\proj\\a\\..\\x.html", "C:/proj/x.html"]), None, None),
            [PathBuf::from("C:\\proj\\x.html")]
        );
        assert_eq!(
            html_targets(&s(&["./a/./b.html", "../c.html"]), Some("C:\\proj\\sub"), None),
            [
                PathBuf::from("C:\\proj\\sub\\a\\b.html"),
                PathBuf::from("C:\\proj\\c.html")
            ]
        );
    }

    #[test]
    fn a_path_that_is_not_absolute_on_this_side_is_dropped() {
        // A drive-relative, a rooted and a Linux path, with no distro to read the last one in.
        assert!(html_targets(&s(&["C:a.html", "\\a.html", "/home/u/a.html"]), Some("C:\\proj"), None).is_empty());
    }

    #[test]
    fn a_wsl_sessions_path_on_a_windows_drive_becomes_that_drives_path() {
        assert_eq!(
            html_targets(&s(&["/mnt/c/proj/site/index.html"]), None, Some("Ubuntu")),
            [PathBuf::from("C:\\proj\\site\\index.html")]
        );
        assert_eq!(
            html_targets(&s(&["site/index.html"]), Some("/mnt/d/proj"), Some("Ubuntu")),
            [PathBuf::from("D:\\proj\\site\\index.html")]
        );
    }

    #[test]
    fn a_wsl_sessions_path_inside_the_distro_is_its_share_path_which_the_file_guard_refuses() {
        // Decision 29: the guard is not changed here, so this target is made and then refused by `classify`.
        let t = html_targets(&s(&["/home/u/site/index.html"]), None, Some("Ubuntu"));
        assert_eq!(t, [PathBuf::from(r"\\wsl.localhost\Ubuntu\home\u\site\index.html")]);
        assert!(crate::server::files::refused(&t[0].to_string_lossy()));
    }

    #[test]
    fn eighty_files_are_eighty_targets() {
        // The queue's own limit, not this function, drops what is too many.
        let many: Vec<String> = (0..80).map(|n| format!("C:\\p\\{n}.html")).collect();
        assert_eq!(html_targets(&many, None, None).len(), 80);
    }
}
