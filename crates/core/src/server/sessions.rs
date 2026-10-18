use super::AppState;
use crate::auth;
use crate::profiles::detect::{agent_command, default_shell, launch_argv, launch_argv_resume};
use crate::pty::{FreezeError, Session, SpawnSpec};
use crate::registry::{Relaunch, ResumeRefusal};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{
    ColorScheme, CreateSessionRequest, CreateSessionResponse, InputRequest, RenameRequest, ReopenRequest,
    ReorderRequest, RestartRequest, ResumeError, SessionInfo, SessionMode, SessionState, SessionsSnapshot, ShellFamily,
};
use marshell_protocol::brand;
use serde::Deserialize;
use std::path::PathBuf;

type ApiError = (StatusCode, String);

/// The profile id of a session started from an explicit `shell` argv.
const CUSTOM_PROFILE: &str = "custom";

/// The name an agent session gets when the request names none.
/// None for an id that is not a launcher agent; callers validate the id first.
fn agent_display_name(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("Claude Code"),
        "codex" => Some("Codex"),
        "antigravity" => Some("Antigravity"),
        _ => None,
    }
}

/// The `COLORFGBG` value (foreground;background as ANSI palette numbers) for a terminal's scheme.
/// Some programs read it at start to choose a dark or light look.
fn colorfgbg(scheme: ColorScheme) -> &'static str {
    match scheme {
        ColorScheme::Dark => "15;0",
        ColorScheme::Light => "0;15",
    }
}

/// The variables a session starts with before ours: `COLORFGBG` for the scheme (none when the
/// scheme is unknown), then the profile's own, which come after so a profile can override it.
fn spawn_env(scheme: Option<ColorScheme>, profile_env: &[(String, String)]) -> Vec<(String, String)> {
    let mut env = Vec::new();
    if let Some(scheme) = scheme {
        env.push(("COLORFGBG".to_string(), colorfgbg(scheme).to_string()));
    }
    env.extend(profile_env.iter().cloned());
    env
}

/// What to run: the profile it came from, the argv, and the profile's env and folder.
pub(super) struct Resolved {
    pub(super) profile_id: String,
    pub(super) profile_name: String,
    family: ShellFamily,
    argv: Vec<String>,
    env: Vec<(String, String)>,
    cwd: Option<String>,
    /// Shell integration is on, so the output carries prompt marks.
    marks: bool,
}

/// Finds the profile (the default when `profile_id` is None) and builds the argv for the agent.
/// With `resume` the agent is told to continue its last conversation, and there is no prompt.
/// The mode becomes the agent's own flags; a plain shell has none.
pub(super) fn resolve(
    st: &AppState,
    profile_id: Option<&str>,
    agent: Option<&str>,
    prompt: Option<&str>,
    resume: bool,
    mode: Option<SessionMode>,
) -> Result<Resolved, ApiError> {
    // Copy the profile out so the lock is gone before any await.
    let profile = {
        let doc = st.0.profiles.lock().unwrap();
        let id = profile_id.filter(|id| !id.is_empty()).unwrap_or(&doc.default_id);
        doc.profiles.iter().find(|p| p.id == id).cloned()
    };
    match (profile, profile_id) {
        (Some(p), _) => {
            let argv = if resume {
                launch_argv_resume(&p, agent, mode)
            } else {
                launch_argv(&p, agent, prompt, mode)
            };
            let mut argv = argv.map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
            let mut env = p.env.clone();
            let mut marks = false;
            // Shell integration is for a plain shell; an agent session runs the agent instead.
            if agent.is_none()
                && let Some(dir) = &st.0.shell_dir
                && let Ok(applied) = crate::shell::integrate(&p, dir, &|k| std::env::var(k).ok())
            {
                argv = applied.argv;
                // After the profile's env, so ours win.
                env.extend(applied.env);
                marks = true;
            }
            Ok(Resolved {
                profile_id: p.id,
                profile_name: p.name,
                family: p.family,
                argv,
                env,
                cwd: p.cwd,
                marks,
            })
        }
        // A reopened session that started from an explicit argv has no argv to restore.
        (None, Some(CUSTOM_PROFILE)) => Ok(custom(default_shell())),
        (None, id) => Err((
            StatusCode::BAD_REQUEST,
            format!("Unknown profile: {}.", id.unwrap_or("(default)")),
        )),
    }
}

