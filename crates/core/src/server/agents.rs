//! What Marshell knows about an agent's hook setup, and the previews that install and remove it.
//! Only `claude`, `codex` and `antigravity` have an adapter; any other agent is a 404. Applying goes through `/v1/config/apply`.

use super::AppState;
use crate::agents::account::{claude_status, codex_status};
use crate::agents::antigravity::hooks as agy;
use crate::agents::claude::{self, hooks};
use crate::agents::codex::{self, trust};
use crate::agents::install::{codex_newer, install_kind, update_text};
use crate::agents::settings_view::{control, has_bom, masked_render, parse_masked, read_managed, text_problem, view};
use crate::agents::skill;
use crate::agents::{parse_any_version, run_version_text};
use crate::bridge;
use crate::config::jsonc::{parse_problem, read_value};
use crate::config::preview::{MAX_CONFIG_BYTES, PlannedFile, TOO_LARGE_DETAIL, unified_diff};
use crate::config::service::ApplyError;
use crate::paths;
use crate::profiles::detect::{agent_command, agent_exe};
use axum::Json;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::account::{AccountStatus, SignIn};
use marshell_protocol::agents::{
    AgentFound, AgentSetup, HookInstall, HookPlanRequest, ParseProblem, RemovePreview, SettingsState, SkillInstall,
};
use marshell_protocol::config::{
    Applies, ApplyConflict, ApplyRequest, ApplyResult, ChangeSet, ConfigPreview, ConfigScope, EditOp, ExpectedHash,
    FileEdit, FileStatus, RefuseReason,
};
use marshell_protocol::install::{AgentInstall, InstallKind};
use marshell_protocol::settings_view::{
    AgentSettingsView, FieldControl, FieldLock, RawLock, SettingEdit, SettingsChange, SettingsPlan,
    SettingsPlanRequest, SettingsUndoRequest, ViewState,
};
use serde_json::Value;
use std::cell::Cell;
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

impl Adapter {
    /// The CLI's name in a plan's title: "Install the marshell skill for Codex".
    fn cli_name(self) -> &'static str {
        let id = match self {
            Adapter::Claude => AGENT,
            Adapter::Codex => CODEX,
            Adapter::Antigravity => ANTIGRAVITY,
        };
        skill::cli_name(id).unwrap_or(id)
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

    /// Where `agent`'s `marshell` skill file goes. A skill home named by a test replaces the user's home and the
    /// process environment; otherwise it is the user's home, with `CLAUDE_CONFIG_DIR` from the environment.
    fn skill_file(&self, agent: &str) -> Result<PathBuf, Fail> {
        let path = match &self.0.skill_home {
            Some(home) => skill::skill_path(agent, &|_| None, home),
            None => {
                let home = super::files::home_dir().ok_or_else(|| failed("the user's home folder is not known"))?;
                skill::skill_path(agent, &|k| std::env::var(k).ok(), &home)
            }
        };
        path.ok_or_else(|| (StatusCode::NOT_FOUND, "no such agent".to_string()))
    }

