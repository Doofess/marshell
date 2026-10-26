//! What Marshell knows about an agent's hook setup, and the previews that install and remove it.
//! Only `claude`, `codex` and `antigravity` have an adapter; any other agent is a 404. Applying goes through `/v1/config/apply`.

use super::AppState;
use crate::agents::account::{claude_status, codex_status};
use crate::agents::antigravity::hooks as agy;
use crate::agents::claude::{self, hooks};
use crate::agents::codex::{self, trust};
use crate::agents::install::{codex_newer, install_kind, update_text};
use crate::agents::{parse_any_version, run_version_text};
use crate::bridge;
use crate::config::jsonc::{parse_problem, read_value};
use crate::paths;
use crate::profiles::detect::{agent_command, agent_exe};
use axum::Json;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::StatusCode;
use marshell_protocol::account::{AccountStatus, SignIn};
use marshell_protocol::agents::{
    AgentFound, AgentSetup, HookInstall, HookPlanRequest, ParseProblem, RemovePreview, SettingsState,
};
use marshell_protocol::config::ConfigPreview;
use marshell_protocol::install::{AgentInstall, InstallKind};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

const AGENT: &str = "claude";
const CODEX: &str = "codex";
const ANTIGRAVITY: &str = "antigravity";
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// A status and the message the route replies with.
type Fail = (StatusCode, String);

fn failed(err: impl std::fmt::Display) -> Fail {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

/// The agents that have a hook adapter.
#[derive(Clone, Copy)]
enum Adapter {
    Claude,
    Codex,
    Antigravity,
}

fn adapter(agent: &str) -> Result<Adapter, Fail> {
    match agent {
        AGENT => Ok(Adapter::Claude),
        CODEX => Ok(Adapter::Codex),
        ANTIGRAVITY => Ok(Adapter::Antigravity),
        _ => Err((StatusCode::NOT_FOUND, "no such agent".to_string())),
    }
}

/// A plan the adapter refuses is a 422 that names the file and says nothing will change.
fn refused(err: &hooks::PlanError) -> Fail {
    let message = match err {
        hooks::PlanError::DoesNotParse { path, problem } => format!(
            "{} does not parse (line {}), so Marshell will not edit it.",
            path.display(),
            problem.0
        ),
        hooks::PlanError::NotAnObject { path, pointer } => format!(
            "{} has a {pointer} that is not an object, so Marshell will not edit it.",
            path.display()
        ),
        hooks::PlanError::BridgePath { .. } => format!("{err}."),
    };
    (StatusCode::UNPROCESSABLE_ENTITY, message)
}

/// The text of `path`: `None` when the file is missing. Text that is not UTF-8 does not parse.
fn read_text(path: &Path) -> Result<Option<String>, hooks::PlanError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(hooks::PlanError::DoesNotParse {
            path: path.to_path_buf(),
            problem: (1, e.to_string()),
        }),
    }
}

/// The user's home folder, the settings file and the chain file's text.
struct Inputs {
    user_home: PathBuf,
    settings_path: PathBuf,
    text: Option<String>,
    chain_text: Option<String>,
}

/// The user home, Antigravity's two files and the chain, as text (`None` for a missing file).
struct AgyInputs {
    user_home: PathBuf,
    hooks_text: Option<String>,
    settings_text: Option<String>,
    chain_text: Option<String>,
}

/// The user's home folder, Codex's `hooks.json` and its text.
struct CodexInputs {
    hooks_path: PathBuf,
    text: Option<String>,
}

impl AppState {
    fn codex_hooks_path(&self, user_home: &Path) -> PathBuf {
        match &self.0.codex_hooks {
            Some(path) => path.clone(),
            None => codex::hooks::hooks_path(&|k| std::env::var(k).ok(), user_home),
        }
    }

    /// The user home Antigravity's files sit under.
    fn agy_user_home(&self) -> Result<PathBuf, Fail> {
        match &self.0.agy_home {
            Some(home) => Ok(home.clone()),
            None => super::files::home_dir().ok_or_else(|| failed("the user's home folder is not known")),
        }
    }