fn custom(argv: Vec<String>) -> Resolved {
    Resolved {
        profile_id: CUSTOM_PROFILE.into(),
        profile_name: "Shell".into(),
        family: ShellFamily::Posix,
        argv,
        env: Vec::new(),
        cwd: None,
        marks: false,
    }
}

/// Refuses an agent whose program is not on PATH. The shell would start anyway and say so
/// itself, so look for the agent first. A WSL distro has its own PATH: those are not checked.
fn check_agent_installed(st: &AppState, run: &Resolved, agent: Option<&str>) -> Result<(), ApiError> {
    if let Some(agent) = agent
        && run.family != ShellFamily::Wsl
        && let Some(command) = agent_command(agent)
        && !(st.0.agent_on_path)(command)
    {
        let name = agent_display_name(agent).unwrap_or(command);
        return Err((
            StatusCode::BAD_REQUEST,
            format!("{name} isn't installed or isn't on PATH."),
        ));
    }
    Ok(())
}

/// The folder a session starts in: the requested one, else the profile's, else the home folder.
pub(super) fn start_folder(run: &Resolved, requested: Option<String>) -> String {
    let home = dirs::home_dir().map(|p| p.to_string_lossy().into_owned());
    requested
        .filter(|c| !c.is_empty())
        .or(run.cwd.clone())
        .or(home)
        .unwrap_or_default()
}

/// Reserves a number, starts the process and records it. A failed spawn gives the number back.
#[allow(clippy::too_many_arguments)]
async fn start_session(
    st: &AppState,
    run: Resolved,
    agent: Option<String>,
    name: Option<String>,
    cwd: Option<String>,
    size: (u16, u16),
    scheme: Option<ColorScheme>,
    mode: Option<SessionMode>,
) -> Result<CreateSessionResponse, ApiError> {
    check_agent_installed(st, &run, agent.as_deref())?;
    let cwd = start_folder(&run, cwd);
    // The base name: the requested one, else the agent, else the profile.
    let name = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            let agent_name = agent.as_deref().and_then(agent_display_name);
            agent_name.map_or_else(|| run.profile_name.clone(), String::from)
        });

    let tab_id = auth::new_id("t");
    // The id is new, so the claim always succeeds. Held until the process is in the map, so a
    // Restart that finds the reserved row cannot start a second process for it.
    let _claim = st.claim_start(&tab_id);
    let (number, generation) = st.0.registry.reserve(
        &tab_id,
        Relaunch {
            profile_id: run.profile_id.clone(),
            agent: agent.clone(),
            cwd: cwd.clone(),
            mode,
            name,
        },
    );
    if let Err(err) = launch(st, &tab_id, number, generation, run, agent, cwd, size, scheme).await {
        // A spawn failed, or the tab was closed while it started (then there is no open row to release).
        st.0.registry.release(&tab_id);
        return Err(err);
    }
    Ok(CreateSessionResponse { tab_id, number })
}

/// Starts the process for a tab that is already in the registry, and stores its session.
/// The caller undoes the registry change when this fails.
#[allow(clippy::too_many_arguments)]
async fn launch(
    st: &AppState,
    tab_id: &str,
    number: u32,
    generation: u64,
    run: Resolved,
    agent: Option<String>,
    cwd: String,
    size: (u16, u16),
    scheme: Option<ColorScheme>,
) -> Result<(), ApiError> {
    spawn_into_map(st, tab_id, number, generation, run, agent, cwd, size, scheme).await?;
    // A close can land between reserve and insert, when the map has nothing to kill yet.
    // Checking after the insert catches it: a later close finds the session in the map.
    if !st.0.registry.is_open(tab_id) {
        if let Some(session) = st.remove_session(tab_id) {
            session.kill();
        }
        return Err((
            StatusCode::CONFLICT,
            "The session was closed while it was starting.".into(),
        ));
    }
    Ok(())
}

