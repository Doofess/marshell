//! POST /v1/sessions/{tab}/changes: what the checkout the shell is in has changed, for the Changes
//! panel; and the marks the Files tree draws from the same check.
//!
//! The checkout is the one the shell reports, not the launch one: the panel follows the person.
//! `changes_target` picks it and refuses a network folder before any git runs.

use super::AppState;
use super::files::{Root, checkout_above, checkout_of, links_refused, refused, row_of};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{ChangeMark, ChangesKind, ChangesReply, ChangesRequest, DiffStatus, SessionInfo};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::changes::{CHECK_BUDGET, ChangesCache, ChangesError, Check, READ_BUDGET, Target, reply_stamp};

/// The most marks a tree reply carries.
pub(super) const MAX_MARKS: usize = 2_000;

/// Why a folder has no checkout to read: the reply that says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NoTarget {
    pub kind: ChangesKind,
    /// The plain folder, as the shell spelled it.
    pub folder: String,
    pub reason: Option<String>,
}

impl NoTarget {
    fn into_reply(self) -> ChangesReply {
        empty_reply(self.kind, self.folder, self.reason)
    }
}

/// A reply that carries no files: the kind, the folder and, for a refusal or a failure, the sentence.
fn empty_reply(kind: ChangesKind, folder: String, reason: Option<String>) -> ChangesReply {
    ChangesReply {
        kind,
        stamp: String::new(),
        folder,
        moved: false,
        branch: None,
        detached: None,
        base: None,
        unrelated: false,
        ahead: 0,
        uncommitted: false,
        files: Vec::new(),
        diff: String::new(),
        truncated: false,
        more_new: 0,
        reason,
    }
}

fn network_sentence(folder: &str) -> String {
    format!("{folder} is on a network share, so Changes doesn't read it.")
}

/// The folder the panel looks at: the one the shell reported, else the launch folder. None when there is neither.
pub(super) fn candidate_of(row: &SessionInfo, reported: Option<PathBuf>) -> Option<PathBuf> {
    reported
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| (!row.cwd.is_empty()).then(|| PathBuf::from(&row.cwd)))
}

/// Whether the core may not touch this folder: a network or device path, or a link that leads to one.
fn off_limits(path: &std::path::Path) -> bool {
    path.to_str().is_none_or(refused) || links_refused(path)
}

/// Whether `checkout` is not the one the session was launched in.
pub(super) fn moved_from_launch(row: &SessionInfo, checkout: &std::path::Path) -> bool {
    let checkout = checkout.to_string_lossy();
    checkout_of(row).is_none_or(|launch| !crate::projects::same_path(&launch.to_string_lossy(), &checkout))
}

/// The checkout to read. This is not `project_root`: that keeps a launch checkout ahead of the shell's folder
/// and drops a refused folder for the launch one, so a shell in a network folder would show the launch checkout's
/// changes. Here the reported folder decides, and a refused one is refused, never replaced.
pub(super) fn changes_target(row: &SessionInfo, reported: Option<PathBuf>) -> Result<Target, NoTarget> {
    let Some(candidate) = candidate_of(row, reported) else {
        return Err(NoTarget {
            kind: ChangesKind::NoRepo,
            folder: String::new(),
            reason: None,
        });
    };
    let folder = candidate.to_string_lossy().into_owned();
    let refusal = || NoTarget {
        kind: ChangesKind::Refused,
        reason: Some(network_sentence(&folder)),
        folder: folder.clone(),
    };
    if off_limits(&candidate) {
        return Err(refusal());
    }
    let Some(checkout) = checkout_above(&candidate) else {
        return Err(NoTarget {
            kind: ChangesKind::NoRepo,
            folder: folder.clone(),
            reason: None,
        });
    };
    // The checkout may be above the folder, or reached through a link the folder's own check did not see.
    if off_limits(&checkout) {
        return Err(refusal());
    }
    Ok(Target {
        moved: moved_from_launch(row, &checkout),
        checkout,
        task_base: None,
    })
}