    fn agy_inputs(&self) -> Result<AgyInputs, Fail> {
        let user_home = self.agy_user_home()?;
        let hooks_text = read_text(&agy::hooks_path(&user_home)).map_err(|e| refused(&e))?;
        let settings_text = read_text(&agy::settings_path(&user_home)).map_err(|e| refused(&e))?;
        // The chain is Marshell's own file; a chain that cannot be read is the same as none.
        let chain_text = std::fs::read_to_string(paths::statusline_chain(&self.0.home, ANTIGRAVITY)).ok();
        Ok(AgyInputs {
            user_home,
            hooks_text,
            settings_text,
            chain_text,
        })
    }

    fn codex_inputs(&self) -> Result<CodexInputs, Fail> {
        let user_home = super::files::home_dir().ok_or_else(|| failed("the user's home folder is not known"))?;
        let hooks_path = self.codex_hooks_path(&user_home);
        let text = read_text(&hooks_path).map_err(|e| refused(&e))?;
        Ok(CodexInputs { hooks_path, text })
    }

    /// The Codex version of `exe`, or the one a test pinned. May run a process, so async callers go
    /// through `spawn_blocking`.
    pub(crate) fn codex_version(&self, exe: Option<&Path>) -> Option<String> {
        match &self.0.codex_version {
            Some(pinned) => pinned.clone(),
            None => exe.and_then(|exe| self.version_of(exe, parse_any_version)),
        }
    }

    fn claude_settings_path(&self, user_home: &Path) -> PathBuf {
        match &self.0.claude_settings {
            Some(path) => path.clone(),
            None => hooks::settings_path(&|k| std::env::var(k).ok(), user_home),
        }
    }

    fn inputs(&self) -> Result<Inputs, Fail> {
        let user_home = super::files::home_dir().ok_or_else(|| failed("the user's home folder is not known"))?;
        let settings_path = self.claude_settings_path(&user_home);
        let text = read_text(&settings_path).map_err(|e| refused(&e))?;
        // The chain is Marshell's own file; a chain that cannot be read is the same as none.
        let chain_text = std::fs::read_to_string(paths::statusline_chain(&self.0.home, AGENT)).ok();
        Ok(Inputs {
            user_home,
            settings_path,
            text,
            chain_text,
        })
    }