/// Starts the process and stores its session in the sessions map, without touching the registry.
/// The exit and mark callbacks name the tab in the registry and do nothing while it has no row:
/// the shell panel starts here and only gets a row when it is adopted. The exit callback carries
/// `generation`, so a process that a restart replaced cannot mark its successor Exited.
#[allow(clippy::too_many_arguments)]
pub(super) async fn spawn_into_map(
    st: &AppState,
    tab_id: &str,
    number: u32,
    generation: u64,
    run: Resolved,
    agent: Option<String>,
    cwd: String,
    size: (u16, u16),
    scheme: Option<ColorScheme>,
) -> Result<(), ApiError> {
    // The profile's variables first, so ours win.
    let mut env = spawn_env(scheme, &run.env);
    env.extend([
        (brand::env_var("TAB_ID"), tab_id.to_string()),
        (brand::env_var("SESSION_NUMBER"), number.to_string()),
        (brand::env_var("AGENT"), agent.unwrap_or_else(|| "shell".into())),
        ("TERM".into(), "xterm-256color".into()),
        ("COLORTERM".into(), "truecolor".into()),
    ]);
    let registry = st.0.registry.clone();
    let exited_tab = tab_id.to_string();
    let on_mark: Option<crate::pty::MarkCallback> = if run.marks {
        let registry = st.0.registry.clone();
        let wake = st.0.foreground_wake.clone();
        let tab = tab_id.to_string();
        Some(Box::new(move |mark| {
            let started = mark == crate::pty::Mark::Executed;
            registry.mark(&tab, mark);
            // A command just started: look at what it runs now, not up to a second later. After the
            // registry knows the command runs, which is what lets a program be shown for the row.
            if started {
                wake.notify_one();
            }
        }))
    } else {
        None
    };
    // `spec` takes the cwd; the closure below records the same folder.
    let marshell_home = st.0.home.clone();
    let project = cwd.clone();
    let unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let recording = crate::paths::recordings_dir(&st.0.home).join(format!("{tab_id}-{unix_ms}.cast"));
    let shell = run.argv.first().cloned();
    let spec = SpawnSpec {
        argv: run.argv,
        cwd: (!cwd.is_empty()).then(|| PathBuf::from(cwd)),
        env,
        cols: size.0,
        rows: size.1,
        on_exit: Some(Box::new(move |code| {
            registry.mark_exited(&exited_tab, generation, code)
        })),
        on_mark,
        record: Some((recording, shell)),
    };
    let id = tab_id.to_string();
    // Spawning touches the OS (process creation) and recording writes a file:
    // keep both off the async worker threads.
    let spawned = tokio::task::spawn_blocking(move || {
        let session = Session::spawn(id, spec)?;
        if !project.is_empty() {
            // The folder the process really started in. `record` skips the user's home folder.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            if let Err(err) = crate::projects::record(&marshell_home, std::path::Path::new(&project), now_ms) {
                tracing::warn!("could not record the project: {err:#}");
            }
        }
        Ok::<_, anyhow::Error>(session)
    })
    .await;
    let session = match spawned {
        Ok(Ok(session)) => session,
        // No prefix here: the UI already says "Couldn't start a shell." and appends this text.
        Ok(Err(e)) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))),
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    st.insert_session(session);
    Ok(())
}

pub async fn create(
    State(st): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, ApiError> {
    if let Some(agent) = req.agent.as_deref().filter(|a| agent_command(a).is_none()) {
        return Err((StatusCode::BAD_REQUEST, format!("Unknown agent: {agent}.")));
    }
    // An explicit argv keeps the old behaviour: run it as given, as a custom profile.
    // A plain shell has no mode.
    let mode = req.mode.filter(|_| req.agent.is_some() && req.shell.is_none());
    let (run, agent) = match req.shell {
        Some(argv) => {
            if argv.is_empty() {
                // `CommandBuilder::from_argv` would panic on an empty argv; refuse it before spawning.
                return Err((StatusCode::BAD_REQUEST, "The shell argv is empty.".into()));
            }
            (custom(argv), None)
        }
        None => (
            resolve(
                &st,
                req.profile_id.as_deref(),
                req.agent.as_deref(),
                req.prompt.as_deref(),
                false,
                mode,
            )?,
            req.agent,
        ),
    };
    let size = (req.cols, req.rows);
    // What the launcher offers next time for this folder. An explicit argv is not a launcher choice.
    let folder = req
        .cwd
        .clone()
        .filter(|c| !c.is_empty() && run.profile_id != CUSTOM_PROFILE);
    let choice = folder.map(|folder| (folder, run.profile_id.clone(), agent.clone()));
    let created = start_session(&st, run, agent, req.name, req.cwd, size, req.color_scheme, mode).await?;
    if let Some((folder, profile_id, agent)) = choice {
        remember_choice(&st, folder, profile_id, agent, mode).await;
    }
    Ok(Json(created))
}

/// Saves the launcher choice in layout.json under the layout lock, like `put_view`.
/// The session is already running, so a failed save is only logged.
async fn remember_choice(
    st: &AppState,
    folder: String,
    profile_id: String,
    agent: Option<String>,
    mode: Option<SessionMode>,
) {
    let state = st.clone();
    let saved = tokio::task::spawn_blocking(move || {
        let mut current = state.0.layout.lock().unwrap_or_else(|e| e.into_inner());
        let mut next = current.clone();
        crate::layout::remember(&mut next, &folder, profile_id, agent, mode);
        let result = crate::layout::save(&state.0.home, &next);
        // The memory is updated even when the save failed, so the launcher still remembers this run.
        *current = next;
        result
    })
    .await;
    match saved {
        Ok(Ok(())) => {}
        Ok(Err(err)) => tracing::warn!("could not save the launcher choice: {err:#}"),
        Err(err) => tracing::warn!("saving the launcher choice failed: {err}"),
    }
}

pub async fn list(State(st): State<AppState>) -> Json<SessionsSnapshot> {
    Json(st.0.registry.snapshot())
}

pub async fn rename(
    State(st): State<AppState>,
    Path(tab): Path<String>,
    Json(req): Json<RenameRequest>,
) -> Result<StatusCode, ApiError> {
    if req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "The name is empty.".into()));
    }
    if st.0.registry.rename(&tab, &req.name) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "No such session.".into()))
    }
}

