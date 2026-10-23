//! The History view's routes: the list, full-text search, one conversation and Resume.
//! Only Claude Code's conversations are recorded so far, so any other agent is a 404. An agent and a session id
//! from the URL are checked before anything touches the index or a file. Every call into the history and every
//! file read runs on a blocking thread.

use super::AppState;
use super::sessions::{NO_CONVERSATION, ResumeRequest, cannot_resume, resolve, start_session};
use crate::agents::claude::transcript;
use crate::history::recorder::{BookmarkRefusal, DeleteRefusal, History};
use crate::history::{SessionMeta, archive, transcript_allowed, valid_session_id};
use crate::profiles::detect::ResumeTarget;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{CreateSessionResponse, SessionState};
use marshell_protocol::history::{Bookmarked, Conversation, HistoryEntry, HistoryPatch, HistoryQuery, SearchHit};
use serde::Deserialize;
use std::sync::Arc;

const AGENT: &str = "claude";

/// Resume's refusal when a live tab already holds the conversation.
const RUNNING_IN_A_TAB: &str = "This conversation is already running in a tab. Use Go to session.";

/// A status and the message the route replies with.
type Fail = (StatusCode, String);

fn no_such() -> Fail {
    (StatusCode::NOT_FOUND, "No such conversation.".into())
}

fn off() -> Fail {
    (StatusCode::SERVICE_UNAVAILABLE, "History is off for this run.".into())
}

fn failed(err: impl std::fmt::Display) -> Fail {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}"))
}

/// The agent and id a URL names, checked before any lookup: only Claude's, and an id safe for a file name
/// and a command line.
fn check(agent: &str, session_id: &str) -> Result<(), Fail> {
    if agent == AGENT && valid_session_id(session_id) {
        Ok(())
    } else {
        Err(no_such())
    }
}

/// Runs `work` on a blocking thread.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, Fail> + Send + 'static) -> Result<T, Fail> {
    tokio::task::spawn_blocking(work).await.map_err(failed)?
}

/// 503 when history is off for this run.
fn require_on(history: &History) -> Result<(), Fail> {
    if history.is_on() { Ok(()) } else { Err(off()) }
}