    /// Puts the bridge back into `<home>/bin` before hooks that run it are planned. Only the app knows where its
    /// programs are, so this does nothing without a programs folder (tests) or when the bridge is there.
    fn ensure_bridge(&self, want: &HookPlanRequest) -> Result<(), Fail> {
        let bridge = paths::bridge_exe(&self.0.home);
        let Some(dir) = self
            .0
            .programs_dir
            .as_deref()
            .filter(|_| want.status && !bridge.exists())
        else {
            return Ok(());
        };
        bridge::ensure(dir, &self.0.home).map_err(|e| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!(
                    "Marshell's hook program is missing from {} and could not be put back ({e}), so setting up would leave hooks that fail each time one fires. Reinstall Marshell, then set up again.",
                    bridge.display()
                ),
            )
        })
    }

    /// The `claude` that sessions run: a profile that runs it itself, else the one on PATH.
    pub(super) fn claude_exe(&self) -> Option<PathBuf> {
        self.agent_exe_of(AGENT)
    }

    /// The file sessions run for `agent`: a profile that runs it itself, else the one on PATH.
    pub(crate) fn agent_exe_of(&self, agent: &str) -> Option<PathBuf> {
        // The lock is dropped before PATH is searched. An adapter's program can differ from its id (`agy`).
        let from_profile = agent_exe(&self.0.profiles.lock().unwrap().profiles, agent);
        from_profile.or_else(|| self.agent_finder(&[]).path_of(agent_command(agent).unwrap_or(agent)))
    }

    /// Where an adapter agent's program is: a profile that runs it, else its command on PATH. May touch the disk.
    pub(super) fn adapter_exe(
        &self,
        profiles: &[marshell_protocol::api::Profile],
        agent: &str,
        finder: &super::AgentFinder,
    ) -> Option<PathBuf> {
        agent_exe(profiles, agent).or_else(|| agent_command(agent).and_then(|c| finder.path_of(c)))
    }

    /// The Claude Code version of `exe`, or the one a test pinned. Runs a process on a cold cache, so
    /// async callers go through `spawn_blocking`.
    pub(crate) fn claude_version(&self, exe: Option<&Path>) -> Option<String> {
        match &self.0.claude_version {
            Some(pinned) => pinned.clone(),
            None => exe.and_then(|exe| self.version_of(exe, claude::parse_version)),
        }
    }

    /// The agent's version, preserving Claude's strict parser and the version pinned by tests.
    pub(crate) fn agent_version(&self, agent: &str, exe: Option<&Path>) -> Option<String> {
        if agent == AGENT {
            self.claude_version(exe)
        } else if agent == CODEX && self.0.codex_version.is_some() {
            self.codex_version(exe)
        } else if agent == ANTIGRAVITY
            && let Some(pinned) = &self.0.agy_version
        {
            pinned.clone()
        } else {
            exe.and_then(|exe| self.version_of(exe, parse_any_version))
        }
    }

    /// Drops the remembered version of `exe`: an npm update can leave the shim's modified time as it was.
    pub(crate) fn forget_version(&self, exe: &Path) {
        forget_cached(&self.0.versions, exe);
    }

    /// The folder Codex keeps its own files in: where its `hooks.json` is.
    fn codex_dir(&self) -> Option<PathBuf> {
        let user_home = match super::files::home_dir() {
            Some(home) => home,
            None if self.0.codex_hooks.is_some() => PathBuf::new(),
            None => return None,
        };
        self.codex_hooks_path(&user_home).parent().map(Path::to_path_buf)
    }

    /// Cache stdout per executable and modified time; parse on every read so a generic lookup
    /// cannot make Claude's strict gate accept output it would otherwise reject.
    fn version_of(&self, exe: &Path, parse: fn(&str) -> Option<String>) -> Option<String> {
        let modified = std::fs::metadata(exe).and_then(|m| m.modified()).ok();
        let text = cached_version(&self.0.versions, exe, modified, || {
            run_version_text(exe, VERSION_TIMEOUT).filter(|text| parse(text).is_some())
        })?;
        parse(&text)
    }
}

/// The version `cache` holds for `exe` at this modified time, else what `ask` reads. A version that
/// could not be read is not kept, so the next call tries again. The lock is not held while `ask` runs.
fn cached_version(
    cache: &Mutex<HashMap<PathBuf, (Option<SystemTime>, String)>>,
    exe: &Path,
    modified: Option<SystemTime>,
    ask: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let known = cache.lock().unwrap().get(exe).cloned();
    if let Some((at, version)) = known
        && at == modified
    {
        return Some(version);
    }
    let version = ask()?;
    cache
        .lock()
        .unwrap()
        .insert(exe.to_path_buf(), (modified, version.clone()));
    Some(version)
}

/// Drops what `cache` knows of `exe`, so the next read asks the program again.
fn forget_cached(cache: &Mutex<HashMap<PathBuf, (Option<SystemTime>, String)>>, exe: &Path) {
    cache.lock().unwrap().remove(exe);
}

