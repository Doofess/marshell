use super::AppState;
use super::agents::install_kind_of;
use crate::agents::codex::supports_no_daemon;
use crate::agents::identify::{Missing, find};
use crate::agents::install::update_argv;
use crate::agents::manifest::mode_label;
use crate::agents::presets::{self, PresetLaunch};
use crate::auth;
use crate::profiles::detect::{
    PresetArg, Program, ResumeTarget, agent_command, command_on_path, default_shell, launch_argv, launch_argv_resume,
    line_argv, mangles_double_quotes, mode_args, preset_line,
};
use crate::pty::{FreezeError, Session, SpawnSpec};
use crate::registry::{Relaunch, ResumeRefusal};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{
    CliTask, CliTaskKind, ColorScheme, CreateSessionRequest, CreateSessionResponse, InputRequest, RenameRequest,
    ReopenRequest, ReorderRequest, RestartRequest, ResumeError, SessionInfo, SessionMode, SessionState,
    SessionsSnapshot, ShellFamily,
};
use marshell_protocol::brand;
use marshell_protocol::hook::PeekRows;
use marshell_protocol::install::InstallKind;
use marshell_protocol::tools::ToolLaunch;
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
/// scheme is unknown), then the profile's own, which come after so a profile can override it. `PATH` has
/// Marshell's `bin` folder in front (the profile's own `PATH` if it sets one, else the app's), so an agent in
/// the session runs `marshell` by name. That is this session's environment only; nothing is written to the
/// person's own PATH. A WSL session is a Linux that can't run it (slice 6j).
fn spawn_env(
    scheme: Option<ColorScheme>,
    profile_env: &[(String, String)],
    home: &std::path::Path,
    family: ShellFamily,
) -> Vec<(String, String)> {
    let mut env = Vec::new();
    if let Some(scheme) = scheme {
        env.push(("COLORFGBG".to_string(), colorfgbg(scheme).to_string()));
    }
    env.extend(profile_env.iter().cloned());
    if family != ShellFamily::Wsl {
        let is_path = |key: &str| {
            if cfg!(windows) {
                key.eq_ignore_ascii_case("PATH")
            } else {
                key == "PATH"
            }
        };
        let slot = env.iter().position(|(k, _)| is_path(k));
        let rest = match slot {
            Some(i) => std::ffi::OsString::from(&env[i].1),
            None => std::env::var_os("PATH").unwrap_or_default(),
        };
        // Split first: `join_paths` would take a whole PATH as one folder and quote it.
        let parts = std::iter::once(crate::paths::bin_dir(home)).chain(std::env::split_paths(&rest));
        // A folder that can't be in a PATH (it holds the separator) is left out rather than breaking the PATH.
        if let Ok(joined) = std::env::join_paths(parts) {
            let joined = joined.to_string_lossy().into_owned();
            match slot {
                Some(i) => env[i].1 = joined,
                None => env.push(("PATH".to_string(), joined)),
            }
        }
    }
    env
}

/// Refuses a preset that is unknown, or that is not on Claude Code. A DeepSeek launch changes Claude Code's
/// environment only, so it has no meaning for another agent or a plain shell.
fn check_preset(agent: Option<&str>, preset: &str) -> Result<(), ApiError> {
    match preset {
        presets::DEEPSEEK if agent == Some("claude") => Ok(()),
        presets::DEEPSEEK => Err((StatusCode::BAD_REQUEST, presets::CLAUDE_ONLY.into())),
        other => Err((StatusCode::BAD_REQUEST, format!("Unknown preset: {other}."))),
    }
}

/// The environment a preset adds, built from the key as it is now. A refusal
/// (no key, a WSL profile) is a 400 and never starts anything.
fn preset_launch(st: &AppState, preset: &str, family: ShellFamily) -> Result<PresetLaunch, ApiError> {
    match preset {
        presets::DEEPSEEK => {
            presets::deepseek((st.0.deepseek_key)(), family).map_err(|message| (StatusCode::BAD_REQUEST, message))
        }
        other => Err((StatusCode::BAD_REQUEST, format!("Unknown preset: {other}."))),
    }
}

/// What to run: the profile it came from, the argv, and the profile's env and folder.
/// No `Debug`: `env` can hold a preset's key.
pub(super) struct Resolved {
    pub(super) profile_id: String,
    pub(super) profile_name: String,
    family: ShellFamily,
    argv: Vec<String>,
    env: Vec<(String, String)>,
    /// Variables the child must not inherit, on top of the session markers: a preset's.
    env_remove: Vec<String>,
    /// Inherited variables the child must not get, decided at spawn from the environment the pty really builds.
    env_strip: Option<fn(&str) -> bool>,
    cwd: Option<String>,
    /// Shell integration is on, so the output carries prompt marks.
    marks: bool,
    /// The line a tool preset runs first: it holds what the person typed, so it goes to the spawn and nowhere else.
    initial_command: Option<String>,
}