/// `target` with the base branch of the task whose worktree it is.
fn with_task_base(home: &std::path::Path, mut target: Target) -> Target {
    target.task_base = crate::tasks::task_at(home, &target.checkout)
        .map(|t| t.base_branch)
        .filter(|b| !b.is_empty());
    target
}

/// The reply for a check or read that failed.
fn error_reply(error: ChangesError, folder: &str, target: &Target) -> ChangesReply {
    match error {
        ChangesError::NoRepo => empty_reply(ChangesKind::NoRepo, folder.to_string(), None),
        ChangesError::Refused(reason) => empty_reply(ChangesKind::Refused, folder.to_string(), Some(reason)),
        ChangesError::Failed(reason) => empty_reply(
            ChangesKind::Failed,
            target.checkout.to_string_lossy().into_owned(),
            Some(reason),
        ),
    }
}

/// The reply to a request. Blocking.
fn answer(
    cache: &ChangesCache,
    home: &std::path::Path,
    row: &SessionInfo,
    reported: Option<PathBuf>,
    req: &ChangesRequest,
    cancel: &AtomicBool,
) -> ChangesReply {
    let folder = candidate_of(row, reported.clone())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let target = match changes_target(row, reported) {
        Ok(target) => with_task_base(home, target),
        Err(none) => return none.into_reply(),
    };
    let check = match cache.check(home, &target, Instant::now() + CHECK_BUDGET, cancel) {
        Ok(check) => check,
        Err(e) => return error_reply(e, &folder, &target),
    };
    let stamp = reply_stamp(&check, req.hide_whitespace);
    if req.stamp.as_deref() == Some(stamp.as_str()) {
        return ChangesReply {
            stamp,
            ..empty_reply(ChangesKind::Same, target.checkout.to_string_lossy().into_owned(), None)
        };
    }
    match cache.read(home, &target, req.hide_whitespace, Instant::now() + READ_BUDGET, cancel) {
        Ok(reply) => reply,
        Err(e) => error_reply(e, &folder, &target),
    }
}

