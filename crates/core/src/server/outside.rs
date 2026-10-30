//! `GET .../resume-command` and `POST .../open-outside`, for a session and for a conversation in History. The
//! command and the console are built only from what the core holds: the tab, its history row and its profile. No
//! request carries a command line, and an id reaches one only after `sidecar_allowed` (`resume_session`, `look_up`).

use super::AppState;
use super::history::{Found, gone, look_up};
use super::sessions::{
    Resume, cannot_resume, cannot_start_again, check_agent_installed, resolve, resume_session, start_folder,
};
use crate::agents::presets;
use crate::outside::{ConversationKey, console_env, copy_line, lookup_path};
use crate::profiles::detect::{Program, agent_command, resume_line, session_base_env};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{Profile, ResumeCommand, ResumeForm, SessionInfo, SessionMode, ShellFamily};
use std::path::PathBuf;

/// Why a running session can't be opened outside: two CLIs must not write one conversation.
const END_FIRST: &str = "End the session first.";
/// Why a conversation can't be resumed or opened again while the console Marshell opened for it still runs.
pub(super) const OPEN_IN_TERMINAL: &str = "It's open in a system terminal.";
const NO_AGENT: &str = "A plain terminal has no resume command.";
const CONTROL_IN_FOLDER: &str =
    "Its folder's name holds a line break or another control character, so there is no one-line command to copy.";
const CONTROL_IN_COMMAND: &str =
    "Its command holds a line break or another control character, so there is no one-line command to copy.";

/// The conversation a start of `agent` with `resume` in `folder` would write, if it is one. A plain shell has none,
/// and a start with no resume flag is a new conversation, so neither is kept. "Continue the last conversation" is
/// keyed by agent and folder, which is as close as Marshell can say which one that is.
pub(super) fn console_key(agent: Option<&str>, resume: &Resume, folder: &str) -> Option<ConversationKey> {
    let agent = agent?;
    match resume {
        Resume::Session(id) => Some(ConversationKey::by_id(agent, id)),
        Resume::Last => Some(ConversationKey::last_in(agent, folder)),
        Resume::Fresh => None,
    }
}

/// Whether a console Marshell opened still holds the conversation `resume` would start. A Resume or a Restart asks
/// this before it starts anything.
pub(super) fn in_console(st: &AppState, agent: Option<&str>, resume: &Resume, folder: &str) -> bool {
    console_key(agent, resume, folder).is_some_and(|key| st.0.consoles.is_open(&key))
}

/// What the command or the console is built from, taken from the core's own record.
struct Plan {
    agent: String,
    cwd: String,
    mode: Option<SessionMode>,
    /// None: the default profile.
    profile_id: Option<String>,
    resume: Resume,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Copy,
    Open,
}

/// The refusal for a preset's session: it needs the environment Marshell builds for it, which never leaves Marshell.
fn preset_refusal(preset: &str, purpose: Purpose) -> String {
    let name = match preset {
        presets::DEEPSEEK => "DeepSeek",
        other => other,
    };
    match purpose {
        Purpose::Copy => format!("A {name} session needs Marshell to start it, so there is no command to copy."),
        Purpose::Open => format!("A {name} session needs Marshell to start it."),
    }
}

/// The refusal for a manifest agent with no `continue` arguments, if `agent` is one.
fn no_resume(st: &AppState, agent: &str) -> Option<String> {
    let agents = st.agents();
    let m = agents.manifest(agent)?;
    m.continue_args
        .is_none()
        .then(|| format!("{} has no resume command.", m.name))
}

/// `copy_line`, refusing a folder whose name, or a command that, would make the result more than one line.
fn one_line(family: ShellFamily, cwd: &str, line: &str) -> Result<String, &'static str> {
    if cwd.chars().any(char::is_control) {
        return Err(CONTROL_IN_FOLDER);
    }
    let copied = copy_line(family, cwd, line);
    if copied.chars().any(char::is_control) {
        return Err(CONTROL_IN_COMMAND);
    }
    Ok(copied)
}

fn tab_plan(st: &AppState, info: &SessionInfo, purpose: Purpose) -> Result<Plan, String> {
    if let Some(preset) = &info.preset {
        return Err(preset_refusal(preset, purpose));
    }
    let Some(agent) = info.agent.clone() else {
        return Err(NO_AGENT.into());
    };
    if let Some(reason) = no_resume(st, &agent) {
        return Err(reason);
    }
    let resume = resume_session(st, &info.tab_id, Some(&agent)).map_err(String::from)?;
    Ok(Plan {
        agent,
        cwd: info.cwd.clone(),
        mode: info.mode,
        profile_id: Some(info.profile_id.clone()),
        resume,
    })
}

