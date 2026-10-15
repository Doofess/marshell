use super::AppState;
use crate::auth;
use crate::profiles::detect::{agent_command, default_shell, launch_argv};
use crate::pty::{Session, SpawnSpec};
use crate::registry::Relaunch;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use marshell_protocol::api::{
    CreateSessionRequest, CreateSessionResponse, RenameRequest, ReopenRequest, ReorderRequest, SessionsSnapshot,
};
use marshell_protocol::brand;
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

/// What to run: the profile it came from, the argv, and the profile's env and folder.
struct Resolved {
    profile_id: String,
    profile_name: String,
    argv: Vec<String>,
    env: Vec<(String, String)>,
    cwd: Option<String>,
}

/// Finds the profile (the default when `profile_id` is None) and builds the argv for the agent.
fn resolve(
    st: &AppState,
    profile_id: Option<&str>,
    agent: Option<&str>,
    prompt: Option<&str>,
) -> Result<Resolved, ApiError> {
    // Copy the profile out so the lock is gone before any await.
    let profile = {
        let doc = st.0.profiles.lock().unwrap();
        let id = profile_id.filter(|id| !id.is_empty()).unwrap_or(&doc.default_id);
        doc.profiles.iter().find(|p| p.id == id).cloned()
    };
    match (profile, profile_id) {
        (Some(p), _) => {
            let argv = launch_argv(&p, agent, prompt).map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
            Ok(Resolved {
                profile_id: p.id,
                profile_name: p.name,
                argv,
                env: p.env,
                cwd: p.cwd,
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
        argv,
        env: Vec::new(),
        cwd: None,
    }
}

/// Reserves a number, starts the process and records it. A failed spawn gives the number back.
async fn start_session(
    st: &AppState,
    run: Resolved,
    agent: Option<String>,
    name: Option<String>,
    cwd: Option<String>,
    size: (u16, u16),
) -> Result<CreateSessionResponse, ApiError> {
    let home = dirs::home_dir().map(|p| p.to_string_lossy().into_owned());
    let cwd = cwd.filter(|c| !c.is_empty()).or(run.cwd).or(home).unwrap_or_default();
    // The base name: the requested one, else the agent, else the profile.
    let name = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            let agent_name = agent.as_deref().and_then(agent_display_name);
            agent_name.map_or(run.profile_name, String::from)
        });

    let tab_id = auth::new_id("t");
    let number = st.0.registry.reserve(
        &tab_id,
        Relaunch {
            profile_id: run.profile_id,
            agent: agent.clone(),
            cwd: cwd.clone(),
            name,
        },
    );

    // The profile's variables first, so ours win.
    let mut env = run.env;
    env.extend([
        (brand::env_var("TAB_ID"), tab_id.clone()),
        (brand::env_var("SESSION_NUMBER"), number.to_string()),
        (brand::env_var("AGENT"), agent.unwrap_or_else(|| "shell".into())),
        ("TERM".into(), "xterm-256color".into()),
        ("COLORTERM".into(), "truecolor".into()),
    ]);
    let registry = st.0.registry.clone();
    let exited_tab = tab_id.clone();
    // `spec` takes the cwd; the closure below records the same folder.
    let marshell_home = st.0.home.clone();
    let project = cwd.clone();
    let spec = SpawnSpec {
        argv: run.argv,
        cwd: (!cwd.is_empty()).then(|| PathBuf::from(cwd)),
        env,
        cols: size.0,
        rows: size.1,
        on_exit: Some(Box::new(move |code| registry.mark_exited(&exited_tab, code))),
    };
    let id = tab_id.clone();
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
        Ok(Err(e)) => {
            st.0.registry.release(&tab_id);
            // No prefix here: the UI already says "Couldn't start a shell." and appends this text.
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")));
        }
        Err(e) => {
            st.0.registry.release(&tab_id);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    };
    st.insert_session(session);
    // A close can land between reserve and insert, when the map has nothing to kill yet.
    // Checking after the insert catches it: a later close finds the session in the map.
    if !st.0.registry.is_open(&tab_id) {
        if let Some(session) = st.remove_session(&tab_id) {
            session.kill();
        }
        return Err((
            StatusCode::CONFLICT,
            "The session was closed while it was starting.".into(),
        ));
    }
    Ok(CreateSessionResponse { tab_id, number })
}

pub async fn create(
    State(st): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, ApiError> {
    if let Some(agent) = req.agent.as_deref().filter(|a| agent_command(a).is_none()) {
        return Err((StatusCode::BAD_REQUEST, format!("Unknown agent: {agent}.")));
    }
    // An explicit argv keeps the old behaviour: run it as given, as a custom profile.
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
            )?,
            req.agent,
        ),
    };
    let created = start_session(&st, run, agent, req.name, req.cwd, (req.cols, req.rows)).await?;
    Ok(Json(created))
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
    let run = resolve(&st, Some(&saved.profile_id), saved.agent.as_deref(), None)?;
    // The window resizes the terminal as soon as it attaches.
    let created = start_session(&st, run, saved.agent, Some(saved.name), Some(saved.cwd), (80, 24)).await?;
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