pub async fn reorder(State(st): State<AppState>, Json(req): Json<ReorderRequest>) -> Result<StatusCode, ApiError> {
    if st.0.registry.reorder(&req.tab_ids) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "The list must hold every open session exactly once.".into(),
        ))
    }
}

/// Types text into the session as if the user had typed it. The bytes go to the pty as they are;
/// no Enter is added, so the user reads it first and presses Enter themselves. A line break in the
/// text would submit it, so that is refused.
pub async fn input(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<InputRequest>) -> Response {
    let Some(session) = st.session(&tab) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if session.is_frozen() {
        return cannot_resume("It is paused.");
    }
    if req.text.contains(['\r', '\n']) {
        return (StatusCode::BAD_REQUEST, "The text can't contain a line break.").into_response();
    }
    // Typed text answers in the terminal, the same as a key in the pane.
    st.holds().release_tab(&tab);
    session.write_input(req.text.into_bytes());
    StatusCode::NO_CONTENT.into_response()
}

/// Where the session's asciicast recording is. 404 for an unknown tab, and for one that has no recording.
pub async fn recording(State(st): State<AppState>, Path(tab): Path<String>) -> Response {
    match st.session(&tab).and_then(|s| s.recording_path()) {
        Some(path) => Json(serde_json::json!({ "path": path })).into_response(),
        None => (StatusCode::NOT_FOUND, "This session has no recording.").into_response(),
    }
}

/// Always 204, also when the tab is already closed or unknown, so a retry is harmless.
pub async fn close(State(st): State<AppState>, Path(tab): Path<String>) -> StatusCode {
    st.0.registry.close(&tab);
    if let Some(session) = st.remove_session(&tab) {
        session.kill();
    }
    StatusCode::NO_CONTENT
}

/// The body is optional: an empty body reopens the most recently closed session.
pub async fn reopen(State(st): State<AppState>, body: Bytes) -> Result<Json<CreateSessionResponse>, ApiError> {
    let wanted = if body.iter().all(u8::is_ascii_whitespace) {
        None
    } else {
        serde_json::from_slice::<ReopenRequest>(&body)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
            .tab_id
    };
    // Peek, not take: if the start fails below, the entry stays in the closed list.
    let Some((closed_id, saved)) = st.0.registry.peek_closed(wanted.as_deref()) else {
        return Err((StatusCode::NOT_FOUND, "Nothing to reopen.".into()));
    };
    // A fresh session from the saved profile, agent, folder and name (not the old row).
    let run = resolve(
        &st,
        Some(&saved.profile_id),
        saved.agent.as_deref(),
        None,
        false,
        saved.mode,
    )?;
    // The window resizes the terminal as soon as it attaches.
    // The saved session has no colour scheme, so a reopened one starts without COLORFGBG.
    let created = start_session(
        &st,
        run,
        saved.agent,
        Some(saved.name),
        Some(saved.cwd),
        (80, 24),
        None,
        saved.mode,
    )
    .await?;
    // Only now remove the entry. If another reopen took it first, drop the duplicate we started.
    if st.0.registry.take_closed(Some(&closed_id)).is_none() {
        st.0.registry.release(&created.tab_id);
        if let Some(session) = st.remove_session(&created.tab_id) {
            session.kill();
        }
        return Err((StatusCode::NOT_FOUND, "Nothing to reopen.".into()));
    }
    Ok(Json(created))
}