async fn history_plan(st: &AppState, agent: &str, session_id: &str, purpose: Purpose) -> Result<Plan, Box<Response>> {
    let found = look_up(st, agent, session_id, purpose == Purpose::Open)
        .await
        .map_err(|err| Box::new(err.into_response()))?;
    let meta = match found {
        Found::Usable(meta, _) => *meta,
        Found::Gone => return Err(Box::new(cannot_resume(gone(agent)))),
        Found::RunningInATab => return Err(Box::new(cannot_resume(END_FIRST))),
    };
    // The conversation's tab, if Marshell still has it: a DeepSeek conversation needs DeepSeek again.
    let preset = meta
        .preset
        .clone()
        .or_else(|| st.registry().preset_of_tab(&meta.tab_id));
    if let Some(preset) = preset {
        return Err(Box::new(cannot_resume(preset_refusal(&preset, purpose))));
    }
    if purpose == Purpose::Open
        && !meta.cwd.is_empty()
        && !tokio::fs::metadata(&meta.cwd).await.is_ok_and(|m| m.is_dir())
    {
        return Err(Box::new(cannot_resume(format!(
            "Its folder no longer exists: {}",
            meta.cwd
        ))));
    }
    Ok(Plan {
        agent: agent.to_string(),
        cwd: meta.cwd,
        mode: None,
        profile_id: None,
        // `look_up` found it only after `sidecar_allowed`, so the id is safe in a command line.
        resume: Resume::Session(meta.session_id),
    })
}

/// The profile whose shell the copied line is for: the session's, else the default.
fn copy_profile(st: &AppState, id: Option<&str>) -> Result<Profile, String> {
    let doc = st.0.profiles.lock().unwrap();
    id.and_then(|id| doc.profiles.iter().find(|p| p.id == id))
        .or_else(|| doc.profiles.iter().find(|p| p.id == doc.default_id))
        .cloned()
        .ok_or_else(|| "Its shell profile was removed.".to_string())
}

/// The program word of the copied line: the adapter's command, else the manifest's first. No lookup on PATH.
fn program_word(st: &AppState, agent: &str) -> String {
    agent_command(agent)
        .map(String::from)
        .or_else(|| st.agents().manifest(agent).and_then(|m| m.commands.first().cloned()))
        .unwrap_or_else(|| agent.to_string())
}

fn command_of(st: &AppState, plan: &Plan) -> Result<ResumeCommand, String> {
    let profile = copy_profile(st, plan.profile_id.as_deref())?;
    let word = program_word(st, &plan.agent);
    let line = resume_line(
        &profile,
        &st.agents(),
        &plan.agent,
        Program::of_command(&word),
        plan.mode,
        plan.resume.target(),
    )?;
    let command = one_line(profile.family, &plan.cwd, &line)?;
    let form = match plan.resume {
        Resume::Session(_) => ResumeForm::ById,
        Resume::Last => ResumeForm::ContinueLast,
        Resume::Fresh => ResumeForm::Fresh,
    };
    Ok(ResumeCommand { command, form })
}

fn answer(result: Result<ResumeCommand, String>) -> Response {
    match result {
        Ok(command) => Json(command).into_response(),
        Err(reason) => cannot_resume(reason),
    }
}

/// `GET /v1/sessions/{tab}/resume-command`: works on every agent session, running or not.
pub async fn tab_resume_command(State(st): State<AppState>, Path(tab): Path<String>) -> Response {
    let Some(info) = st.registry().row(&tab) else {
        return (StatusCode::NOT_FOUND, "No such session.").into_response();
    };
    answer(tab_plan(&st, &info, Purpose::Copy).and_then(|plan| command_of(&st, &plan)))
}

/// `GET /v1/history/{agent}/{session_id}/resume-command`.
pub async fn history_resume_command(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
) -> Response {
    match history_plan(&st, &agent, &session_id, Purpose::Copy).await {
        Ok(plan) => answer(command_of(&st, &plan)),
        Err(response) => *response,
    }
}