    /// The bytes of the skill file: `None` when it is missing. A path the config guard refuses, a file over the
    /// config service's size limit and a file that cannot be read are each a 422 that says why.
    fn skill_bytes(&self, path: &Path) -> Result<Option<Vec<u8>>, Fail> {
        let unavailable = |message: String| (StatusCode::UNPROCESSABLE_ENTITY, message);
        if let Some(detail) = self.0.config.path_refusal(path) {
            return Err(unavailable(format!(
                "Marshell will not edit {}: {detail}",
                path.display()
            )));
        }
        if std::fs::metadata(path).is_ok_and(|meta| meta.len() > MAX_CONFIG_BYTES) {
            return Err(unavailable(format!(
                "Marshell will not edit {}: {TOO_LARGE_DETAIL}",
                path.display()
            )));
        }
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(unavailable(format!(
                "{} can't be read ({e}), so Marshell will not change it.",
                path.display()
            ))),
        }
    }

    /// The skill's install state, path and, when it is unavailable, the problem, for `agent`'s setup.
    fn skill_of(&self, agent: &str) -> Result<(SkillInstall, String, Option<String>), Fail> {
        let path = self.skill_file(agent)?;
        let (state, problem) = match self.skill_bytes(&path) {
            Ok(bytes) => (skill::state_of(bytes.as_deref()), None),
            Err((_, problem)) => (SkillInstall::Unavailable, Some(problem)),
        };
        Ok((state, path.to_string_lossy().into_owned(), problem))
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
    let (skill, skill_path, skill_problem) = st.skill_of(AGENT)?;

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
        skill,
        skill_path,
        skill_problem,
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
    let (skill, skill_path, skill_problem) = st.skill_of(CODEX)?;

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
        skill,
        skill_path,
        skill_problem,
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
    let (skill, skill_path, skill_problem) = st.skill_of(ANTIGRAVITY)?;

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
        skill,
        skill_path,
        skill_problem,
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

/// How `exe` was installed. Only the match looks at the resolved path (a WinGet link points into `Packages`); the caller
/// keeps `exe` as found. Touches the disk.
pub(super) fn install_kind_of(agent: &str, exe: &Path) -> InstallKind {
    let resolved = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    install_kind(agent, &resolved, &|p| p.is_file())
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
    let install = install_kind_of(agent, &exe);
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

/// An agent CLI's user settings file, read and never written, with secrets masked before anything leaves. A file that
/// doesn't parse or can't be read gives no values and no text.
pub async fn settings_view(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
) -> Result<Json<AgentSettingsView>, Fail> {
    let kind = adapter(&agent)?;
    let view = tokio::task::spawn_blocking(move || settings_view_of(&st, &agent, kind))
        .await
        .map_err(failed)??;
    Ok(Json(view))
}

/// Where an agent's user settings file is, and Claude's managed settings (the first managed file that parses).
fn settings_file(st: &AppState, kind: Adapter) -> Result<(PathBuf, Option<(PathBuf, Value)>), Fail> {
    let no_home = || failed("the user's home folder is not known");
    Ok(match kind {
        Adapter::Claude => {
            let user_home = super::files::home_dir()
                .or_else(|| st.0.claude_settings.as_ref().map(|_| PathBuf::new()))
                .ok_or_else(no_home)?;
            // The managed files a test names, else the organization's folder.
            let managed_paths = match &st.0.claude_compact_paths {
                Some((_, managed)) => managed.clone(),
                None => claude::compact::managed_settings_paths(),
            };
            (st.claude_settings_path(&user_home), read_managed(&managed_paths))
        }
        Adapter::Codex => (st.codex_dir().ok_or_else(no_home)?.join("config.toml"), None),
        Adapter::Antigravity => (agy::settings_path(&st.agy_user_home()?), None),
    })
}

fn settings_view_of(st: &AppState, agent: &str, kind: Adapter) -> Result<AgentSettingsView, Fail> {
    settings_read(st, agent, kind).map(|(view, _)| view)
}

/// The settings view and whether the file's text starts with a byte-order mark, from one read.
fn settings_read(st: &AppState, agent: &str, kind: Adapter) -> Result<(AgentSettingsView, bool), Fail> {
    let (path, managed) = settings_file(st, kind)?;
    let text = std::fs::read_to_string(&path);
    let had_bom = text.as_deref().is_ok_and(has_bom);
    Ok((view(agent, &path, text, managed), had_bom))
}

/// Codex's `version.json` in `dir`, read against the installed `version`. A file that can't be read is no news.
fn codex_newer_in(dir: &Path, version: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("version.json")).ok()?;
    codex_newer(&text, version)
}

// ---- planning and undoing a settings change ----

const NOT_THIS_FILE: &str = "That snapshot isn't of this agent's settings file.";

/// Why a settings change isn't made: a plain sentence, or a 409 naming the files that changed since the person looked.
enum Refusal {
    Plain(StatusCode, String),
    Changed(Vec<String>),
}

impl From<Fail> for Refusal {
    fn from((status, message): Fail) -> Self {
        Refusal::Plain(status, message)
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        match self {
            Refusal::Plain(status, message) => (status, message).into_response(),
            Refusal::Changed(paths) => {
                let conflict = ApplyConflict {
                    reason: "changed".to_string(),
                    paths,
                };
                (StatusCode::CONFLICT, Json(conflict)).into_response()
            }
        }
    }
}

fn unprocessable(message: impl Into<String>) -> Refusal {
    Refusal::Plain(StatusCode::UNPROCESSABLE_ENTITY, message.into())
}

fn changed_since(paths: Vec<String>) -> Refusal {
    Refusal::Changed(paths)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Plans a change to a company CLI's user settings file. Everything is checked before a ChangeSet is made, no
/// message quotes a value, and the diff never shows a value the settings view hides.
pub async fn settings_plan(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
    Json(req): Json<SettingsPlanRequest>,
) -> Response {
    let kind = match adapter(&agent) {
        Ok(kind) => kind,
        Err(fail) => return fail.into_response(),
    };
    match tokio::task::spawn_blocking(move || settings_plan_of(&st, &agent, kind, req)).await {
        Ok(Ok(plan)) => Json(plan).into_response(),
        Ok(Err(refusal)) => refusal.into_response(),
        Err(e) => failed(e).into_response(),
    }
}

fn settings_plan_of(
    st: &AppState,
    agent: &str,
    kind: Adapter,
    req: SettingsPlanRequest,
) -> Result<SettingsPlan, Refusal> {
    let (view, had_bom) = settings_read(st, agent, kind).map_err(Refusal::from)?;
    match view.state {
        ViewState::Unparsable { line } => {
            return Err(unprocessable(format!(
                "The file doesn't parse (line {line}), so Marshell doesn't change it."
            )));
        }
        ViewState::Unreadable => {
            return Err(unprocessable("Marshell can't read the file, so it doesn't change it."));
        }
        ViewState::Ok | ViewState::Missing => {}
    }
    // The form was opened on another version of the file, or on none.
    if view.hash != req.expected_hash {
        return Err(changed_since(vec![view.path]));
    }
    let ops = match &req.change {
        SettingsChange::Form { edits } => form_ops(agent, &view, edits)?,
        SettingsChange::Raw { text } => vec![raw_op(agent, &view, had_bom, text)?],
    };
    let title = match kind {
        Adapter::Claude => "Change Claude Code settings",
        Adapter::Codex => "Change Codex settings",
        Adapter::Antigravity => "Change Antigravity settings",
    };
    let cs = ChangeSet {
        title: title.to_string(),
        reason: "Edited in Settings.".to_string(),
        edits: ops
            .into_iter()
            .map(|op| FileEdit {
                path: view.path.clone(),
                scope: ConfigScope::User,
                op,
                expected_hash: req.expected_hash.clone(),
                applies: Applies::NewSessions,
            })
            .collect(),
    };
    let normalized = Cell::new(false);
    let preview = st.0.config.preview_own(cs, &masked_diff(agent, &normalized));
    Ok(SettingsPlan {
        preview: changes_of(preview)?,
        normalized: normalized.get(),
    })
}

/// The edit ops of a form change, each checked against the control and lock the view gives its key.
fn form_ops(agent: &str, view: &AgentSettingsView, edits: &[SettingEdit]) -> Result<Vec<EditOp>, Refusal> {
    let mut ops = Vec::new();
    for edit in edits {
        let key = edit.key.as_str();
        let Some(control) = control(agent, key) else {
            return Err(unprocessable(format!(
                "{key} isn't one of the settings this form changes."
            )));
        };
        let lock = view
            .curated
            .iter()
            .find(|row| row.key == key)
            .and_then(|row| row.lock.as_ref());
        match lock {
            Some(FieldLock::Managed { path }) => {
                return Err(unprocessable(format!(
                    "{key} is set by {path}, so Marshell can't change it here."
                )));
            }
            Some(FieldLock::NotScalar) => {
                return Err(unprocessable(format!(
                    "{key} is set as a table in the file, so the form can't change it."
                )));
            }
            Some(FieldLock::Hidden) => {
                return Err(unprocessable(format!(
                    "{key} holds a secret value, so the form can't change it."
                )));
            }
            Some(FieldLock::Account) => return Err(unprocessable(ACCOUNT_LINE)),
            None => {}
        }
        let value = edit
            .value
            .as_deref()
            .map(|text| checked(key, &control, text))
            .transpose()?;
        let toml_key = || key.split('.').map(str::to_string).collect();
        let pointer = || format!("/{}", key.replace('.', "/"));
        ops.push(match (agent, value) {
            ("codex", Some(value)) => EditOp::TomlSet {
                key: toml_key(),
                value: value.to_string(),
            },
            ("codex", None) => EditOp::TomlRemove { key: toml_key() },
            (_, Some(value)) => EditOp::JsonSet {
                pointer: pointer(),
                value: value.to_string(),
            },
            (_, None) => EditOp::JsonRemove { pointer: pointer() },
        });
    }
    Ok(ops)
}

/// The JSON value `text` stands for when `control` offers it; the message never quotes `text`.
fn checked(key: &str, control: &FieldControl, text: &str) -> Result<Value, Refusal> {
    let parsed: Option<Value> = serde_json::from_str(text).ok();
    let (ok, message) = match control {
        FieldControl::Text => (
            matches!(&parsed, Some(Value::String(s)) if !s.is_empty()),
            format!("{key} needs text."),
        ),
        FieldControl::Number { min } => (
            parsed
                .as_ref()
                .and_then(Value::as_i64)
                .is_some_and(|n| n >= i64::from(*min)),
            format!("{key} needs a whole number of at least {min}."),
        ),
        FieldControl::Switch => (
            matches!(parsed, Some(Value::Bool(_))),
            format!("{key} needs on or off."),
        ),
        FieldControl::Choice { values } => (
            matches!(&parsed, Some(Value::String(s)) if values.contains(s)),
            format!("{key} needs one of: {}.", values.join(", ")),
        ),
    };
    match parsed {
        Some(value) if ok => Ok(value),
        _ => Err(unprocessable(message)),
    }
}

/// Why a key Antigravity keeps with the Google account (`enableTelemetry`) is never written, from the form or as text.
const ACCOUNT_LINE: &str =
    "Antigravity keeps this with your Google account, not in this file. Change it with /config in Antigravity.";

/// The JSON value of `key` at the top of `text`; None when the text doesn't hold it.
fn top_value(text: &str, key: &str) -> Option<Value> {
    serde_json::from_str::<Value>(text).ok()?.get(key).cloned()
}

/// The op that replaces the file's whole text, when the view gives that text out and the new text parses.
/// A BOM the file had stays, and only one: the text the person edits never carries it.
fn raw_op(agent: &str, view: &AgentSettingsView, had_bom: bool, text: &str) -> Result<EditOp, Refusal> {
    if view.state == ViewState::Missing {
        return Err(unprocessable("There is no file to edit as text."));
    }
    match view.raw_lock {
        Some(RawLock::NotPlainJson) => {
            return Err(unprocessable(
                "Raw editing is off: this file isn't plain JSON, and Marshell saves it as plain JSON.",
            ));
        }
        Some(RawLock::RepeatedKey) => {
            return Err(unprocessable("Raw editing is off: a key appears twice in this file."));
        }
        Some(RawLock::Hidden) | None => {}
    }
    if view.text.is_none() {
        return Err(unprocessable(
            "This file holds secret values, so Marshell doesn't edit it as text.",
        ));
    }
    // Every pasted BOM goes: TOML's lexer skips one more, so a second would reach the file.
    let body = text.trim_start_matches('\u{feff}');
    if let Some(line) = text_problem(agent, body) {
        return Err(unprocessable(format!("The text doesn't parse at line {line}.")));
    }
    if agent == "antigravity" {
        let before = view.text.as_deref().and_then(|t| top_value(t, "enableTelemetry"));
        if top_value(body, "enableTelemetry") != before {
            return Err(unprocessable(ACCOUNT_LINE));
        }
    }
    let content = if had_bom {
        format!("\u{feff}{body}")
    } else {
        body.to_string()
    };
    Ok(EditOp::ReplaceText { content })
}

/// What `preview_shown` takes for a settings file: when the text before or after holds a value the view hides, the
/// diff is of the masked render of both and `normalized` is set. Otherwise the file's real diff stands.
fn masked_diff<'a>(agent: &'a str, normalized: &'a Cell<bool>) -> impl Fn(&PlannedFile) -> Option<String> + 'a {
    move |file| {
        if file.status != FileStatus::Changes {
            return None;
        }
        let text = |bytes: &Option<Vec<u8>>| bytes.as_deref().map(|b| String::from_utf8_lossy(b).into_owned());
        let (before, after) = (text(&file.before), text(&file.after));
        let plain = |text: &Option<String>| {
            text.as_deref()
                .is_none_or(|t| parse_masked(agent, t).is_ok_and(|p| p.plain()))
        };
        if plain(&before) && plain(&after) {
            return None;
        }
        normalized.set(true);
        let render = |text: &Option<String>| {
            text.as_deref()
                .and_then(|t| masked_render(agent, t))
                .unwrap_or_default()
        };
        Some(unified_diff(
            &file.path.to_string_lossy(),
            file.before.is_some(),
            &render(&before),
            &render(&after),
        ))
    }
}

/// The preview when it has a change to apply. A file changed since is a 409; any other refusal, or no change, is a 422.
/// A refusal's detail can quote the file, so only the details that can't are passed on.
fn changes_of(preview: ConfigPreview) -> Result<ConfigPreview, Refusal> {
    for file in &preview.files {
        if let FileStatus::Refused { reason, detail } = &file.status {
            return Err(match reason {
                RefuseReason::Conflict => changed_since(vec![file.path.clone()]),
                RefuseReason::DoesNotParse | RefuseReason::BadPointer | RefuseReason::BadValue => {
                    unprocessable("Marshell can't make this change to the file.")
                }
                _ => unprocessable(detail.clone()),
            });
        }
    }
    if preview.files.iter().all(|f| f.status != FileStatus::Changes) {
        return Err(unprocessable("Nothing changes."));
    }
    Ok(preview)
}

/// Puts a settings file back as a snapshot of it had it, if the file is still as that change left it.
pub async fn settings_undo(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
    Json(req): Json<SettingsUndoRequest>,
) -> Response {
    let kind = match adapter(&agent) {
        Ok(kind) => kind,
        Err(fail) => return fail.into_response(),
    };
    match tokio::task::spawn_blocking(move || settings_undo_of(&st, &agent, kind, &req.snapshot_id)).await {
        Ok(Ok(result)) => Json(result).into_response(),
        Ok(Err(refusal)) => refusal.into_response(),
        Err(e) => failed(e).into_response(),
    }
}

fn settings_undo_of(st: &AppState, agent: &str, kind: Adapter, snapshot_id: &str) -> Result<ApplyResult, Refusal> {
    let (path, _) = settings_file(st, kind).map_err(Refusal::from)?;
    let info = match st.0.config.snapshots().get(snapshot_id) {
        Ok(Some(info)) => info,
        Ok(None) => return Err(Refusal::Plain(StatusCode::NOT_FOUND, "no such snapshot".to_string())),
        Err(e) => return Err(Refusal::from(failed(format!("{e:#}")))),
    };
    // Only a snapshot of this agent's settings file, and of nothing else.
    let [file] = info.files.as_slice() else {
        return Err(unprocessable(NOT_THIS_FILE));
    };
    let target = st.0.config.resolve_target(&path);
    if target.is_none() || target != st.0.config.resolve_target(Path::new(&file.path)) {
        return Err(unprocessable(NOT_THIS_FILE));
    }
    // The file must still be as the change left it: its hash, or gone when the change removed it.
    if file.after_hash.is_none() && path.exists() {
        return Err(changed_since(vec![file.path.clone()]));
    }
    let cs = ChangeSet {
        title: format!("Restore: {}", info.title),
        reason: "Puts the files back as they were before this change.".to_string(),
        edits: vec![FileEdit {
            path: file.path.clone(),
            scope: file.scope,
            op: EditOp::Restore {
                snapshot_id: snapshot_id.to_string(),
            },
            expected_hash: file.after_hash.clone(),
            applies: file.applies,
        }],
    };
    let preview = st.0.config.preview_shown(cs, &masked_diff(agent, &Cell::new(false)));
    let preview = changes_of(preview)?;
    let apply = ApplyRequest {
        changeset_id: preview.changeset_id.clone(),
        expected_hashes: preview
            .files
            .iter()
            .map(|f| ExpectedHash {
                path: f.path.clone(),
                hash: f.before_hash.clone(),
            })
            .collect(),
    };
    match st.0.config.apply(&apply, now_ms()) {
        Ok(result) => Ok(result),
        Err(ApplyError::Conflict { paths, .. }) => Err(changed_since(paths)),
        Err(ApplyError::UnknownChangeSet) => Err(changed_since(vec![file.path.clone()])),
        Err(ApplyError::Failed(e)) => Err(Refusal::from(failed(format!("{e:#}")))),
    }
}

pub async fn plan(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
    Json(want): Json<HookPlanRequest>,
) -> Result<Json<ConfigPreview>, Fail> {
    let adapter = adapter(&agent)?;
    let preview = tokio::task::spawn_blocking(move || {
        let mut changes = match adapter {
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
        if want.skill {
            // A skill Marshell can't touch is left out: it must not block the hooks.
            let path = st.skill_file(&agent)?;
            if let Ok(bytes) = st.skill_bytes(&path) {
                changes.edits.extend(skill::install_edit(&path, bytes.as_deref()));
            }
        }
        Ok::<_, Fail>(st.0.config.preview(changes))
    })
    .await
    .map_err(failed)??;
    Ok(Json(preview))
}

/// Installs, or updates, the `marshell` skill on its own. An installed skill gives a preview with no files.
pub async fn skill_plan(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
) -> Result<Json<ConfigPreview>, Fail> {
    let adapter = adapter(&agent)?;
    let preview = tokio::task::spawn_blocking(move || {
        let path = st.skill_file(&agent)?;
        let bytes = st.skill_bytes(&path)?;
        let changes = skill::install_plan(adapter.cli_name(), &path, bytes.as_deref());
        Ok::<_, Fail>(st.0.config.preview(changes))
    })
    .await
    .map_err(failed)??;
    Ok(Json(preview))
}

/// Takes the `marshell` skill out. A missing file gives a preview with no files.
pub async fn skill_remove_plan(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
) -> Result<Json<ConfigPreview>, Fail> {
    let adapter = adapter(&agent)?;
    let preview = tokio::task::spawn_blocking(move || {
        let path = st.skill_file(&agent)?;
        let bytes = st.skill_bytes(&path)?;
        if skill::state_of(bytes.as_deref()) == SkillInstall::Edited {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!(
                    "{} was edited after Marshell installed it, so Marshell leaves it.",
                    path.display()
                ),
            ));
        }
        let changes = skill::remove_plan(adapter.cli_name(), &path, bytes.as_deref());
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