#[derive(Deserialize)]
pub struct ResumeRequest {
    cols: u16,
    rows: u16,
}

/// A 409 whose body tells the user why Resume cannot start the tab.
pub(super) fn cannot_resume(reason: impl Into<String>) -> Response {
    (StatusCode::CONFLICT, Json(ResumeError { reason: reason.into() })).into_response()
}

/// Why the tab's profile or folder rules out starting it again (Resume and Restart), if they do.
fn cannot_start_again(st: &AppState, info: &SessionInfo) -> Option<String> {
    let profile_known = info.profile_id == CUSTOM_PROFILE
        || st
            .0
            .profiles
            .lock()
            .unwrap()
            .profiles
            .iter()
            .any(|p| p.id == info.profile_id);
    if !profile_known {
        return Some("Its shell profile was removed.".into());
    }
    if !info.cwd.is_empty() && !std::path::Path::new(&info.cwd).is_dir() {
        return Some(format!("Its folder no longer exists: {}", info.cwd));
    }
    None
}

/// Starts an Ended or Exited tab again in the same row, with the CLI's own continue flag for an agent.
pub async fn resume(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<ResumeRequest>) -> Response {
    let Some(info) = st.0.registry.snapshot().sessions.into_iter().find(|s| s.tab_id == tab) else {
        return (StatusCode::NOT_FOUND, "No such session.").into_response();
    };
    // Held until the new process is in the map. Another start shows the row Running already.
    let Some(_claim) = st.claim_start(&tab) else {
        return cannot_resume("It is already running.");
    };
    // These checks come before begin_resume, so a refusal leaves the tab as it was.
    if info.state == SessionState::Running {
        return cannot_resume("It is already running.");
    }
    if let Some(reason) = cannot_start_again(&st, &info) {
        return cannot_resume(reason);
    }
    let run = match resolve(
        &st,
        Some(&info.profile_id),
        info.agent.as_deref(),
        None,
        true,
        info.mode,
    ) {
        Ok(run) => run,
        Err(err) => return err.into_response(),
    };
    if let Err((_, message)) = check_agent_installed(&st, &run, info.agent.as_deref()) {
        return cannot_resume(message);
    }
    // The check and the change to Running happen under one lock: of two resumes at once, one wins.
    let (saved, previous, generation) = match st.0.registry.begin_resume(&tab) {
        Ok(started) => started,
        Err(ResumeRefusal::NotFound) => return (StatusCode::NOT_FOUND, "No such session.").into_response(),
        Err(ResumeRefusal::AlreadyRunning) => return cannot_resume("It is already running."),
    };
    // An Exited tab still has its old (finished) process in the map.
    if let Some(old) = st.remove_session(&tab) {
        old.kill();
    }
    // The tab keeps its row, so its name is not touched: no naming path runs here.
    // The saved tab has no colour scheme, so a resumed session starts without COLORFGBG.
    match launch(
        &st,
        &tab,
        info.number,
        generation,
        run,
        saved.agent,
        saved.cwd,
        (req.cols, req.rows),
        None,
    )
    .await
    {
        Ok(()) => Json(CreateSessionResponse {
            tab_id: tab,
            number: info.number,
        })
        .into_response(),
        Err(err) => {
            st.0.registry.set_state(&tab, previous);
            err.into_response()
        }
    }
}

/// Freezes the session's process tree, or thaws it. Holds the tab's start claim meanwhile, so a
/// Resume or Restart cannot replace the process under it. Pausing twice, or continuing a session
/// that isn't paused, changes nothing and still answers 204.
async fn set_frozen(st: AppState, tab: String, frozen: bool) -> Response {
    // The claim comes first: a Resume or Restart that finished before it has already bumped the
    // generation, and one that starts after it is refused. So the generation read below is the
    // one of the process this call acts on.
    let Some(_claim) = st.claim_start(&tab) else {
        return cannot_resume("It is already starting.");
    };
    let Some(generation) = st.0.registry.generation(&tab) else {
        return (StatusCode::NOT_FOUND, "No such session.").into_response();
    };
    let session = st.session(&tab);
    if frozen {
        let Some(session) = session else {
            return cannot_resume(FreezeError::NotRunning.to_string());
        };
        if let Err(err) = session.freeze() {
            return cannot_resume(err.to_string());
        }
    } else if let Some(session) = session {
        session.thaw();
    }
    st.0.registry.set_paused(&tab, generation, frozen);
    StatusCode::NO_CONTENT.into_response()
}