pub(crate) fn setup_of(st: &AppState) -> Result<AgentSetup, Fail> {
    let home = &st.0.home;
    let user_home = super::files::home_dir().ok_or_else(|| failed("the user's home folder is not known"))?;
    let settings_path = st.claude_settings_path(&user_home);

    let exe = st.claude_exe();
    let version = st.claude_version(exe.as_deref());

    let mut setup = AgentSetup {
        agent: AGENT.to_string(),
        found: exe.is_some(),
        version,
        exe_path: exe.map(|p| p.to_string_lossy().into_owned()),
        settings_path: settings_path.to_string_lossy().into_owned(),
        hooks_path: None,
        settings_state: SettingsState::Ok,
        parse_error: None,
        hooks: HookInstall::None,
        approve_installed: false,
        has_own_statusline: false,
        chained: false,
        needs_trust: false,
        trust: None,
        bridge_path: paths::bridge_exe(home).to_string_lossy().into_owned(),
        snapshots_dir: paths::snapshots_dir(home).to_string_lossy().into_owned(),
    };

    let text = match std::fs::read_to_string(&settings_path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            setup.settings_state = SettingsState::Missing;
            return Ok(setup);
        }
        Err(e) => {
            setup.settings_state = SettingsState::Unparsable;
            setup.parse_error = Some(ParseProblem {
                line: 1,
                message: e.to_string(),
            });
            return Ok(setup);
        }
    };
    // A file whose `hooks` or `statusLine` has the wrong shape still reads as valid JSON here, so it
    // is `Ok` with what `inspect` can see; the plan routes refuse it with a 422.
    let settings = match (parse_problem(&text), read_value(&text)) {
        (None, Ok(settings)) => settings,
        (problem, _) => {
            let (line, message) = problem.unwrap_or((1, "the file does not parse".to_string()));
            setup.settings_state = SettingsState::Unparsable;
            setup.parse_error = Some(ParseProblem { line, message });
            return Ok(setup);
        }
    };
    if std::fs::metadata(&settings_path).is_ok_and(|m| m.permissions().readonly()) {
        setup.settings_state = SettingsState::ReadOnly;
    }
    let chain = std::fs::read_to_string(paths::statusline_chain(home, AGENT))
        .ok()
        .and_then(|t| read_value(&t).ok());
    let found = hooks::inspect(&settings, chain.as_ref(), home);
    setup.hooks = found.install;
    setup.approve_installed = found.approve_installed;
    setup.has_own_statusline = found.has_own_statusline;
    setup.chained = found.chained;
    Ok(setup)
}

/// Codex's setup: the hooks file, and the `config.toml` beside it read (never written) for trust.
pub(crate) fn codex_setup_of(st: &AppState) -> Result<AgentSetup, Fail> {
    let home = &st.0.home;
    let user_home = super::files::home_dir().ok_or_else(|| failed("the user's home folder is not known"))?;
    let hooks_path = st.codex_hooks_path(&user_home);
    let exe = st.agent_exe_of(CODEX);
    let version = st.codex_version(exe.as_deref());

    let mut setup = AgentSetup {
        agent: CODEX.to_string(),
        found: exe.is_some(),
        version,
        exe_path: exe.map(|p| p.to_string_lossy().into_owned()),
        settings_path: hooks_path.to_string_lossy().into_owned(),
        hooks_path: None,
        settings_state: SettingsState::Ok,
        parse_error: None,
        hooks: HookInstall::None,
        approve_installed: false,
        has_own_statusline: false,
        chained: false,
        needs_trust: true,
        trust: None,
        bridge_path: paths::bridge_exe(home).to_string_lossy().into_owned(),
        snapshots_dir: paths::snapshots_dir(home).to_string_lossy().into_owned(),
    };

    let text = match std::fs::read_to_string(&hooks_path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            setup.settings_state = SettingsState::Missing;
            return Ok(setup);
        }
        Err(e) => {
            setup.settings_state = SettingsState::Unparsable;
            setup.parse_error = Some(ParseProblem {
                line: 1,
                message: e.to_string(),
            });
            return Ok(setup);
        }
    };
    let hooks_value = match (parse_problem(&text), read_value(&text)) {
        (None, Ok(value)) => value,
        (problem, _) => {
            let (line, message) = problem.unwrap_or((1, "the file does not parse".to_string()));
            setup.settings_state = SettingsState::Unparsable;
            setup.parse_error = Some(ParseProblem { line, message });
            return Ok(setup);
        }
    };
    if std::fs::metadata(&hooks_path).is_ok_and(|m| m.permissions().readonly()) {
        setup.settings_state = SettingsState::ReadOnly;
    }
    let found = codex::hooks::inspect(&hooks_value, home);
    setup.hooks = found.install;
    setup.approve_installed = found.approve_installed;
    // Read only. A missing or unreadable `config.toml` is no record of trust.
    let config_toml = std::fs::read_to_string(hooks_path.with_file_name("config.toml")).ok();
    setup.trust = trust::state(&hooks_path, &hooks_value, config_toml.as_deref());
    Ok(setup)
}

