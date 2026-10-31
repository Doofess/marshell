//! The History view's routes: the list, full-text search, one conversation, its marks, export, Resume and Fork.
//! Claude Code and Codex conversations are kept; any other agent is a 404. An agent and a session id
//! from the URL are checked before anything touches the index or a file. Every call into the history and every
//! file read runs on a blocking thread.

use super::AppState;
use super::sessions::{NO_CONVERSATION, ResumeRequest, cannot_resume, resolve, start_session};
use crate::history::cast::{self, FrameStart, FramesError};
use crate::history::export::{AsciicastError, ExportFormat, export_text, to_asciicast};
use crate::history::marks::normalize_name;
use crate::history::recorder::{BookmarkRefusal, BookmarkTurnRefusal, DeleteRefusal, History};
use crate::history::{SessionMeta, archive, parse_for, recordings, sidecar_allowed_in, valid_session_id};
use crate::paths;
use crate::profiles::detect::ResumeTarget;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::CreateSessionResponse;
use marshell_protocol::history::{
    Bookmarked, Conversation, HistoryEntry, HistoryPatch, HistoryQuery, RecordingFrames, RecordingPart,
    RetentionLimits, RetentionPreview, SearchHit,
};
use serde::Deserialize;
use std::sync::Arc;

/// The agents whose conversations the History view opens.
const AGENTS: [&str; 2] = ["claude", "codex"];

/// Resume's refusal when a live tab already holds the conversation.
pub(super) const RUNNING_IN_A_TAB: &str = "This conversation is already running in a tab. Use Go to session.";

/// Fork starts from a Claude Code conversation only.
const FORK_CLAUDE_ONLY: &str = "Fork works only for Claude Code conversations.";

