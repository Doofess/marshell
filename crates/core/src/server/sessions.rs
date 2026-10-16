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
    ColorScheme, CreateSessionRequest, CreateSessionResponse, RenameRequest, ReopenRequest, ReorderRequest,
    SessionsSnapshot, ShellFamily,
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
struct Resolved {
    profile_id: String,
    profile_name: String,
    family: ShellFamily,
    argv: Vec<String>,
    env: Vec<(String, String)>,
    cwd: Option<String>,
    /// Shell integration is on, so the output carries prompt marks.
    marks: bool,
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
            let mut argv = launch_argv(&p, agent, prompt).map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
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

/// Reserves a number, starts the process and records it. A failed spawn gives the number back.
async fn start_session(
    st: &AppState,
    run: Resolved,
    agent: Option<String>,
    name: Option<String>,
    cwd: Option<String>,
    size: (u16, u16),
    scheme: Option<ColorScheme>,
) -> Result<CreateSessionResponse, ApiError> {
    // The shell starts even when the agent is missing, so look for the agent first.
    // A WSL distro has its own PATH, which the check cannot see: those are not checked.
    if let Some(agent) = agent.as_deref()
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
    let mut env = spawn_env(scheme, &run.env);
    env.extend([
        (brand::env_var("TAB_ID"), tab_id.clone()),
        (brand::env_var("SESSION_NUMBER"), number.to_string()),
        (brand::env_var("AGENT"), agent.unwrap_or_else(|| "shell".into())),
        ("TERM".into(), "xterm-256color".into()),
        ("COLORTERM".into(), "truecolor".into()),
    ]);
    let registry = st.0.registry.clone();
    let exited_tab = tab_id.clone();
    let on_mark: Option<crate::pty::MarkCallback> = if run.marks {
        let registry = st.0.registry.clone();
        let tab = tab_id.clone();
        Some(Box::new(move |mark| registry.mark(&tab, mark)))
    } else {
        None
    };
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
        on_mark,
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
    let size = (req.cols, req.rows);
    let created = start_session(&st, run, agent, req.name, req.cwd, size, req.color_scheme).await?;
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
    // The saved session has no colour scheme, so a reopened one starts without COLORFGBG.
    let created = start_session(&st, run, saved.agent, Some(saved.name), Some(saved.cwd), (80, 24), None).await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str, value: &str) -> (String, String) {
        (name.to_string(), value.to_string())
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