/// Antigravity's setup: `hooks.json` and `settings.json`, both read and neither written.
pub(crate) fn agy_setup_of(st: &AppState) -> Result<AgentSetup, Fail> {
    let home = &st.0.home;
    let user_home = st.agy_user_home()?;
    let (hooks_file, settings_file) = (agy::hooks_path(&user_home), agy::settings_path(&user_home));
    let exe = st.agent_exe_of(ANTIGRAVITY);
    let version = st.agent_version(ANTIGRAVITY, exe.as_deref());

    let mut setup = AgentSetup {
        agent: ANTIGRAVITY.to_string(),
        found: exe.is_some(),
        version,
        exe_path: exe.map(|p| p.to_string_lossy().into_owned()),
        settings_path: settings_file.to_string_lossy().into_owned(),
        hooks_path: Some(hooks_file.to_string_lossy().into_owned()),
        settings_state: SettingsState::Ok,
        parse_error: None,
        hooks: HookInstall::None,
        approve_installed: false,
        has_own_statusline: false,
        chained: false,
        needs_trust: false,
        trust: None,
        bridge_path: paths::bridge_exe(home).to_string_lossy().into_owned(),
        snapshots_dir: paths::snapshots_dir(home).to_string_lossy().into_owned(),
    };

    // A missing file is `None`; one that cannot be read or parsed makes the state unparsable.
    let mut readonly = false;
    let mut values = Vec::new();
    for file in [&hooks_file, &settings_file] {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                values.push(None);
                continue;
            }
            Err(e) => {
                setup.settings_state = SettingsState::Unparsable;
                setup.parse_error = Some(ParseProblem {
                    line: 1,
                    message: format!("{}: {e}", file.display()),
                });
                return Ok(setup);
            }
        };
        match (parse_problem(&text), read_value(&text)) {
            (None, Ok(value)) => values.push(Some(value)),
            (problem, _) => {
                let (line, message) = problem.unwrap_or((1, "the file does not parse".to_string()));
                setup.settings_state = SettingsState::Unparsable;
                setup.parse_error = Some(ParseProblem {
                    line,
                    message: format!("{}: {message}", file.display()),
                });
                return Ok(setup);
            }
        }
        readonly |= std::fs::metadata(file).is_ok_and(|m| m.permissions().readonly());
    }
    if values.iter().all(Option::is_none) {
        setup.settings_state = SettingsState::Missing;
    } else if readonly {
        setup.settings_state = SettingsState::ReadOnly;
    }
    let chain = std::fs::read_to_string(paths::statusline_chain(home, ANTIGRAVITY))
        .ok()
        .and_then(|t| read_value(&t).ok());
    let found = agy::inspect(values[0].as_ref(), values[1].as_ref(), chain.as_ref(), home);
    setup.hooks = found.install;
    setup.approve_installed = found.approve_installed;
    setup.has_own_statusline = found.has_own_statusline;
    setup.chained = found.chained;
    Ok(setup)
}

pub async fn setup(State(st): State<AppState>, UrlPath(agent): UrlPath<String>) -> Result<Json<AgentSetup>, Fail> {
    let adapter = adapter(&agent)?;
    let setup = tokio::task::spawn_blocking(move || match adapter {
        Adapter::Claude => setup_of(&st),
        Adapter::Codex => codex_setup_of(&st),
        Adapter::Antigravity => agy_setup_of(&st),
    })
    .await
    .map_err(failed)??;
    Ok(Json(setup))
}