/// Sets the flag when it is dropped, so a request the app aborts stops the blocking work (as the search's does).
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// POST /v1/sessions/{tab}/changes
pub async fn changes(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<ChangesRequest>) -> Response {
    let reported = st.session(&tab).and_then(|s| s.current_cwd());
    let Some(row) = row_of(&st, &tab) else {
        return (StatusCode::NOT_FOUND, "There is no such session.").into_response();
    };
    if candidate_of(&row, reported.clone()).is_none() {
        return (StatusCode::NOT_FOUND, "This session has no folder.").into_response();
    }
    let home = st.home().to_path_buf();
    let cache = st.changes_arc();
    let cancel = Arc::new(AtomicBool::new(false));
    let _stop = CancelOnDrop(cancel.clone());
    let done = tokio::task::spawn_blocking(move || answer(&cache, &home, &row, reported, &req, &cancel)).await;
    match done {
        Ok(reply) => Json(reply).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The root's path below the checkout, with a trailing `/` ("" at the checkout itself). None when the root
/// isn't below it.
fn root_prefix(root: &std::path::Path, checkout: &std::path::Path) -> Option<String> {
    let below = root.strip_prefix(checkout).ok()?;
    let mut prefix = String::new();
    for part in below.components() {
        prefix.push_str(part.as_os_str().to_str()?);
        prefix.push('/');
    }
    Some(prefix)
}

/// The changed files under the root, as the tree marks them: the check's entries that aren't deleted, then its
/// new files, relative to the root, at most `MAX_MARKS`.
fn marks_of(check: &Check, prefix: &str) -> Vec<ChangeMark> {
    let entries = check
        .entries
        .iter()
        .filter(|e| e.status != DiffStatus::Deleted)
        .map(|e| (e.path.as_str(), e.status));
    let new_files = check.new_files.iter().map(|p| (p.as_str(), DiffStatus::Untracked));
    entries
        .chain(new_files)
        .filter_map(|(path, status)| {
            path.strip_prefix(prefix).map(|rest| ChangeMark {
                path: rest.to_string(),
                status,
            })
        })
        .take(MAX_MARKS)
        .collect()
}

/// The tree's marks for its root. Blocking, and never waits for git: with no check yet it answers none and
/// the next poll has them (`ChangesCache::marks`).
pub(super) fn tree_marks(st: &AppState, row: Option<&SessionInfo>, root: &Root) -> Vec<ChangeMark> {
    let (checkout, root_path) = match root {
        Root::Plain(_) => return Vec::new(),
        Root::Checkout(top) => (top, top),
        Root::InCheckout { root, checkout } => (checkout, root),
    };
    if off_limits(checkout) {
        return Vec::new();
    }
    let Some(prefix) = root_prefix(root_path, checkout) else {
        return Vec::new();
    };
    let target = with_task_base(
        st.home(),
        Target {
            checkout: checkout.clone(),
            moved: row.is_some_and(|r| moved_from_launch(r, checkout)),
            task_base: None,
        },
    );
    match st.changes_arc().marks(st.home().to_path_buf(), target) {
        Some(check) => marks_of(&check, &prefix),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::{CommandState, SessionState};

    /// A folder with a `.git` folder in it, so the file-system walk finds a checkout (no git runs).
    fn checkout(base: &std::path::Path, name: &str) -> PathBuf {
        let dir = base.join(name);
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        crate::projects::canonical(&dir)
    }

    fn row(cwd: &std::path::Path) -> SessionInfo {
        let text = cwd.to_string_lossy().into_owned();
        SessionInfo {
            tab_id: "t1".into(),
            number: 1,
            name: "s1".into(),
            agent: None,
            profile_id: "p".into(),
            cwd: text.clone(),
            project: text,
            mode: None,
            state: SessionState::Running,
            command: CommandState::Unknown,
            paused: false,
            foreground: None,
            live: None,
            activity: None,
            status_note: None,
            muted: false,
            elevated: false,
            attention: None,
            stuck: None,
            preset: None,
            ports: Vec::new(),
            script: None,
            setup: None,
            references: None,
        }
    }

    #[test]
    fn target_prefers_the_reported_folder_over_a_launch_checkout() {
        let tmp = tempfile::tempdir().unwrap();
        let base = crate::projects::canonical(tmp.path());
        let a = checkout(&base, "a");
        let b = checkout(&base, "b");
        let target = changes_target(&row(&a), Some(b.clone())).unwrap();
        assert_eq!(target.checkout, b);
        assert!(target.moved);
        // Without a reported folder the launch folder is the candidate, and the shell has not moved.
        let target = changes_target(&row(&a), None).unwrap();
        assert_eq!(target.checkout, a);
        assert!(!target.moved);
    }

    #[test]
    fn target_refuses_a_reported_network_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let base = crate::projects::canonical(tmp.path());
        let a = checkout(&base, "a");
        let share = PathBuf::from(r"\\wsl.localhost\Ubuntu\home");
        let none = changes_target(&row(&a), Some(share)).unwrap_err();
        assert_eq!(none.kind, ChangesKind::Refused);
        assert_eq!(
            none.reason.as_deref(),
            Some(r"\\wsl.localhost\Ubuntu\home is on a network share, so Changes doesn't read it.")
        );
        assert_eq!(none.folder, r"\\wsl.localhost\Ubuntu\home");
    }

    #[test]
    fn target_in_a_folder_outside_git_is_no_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let base = crate::projects::canonical(tmp.path());
        let plain = base.join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let none = changes_target(&row(&plain), None).unwrap_err();
        assert_eq!(none.kind, ChangesKind::NoRepo);
        assert_eq!(none.folder, plain.to_string_lossy());
    }

    #[test]
    fn the_root_prefix_is_its_path_below_the_checkout() {
        assert_eq!(
            root_prefix(std::path::Path::new("/r/sub/deep"), std::path::Path::new("/r")).as_deref(),
            Some("sub/deep/")
        );
        assert_eq!(
            root_prefix(std::path::Path::new("/r"), std::path::Path::new("/r")).as_deref(),
            Some("")
        );
        assert_eq!(
            root_prefix(std::path::Path::new("/x"), std::path::Path::new("/r")),
            None
        );
    }
}