/// Finds the profile (the default when `profile_id` is None) and builds the argv for the agent.
/// With a `resume` target the agent is told to continue that conversation, and there is no prompt.
/// The mode becomes the agent's own flags; a plain shell has none.
/// A Codex launch reads Codex's version first (off the async workers), to know whether it takes `--no-daemon`.
/// A `preset` adds its environment and takes the inherited provider variables away. It is looked at again on every
/// start (a Restart, a Resume, a Reopen), so a key that is gone is a 400 and never a plain Claude session.
pub(super) async fn resolve(
    st: &AppState,
    profile_id: Option<&str>,
    agent: Option<&str>,
    prompt: Option<&str>,
    resume: Option<ResumeTarget<'_>>,
    mode: Option<SessionMode>,
    preset: Option<&str>,
) -> Result<Resolved, ApiError> {
    if let Some(preset) = preset {
        check_preset(agent, preset)?;
    }
    let no_daemon = agent == Some("codex") && codex_takes_no_daemon(st).await;
    let agents = st.agents();
    // A manifest agent has a mode only where its manifest maps one.
    if let (Some(m), Some(mode)) = (agent.and_then(|a| agents.manifest(a)), mode)
        && mode_args(&agents, &m.id, mode).is_none()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("{} has no {} mode.", m.name, mode_label(mode)),
        ));
    }
    // Copy the profile out so the lock is gone before any await.
    let profile = {
        let doc = st.0.profiles.lock().unwrap();
        let id = profile_id.filter(|id| !id.is_empty()).unwrap_or(&doc.default_id);
        doc.profiles.iter().find(|p| p.id == id).cloned()
    };
    match (profile, profile_id) {
        (Some(p), _) => {
            // Before anything is looked up: the refusal is the same whether or not the program is installed.
            let launch = preset.map(|id| preset_launch(st, id, p.family)).transpose()?;
            let command = match agent {
                None => None,
                Some(id) => Some(program_command(st, id, p.family == ShellFamily::Wsl, &p.env).await?),
            };
            let spec = agent
                .zip(command.as_deref())
                .map(|(id, c)| (id, Program::of_command(c)));
            let argv = match resume {
                Some(target) => launch_argv_resume(&p, &agents, spec, mode, target, no_daemon),
                None => launch_argv(&p, &agents, spec, prompt, mode, no_daemon),
            };
            let mut argv = argv.map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
            let mut env = p.env.clone();
            let mut env_remove = Vec::new();
            let mut env_strip = None;
            if let Some(launch) = launch {
                // The profile's own provider variables too: an ANTHROPIC_API_KEY set there must not reach the preset's endpoint.
                env.retain(|(name, _)| !presets::strips(name));
                env.extend(launch.env);
                env_remove = launch.env_remove;
                env_strip = Some(presets::strips as fn(&str) -> bool);
            }
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
                env_remove,
                env_strip,
                cwd: p.cwd,
                marks,
                initial_command: None,
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

/// The program word an agent starts with. An adapter's is fixed. A manifest agent's is the first of its
/// commands that resolves on PATH as that agent (off the async workers: a version check may run its
/// `--version`); a WSL distro has its own PATH, so there it is the first command. An unknown id gets
/// itself, and `launch_argv` refuses it.
async fn program_command(
    st: &AppState,
    id: &str,
    wsl: bool,
    profile_env: &[(String, String)],
) -> Result<String, ApiError> {
    if let Some(command) = agent_command(id) {
        return Ok(command.to_string());
    }
    let Some(m) = st.agents().manifest(id).cloned() else {
        return Ok(id.to_string());
    };
    if wsl {
        return Ok(m.commands.first().cloned().unwrap_or_else(|| id.to_string()));
    }
    let (finder, versions) = (st.agent_finder(profile_env), st.0.agent_versions.clone());
    let name = m.name.clone();
    let found = tokio::task::spawn_blocking(move || find(&m, &|c| finder.path_of(c), &|p| versions.get(p)))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match found {
        Ok(found) => Ok(found.command),
        Err(Missing::NotOnPath | Missing::Rejected { .. } | Missing::WrongVersion { .. }) => Err((
            StatusCode::BAD_REQUEST,
            format!("{name} isn't installed, or isn't on PATH."),
        )),
    }
}

/// Whether the Codex that sessions run accepts `--no-daemon` (slice 6a decision 14). A cold version
/// cache runs `codex --version`, so this goes through `spawn_blocking`.
async fn codex_takes_no_daemon(st: &AppState) -> bool {
    let asker = st.clone();
    tokio::task::spawn_blocking(move || {
        let exe = asker.agent_exe_of("codex");
        supports_no_daemon(asker.codex_version(exe.as_deref()).as_deref())
    })
    .await
    .unwrap_or(false)
}

fn custom(argv: Vec<String>) -> Resolved {
    Resolved {
        profile_id: CUSTOM_PROFILE.into(),
        profile_name: "Shell".into(),
        family: ShellFamily::Posix,
        argv,
        env: Vec::new(),
        env_remove: Vec::new(),
        env_strip: None,
        cwd: None,
        marks: false,
        initial_command: None,
    }
}

/// The argv of a sign-in or update tab for `agent`, built from the CLI's own command. `exe` is the file as found
/// (a `.cmd` shim stays a shim); `install` is how it was installed, which picks the update command.
pub(super) fn cli_task_argv(
    agent: &str,
    kind: CliTaskKind,
    exe: &std::path::Path,
    install: &InstallKind,
) -> Result<Vec<String>, ApiError> {
    let name =
        agent_display_name(agent).ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Unknown agent: {agent}.")))?;
    match kind {
        CliTaskKind::SignIn => {
            let args: &[&str] = match agent {
                "claude" => &["auth", "login"],
                "codex" => &["login"],
                _ => return Err((StatusCode::BAD_REQUEST, format!("{name} has no sign-in command."))),
            };
            // A shim is not a program the pty can start: cmd.exe runs it.
            let shim = exe
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
            let mut argv: Vec<String> = if shim {
                ["cmd.exe", "/d", "/c"].map(String::from).to_vec()
            } else {
                Vec::new()
            };
            argv.push(exe.to_string_lossy().into_owned());
            argv.extend(args.iter().map(|a| a.to_string()));
            Ok(argv)
        }
        CliTaskKind::Update => update_argv(agent, install, exe).ok_or_else(|| {
            (
                StatusCode::CONFLICT,
                format!("Marshell can't tell how {name} was installed."),
            )
        }),
    }
}

/// The refusal for a CLI-task request that also names something else to run.
const CLI_TASK_ONLY: &str = "A sign-in or update tab takes nothing else.";

/// Starts a sign-in or update tab. The core builds the command from the CLI it finds, and the tab runs it as a custom
/// argv in the home folder. It is remembered for no folder, and a restart or a reopen starts the default shell, so an
/// update never runs again by itself. A sign-in tab is not recorded: what is typed and printed there is a credential's.
async fn start_cli_task(
    st: &AppState,
    req: CreateSessionRequest,
    task: CliTask,
) -> Result<CreateSessionResponse, ApiError> {
    if req.shell.is_some()
        || req.agent.is_some()
        || req.prompt.is_some()
        || req.mode.is_some()
        || req.preset.is_some()
        || req.tool.is_some()
    {
        return Err((StatusCode::BAD_REQUEST, CLI_TASK_ONLY.into()));
    }
    let Some(name) = agent_display_name(&task.agent) else {
        return Err((StatusCode::BAD_REQUEST, format!("Unknown agent: {}.", task.agent)));
    };
    // The same file and install type the Accounts page showed. Looking for it may touch the disk.
    let looked = {
        let (st, agent) = (st.clone(), task.agent.clone());
        tokio::task::spawn_blocking(move || {
            st.agent_exe_of(&agent).map(|exe| {
                let install = install_kind_of(&agent, &exe);
                (exe, install)
            })
        })
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };
    let Some((exe, install)) = looked else {
        return Err((StatusCode::CONFLICT, format!("{name} isn't installed.")));
    };
    let argv = cli_task_argv(&task.agent, task.kind, &exe, &install)?;
    let title = match task.kind {
        CliTaskKind::SignIn => format!("Sign in to {name}"),
        CliTaskKind::Update => format!("Update {name}"),
    };
    start_session(
        st,
        custom(argv),
        None,
        Some(title),
        None,
        (req.cols, req.rows),
        req.color_scheme,
        None,
        None,
        task.kind != CliTaskKind::SignIn,
    )
    .await
}

const TOOL_NO_AGENT: &str = "A tool preset starts without an agent.";

/// Turns a launch preset and the values for its fields into what to run: the default (or named) profile's shell, with
/// the preset's command line typed as its first command. Returns the run and the session's name, the preset's own.
/// The program comes from the preset only. Each field value becomes one whole argument of the program, quoted for the
/// shell family or refused where the shell can't quote it. The values are never logged and live only in the returned
/// `Resolved`, which goes to the spawn.
pub(super) async fn resolve_tool(
    st: &AppState,
    profile_id: Option<&str>,
    tool: &ToolLaunch,
) -> Result<(Resolved, String), ApiError> {
    let unknown = || {
        (
            StatusCode::BAD_REQUEST,
            format!("Unknown tool preset: {}/{}.", tool.tool, tool.preset),
        )
    };
    let tools = st.tools();
    let preset = tools
        .profile(&tool.tool)
        .and_then(|p| p.def.presets.iter().find(|x| x.id == tool.preset))
        .cloned()
        .ok_or_else(unknown)?;
    // Every field is asked for, used or not. A line break would end the typed command, and a NUL has no place in one.
    let mut values: Vec<(&str, &str)> = Vec::new();
    for field in &preset.fields {
        let given = tool.fields.get(&field.id).map_or("", String::as_str);
        let value = given.trim();
        if value.is_empty() || given.contains(['\n', '\r', '\0']) {
            return Err((StatusCode::BAD_REQUEST, format!("Fill in {}.", field.label)));
        }
        values.push((field.id.as_str(), value));
    }
    // A whole `{field-id}` argument takes the field's value; anything else is the preset's own text.
    let args: Vec<PresetArg> = preset
        .args
        .iter()
        .map(|arg| {
            let id = arg.strip_prefix('{').and_then(|a| a.strip_suffix('}'));
            match id.and_then(|id| values.iter().find(|(f, _)| *f == id)) {
                Some((_, value)) => PresetArg::Field((*value).to_string()),
                None => PresetArg::Literal(arg.clone()),
            }
        })
        .collect();
    // Copy the profile out so the lock is gone before any await.
    let profile = {
        let doc = st.0.profiles.lock().unwrap();
        let id = profile_id.filter(|id| !id.is_empty()).unwrap_or(&doc.default_id);
        doc.profiles.iter().find(|p| p.id == id).cloned()
    };
    let Some(p) = profile else {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Unknown profile: {}.", profile_id.unwrap_or("(default)")),
        ));
    };
    if p.family == ShellFamily::PowerShell
        && mangles_double_quotes(&p)
        && args.iter().any(|a| matches!(a, PresetArg::Field(v) if v.contains('"')))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Windows PowerShell 5 can't pass double quotes to the tool. Use PowerShell 7 or remove the quotes.".into(),
        ));
    }
    let line = preset_line(p.family, &preset.program, &args).map_err(|msg| (StatusCode::BAD_REQUEST, msg))?;
    // A WSL distro has its own PATH: those programs are not checked.
    if p.family != ShellFamily::Wsl {
        let program = preset.program.clone();
        let found = tokio::task::spawn_blocking(move || command_on_path(&program))
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if !found {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("{} isn't installed, or isn't on PATH.", preset.program),
            ));
        }
    }
    let mut env = p.env.clone();
    let (argv, marks) = match &st.0.shell_dir {
        Some(dir) => {
            let (applied, marks) = crate::shell::integrate_with_command(&p, dir, &|k| std::env::var(k).ok(), &line);
            // After the profile's env, so ours win.
            env.extend(applied.env);
            (applied.argv, marks)
        }
        None => (line_argv(&p, line.clone()), false),
    };
    let run = Resolved {
        profile_id: p.id,
        profile_name: p.name,
        family: p.family,
        argv,
        env,
        env_remove: Vec::new(),
        env_strip: None,
        cwd: p.cwd,
        marks,
        // Watched from the first byte: no OSC 633;E arrives for a command nobody typed.
        initial_command: Some(line),
    };
    Ok((run, preset.name))
}