pub async fn list(
    State(st): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<HistoryEntry>>, Fail> {
    let history = st.history();
    let entries = blocking(move || {
        require_on(&history)?;
        history.list(&query).map_err(failed)
    })
    .await?;
    Ok(Json(entries))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    archived: bool,
}

/// An empty or missing query finds nothing.
pub async fn search(
    State(st): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<SearchHit>>, Fail> {
    let history = st.history();
    let hits = blocking(move || {
        require_on(&history)?;
        if query.q.trim().is_empty() {
            return Ok(Vec::new());
        }
        history.search(&query.q, query.archived).map_err(failed)
    })
    .await?;
    Ok(Json(hits))
}

/// The conversation from Marshell's archive copy. A copy that can't be read in full (too big, or damaged)
/// answers with the session's row from the index: its title and folder, marked limited, with no turns.
pub async fn conversation(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
) -> Result<Json<Conversation>, Fail> {
    check(&agent, &session_id)?;
    let history = st.history();
    let home = st.home().to_path_buf();
    let conversation = blocking(move || {
        require_on(&history)?;
        let entry = history.entry(AGENT, &session_id).map_err(failed)?.ok_or_else(no_such)?;
        let marks = history.marks(AGENT, &session_id);
        match archive::read_archive(&home, AGENT, &session_id) {
            Ok(Some(text)) => {
                let mut conversation = transcript::parse(&text, &session_id);
                if let Some(name) = &marks.name {
                    conversation.title = name.clone();
                }
                conversation.tab_id = entry.tab_id;
                conversation.marks = marks;
                Ok(conversation)
            }
            Ok(None) => Err((
                StatusCode::NOT_FOUND,
                "Marshell has no copy of this conversation yet.".into(),
            )),
            Err(err) => {
                tracing::warn!("could not read archived transcript {session_id}: {err:#}");
                Ok(Conversation {
                    agent: entry.agent,
                    session_id: entry.session_id,
                    tab_id: entry.tab_id,
                    title: entry.title,
                    cwd: entry.cwd,
                    branch: entry.branch,
                    model: entry.model,
                    version: entry.version,
                    limited: true,
                    turns: Vec::new(),
                    marks,
                })
            }
        }
    })
    .await?;
    Ok(Json(conversation))
}

pub async fn patch(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
    Json(patch): Json<HistoryPatch>,
) -> Result<Json<HistoryEntry>, Fail> {
    check(&agent, &session_id)?;
    let history = st.history();
    let entry = blocking(move || {
        session_meta(&history, &session_id)?;
        // History also synchronizes a running tab's name while holding the marks lock.
        history.patch(&agent, &session_id, &patch).map_err(patch_failed)?;
        history.entry(&agent, &session_id).map_err(failed)?.ok_or_else(no_such)
    })
    .await?;
    Ok(Json(entry))
}

pub(super) fn patch_failed(message: &'static str) -> Fail {
    match message {
        "No such conversation." => no_such(),
        "History is off for this run." => off(),
        "A conversation can have up to 20 tags."
        | "A tag can be up to 40 characters."
        | "A tag can't contain a comma." => (StatusCode::BAD_REQUEST, message.into()),
        _ => failed(message),
    }
}

pub async fn bookmarks(
    State(st): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<SearchHit>>, Fail> {
    let history = st.history();
    let hits = blocking(move || {
        require_on(&history)?;
        history.bookmarks(&query).map_err(failed)
    })
    .await?;
    Ok(Json(hits))
}

pub async fn bookmark_turn(
    State(st): State<AppState>,
    Path((agent, session_id, turn)): Path<(String, String, u32)>,
) -> Result<StatusCode, Fail> {
    set_bookmark(st, agent, session_id, turn, true).await
}

pub async fn unbookmark_turn(
    State(st): State<AppState>,
    Path((agent, session_id, turn)): Path<(String, String, u32)>,
) -> Result<StatusCode, Fail> {
    set_bookmark(st, agent, session_id, turn, false).await
}

async fn set_bookmark(
    st: AppState,
    agent: String,
    session_id: String,
    turn: u32,
    on: bool,
) -> Result<StatusCode, Fail> {
    check(&agent, &session_id)?;
    let history = st.history();
    blocking(move || {
        session_meta(&history, &session_id)?;
        history.bookmark_turn(&agent, &session_id, turn, on).map_err(failed)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn bookmark_now(State(st): State<AppState>, Path(tab): Path<String>) -> Result<Json<Bookmarked>, Fail> {
    let history = st.history();
    let at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let title = blocking(move || {
        require_on(&history)?;
        history.bookmark_now(&tab, at_ms).map_err(|err| match err {
            BookmarkRefusal::Failed(err) => failed(err),
            err => (StatusCode::CONFLICT, err.to_string()),
        })
    })
    .await?;
    Ok(Json(Bookmarked { title }))
}

pub async fn delete_copy(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
) -> Result<StatusCode, Fail> {
    check(&agent, &session_id)?;
    let history = st.history();
    blocking(move || {
        session_meta(&history, &session_id)?;
        history.delete_copy(&agent, &session_id).map_err(|err| match err {
            DeleteRefusal::Failed(err) => failed(err),
            err => (StatusCode::CONFLICT, err.to_string()),
        })
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The session's sidecar data, once the id is known to be Claude's and valid.
fn session_meta(history: &Arc<History>, session_id: &str) -> Result<SessionMeta, Fail> {
    require_on(history)?;
    history.get(AGENT, session_id).map_err(failed)?.ok_or_else(no_such)
}

/// Starts Claude in a new tab with `--resume <id>`, in the folder the conversation ran in. If Claude's transcript
/// file is gone, the answer is the refusal the tab Resume gives.
pub async fn resume(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
    Json(req): Json<ResumeRequest>,
) -> Response {
    match resume_in_new_tab(st, agent, session_id, req).await {
        Ok(created) => Json(created).into_response(),
        Err(response) => *response,
    }
}

/// A failed answer, boxed so the `Result` stays small.
fn boxed(err: impl IntoResponse) -> Box<Response> {
    Box::new(err.into_response())
}

/// What the lookup before a Resume found.
enum Found {
    Usable(SessionMeta),
    /// Claude's transcript file is gone.
    Gone,
    RunningInATab,
}

async fn resume_in_new_tab(
    st: AppState,
    agent: String,
    session_id: String,
    req: ResumeRequest,
) -> Result<CreateSessionResponse, Box<Response>> {
    check(&agent, &session_id).map_err(boxed)?;
    let history = st.history();
    let id = session_id.clone();
    let running_tabs: Vec<String> = st
        .registry()
        .snapshot()
        .sessions
        .into_iter()
        .filter(|s| s.state == SessionState::Running)
        .map(|s| s.tab_id)
        .collect();
    let meta = blocking(move || {
        let meta = session_meta(&history, &id)?;
        let in_tab = running_tabs.contains(&meta.tab_id)
            && history
                .latest_for_tab(&meta.tab_id)
                .is_some_and(|latest| latest.session_id == meta.session_id);
        if in_tab {
            return Ok(Found::RunningInATab);
        }
        // The row lives in the Marshell home: its path is checked again before a file call.
        let usable = transcript_allowed(&meta.transcript_path, &meta.session_id)
            && std::path::Path::new(&meta.transcript_path).is_file();
        Ok(if usable { Found::Usable(meta) } else { Found::Gone })
    })
    .await
    .map_err(boxed)?;
    let meta = match meta {
        Found::Usable(meta) => meta,
        Found::Gone => return Err(Box::new(cannot_resume(NO_CONVERSATION))),
        Found::RunningInATab => return Err(Box::new(cannot_resume(RUNNING_IN_A_TAB))),
    };
    // The same check the tab Resume makes before it starts anything.
    let folder = meta.cwd.clone();
    if !folder.is_empty() && !tokio::fs::metadata(&folder).await.is_ok_and(|m| m.is_dir()) {
        return Err(Box::new(cannot_resume(format!(
            "Its folder no longer exists: {folder}"
        ))));
    }
    let run = resolve(
        &st,
        None,
        Some(AGENT),
        None,
        Some(ResumeTarget::Session(&meta.session_id)),
        None,
    )
    .map_err(boxed)?;
    start_session(
        &st,
        run,
        Some(AGENT.into()),
        None,
        Some(folder),
        (req.cols, req.rows),
        None,
        None,
    )
    .await
    .map_err(boxed)
}
