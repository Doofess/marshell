//! What Marshell knows about an agent's hook setup, and the previews that install and remove it.
//! Only `claude` has an adapter; any other agent is a 404. Applying goes through `/v1/config/apply`.

use super::AppState;
use crate::agents::claude::{self, hooks};
use crate::agents::{parse_any_version, run_version_text};
use crate::bridge;
use crate::config::jsonc::{parse_problem, read_value};
use crate::paths;
use crate::profiles::detect::{agent_command, agent_exe};
use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use marshell_protocol::agents::{
    AgentFound, AgentSetup, HookInstall, HookPlanRequest, ParseProblem, RemovePreview, SettingsState,
};
use marshell_protocol::config::ConfigPreview;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

const AGENT: &str = "claude";
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// A status and the message the route replies with.
type Fail = (StatusCode, String);

fn failed(err: impl std::fmt::Display) -> Fail {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

fn only_claude(agent: &str) -> Result<(), Fail> {
    if agent == AGENT {
        Ok(())
    } else {
        Err((StatusCode::NOT_FOUND, "no such agent".to_string()))
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

impl AppState {
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
        agent_exe(&self.0.profiles.lock().unwrap().profiles, agent).or_else(|| (self.0.agent_path)(agent))
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
        } else {
            exe.and_then(|exe| self.version_of(exe, parse_any_version))
        }
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
        settings_state: SettingsState::Ok,
        parse_error: None,
        hooks: HookInstall::None,
        approve_installed: false,
        has_own_statusline: false,
        chained: false,
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

pub async fn setup(State(st): State<AppState>, UrlPath(agent): UrlPath<String>) -> Result<Json<AgentSetup>, Fail> {
    only_claude(&agent)?;
    let setup = tokio::task::spawn_blocking(move || setup_of(&st))
        .await
        .map_err(failed)??;
    Ok(Json(setup))
}

/// Which of the agent CLIs Marshell can start are on this machine, in the order claude, codex, antigravity.
pub async fn found(State(st): State<AppState>) -> Result<Json<Vec<AgentFound>>, Fail> {
    let list = tokio::task::spawn_blocking(move || {
        let profiles = st.0.profiles.lock().unwrap().profiles.clone();
        ["claude", "codex", "antigravity"]
            .into_iter()
            .map(|agent| {
                let exe =
                    agent_exe(&profiles, agent).or_else(|| agent_command(agent).and_then(|c| (st.0.agent_path)(c)));
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

pub async fn plan(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
    Json(want): Json<HookPlanRequest>,
) -> Result<Json<ConfigPreview>, Fail> {
    only_claude(&agent)?;
    let preview = tokio::task::spawn_blocking(move || {
        let inputs = st.inputs()?;
        st.ensure_bridge(&want)?;
        let changes = hooks::install_plan(
            &inputs.settings_path,
            &inputs.user_home,
            inputs.text.as_deref(),
            inputs.chain_text.as_deref(),
            &st.0.home,
            &want,
        )
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
    only_claude(&agent)?;
    let removal = tokio::task::spawn_blocking(move || {
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
}