/// Refuses an agent whose program is not on PATH. The shell would start anyway and say so
/// itself, so look for the agent first. A WSL distro has its own PATH: those are not checked.
fn check_agent_installed(st: &AppState, run: &Resolved, agent: Option<&str>) -> Result<(), ApiError> {
    if let Some(agent) = agent
        && run.family != ShellFamily::Wsl
        && let Some(command) = agent_command(agent)
        && !st.agent_finder(&run.env).on_path(command)
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
pub(super) async fn start_session(
    st: &AppState,
    run: Resolved,
    agent: Option<String>,
    name: Option<String>,
    cwd: Option<String>,
    size: (u16, u16),
    scheme: Option<ColorScheme>,
    mode: Option<SessionMode>,
    preset: Option<String>,
    record: bool,
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
            preset,
        },
    );
    if let Err(err) = launch(st, &tab_id, number, generation, run, agent, cwd, size, scheme, record).await {
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
    record: bool,
) -> Result<(), ApiError> {
    spawn_into_map(st, tab_id, number, generation, run, agent, cwd, size, scheme, record).await?;
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
/// `record` is false for a tab whose output must not be kept (a sign-in tab); every other start passes true.
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
    record: bool,
) -> Result<(), ApiError> {
    let agent = agent.unwrap_or_else(|| "shell".into());
    let program = run
        .argv
        .first()
        .and_then(|p| std::path::Path::new(p).file_name())
        .map(|p| p.to_string_lossy().into_owned());
    // The profile's variables first, so ours win.
    let mut env = spawn_env(scheme, &run.env, &st.0.home, run.family);
    env.extend([
        (brand::env_var("TAB_ID"), tab_id.to_string()),
        (brand::env_var("SESSION_NUMBER"), number.to_string()),
        (brand::env_var("AGENT"), agent.clone()),
        ("TERM".into(), "xterm-256color".into()),
        ("COLORTERM".into(), "truecolor".into()),
    ]);
    let registry = st.0.registry.clone();
    let exited_tab = tab_id.to_string();
    // Every session passes on terminal notifications and bells; only one with shell integration has prompt marks.
    let on_mark: Option<crate::pty::MarkCallback> = {
        let registry = st.0.registry.clone();
        let wake = st.0.foreground_wake.clone();
        // Not the `AppState`: it owns this session, so holding it here would be a reference cycle.
        let view = st.view_reader();
        let tab = tab_id.to_string();
        let marks = run.marks;
        Some(Box::new(move |mark| {
            if !marks && !matches!(mark, crate::pty::Mark::Notify(_) | crate::pty::Mark::Bell) {
                return;
            }
            let started = mark == crate::pty::Mark::Executed;
            // Read before the registry lock is taken.
            registry.mark(&tab, mark, &view());
            // A command just started: look at what it runs now, not up to a second later. After the
            // registry knows the command runs, which is what lets a program be shown for the row.
            if started {
                wake.notify_one();
            }
        }))
    };
    // What a profiled tool's output shows goes to the registry, the way marks do. Every session has a matcher; it reads
    // only while the command in front of the shell is a profiled tool.
    let on_tool: crate::pty::ToolCallback = {
        let registry = st.0.registry.clone();
        let tab = tab_id.to_string();
        Box::new(move |event| registry.tool_event(&tab, event))
    };
    // The tools are read at each command, so an edited `tools.json` applies to the next one.
    let classify: crate::pty::Classifier = {
        let tools_now = st.tools_reader();
        std::sync::Arc::new(move |line| crate::tools::watch_for(&tools_now(), line))
    };
    // `spec` takes the cwd; the closure below records the same folder.
    let marshell_home = st.0.home.clone();
    let project = cwd.clone();
    let unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let recording = crate::paths::recordings_dir(&st.0.home).join(format!("{tab_id}-{unix_ms}.cast"));
    let shell = run.argv.first().cloned();
    // The exit callback runs on the PTY wait thread; preserve the caller's subscriber there.
    let exit_dispatch = tracing::dispatcher::get_default(Clone::clone);
    let spec = SpawnSpec {
        argv: run.argv,
        cwd: (!cwd.is_empty()).then(|| PathBuf::from(cwd)),
        env,
        // Every session, shell or agent: the markers would reach a shell's children too. The profile's
        // env still wins, since it is applied after. A preset's list follows the markers.
        env_remove: crate::agents::INHERITED_MARKERS
            .iter()
            .map(|k| k.to_string())
            .chain(run.env_remove)
            .collect(),
        env_strip: run.env_strip,
        cols: size.0,
        rows: size.1,
        on_exit: Some(Box::new(move |code| {
            tracing::dispatcher::with_default(&exit_dispatch, || {
                // The PTY uses -1 when waiting for the child failed, so its exit code is unknown.
                tracing::info!(tab_id = %exited_tab, exit_code = (code != -1).then_some(code), "session ended");
            });
            registry.mark_exited(&exited_tab, generation, code)
        })),
        on_mark,
        on_tool: Some(on_tool),
        record: record.then_some((recording, shell)),
        classify: Some(classify),
        initial_command: run.initial_command,
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
    tracing::info!(tab_id, %agent, program = program.as_deref(), "session started");
    st.insert_session(session);
    Ok(())
}

pub async fn create(
    State(st): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, ApiError> {
    if let Some(task) = req.cli_task.clone() {
        return Ok(Json(start_cli_task(&st, req, task).await?));
    }
    // A tool preset runs in a profile's shell: it takes no agent, prompt or mode, and no argv of its own. Nothing of it
    // is remembered for the folder, and the row's relaunch is the plain shell (a restart never runs the tool again).
    if let Some(tool) = req.tool.as_ref() {
        if req.agent.is_some() || req.prompt.is_some() || req.mode.is_some() || req.preset.is_some() {
            return Err((StatusCode::BAD_REQUEST, TOOL_NO_AGENT.into()));
        }
        if req.shell.is_some() {
            return Err((
                StatusCode::BAD_REQUEST,
                "A tool preset starts in a profile's shell.".into(),
            ));
        }
        let (run, name) = resolve_tool(&st, req.profile_id.as_deref(), tool).await?;
        let created = start_session(
            &st,
            run,
            None,
            Some(name),
            req.cwd,
            (req.cols, req.rows),
            req.color_scheme,
            None,
            None,
            true,
        )
        .await?;
        return Ok(Json(created));
    }
    if let Some(agent) = req.agent.as_deref().filter(|a| !st.agents().is_known(a)) {
        return Err((StatusCode::BAD_REQUEST, format!("Unknown agent: {agent}.")));
    }
    // An explicit argv keeps the old behaviour: run it as given, as a custom profile.
    // A plain shell has no mode.
    let mode = req.mode.filter(|_| req.agent.is_some() && req.shell.is_none());
    // An explicit argv runs as given, so a preset has nothing to change there.
    if let Some(preset) = req.preset.as_deref() {
        check_preset(req.agent.as_deref().filter(|_| req.shell.is_none()), preset)?;
    }
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
                None,
                mode,
                req.preset.as_deref(),
            )
            .await?,
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
    let created = start_session(
        &st,
        run,
        agent,
        req.name,
        req.cwd,
        size,
        req.color_scheme,
        mode,
        req.preset.clone(),
        true,
    )
    .await?;
    if let Some((folder, profile_id, agent)) = choice {
        remember_choice(&st, folder, profile_id, agent, mode, req.preset).await;
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
    preset: Option<String>,
) {
    let state = st.clone();
    let saved = tokio::task::spawn_blocking(move || {
        let mut current = state.0.layout.lock().unwrap_or_else(|e| e.into_inner());
        let mut next = current.clone();
        crate::layout::remember(&mut next, &folder, profile_id, agent, mode, preset);
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

#[derive(Deserialize)]
pub struct PeekQuery {
    #[serde(default = "default_peek_lines")]
    lines: usize,
}

fn default_peek_lines() -> usize {
    30
}

/// Returns the session's last rendered rows, after releasing its output lock.
pub async fn peek(
    State(st): State<AppState>,
    Path(tab): Path<String>,
    Query(query): Query<PeekQuery>,
) -> Result<Json<PeekRows>, ApiError> {
    let session = st
        .session(&tab)
        .ok_or((StatusCode::NOT_FOUND, "No such session.".into()))?;
    let rows = session.peek(query.lines);
    Ok(Json(PeekRows { rows }))
}

pub async fn rename(
    State(st): State<AppState>,
    Path(tab): Path<String>,
    Json(req): Json<RenameRequest>,
) -> Result<StatusCode, ApiError> {
    let name = crate::history::marks::normalize_name(&req.name)
        .ok_or((StatusCode::BAD_REQUEST, "The name is empty.".into()))?;
    tokio::task::spawn_blocking(move || {
        if !st
            .history()
            .rename_tab(&tab, &name)
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}")))?
        {
            return Err((StatusCode::NOT_FOUND, "No such session.".into()));
        }
        Ok(StatusCode::NO_CONTENT)
    })
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
}

#[derive(Deserialize)]
pub struct MutedRequest {
    muted: bool,
}

pub async fn muted(
    State(st): State<AppState>,
    Path(tab): Path<String>,
    Json(req): Json<MutedRequest>,
) -> Result<StatusCode, ApiError> {
    if st.0.registry.set_muted(&tab, req.muted) {
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
    st.registry().note_input(&tab);
    session.write_input(req.text.into_bytes());
    StatusCode::NO_CONTENT.into_response()
}

/// Where the session's asciicast recording is: the plain file, or the compressed one the storage thread leaves
/// after the program has exited. 404 for an unknown tab, and for one that has no recording.
pub async fn recording(State(st): State<AppState>, Path(tab): Path<String>) -> Response {
    match st.session(&tab).and_then(|s| s.recording_path()) {
        Some(path) => {
            let on_disk = path
                .parent()
                .zip(path.file_name().and_then(|name| name.to_str()))
                .and_then(|(dir, name)| crate::history::cast::resolve(dir, name))
                .unwrap_or(path);
            Json(serde_json::json!({ "path": on_disk })).into_response()
        }
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
        None,
        saved.mode,
        saved.preset.as_deref(),
    )
    .await?;
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
        saved.preset,
        true,
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
    pub(super) cols: u16,
    pub(super) rows: u16,
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

/// The refusal when Marshell holds a copy of the tab's Claude conversation but Claude's transcript file is gone.
pub(super) const NO_CONVERSATION: &str = "Claude Code no longer has this conversation, so it can't be resumed.";

/// What a Resume or Restart starts.
#[derive(Debug, PartialEq, Eq)]
enum Resume {
    /// One Claude, Codex or Antigravity conversation, by its session id.
    Session(String),
    /// The CLI's own continue flag: nothing is known about the tab, or the agent is none of those three.
    Last,
    /// A session with no transcript (Claude's never written, Codex's rollout missing): start the CLI with no
    /// resume flag. (Not its continue flag, which could pick up another tab's conversation in the same folder.)
    Fresh,
}

impl Resume {
    /// The target `resolve` is given; None starts the agent plainly.
    fn target(&self) -> Option<ResumeTarget<'_>> {
        match self {
            Resume::Session(id) => Some(ResumeTarget::Session(id)),
            Resume::Last => Some(ResumeTarget::Last),
            Resume::Fresh => None,
        }
    }
}

/// Which conversation a Resume or Restart of `tab` should pick up, from the sessions index: the tab's latest
/// conversation of its own agent. It never falls back to another conversation: `Err(message)` means Marshell
/// had this one and Claude has lost it.
/// - No row of the tab's agent, a row that fails the id and path check, or an agent other than Claude, Codex
///   and Antigravity: `Resume::Last`.
/// - The transcript (Codex's rollout, agy's `transcript_full.jsonl`) is a file: `Resume::Session(id)`.
/// - Claude's transcript is missing and Marshell holds an archive of it: `Err`, the conversation is lost.
/// - Claude's transcript is missing and there is no archive: the session never wrote anything (Claude may create
///   the file only at the first message, after the SessionStart that indexed it), so `Resume::Fresh`.
/// - Codex's rollout is missing: `Resume::Fresh`. Marshell keeps no copy of a rollout, so a deleted one can't be
///   told from one Codex never wrote (slice 6a2 decision 4). So is agy's transcript: the file exists from the
///   first prompt, so a missing one starts fresh.
fn resume_session(st: &AppState, tab: &str, agent: Option<&str>) -> Result<Resume, &'static str> {
    let Some(agent @ ("claude" | "codex" | "antigravity")) = agent else {
        return Ok(Resume::Last);
    };
    let Some(meta) = st.history().latest_for_tab_of(tab, agent) else {
        return Ok(Resume::Last);
    };
    // The row lives in the Marshell home and is checked again before its id reaches a command line or its
    // path a filesystem call. A row that fails counts as unknown.
    if !crate::history::sidecar_allowed(agent, &meta.transcript_path, &meta.session_id) {
        return Ok(Resume::Last);
    }
    if std::path::Path::new(&meta.transcript_path).is_file() {
        return Ok(Resume::Session(meta.session_id));
    }
    // Marshell keeps no copy of an agy transcript either; agy writes the file from the first prompt.
    if agent != "claude" {
        return Ok(Resume::Fresh);
    }
    let (archive, _) = crate::history::archive::archive_paths(st.home(), "claude", &meta.session_id);
    if archive.is_file() {
        Err(NO_CONVERSATION)
    } else {
        Ok(Resume::Fresh)
    }
}

/// Starts an Ended or Exited tab again in the same row. An agent gets the CLI's own continue flag, a Claude or
/// Codex tab whose session is known gets it by id (`--resume <id>`, `resume <id>`), and one whose session has
/// no transcript starts fresh (see `resume_session`).
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
    let session = match resume_session(&st, &tab, info.agent.as_deref()) {
        Ok(session) => session,
        Err(message) => return cannot_resume(message),
    };
    let run = match resolve(
        &st,
        Some(&info.profile_id),
        info.agent.as_deref(),
        None,
        session.target(),
        info.mode,
        info.preset.as_deref(),
    )
    .await
    {
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
        true,
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

/// Starts an agent session again in the same row in another mode, continuing its conversation as Resume does
/// (`--resume <id>` for a known Claude session, a plain start when it never wrote a transcript, otherwise the
/// CLI's own continue flag). The old process, if any, is killed. Every refusal comes before
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
    let session = match resume_session(&st, &tab, info.agent.as_deref()) {
        Ok(session) => session,
        Err(message) => return cannot_resume(message),
    };
    let run = match resolve(
        &st,
        Some(&info.profile_id),
        info.agent.as_deref(),
        None,
        session.target(),
        Some(req.mode),
        info.preset.as_deref(),
    )
    .await
    {
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
        true,
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

/// `POST /v1/sessions/{tab}/focus`: asks the shell to bring its window forward on this session's tab. An ended
/// session that is still open can be focused too.
pub async fn focus(State(st): State<AppState>, Path(tab): Path<String>) -> Response {
    if st.registry().row(&tab).is_none() {
        return (StatusCode::NOT_FOUND, "No such session.").into_response();
    }
    let Some(sink) = st.focus_sink() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Marshell's window isn't there to focus.",
        )
            .into_response();
    };
    sink(tab);
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_query_defaults_to_thirty_lines() {
        let uri = "/v1/sessions/tab/peek".parse().unwrap();
        let query = axum::extract::Query::<PeekQuery>::try_from_uri(&uri).unwrap();
        assert_eq!(query.lines, 30);
        let uri = "/v1/sessions/tab/peek?lines=7".parse().unwrap();
        let query = axum::extract::Query::<PeekQuery>::try_from_uri(&uri).unwrap();
        assert_eq!(query.lines, 7);
    }

    fn resolved_codex_line(version: Option<&str>, resume: Option<ResumeTarget<'_>>) -> String {
        let pwsh = marshell_protocol::api::Profile {
            id: "pwsh".into(),
            name: "pwsh".into(),
            command: vec!["pwsh.exe".into()],
            family: ShellFamily::PowerShell,
            cwd: None,
            env: Vec::new(),
            shell_integration: false,
            terminal_theme: None,
            detected: true,
        };
        let home = tempfile::tempdir().unwrap().keep();
        let st = AppState::new("t".into(), 1, Vec::new(), home, vec![pwsh])
            .with_codex_version(version.map(str::to_string))
            .with_agent_path(|_| None);
        let run = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(resolve(&st, Some("pwsh"), Some("codex"), None, resume, None, None))
            .unwrap();
        run.argv.last().unwrap().clone()
    }

    #[test]
    fn codex_gets_no_daemon_only_from_the_version_that_takes_it() {
        assert_eq!(resolved_codex_line(Some("0.163.0"), None), "& codex --no-daemon");
        assert_eq!(resolved_codex_line(Some("0.162.9"), None), "& codex");
        assert_eq!(resolved_codex_line(None, None), "& codex");
        assert_eq!(
            resolved_codex_line(Some("0.170.2"), Some(ResumeTarget::Last)),
            "& codex resume --last --no-daemon"
        );
        assert_eq!(
            resolved_codex_line(Some("garbage"), Some(ResumeTarget::Last)),
            "& codex resume --last"
        );
    }

    fn argv_of(agent: &str, kind: CliTaskKind, exe: &str, install: &InstallKind) -> Result<Vec<String>, ApiError> {
        cli_task_argv(agent, kind, std::path::Path::new(exe), install)
    }

    fn words(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_sign_in_tab_runs_the_cli_own_command() {
        let unknown = InstallKind::Unknown;
        assert_eq!(
            argv_of("claude", CliTaskKind::SignIn, r"C:\x\claude.exe", &unknown).unwrap(),
            words(&[r"C:\x\claude.exe", "auth", "login"])
        );
        assert_eq!(
            argv_of("codex", CliTaskKind::SignIn, r"C:\x\codex.exe", &unknown).unwrap(),
            words(&[r"C:\x\codex.exe", "login"])
        );
    }

    #[test]
    fn a_cmd_shim_runs_through_cmd_exe() {
        let unknown = InstallKind::Unknown;
        for exe in [r"C:\x\npm\claude.cmd", r"C:\x\npm\CLAUDE.BAT"] {
            assert_eq!(
                argv_of("claude", CliTaskKind::SignIn, exe, &unknown).unwrap(),
                words(&["cmd.exe", "/d", "/c", exe, "auth", "login"])
            );
        }
        // A space in the path stays in one word; the pty quotes it.
        let spaced = r"C:\Users\Home Office\AppData\Roaming\npm\codex.cmd";
        assert_eq!(
            argv_of("codex", CliTaskKind::SignIn, spaced, &unknown).unwrap(),
            words(&["cmd.exe", "/d", "/c", spaced, "login"])
        );
    }

    /// The real thing: a `.cmd` in a folder with a space, as `C:\Users\Home Office\AppData\Roaming\npm\claude.cmd` is,
    /// started through the pty a session uses.
    #[cfg(windows)]
    #[test]
    fn a_cmd_shim_in_a_folder_with_a_space_runs_through_the_pty() {
        use crate::pty::Pull;
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Has Space");
        std::fs::create_dir_all(&folder).unwrap();
        let shim = folder.join("fake-signin.cmd");
        std::fs::write(&shim, "@echo signin-ok %*\r\n").unwrap();
        let argv = cli_task_argv("claude", CliTaskKind::SignIn, &shim, &InstallKind::Unknown).unwrap();
        let spec = SpawnSpec {
            argv,
            cwd: None,
            env: vec![],
            env_remove: vec![],
            env_strip: None,
            cols: 120,
            rows: 24,
            on_exit: None,
            on_mark: None,
            on_tool: None,
            record: None,
            classify: None,
            initial_command: None,
        };
        let session = Session::spawn("t-cmd-shim".into(), spec).unwrap();
        let (generation, _) = session.attach();
        session.resume(0);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let (mut cursor, mut all, mut exit) = (0u64, Vec::new(), None);
        while std::time::Instant::now() < deadline && exit.is_none() {
            match session.pull(cursor, 64 * 1024) {
                Pull::Data { seq, bytes } => {
                    cursor = seq + bytes.len() as u64;
                    all.extend(bytes);
                    session.ack(cursor);
                }
                Pull::Reset { seq, screen } => {
                    cursor = seq;
                    all.extend(screen);
                    session.ack(cursor);
                }
                Pull::Exit(code) => exit = Some(code),
                Pull::Idle => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        session.detach(generation);
        let text = String::from_utf8_lossy(&all).into_owned();
        assert!(text.contains("signin-ok auth login"), "exit {exit:?}, output {text:?}");
        assert_eq!(exit, Some(0), "output {text:?}");
    }

    #[test]
    fn antigravity_has_no_sign_in_command() {
        assert_eq!(
            argv_of(
                "antigravity",
                CliTaskKind::SignIn,
                r"C:\x\agy.exe",
                &InstallKind::Native
            )
            .unwrap_err(),
            (
                StatusCode::BAD_REQUEST,
                "Antigravity has no sign-in command.".to_string()
            )
        );
    }

    #[test]
    fn an_update_tab_runs_the_update_command_or_is_refused() {
        assert_eq!(
            argv_of("claude", CliTaskKind::Update, r"C:\x\claude.exe", &InstallKind::Native).unwrap(),
            words(&[r"C:\x\claude.exe", "update"])
        );
        assert_eq!(
            argv_of("codex", CliTaskKind::Update, r"C:\x\codex.exe", &InstallKind::Unknown).unwrap_err(),
            (
                StatusCode::CONFLICT,
                "Marshell can't tell how Codex was installed.".to_string()
            )
        );
    }

    const TEST_KEY: &str = "sk-marshell-test-0000";

    fn preset_state(family: ShellFamily, key: fn() -> Option<String>) -> AppState {
        let profile = marshell_protocol::api::Profile {
            id: "p".into(),
            name: "p".into(),
            command: vec!["pwsh.exe".into()],
            family,
            cwd: None,
            // A provider variable set on the profile itself must not reach DeepSeek either.
            env: vec![var("ANTHROPIC_API_KEY", "profile-secret"), var("KEEP", "1")],
            shell_integration: false,
            terminal_theme: None,
            detected: true,
        };
        let home = tempfile::tempdir().unwrap().keep();
        AppState::new("t".into(), 1, Vec::new(), home, vec![profile])
            .with_agent_path(|_| None)
            .with_deepseek_key(key)
    }

    fn resolve_preset(st: &AppState, agent: Option<&str>, preset: Option<&str>) -> Result<Resolved, ApiError> {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(resolve(st, Some("p"), agent, None, None, None, preset))
    }

    #[test]
    fn a_deepseek_launch_sets_the_env_and_keeps_the_key_out_of_the_command_line() {
        let st = preset_state(ShellFamily::PowerShell, || Some(TEST_KEY.into()));
        let run = resolve_preset(&st, Some("claude"), Some("deepseek")).unwrap();
        let get = |name: &str| run.env.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
        assert_eq!(get("ANTHROPIC_AUTH_TOKEN"), Some(TEST_KEY));
        assert_eq!(get("ANTHROPIC_BASE_URL"), Some("https://api.deepseek.com/anthropic"));
        assert_eq!(get("KEEP"), Some("1"));
        // The profile's own ANTHROPIC_API_KEY is gone, and what the child inherits is stripped at spawn.
        assert_eq!(get("ANTHROPIC_API_KEY"), None);
        assert!(run.env_remove.contains(&"CLAUDE_CODE_USE_BEDROCK".to_string()));
        assert!(!run.env_remove.contains(&"ANTHROPIC_BASE_URL".to_string()));
        let strip = run.env_strip.expect("a preset launch strips at spawn");
        assert!(strip("ANTHROPIC_API_KEY") && strip("anthropic_custom_headers"));
        assert!(!strip("PATH"));
        // The key is in the child's env and nowhere else the spawn takes.
        assert!(run.argv.iter().all(|word| !word.contains(TEST_KEY)), "{:?}", run.argv);
        let spawn = spawn_env(Some(ColorScheme::Dark), &run.env, std::path::Path::new("h"), run.family);
        assert_eq!(spawn.iter().filter(|(_, v)| v.contains(TEST_KEY)).count(), 1);
        assert_eq!(run.profile_id, "p");

        // Without a preset nothing of this is added.
        let plain = resolve_preset(&st, Some("claude"), None).unwrap();
        assert!(plain.env_remove.is_empty());
        assert!(plain.env_strip.is_none());
        assert!(plain.env.iter().any(|(k, _)| k == "ANTHROPIC_API_KEY"));
        assert!(plain.env.iter().all(|(k, _)| k != "ANTHROPIC_AUTH_TOKEN"));
    }

    #[test]
    fn a_preset_is_refused_with_its_reason() {
        let st = preset_state(ShellFamily::PowerShell, || Some(TEST_KEY.into()));
        let refused = |r: Result<Resolved, ApiError>| r.err().unwrap();
        assert_eq!(
            refused(resolve_preset(&st, Some("codex"), Some("deepseek"))),
            (
                StatusCode::BAD_REQUEST,
                "DeepSeek runs on Claude Code only.".to_string()
            )
        );
        assert_eq!(
            refused(resolve_preset(&st, None, Some("deepseek"))).1,
            "DeepSeek runs on Claude Code only."
        );
        assert_eq!(
            refused(resolve_preset(&st, Some("claude"), Some("x"))),
            (StatusCode::BAD_REQUEST, "Unknown preset: x.".to_string())
        );
        let keyless = preset_state(ShellFamily::PowerShell, || None);
        assert_eq!(
            refused(resolve_preset(&keyless, Some("claude"), Some("deepseek"))),
            (
                StatusCode::BAD_REQUEST,
                "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek.".to_string()
            )
        );
        let wsl = preset_state(ShellFamily::Wsl, || Some(TEST_KEY.into()));
        let (status, message) = refused(resolve_preset(&wsl, Some("claude"), Some("deepseek")));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(message, "DeepSeek starts from a Windows shell profile, not WSL.");
    }

    /// A state whose only profile runs `command` in `family`, with an own tool `mine`. Its presets run `cargo` (always on
    /// PATH under `cargo test`), `no-such-program-marshell-test` (never), and `cargo` with no field.
    fn tool_state(family: ShellFamily, command: &[&str]) -> AppState {
        let profile = marshell_protocol::api::Profile {
            id: "p".into(),
            name: "p".into(),
            command: command.iter().map(|s| s.to_string()).collect(),
            family,
            cwd: None,
            env: Vec::new(),
            shell_integration: true,
            terminal_theme: None,
            detected: true,
        };
        let home = tempfile::tempdir().unwrap().keep();
        std::fs::write(
            crate::paths::tools_file(&home),
            serde_json::json!({"version": 1, "tools": [{
            "id": "mine", "name": "Mine", "commands": ["mine-marshell-test"], "presets": [
                {"id": "build", "name": "Build it", "program": "cargo",
                 "args": ["run", "{target}"], "fields": [{"id": "target", "label": "Target"}]},
                {"id": "gone", "name": "Not installed", "program": "no-such-program-marshell-test"},
                {"id": "plain", "name": "Plain", "program": "cargo", "args": ["--version"]}
            ]}]})
            .to_string(),
        )
        .unwrap();
        AppState::new("t".into(), 1, Vec::new(), home, vec![profile])
    }

    fn launch_of(preset: &str, fields: &[(&str, &str)]) -> ToolLaunch {
        ToolLaunch {
            tool: "mine".into(),
            preset: preset.into(),
            fields: fields.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        }
    }

    fn resolve_launch(st: &AppState, launch: &ToolLaunch) -> Result<(Resolved, String), ApiError> {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(resolve_tool(st, Some("p"), launch))
    }

    #[test]
    fn a_preset_runs_inside_the_integrated_powershell() {
        let st = tool_state(ShellFamily::PowerShell, &["pwsh.exe", "-NoLogo"]);
        let (run, name) = resolve_launch(&st, &launch_of("plain", &[])).unwrap();
        assert_eq!(name, "Plain");
        assert_eq!(run.argv[..4], ["pwsh.exe", "-NoLogo", "-NoExit", "-Command"]);
        let last = run.argv.last().unwrap();
        assert!(last.starts_with(". ([scriptblock]"), "{last}");
        assert!(last.ends_with("; & cargo --version"), "{last}");
        assert_eq!(run.initial_command.as_deref(), Some("& cargo --version"));
        assert!(run.marks);
        assert!(run.env_remove.is_empty() && run.env_strip.is_none());
    }

    #[test]
    fn cmd_runs_a_preset_after_k_with_its_prompt_env() {
        let st = tool_state(ShellFamily::Cmd, &["cmd.exe"]);
        let (run, _) = resolve_launch(&st, &launch_of("build", &[("target", "web:3000")])).unwrap();
        assert_eq!(run.argv, ["cmd.exe", "/k", "cargo run web:3000"]);
        assert!(run.env.iter().any(|(k, v)| k == "PROMPT" && v.contains("]133;")));
        assert_eq!(run.initial_command.as_deref(), Some("cargo run web:3000"));
        // cmd can't quote, so a value that needs it is refused and nothing starts.
        let (status, message) = resolve_launch(&st, &launch_of("build", &[("target", "my dir")]))
            .err()
            .unwrap();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            message,
            "Command Prompt can't pass an argument that needs quoting. Use PowerShell."
        );
    }

    #[test]
    fn a_field_value_with_a_space_is_quoted_per_shell() {
        let fields = [("target", "my dir")];
        let st = tool_state(ShellFamily::PowerShell, &["pwsh.exe"]);
        let (run, _) = resolve_launch(&st, &launch_of("build", &fields)).unwrap();
        assert!(run.argv.last().unwrap().ends_with("; & cargo run 'my dir'"));
        let st = tool_state(ShellFamily::Posix, &["fish", "-l"]);
        let (run, _) = resolve_launch(&st, &launch_of("build", &fields)).unwrap();
        // No integration for fish here, so it runs the way an agent does.
        assert_eq!(run.argv, ["fish", "-l", "-c", "cargo run 'my dir'; exec \"$SHELL\" -l"]);
        assert!(!run.marks);
        assert_eq!(run.initial_command.as_deref(), Some("cargo run 'my dir'"));
        let st = tool_state(ShellFamily::Wsl, &["wsl.exe"]);
        let (run, _) = resolve_launch(&st, &launch_of("build", &fields)).unwrap();
        assert_eq!(
            run.argv,
            ["wsl.exe", "--exec", "bash", "-lc", "cargo run 'my dir'; exec bash -l"]
        );
    }

    #[test]
    fn a_missing_field_is_refused_with_its_label() {
        let st = tool_state(ShellFamily::PowerShell, &["pwsh.exe"]);
        let refused = |fields: &[(&str, &str)]| resolve_launch(&st, &launch_of("build", fields)).err().unwrap();
        let want = (StatusCode::BAD_REQUEST, "Fill in Target.".to_string());
        assert_eq!(refused(&[]), want);
        assert_eq!(refused(&[("target", "")]), want);
        assert_eq!(refused(&[("target", "  ")]), want);
        for bad in ["a\nb", "a\rb", "a\0b", "a\n"] {
            assert_eq!(refused(&[("target", bad)]), want, "{bad:?}");
        }
    }

    #[test]
    fn an_unknown_preset_or_a_missing_program_is_refused() {
        let st = tool_state(ShellFamily::PowerShell, &["pwsh.exe"]);
        let refused = |launch: ToolLaunch| resolve_launch(&st, &launch).err().unwrap();
        assert_eq!(
            refused(launch_of("nope", &[])),
            (StatusCode::BAD_REQUEST, "Unknown tool preset: mine/nope.".to_string())
        );
        let mut other = launch_of("build", &[]);
        other.tool = "nothing".into();
        assert_eq!(refused(other).1, "Unknown tool preset: nothing/build.");
        assert_eq!(
            refused(launch_of("gone", &[])),
            (
                StatusCode::BAD_REQUEST,
                "no-such-program-marshell-test isn't installed, or isn't on PATH.".to_string()
            )
        );
        // A shipped preset is valid too (it is refused here only when its program is missing).
        let shipped = ToolLaunch {
            tool: "vite".into(),
            preset: "dev".into(),
            fields: Default::default(),
        };
        let result = resolve_launch(&st, &shipped);
        assert!(result.is_ok() || result.err().unwrap().1.ends_with("isn't installed, or isn't on PATH."));
    }

    #[test]
    fn a_field_naming_a_program_cannot_change_the_program() {
        // The program is the preset's; a value that looks like a command stays an argument.
        let st = tool_state(ShellFamily::Posix, &["fish"]);
        let (run, _) = resolve_launch(&st, &launch_of("build", &[("target", "x'; calc; '")])).unwrap();
        assert_eq!(run.initial_command.as_deref(), Some("cargo run 'x'\\''; calc; '\\'''"));
    }

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

    /// A SessionStart-like hook body that gives `tab` an index row for session `id` with its transcript at `path`.
    fn note_session(st: &AppState, tab: &str, id: &str, path: &std::path::Path) {
        let body = serde_json::json!({
            "hook_event_name": "SessionStart",
            "session_id": id,
            "transcript_path": path.to_string_lossy(),
            "cwd": path.parent().unwrap().to_string_lossy(),
        });
        assert!(st.history().note_event("claude", tab, None, &body, 1).is_none());
    }

    #[test]
    fn resume_session_prefers_the_tab_id() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        // No row yet: the CLI's own continue flag.
        assert_eq!(resume_session(&st, "t1", Some("claude")), Ok(Resume::Last));
        let transcript = work.path().join("abc-1.jsonl");
        std::fs::write(
            &transcript,
            "{}
",
        )
        .unwrap();
        note_session(&st, "t1", "abc-1", &transcript);
        assert_eq!(
            resume_session(&st, "t1", Some("claude")),
            Ok(Resume::Session("abc-1".to_string()))
        );
        // Another tab has no row, and Claude's row is not Codex's.
        assert_eq!(resume_session(&st, "t2", Some("claude")), Ok(Resume::Last));
        assert_eq!(resume_session(&st, "t1", Some("codex")), Ok(Resume::Last));
        assert_eq!(resume_session(&st, "t1", None), Ok(Resume::Last));
        // Marshell holds a copy of it, but Claude's file is gone: the conversation is lost.
        let meta = st.history().latest_for_tab("t1").unwrap();
        crate::history::archive::copy_transcript(home.path(), &meta).unwrap();
        std::fs::remove_file(&transcript).unwrap();
        assert_eq!(resume_session(&st, "t1", Some("claude")), Err(NO_CONVERSATION));
    }

    #[test]
    fn resume_session_ignores_an_unsafe_row() {
        use crate::history::{SessionMeta, index::Index};
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        let row = |tab: &str, id: &str, path: &str| SessionMeta {
            agent: "claude".into(),
            session_id: id.into(),
            tab_id: tab.into(),
            cwd: String::new(),
            transcript_path: path.into(),
            started_ms: 1,
            updated_ms: 2,
            previous_in_tab: None,
            recordings: vec![],
            client: Default::default(),
            preset: None,
        };
        // An id that would reach the command line, with a transcript that really exists.
        let hostile = work.path().join("x; calc.jsonl");
        std::fs::write(&hostile, "{}\n").unwrap();
        // A transcript on a share.
        let unc = row("t2", "abc-2", r"\\host\share\abc-2.jsonl");
        // A row written to the index directly, as a damaged or planted index would hold it.
        let (index, _) = Index::open(home.path()).unwrap();
        index.upsert(&row("t1", "x; calc", &hostile.to_string_lossy())).unwrap();
        index.upsert(&unc).unwrap();
        // Neither row is used, and neither counts as a known conversation that was lost.
        assert_eq!(resume_session(&st, "t1", Some("claude")), Ok(Resume::Last));
        assert_eq!(resume_session(&st, "t2", Some("claude")), Ok(Resume::Last));
    }

    #[test]
    fn a_session_without_a_transcript_or_archive_starts_fresh() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        // SessionStart indexed the session, but Claude has not written its first message yet.
        let transcript = work.path().join("new-1.jsonl");
        note_session(&st, "t1", "new-1", &transcript);
        let fresh = resume_session(&st, "t1", Some("claude"));
        assert_eq!(fresh, Ok(Resume::Fresh));
        // Fresh means no resume flag at all, not even `--continue`.
        assert_eq!(Resume::Fresh.target(), None);
        assert_eq!(Resume::Last.target(), Some(ResumeTarget::Last));
        // The row is Claude's, so a Codex tab has none of its own: Codex's own continue flag.
        assert_eq!(resume_session(&st, "t1", Some("codex")), Ok(Resume::Last));
    }

    const CODEX_START: &str = include_str!("../../tests/fixtures/codex/hooks/0.163.0/SessionStart.json");

    /// Codex's SessionStart for `id` in `tab` at `at_ms`, with its rollout named as Codex names it, under `dir`.
    /// Returns the rollout's path; the file itself is not written.
    fn note_codex(st: &AppState, tab: &str, id: &str, dir: &std::path::Path, at_ms: u64) -> std::path::PathBuf {
        let rollout = dir.join(format!("rollout-2026-10-25T10-00-00-{id}.jsonl"));
        let mut body: serde_json::Value = serde_json::from_str(CODEX_START).unwrap();
        body["session_id"] = serde_json::json!(id);
        body["transcript_path"] = serde_json::json!(rollout.to_string_lossy());
        body["cwd"] = serde_json::json!(dir.to_string_lossy());
        st.history().note_event("codex", tab, None, &body, at_ms);
        rollout
    }

    const CODEX_ID: &str = "019a2b3c-4d5e-7f60-8a9b-0c1d2e3f4a5b";

    #[test]
    fn a_codex_tab_resumes_its_own_conversation_by_id() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        let rollout = note_codex(&st, "t1", CODEX_ID, work.path(), 1);
        std::fs::write(&rollout, "{}\n").unwrap();
        assert_eq!(
            resume_session(&st, "t1", Some("codex")),
            Ok(Resume::Session(CODEX_ID.to_string()))
        );
    }

    #[test]
    fn a_codex_tab_with_no_conversation_on_record_resumes_the_last() {
        let home = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        assert_eq!(resume_session(&st, "t1", Some("codex")), Ok(Resume::Last));
    }

    /// Marshell keeps no copy of a Codex rollout, so a missing file can't be told from one Codex never wrote:
    /// Codex starts fresh, and never with another tab's conversation (slice 6a2 decision 4).
    #[test]
    fn a_codex_tab_whose_rollout_is_gone_starts_fresh() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        note_codex(&st, "t1", CODEX_ID, work.path(), 1);
        assert_eq!(resume_session(&st, "t1", Some("codex")), Ok(Resume::Fresh));
    }

    #[test]
    fn a_codex_row_that_fails_the_check_resumes_the_last() {
        use crate::history::{SessionMeta, index::Index};
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        let notes = work.path().join("notes.txt");
        std::fs::write(&notes, "{}\n").unwrap();
        let row = |tab: &str, id: &str, path: &str| SessionMeta {
            agent: "codex".into(),
            session_id: id.into(),
            tab_id: tab.into(),
            cwd: String::new(),
            transcript_path: path.into(),
            started_ms: 1,
            updated_ms: 2,
            previous_in_tab: None,
            recordings: vec![],
            client: Default::default(),
            preset: None,
        };
        // Rows written to the index directly, as a damaged or planted index would hold them: a file that is not a
        // rollout, a relative path, and an id that would reach the command line.
        let (index, _) = Index::open(home.path()).unwrap();
        index.upsert(&row("t1", CODEX_ID, &notes.to_string_lossy())).unwrap();
        index.upsert(&row("t2", CODEX_ID, "sessions/rollout-x.jsonl")).unwrap();
        let hostile = work.path().join("rollout-x.jsonl");
        std::fs::write(&hostile, "{}\n").unwrap();
        index.upsert(&row("t3", "x; calc", &hostile.to_string_lossy())).unwrap();
        for tab in ["t1", "t2", "t3"] {
            assert_eq!(resume_session(&st, tab, Some("codex")), Ok(Resume::Last), "{tab}");
        }
    }

    /// A CLI started from inside another inherits the tab id, so a tab can hold rows of two agents. A newer row of
    /// the other agent never hides the tab's own conversation.
    #[test]
    fn a_tab_resumes_only_its_own_agents_conversation() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        let transcript = work.path().join("abc-1.jsonl");
        std::fs::write(&transcript, "{}\n").unwrap();
        note_session(&st, "t1", "abc-1", &transcript);
        let rollout = note_codex(&st, "t1", CODEX_ID, work.path(), 50);
        std::fs::write(&rollout, "{}\n").unwrap();
        assert_eq!(
            resume_session(&st, "t1", Some("claude")),
            Ok(Resume::Session("abc-1".to_string()))
        );
        assert_eq!(
            resume_session(&st, "t1", Some("codex")),
            Ok(Resume::Session(CODEX_ID.to_string()))
        );
    }

    #[test]
    fn a_resumed_codex_tab_runs_resume_with_its_id() {
        assert_eq!(
            resolved_codex_line(Some("0.163.0"), Some(ResumeTarget::Session(CODEX_ID))),
            format!("& codex resume {CODEX_ID} --no-daemon")
        );
    }

    const AGY_PRE: &str = include_str!("../../tests/fixtures/antigravity/hooks/1.4.7/PreInvocation.json");
    const AGY_ID: &str = "00000000-0000-4000-8000-000000000002";

    /// agy's PreInvocation for `id` in `tab`, as the bridge posts it, with agy's transcript path under `dir`.
    /// The tab is opened in the registry first, as it is when a hook arrives. Returns the transcript's path;
    /// the file itself is not written.
    fn note_agy(st: &AppState, tab: &str, id: &str, dir: &std::path::Path, at_ms: u64) -> std::path::PathBuf {
        let transcript = dir
            .join("antigravity-cli")
            .join("brain")
            .join(id)
            .join(".system_generated")
            .join("logs")
            .join("transcript_full.jsonl");
        st.registry().reserve(
            tab,
            crate::registry::Relaunch {
                profile_id: "pwsh".into(),
                agent: Some("antigravity".into()),
                cwd: dir.to_string_lossy().into_owned(),
                mode: None,
                name: "agy".into(),
                preset: None,
            },
        );
        let mut body: serde_json::Value = serde_json::from_str(AGY_PRE).unwrap();
        body["hook_event_name"] = serde_json::json!("PreInvocation");
        body["conversationId"] = serde_json::json!(id);
        body["transcriptPath"] = serde_json::json!(transcript.to_string_lossy());
        st.history().note_event("antigravity", tab, None, &body, at_ms);
        transcript
    }

    #[test]
    fn resume_of_an_antigravity_tab_uses_its_conversation() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        let transcript = note_agy(&st, "t1", AGY_ID, work.path(), 1);
        // agy writes the file from the first prompt.
        std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        std::fs::write(&transcript, "{}\n").unwrap();
        assert_eq!(
            resume_session(&st, "t1", Some("antigravity")),
            Ok(Resume::Session(AGY_ID.to_string()))
        );
        // The target the launch is given turns into `--conversation <id>`.
        assert_eq!(
            resume_session(&st, "t1", Some("antigravity")).unwrap().target(),
            Some(ResumeTarget::Session(AGY_ID))
        );
        // The conversation is the tab's own: another tab continues.
        assert_eq!(resume_session(&st, "t2", Some("antigravity")), Ok(Resume::Last));
    }

    #[test]
    fn an_antigravity_tab_with_none_on_record_continues() {
        let home = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        assert_eq!(resume_session(&st, "t1", Some("antigravity")), Ok(Resume::Last));
    }

    /// agy's transcript exists from the first prompt, so a missing one is a conversation that is gone: start
    /// fresh, never with `--continue`, which could pick up another tab's.
    #[test]
    fn an_antigravity_tab_whose_transcript_is_gone_starts_fresh() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let st = AppState::new("t".into(), 1, Vec::new(), home.path().to_path_buf(), Vec::new());
        note_agy(&st, "t1", AGY_ID, work.path(), 1);
        assert_eq!(resume_session(&st, "t1", Some("antigravity")), Ok(Resume::Fresh));
    }

    #[test]
    fn colorfgbg_values() {
        assert_eq!(colorfgbg(ColorScheme::Dark), "15;0");
        assert_eq!(colorfgbg(ColorScheme::Light), "0;15");
    }

    /// The env without `PATH`, which `spawn_env` always adds.
    fn without_path(env: Vec<(String, String)>) -> Vec<(String, String)> {
        env.into_iter().filter(|(k, _)| k != "PATH").collect()
    }

    #[test]
    fn spawn_env_puts_colorfgbg_before_profile_env() {
        let home = std::path::Path::new("h");
        // The profile sets its own COLORFGBG: it comes last, so it wins.
        let env = spawn_env(
            Some(ColorScheme::Dark),
            &[var("COLORFGBG", "7;0")],
            home,
            ShellFamily::Posix,
        );
        let last = env.iter().rev().find(|(k, _)| k == "COLORFGBG").unwrap();
        assert_eq!(last.1, "7;0");

        let env = spawn_env(Some(ColorScheme::Dark), &[], home, ShellFamily::Posix);
        assert!(env.contains(&var("COLORFGBG", "15;0")));
    }

    #[test]
    fn spawn_env_without_a_scheme_sets_no_colorfgbg() {
        let env = spawn_env(None, &[var("A", "1")], std::path::Path::new("h"), ShellFamily::Posix);
        assert_eq!(without_path(env), vec![var("A", "1")]);
    }

    #[test]
    fn spawn_env_puts_bin_first_on_path() {
        let home = std::path::Path::new("C:/m home");
        let bin = crate::paths::bin_dir(home);
        let sep = if cfg!(windows) { ';' } else { ':' };
        let joined = |rest: &std::ffi::OsStr| format!("{}{sep}{}", bin.display(), rest.to_string_lossy());
        // A profile PATH: the bin dir goes in front of it, and the profile's entry is the one that changes.
        let env = spawn_env(None, &[var("PATH", "C:\\a"), var("A", "1")], home, ShellFamily::Posix);
        assert_eq!(
            env,
            vec![var("PATH", &joined(std::ffi::OsStr::new("C:\\a"))), var("A", "1")]
        );
        // No profile PATH: the bin dir, then the app's own PATH.
        let own = std::env::var_os("PATH").unwrap_or_default();
        let env = spawn_env(None, &[], home, ShellFamily::Posix);
        assert_eq!(env, vec![var("PATH", &joined(&own))]);
        // A WSL session runs in a Linux that has no use for it (slice 6j).
        let env = spawn_env(None, &[var("A", "1")], home, ShellFamily::Wsl);
        assert_eq!(env, vec![var("A", "1")]);
    }
}