const NO_COPY: &str = "Marshell has no copy of this conversation yet.";

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
    if AGENTS.contains(&agent) && valid_session_id(session_id) {
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

/// The copy of conversations run outside Marshell: the switch and the progress. Reads atomics only.
pub async fn import_status(State(st): State<AppState>) -> Json<marshell_protocol::history::ImportStatus> {
    Json(st.import_status())
}

/// What a cleanup with these limits would remove now. 400 for a limit that isn't one of the choices; 503 when
/// history is off for this run, as there is no storage thread then.
pub async fn storage_preview(
    State(st): State<AppState>,
    Json(limits): Json<RetentionLimits>,
) -> Result<Json<RetentionPreview>, Fail> {
    crate::settings::check_limits(limits.limit_gb, limits.limit_days).map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
    let storage = st.storage().ok_or_else(off)?;
    let preview = blocking(move || Ok(storage.preview(&limits))).await?;
    Ok(Json(preview))
}

/// What Marshell keeps on disk, with the saved limits, the last cleanup and the backup. 503 when history is
/// off for this run, as there is no storage thread then.
pub async fn storage_report(
    State(st): State<AppState>,
) -> Result<Json<marshell_protocol::history::StorageReport>, Fail> {
    let storage = st.storage().ok_or_else(off)?;
    let settings = st.settings_snapshot();
    let limits = RetentionLimits {
        limit_gb: settings.history_limit_gb,
        limit_days: settings.history_limit_days,
    };
    let backup_dir = settings.history_backup_dir;
    let mut report = blocking(move || Ok(storage.usage(limits))).await?;
    // The folder is the saved one, so a change shows at once. The thread's last copy and error count only when
    // they were for that same folder.
    let same = report.backup.dir == backup_dir;
    report.backup = marshell_protocol::history::BackupStatus {
        dir: backup_dir,
        last_copy_ms: report.backup.last_copy_ms.filter(|_| same),
        error: report.backup.error.filter(|_| same),
    };
    Ok(Json(report))
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
    #[serde(default)]
    headless: bool,
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
        history.search(&query.q, query.archived, query.headless).map_err(failed)
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
    let conversation = blocking(move || load_conversation(&history, &home, &agent, &session_id)).await?;
    Ok(Json(conversation))
}

/// The conversation from Marshell's copy, for the view and for export. Blocking.
fn load_conversation(
    history: &History,
    home: &std::path::Path,
    agent: &str,
    session_id: &str,
) -> Result<Conversation, Fail> {
    require_on(history)?;
    let entry = history.entry(agent, session_id).map_err(failed)?.ok_or_else(no_such)?;
    let marks = history.marks(agent, session_id);
    match archive::read_archive(home, agent, session_id) {
        Ok(Some(text)) => {
            let mut conversation = parse_for(agent, &text, session_id).ok_or_else(no_such)?;
            if let Some(name) = &marks.name {
                conversation.title = name.clone();
            }
            conversation.tab_id = entry.tab_id;
            conversation.marks = marks;
            Ok(conversation)
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, NO_COPY.into())),
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
}

#[derive(Deserialize)]
pub struct ExportQuery {
    #[serde(default = "markdown")]
    format: ExportFormat,
    #[serde(default = "yes")]
    redact: bool,
}

fn markdown() -> ExportFormat {
    ExportFormat::Markdown
}

fn yes() -> bool {
    true
}

/// The conversation as markdown (the default), JSON or an asciicast of its recording, with secrets and personal
/// paths hidden unless `redact=false`. The text is built here; the Save dialog is the shell's.
pub async fn export(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, Fail> {
    check(&agent, &session_id)?;
    let history = st.history();
    let home = st.home().to_path_buf();
    let (format, redact) = (query.format, query.redact);
    let text = blocking(move || match format {
        ExportFormat::Asciicast => asciicast(&history, &home, &agent, &session_id, redact),
        _ => {
            let conversation = load_conversation(&history, &home, &agent, &session_id)?;
            Ok(export_text(conversation, format, redact))
        }
    })
    .await?;
    let kind = match format {
        ExportFormat::Markdown => "text/markdown; charset=utf-8",
        ExportFormat::Json => "application/json",
        ExportFormat::Asciicast => "application/x-asciicast",
    };
    Ok(([(header::CONTENT_TYPE, kind)], text).into_response())
}

const NO_RECORDING: &str = "Marshell has no recording of this conversation.";
const RECORDING_TOO_LARGE: &str = "This recording is over 64 MiB, too large to export.";

/// The conversation's recording as an asciicast, titled with its name or its title. Blocking.
fn asciicast(
    history: &History,
    home: &std::path::Path,
    agent: &str,
    session_id: &str,
    redact: bool,
) -> Result<String, Fail> {
    let parts = recording_parts(history, home, agent, session_id)?;
    if parts.is_empty() {
        return Err((StatusCode::NOT_FOUND, NO_RECORDING.into()));
    }
    let entry = history.entry(agent, session_id).map_err(failed)?.ok_or_else(no_such)?;
    let title = history.marks(agent, session_id).name.unwrap_or(entry.title);
    let redactor = redact.then(crate::diagnostics::redact::global);
    to_asciicast(home, &parts, &title, redactor).map_err(|err| match err {
        AsciicastError::TooLarge => (StatusCode::PAYLOAD_TOO_LARGE, RECORDING_TOO_LARGE.into()),
        AsciicastError::Failed(err) => failed(err),
    })
}

const NO_PART: &str = "No such part of the recording.";

/// The most event data one frames answer carries.
const FRAMES_BUDGET: usize = 1 << 20;

/// The parts of the conversation's recording, from its sidecar. The span starts at the first prompt whose time
/// is known (the index has the turn times, so the transcript isn't parsed again) and ends at the sidecar's last
/// update. Blocking.
fn recording_parts(
    history: &History,
    home: &std::path::Path,
    agent: &str,
    session_id: &str,
) -> Result<Vec<RecordingPart>, Fail> {
    require_on(history)?;
    let meta = archive::read_meta(home, agent, session_id)
        .map_err(failed)?
        .ok_or_else(no_such)?;
    let from = history
        .first_prompt_ms(agent, session_id)
        .map_err(failed)?
        .unwrap_or(meta.started_ms);
    Ok(recordings::parts(home, &meta, (from, meta.updated_ms), history.casts()))
}

/// The recording files that cover the conversation, in time order; empty when it has none.
pub async fn recordings(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
) -> Result<Json<Vec<RecordingPart>>, Fail> {
    check(&agent, &session_id)?;
    let history = st.history();
    let home = st.home().to_path_buf();
    let parts = blocking(move || recording_parts(&history, &home, &agent, &session_id)).await?;
    Ok(Json(parts))
}

#[derive(Deserialize)]
pub struct FramesQuery {
    from_ms: Option<u64>,
    to_ms: Option<u64>,
    offset: Option<u64>,
}

/// A stretch of one part's events. `offset` (from an earlier answer's `next_offset`) continues where it stopped;
/// otherwise `from_ms` (default: the part's start) seeks, and the answer carries the screen at that moment.
/// `to_ms` is exclusive and defaults to the end of the part.
pub async fn recording_frames(
    State(st): State<AppState>,
    Path((agent, session_id, part)): Path<(String, String, u32)>,
    Query(query): Query<FramesQuery>,
) -> Result<Json<RecordingFrames>, Fail> {
    check(&agent, &session_id)?;
    let history = st.history();
    let home = st.home().to_path_buf();
    let frames = blocking(move || {
        let parts = recording_parts(&history, &home, &agent, &session_id)?;
        let part = parts
            .into_iter()
            .find(|p| p.index == part)
            .ok_or((StatusCode::NOT_FOUND, NO_PART.into()))?;
        let start = match query.offset {
            Some(offset) => FrameStart::Offset(offset),
            None => FrameStart::At(query.from_ms.unwrap_or(part.started_ms)),
        };
        // The part's last event is at `ended_ms`, and `to_ms` is exclusive.
        let to_ms = query.to_ms.unwrap_or(part.ended_ms + 1);
        let path =
            cast::resolve(&paths::recordings_dir(&home), &part.file).ok_or((StatusCode::NOT_FOUND, NO_PART.into()))?;
        cast::frames(&path, part.started_ms, start, to_ms, FRAMES_BUDGET).map_err(|err| match err {
            FramesError::NotABoundary => (StatusCode::BAD_REQUEST, err.to_string()),
            FramesError::Failed(err) => failed(err),
        })
    })
    .await?;
    Ok(Json(RecordingFrames {
        cols: frames.cols,
        rows: frames.rows,
        screen: frames.screen,
        events: frames.events,
        next_offset: frames.next_offset,
    }))
}

pub async fn patch(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
    Json(patch): Json<HistoryPatch>,
) -> Result<Json<HistoryEntry>, Fail> {
    check(&agent, &session_id)?;
    let history = st.history();
    let entry = blocking(move || {
        session_meta(&history, &agent, &session_id)?;
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
        session_meta(&history, &agent, &session_id)?;
        history
            .bookmark_turn(&agent, &session_id, turn, on)
            .map_err(|err| match err {
                BookmarkTurnRefusal::Failed(err) => failed(err),
                err => (StatusCode::CONFLICT, err.to_string()),
            })
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
        session_meta(&history, &agent, &session_id)?;
        history.delete_copy(&agent, &session_id).map_err(|err| match err {
            DeleteRefusal::Failed(err) => failed(err),
            err => (StatusCode::CONFLICT, err.to_string()),
        })
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The session's sidecar data, once the id is known to be Claude's and valid.
fn session_meta(history: &Arc<History>, agent: &str, session_id: &str) -> Result<SessionMeta, Fail> {
    require_on(history)?;
    history.get(agent, session_id).map_err(failed)?.ok_or_else(no_such)
}

/// Starts Claude in a new tab with `--resume <id>`, in the folder the conversation ran in, under the name the
/// person gave the conversation. If Claude's transcript file is gone, the answer is the refusal the tab Resume
/// gives.
pub async fn resume(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
    Json(req): Json<ResumeRequest>,
) -> Response {
    answer(open_in_new_tab(st, agent, session_id, req, false).await)
}

/// Like Resume, but Claude starts a new conversation from this one (`--fork-session`), so it is allowed while the
/// conversation runs in a tab. The tab is named `{name} (fork)` when the conversation has a name.
pub async fn fork(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
    Json(req): Json<ResumeRequest>,
) -> Response {
    answer(open_in_new_tab(st, agent, session_id, req, true).await)
}

fn answer(result: Result<CreateSessionResponse, Box<Response>>) -> Response {
    match result {
        Ok(created) => Json(created).into_response(),
        Err(response) => *response,
    }
}

/// Why a conversation can't be resumed when its source file is gone.
pub(super) fn gone(agent: &str) -> String {
    if agent == "codex" {
        "Codex no longer has this conversation, so it can't be resumed.".into()
    } else {
        NO_CONVERSATION.into()
    }
}

/// A failed answer, boxed so the `Result` stays small.
fn boxed(err: impl IntoResponse) -> Box<Response> {
    Box::new(err.into_response())
}

/// What the lookup before a Resume or Fork found.
pub(super) enum Found {
    /// The conversation, and the name the person gave it, if any.
    Usable(Box<SessionMeta>, Option<String>),
    /// Claude's transcript file is gone.
    Gone,
    RunningInATab,
}

/// The tab's name for a fork: `{name} (fork)`, with the name cut so the suffix survives the name limit.
fn fork_name(name: &str) -> Option<String> {
    const SUFFIX: &str = " (fork)";
    let room = 120 - SUFFIX.len();
    normalize_name(&format!("{}{SUFFIX}", name.chars().take(room).collect::<String>()))
}

/// Finds the conversation a Resume, a Fork, a copied command or an outside terminal starts from: its sidecar, the
/// preset it ran with and the name its tab opens under. `refuse_running` answers `RunningInATab` when a live tab holds
/// it as its latest conversation. `Gone` is a transcript that fails the path check or is no longer a file.
pub(super) async fn look_up(st: &AppState, agent: &str, session_id: &str, refuse_running: bool) -> Result<Found, Fail> {
    check(agent, session_id)?;
    let history = st.history();
    let (id, agent) = (session_id.to_string(), agent.to_string());
    let running_tabs: Vec<String> = st
        .registry()
        .snapshot()
        .sessions
        .into_iter()
        .filter(|s| s.running())
        .map(|s| s.tab_id)
        .collect();
    let home = st.home().to_path_buf();
    blocking(move || {
        let mut meta = session_meta(&history, &agent, &id)?;
        // The index does not keep the preset or the WSL distro; the sidecar does.
        let stored = crate::history::archive::read_meta(&home, &agent, &id).ok().flatten();
        meta.preset = stored.as_ref().and_then(|stored| stored.preset.clone());
        meta.distro = stored.and_then(|stored| stored.distro);
        let in_tab = refuse_running
            && running_tabs.contains(&meta.tab_id)
            && history
                .latest_for_tab(&meta.tab_id)
                .is_some_and(|latest| latest.session_id == meta.session_id);
        if in_tab {
            return Ok(Found::RunningInATab);
        }
        // The row lives in the Marshell home: its path is checked again before a file call.
        let usable = sidecar_allowed_in(&agent, &meta.transcript_path, &meta.session_id, meta.distro.as_deref())
            && std::path::Path::new(&meta.transcript_path).is_file();
        if !usable {
            return Ok(Found::Gone);
        }
        let name = history.marks(&agent, &meta.session_id).name;
        Ok(Found::Usable(Box::new(meta), name))
    })
    .await
}

/// The id of the WSL profile that starts `distro` (its `-d` names it, whatever the case), if there is one.
fn wsl_profile_of(st: &AppState, distro: &str) -> Option<String> {
    st.profiles_snapshot()
        .profiles
        .into_iter()
        .find(|p| {
            matches!(p.family, marshell_protocol::api::ShellFamily::Wsl)
                && crate::wsl::distro_of_argv(&p.command).is_some_and(|d| d.eq_ignore_ascii_case(distro))
        })
        .map(|p| p.id)
}

async fn open_in_new_tab(
    st: AppState,
    agent: String,
    session_id: String,
    req: ResumeRequest,
    fork: bool,
) -> Result<CreateSessionResponse, Box<Response>> {
    check(&agent, &session_id).map_err(boxed)?;
    if fork && agent != "claude" {
        return Err(Box::new(cannot_resume(FORK_CLAUDE_ONLY)));
    }
    let agent_name = agent.clone();
    let meta = look_up(&st, &agent, &session_id, !fork).await.map_err(boxed)?;
    let (meta, tab_name) = match meta {
        Found::Usable(meta, name) => (*meta, name),
        Found::Gone => return Err(Box::new(cannot_resume(gone(&agent_name)))),
        Found::RunningInATab => return Err(Box::new(cannot_resume(RUNNING_IN_A_TAB))),
    };
    // A console Marshell opened for it still runs: a Resume would be a second CLI on one conversation. A fork
    // starts a new conversation, so it may go on.
    if !fork
        && super::outside::in_console(
            &st,
            Some(&agent_name),
            &super::sessions::Resume::Session(meta.session_id.clone()),
            "",
        )
    {
        return Err(Box::new(cannot_resume(super::outside::OPEN_IN_TERMINAL)));
    }
    let tab_name = if fork {
        tab_name.as_deref().and_then(fork_name)
    } else {
        tab_name
    };
    // A conversation that ran in a WSL distro starts in that distro's profile again; the folder is then a Windows path
    // (a `\\wsl.localhost\<distro>\...` one, or a drive) that `wsl.exe` maps to the distro's own. Decided before the
    // folder is touched: a UNC folder may wake a stopped distro.
    let profile_id = match meta.distro.as_deref() {
        Some(distro) => Some(
            wsl_profile_of(&st, distro)
                .ok_or_else(|| Box::new(cannot_resume(format!("No WSL profile for {distro}."))))?,
        ),
        None => None,
    };
    // The same check the tab Resume makes before it starts anything.
    let folder = meta.cwd.clone();
    if !folder.is_empty() && !tokio::fs::metadata(&folder).await.is_ok_and(|m| m.is_dir()) {
        return Err(Box::new(cannot_resume(format!(
            "Its folder no longer exists: {folder}"
        ))));
    }
    let target = if fork {
        ResumeTarget::Fork(&meta.session_id)
    } else {
        ResumeTarget::Session(&meta.session_id)
    };
    // The conversation's tab, if Marshell still has it: a DeepSeek conversation opens on DeepSeek again.
    let preset = meta
        .preset
        .clone()
        .or_else(|| st.registry().preset_of_tab(&meta.tab_id));
    let run = resolve(
        &st,
        profile_id.as_deref(),
        Some(agent_name.as_str()),
        None,
        Some(target),
        None,
        preset.as_deref(),
    )
    .await
    .map_err(boxed)?;
    let created = start_session(
        &st,
        run,
        Some(agent_name.clone()),
        tab_name.clone(),
        Some(folder),
        (req.cols, req.rows),
        None,
        None,
        preset,
        true,
    )
    .await
    .map_err(boxed)?;
    // The conversation Claude starts in that tab takes the same name.
    if tab_name.is_some() {
        st.history().name_tab(&created.tab_id, tab_name.as_deref());
    }
    Ok(created)
}