/// Freezes a running session: the program and everything it started.
pub async fn pause(State(st): State<AppState>, Path(tab): Path<String>) -> Response {
    set_frozen(st, tab, true).await
}

/// Thaws a paused session. (`continue` is a keyword.)
pub async fn cont(State(st): State<AppState>, Path(tab): Path<String>) -> Response {
    set_frozen(st, tab, false).await
}

/// Starts an agent session again in the same row in another mode, continuing its last conversation
/// (the CLI's own continue flag). The old process, if any, is killed. Every refusal comes before
/// that, so a refused restart leaves the session running.
pub async fn restart(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<RestartRequest>) -> Response {
    let Some(info) = st.0.registry.snapshot().sessions.into_iter().find(|s| s.tab_id == tab) else {
        return (StatusCode::NOT_FOUND, "No such session.").into_response();
    };
    if info.agent.is_none() {
        return cannot_resume("Only an agent session has a mode.");
    }
    // Held until the new process is in the map, so a second Restart or a Resume cannot start one too.
    let Some(_claim) = st.claim_start(&tab) else {
        return cannot_resume("It is already starting.");
    };
    if let Some(reason) = cannot_start_again(&st, &info) {
        return cannot_resume(reason);
    }
    let run = match resolve(
        &st,
        Some(&info.profile_id),
        info.agent.as_deref(),
        None,
        true,
        Some(req.mode),
    ) {
        Ok(run) => run,
        Err(err) => return err.into_response(),
    };
    if let Err((_, message)) = check_agent_installed(&st, &run, info.agent.as_deref()) {
        return cannot_resume(message);
    }
    // From here the restart goes ahead. The row takes the new mode and a new generation first, so the
    // old process's exit (which the kill below causes) no longer touches it.
    let (saved, generation) = match st.0.registry.begin_restart(&tab, req.mode) {
        Ok(started) => started,
        Err(_) => return (StatusCode::NOT_FOUND, "No such session.").into_response(),
    };
    // No lock is held here: `remove_session` drops the map lock before it returns, and `kill` only
    // signals the process tree and returns.
    if let Some(old) = st.remove_session(&tab) {
        old.kill();
    }
    match launch(
        &st,
        &tab,
        info.number,
        generation,
        run,
        saved.agent,
        saved.cwd,
        (req.cols, req.rows),
        None,
    )
    .await
    {
        Ok(()) => Json(CreateSessionResponse {
            tab_id: tab,
            number: info.number,
        })
        .into_response(),
        Err(err) => {
            // The old process is gone, so the row cannot go back to Running. Only this run's generation counts:
            // a newer restart of the same tab keeps its own state.
            st.0.registry.mark_exited(&tab, generation, -1);
            err.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str, value: &str) -> (String, String) {
        (name.to_string(), value.to_string())
    }

    /// Two starts of one tab at once (a Restart during a Resume's spawn, or two Restarts) would
    /// both put a process in the map, and the first would run on unseen. One claim per tab.
    #[test]
    fn one_start_per_tab_at_a_time() {
        let home = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        let first = st.claim_start("a").expect("the first start claims the tab");
        assert!(st.claim_start("a").is_none());
        assert!(st.claim_start("b").is_some());
        drop(first);
        assert!(st.claim_start("a").is_some());
    }

    #[test]
    fn colorfgbg_values() {
        assert_eq!(colorfgbg(ColorScheme::Dark), "15;0");
        assert_eq!(colorfgbg(ColorScheme::Light), "0;15");
    }

    #[test]
    fn spawn_env_puts_colorfgbg_before_profile_env() {
        // The profile sets its own COLORFGBG: it comes last, so it wins.
        let env = spawn_env(Some(ColorScheme::Dark), &[var("COLORFGBG", "7;0")]);
        let last = env.iter().rev().find(|(k, _)| k == "COLORFGBG").unwrap();
        assert_eq!(last.1, "7;0");

        let env = spawn_env(Some(ColorScheme::Dark), &[]);
        assert!(env.contains(&var("COLORFGBG", "15;0")));
    }

    #[test]
    fn spawn_env_without_a_scheme_sets_no_colorfgbg() {
        let env = spawn_env(None, &[var("A", "1")]);
        assert_eq!(env, vec![var("A", "1")]);
    }
}