/// Which of the agent CLIs Marshell can start are on this machine, in the order claude, codex, antigravity.
pub async fn found(State(st): State<AppState>) -> Result<Json<Vec<AgentFound>>, Fail> {
    let list = tokio::task::spawn_blocking(move || {
        let profiles = st.0.profiles.lock().unwrap().profiles.clone();
        let finder = st.agent_finder(&[]);
        ["claude", "codex", "antigravity"]
            .into_iter()
            .map(|agent| {
                let exe = st.adapter_exe(&profiles, agent, &finder);
                AgentFound {
                    agent: agent.to_string(),
                    found: exe.is_some(),
                    exe_path: exe.map(|p| p.to_string_lossy().into_owned()),
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(failed)?;
    Ok(Json(list))
}

#[derive(serde::Deserialize)]
pub struct InstallQuery {
    /// Read the version from the program again, not from what was remembered.
    #[serde(default)]
    recheck: bool,
}

/// The install type, version and update command of one agent CLI, and Codex's note about a newer version.
pub async fn install(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
    Query(query): Query<InstallQuery>,
) -> Result<Json<AgentInstall>, Fail> {
    adapter(&agent)?;
    let info = tokio::task::spawn_blocking(move || install_of(&st, &agent, query.recheck))
        .await
        .map_err(failed)?;
    Ok(Json(info))
}

fn install_of(st: &AppState, agent: &str, recheck: bool) -> AgentInstall {
    let Some(exe) = st.agent_exe_of(agent) else {
        return AgentInstall {
            agent: agent.to_string(),
            found: false,
            version: None,
            install: InstallKind::Unknown,
            update_command: None,
            newer_version: None,
        };
    };
    if recheck {
        st.forget_version(&exe);
    }
    let version = st.agent_version(agent, Some(&exe));
    // Only the match looks at the resolved path (a WinGet link points into `Packages`); what is shown keeps the path as found.
    let resolved = std::fs::canonicalize(&exe).unwrap_or_else(|_| exe.clone());
    let install = install_kind(agent, &resolved, &|p| p.is_file());
    let newer_version = if agent == CODEX {
        version
            .as_deref()
            .zip(st.codex_dir())
            .and_then(|(version, dir)| codex_newer_in(&dir, version))
    } else {
        None
    };
    AgentInstall {
        agent: agent.to_string(),
        found: true,
        version,
        update_command: update_text(agent, &install),
        install,
        newer_version,
    }
}

/// Whether one agent CLI is signed in, asked of the CLI's own status command. Its output stays in here: only the state,
/// the email, the plan and how it is signed in go out.
pub async fn account(State(st): State<AppState>, UrlPath(agent): UrlPath<String>) -> Result<Json<AccountStatus>, Fail> {
    let kind = adapter(&agent)?;
    let status = tokio::task::spawn_blocking(move || account_of(&st, &agent, kind))
        .await
        .map_err(failed)?;
    Ok(Json(status))
}

fn account_of(st: &AppState, agent: &str, kind: Adapter) -> AccountStatus {
    let mut status = AccountStatus {
        agent: agent.to_string(),
        sign_in: SignIn::Unknown,
        email: None,
        plan: None,
        method: None,
        checked_ms: 0,
    };
    match kind {
        // Antigravity has no status command, so nothing is run.
        Adapter::Antigravity => status.sign_in = SignIn::NoCommand,
        Adapter::Claude => {
            if let Some(exe) = st.agent_exe_of(agent)
                && let Some((code, out)) = (st.0.status_runner)(&exe, &["auth", "status"])
            {
                (status.sign_in, status.email, status.plan) = claude_status(&out, code);
            }
        }
        Adapter::Codex => {
            if let Some(exe) = st.agent_exe_of(agent)
                && let Some((code, out)) = (st.0.status_runner)(&exe, &["login", "status"])
            {
                (status.sign_in, status.method) = codex_status(&out, code);
            }
        }
    }
    status.checked_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    status
}

/// Codex's `version.json` in `dir`, read against the installed `version`. A file that can't be read is no news.
fn codex_newer_in(dir: &Path, version: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("version.json")).ok()?;
    codex_newer(&text, version)
}

pub async fn plan(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
    Json(want): Json<HookPlanRequest>,
) -> Result<Json<ConfigPreview>, Fail> {
    let adapter = adapter(&agent)?;
    let preview = tokio::task::spawn_blocking(move || {
        let changes = match adapter {
            Adapter::Claude => {
                let inputs = st.inputs()?;
                st.ensure_bridge(&want)?;
                hooks::install_plan(
                    &inputs.settings_path,
                    &inputs.user_home,
                    inputs.text.as_deref(),
                    inputs.chain_text.as_deref(),
                    &st.0.home,
                    &want,
                )
            }
            Adapter::Codex => {
                let inputs = st.codex_inputs()?;
                st.ensure_bridge(&want)?;
                codex::hooks::install_plan(&inputs.hooks_path, inputs.text.as_deref(), &st.0.home, &want)
            }
            Adapter::Antigravity => {
                let inputs = st.agy_inputs()?;
                st.ensure_bridge(&want)?;
                agy::install_plan(
                    &inputs.user_home,
                    inputs.hooks_text.as_deref(),
                    inputs.settings_text.as_deref(),
                    inputs.chain_text.as_deref(),
                    &st.0.home,
                    &want,
                )
            }
        }
        .map_err(|e| refused(&e))?;
        Ok::<_, Fail>(st.0.config.preview(changes))
    })
    .await
    .map_err(failed)??;
    Ok(Json(preview))
}

pub async fn remove_plan(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
) -> Result<Json<RemovePreview>, Fail> {
    let adapter = adapter(&agent)?;
    let removal = tokio::task::spawn_blocking(move || match adapter {
        Adapter::Claude => {
            let inputs = st.inputs()?;
            let plan = hooks::remove_plan(
                &inputs.settings_path,
                &inputs.user_home,
                inputs.text.as_deref(),
                inputs.chain_text.as_deref(),
                &st.0.home,
            )
            .map_err(|e| refused(&e))?;
            Ok::<_, Fail>(RemovePreview {
                preview: st.0.config.preview(plan.changes),
                statusline_left: plan.statusline_left,
            })
        }
        Adapter::Codex => {
            let inputs = st.codex_inputs()?;
            let changes = codex::hooks::remove_plan(&inputs.hooks_path, inputs.text.as_deref(), &st.0.home)
                .map_err(|e| refused(&e))?;
            Ok(RemovePreview {
                preview: st.0.config.preview(changes),
                statusline_left: false,
            })
        }
        Adapter::Antigravity => {
            let inputs = st.agy_inputs()?;
            let changes = agy::remove_plan(
                &inputs.user_home,
                inputs.hooks_text.as_deref(),
                inputs.settings_text.as_deref(),
                inputs.chain_text.as_deref(),
                &st.0.home,
            )
            .map_err(|e| refused(&e))?;
            Ok(RemovePreview {
                preview: st.0.config.preview(changes),
                statusline_left: false,
            })
        }
    })
    .await
    .map_err(failed)??;
    Ok(Json(removal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn at(secs: u64) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
    }

    #[test]
    fn a_version_is_asked_once_while_the_file_stays_the_same() {
        let cache = Mutex::new(HashMap::new());
        let exe = Path::new("claude");
        let runs = Cell::new(0);
        let ask = || {
            runs.set(runs.get() + 1);
            Some("2.1.303".to_string())
        };
        assert_eq!(cached_version(&cache, exe, at(10), ask).as_deref(), Some("2.1.303"));
        assert_eq!(cached_version(&cache, exe, at(10), ask).as_deref(), Some("2.1.303"));
        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn a_changed_file_is_asked_again() {
        let cache = Mutex::new(HashMap::new());
        let exe = Path::new("claude");
        cached_version(&cache, exe, at(10), || Some("2.1.303".to_string()));
        let after_update = cached_version(&cache, exe, at(20), || Some("2.2.0".to_string()));
        assert_eq!(after_update.as_deref(), Some("2.2.0"));
        assert_eq!(cached_version(&cache, exe, at(20), || None).as_deref(), Some("2.2.0"));
    }

    #[test]
    fn a_version_that_could_not_be_read_is_not_remembered() {
        let cache = Mutex::new(HashMap::new());
        let exe = Path::new("claude");
        assert_eq!(cached_version(&cache, exe, at(10), || None), None);
        let retry = cached_version(&cache, exe, at(10), || Some("2.1.303".to_string()));
        assert_eq!(retry.as_deref(), Some("2.1.303"));
    }

    #[test]
    fn recheck_drops_the_cached_version() {
        let cache = Mutex::new(HashMap::new());
        let exe = Path::new("claude");
        let runs = Cell::new(0);
        let ask = || {
            runs.set(runs.get() + 1);
            Some("2.1.303".to_string())
        };
        cached_version(&cache, exe, at(10), ask);
        cached_version(&cache, exe, at(10), ask);
        assert_eq!(runs.get(), 1);
        forget_cached(&cache, exe);
        cached_version(&cache, exe, at(10), ask);
        assert_eq!(runs.get(), 2);
    }
}