/// Runs the resume in a new console window: the profile's shell runs the agent and stays open after it. The profile's
/// own variables are all it gets (a preset never reaches here), and none of Marshell's session markers.
async fn open(st: &AppState, plan: Plan) -> Response {
    let run = match resolve(
        st,
        plan.profile_id.as_deref(),
        Some(&plan.agent),
        None,
        plan.resume.target(),
        plan.mode,
        None,
    )
    .await
    {
        Ok(run) => run,
        Err(err) => return err.into_response(),
    };
    if let Err((_, message)) = check_agent_installed(st, &run, Some(&plan.agent)) {
        return cannot_resume(message);
    }
    let folder = start_folder(&run, Some(plan.cwd.clone()));
    // Taken before anything starts and held until the console is in the map, so a second open of the conversation
    // is refused while this one is starting. No lock is held across the spawn.
    let reservation = match console_key(Some(&plan.agent), &plan.resume, &folder) {
        Some(key) => match st.0.consoles.reserve(key) {
            Some(reservation) => Some(reservation),
            None => return cannot_resume(OPEN_IN_TERMINAL),
        },
        None => None,
    };
    let cwd = PathBuf::from(&folder);
    let profile_env = run.env.clone();
    let bin = crate::paths::bin_dir(st.home());
    let read_tab_path = st.0.tab_path;
    let argv = run.argv;
    let open_console = st.console_opener();
    // The environment and the PATH are read as a session's are (the registry on Windows), off the async workers.
    let started = tokio::task::spawn_blocking(move || {
        let env = console_env(session_base_env(), &profile_env, &bin);
        let path = lookup_path(&profile_env, read_tab_path(), &bin);
        open_console(&argv, &env, path.as_deref(), &cwd)
    })
    .await;
    match started {
        Ok(Ok(probe)) => {
            if let Some(reservation) = reservation {
                reservation.finish(probe);
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(Err(err)) if err.kind() == std::io::ErrorKind::Unsupported => cannot_resume(err.to_string()),
        // The page adds "Couldn't open a system terminal: " itself.
        Ok(Err(err)) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

/// `POST /v1/sessions/{tab}/open-outside`: an Ended or Exited tab only, so two CLIs never write one conversation.
pub async fn tab_open_outside(State(st): State<AppState>, Path(tab): Path<String>) -> Response {
    let Some(info) = st.registry().row(&tab) else {
        return (StatusCode::NOT_FOUND, "No such session.").into_response();
    };
    // A preset's refusal is final, so it comes before "End the session first.".
    if let Some(preset) = &info.preset {
        return cannot_resume(preset_refusal(preset, Purpose::Open));
    }
    // Held until the console is started, so a Resume can't start the tab in between.
    let Some(_claim) = st.claim_start(&tab) else {
        return cannot_resume(END_FIRST);
    };
    let Some(info) = st.registry().row(&tab) else {
        return (StatusCode::NOT_FOUND, "No such session.").into_response();
    };
    if info.running() {
        return cannot_resume(END_FIRST);
    }
    if let Some(reason) = cannot_start_again(&st, &info) {
        return cannot_resume(reason);
    }
    match tab_plan(&st, &info, Purpose::Open) {
        Ok(plan) => open(&st, plan).await,
        Err(reason) => cannot_resume(reason),
    }
}

/// `POST /v1/history/{agent}/{session_id}/open-outside`.
pub async fn history_open_outside(
    State(st): State<AppState>,
    Path((agent, session_id)): Path<(String, String)>,
) -> Response {
    match history_plan(&st, &agent, &session_id, Purpose::Open).await {
        Ok(plan) => open(&st, plan).await,
        Err(response) => *response,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::detect::ResumeTarget;
    use marshell_protocol::api::Profile;

    fn profile(id: &str, family: ShellFamily, command: &[&str]) -> Profile {
        Profile {
            id: id.into(),
            name: id.into(),
            command: command.iter().map(|c| c.to_string()).collect(),
            family,
            cwd: None,
            env: vec![("EDITOR".into(), "vim".into())],
            shell_integration: true,
            terminal_theme: None,
            detected: true,
        }
    }

    #[tokio::test]
    async fn open_outside_argv_keeps_the_shell_open() {
        let home = tempfile::tempdir().unwrap();
        let st = AppState::new(
            "t".into(),
            1,
            Vec::new(),
            home.path().to_path_buf(),
            vec![
                profile("ps", ShellFamily::PowerShell, &["pwsh.exe"]),
                profile("cmd", ShellFamily::Cmd, &["cmd.exe"]),
                profile("sh", ShellFamily::Posix, &["bash"]),
            ],
        );
        let line = "claude --resume abc --permission-mode plan";
        let run = |id: &'static str| {
            let st = st.clone();
            async move {
                resolve(
                    &st,
                    Some(id),
                    Some("claude"),
                    None,
                    Some(ResumeTarget::Session("abc")),
                    Some(SessionMode::Plan),
                    None,
                )
                .await
                .ok()
                .unwrap()
            }
        };
        let ps = run("ps").await;
        assert_eq!(
            ps.argv,
            ["pwsh.exe", "-NoExit", "-Command", format!("& {line}").as_str()]
        );
        let cmd = run("cmd").await;
        assert_eq!(cmd.argv, ["cmd.exe", "/k", line]);
        let sh = run("sh").await;
        assert_eq!(sh.argv, ["bash", "-c", format!("{line}; exec \"$SHELL\" -l").as_str()]);
        // The profile's own variables are all it has: no preset's, and no `bin` folder in a PATH.
        assert_eq!(ps.env, [("EDITOR".to_string(), "vim".to_string())]);
    }

    #[test]
    fn open_outside_one_line_refuses_a_folder_with_a_control_character() {
        assert!(one_line(ShellFamily::Posix, "/a\nb", "claude").is_err());
        assert!(one_line(ShellFamily::PowerShell, "C:/a\tb", "claude").is_err());
        // The command itself: a manifest's `commands` or `continue` could hold a line break.
        assert_eq!(
            one_line(ShellFamily::Posix, "/a", "claude --x\ny"),
            Err(CONTROL_IN_COMMAND)
        );
        assert_eq!(one_line(ShellFamily::Posix, "", "claude\r"), Err(CONTROL_IN_COMMAND));
        assert!(one_line(ShellFamily::Cmd, "C:\\a", "claude\u{1b}[31m").is_err());
        assert_eq!(
            one_line(ShellFamily::Posix, "/a b", "claude").unwrap(),
            "cd '/a b' && claude"
        );
    }
}
