//! What Marshell knows about an agent's hook setup, and the previews that install and remove it.
//! Only `claude` has an adapter; any other agent is a 404. Applying goes through `/v1/config/apply`.

use super::AppState;
use crate::agents::claude::{self, hooks};
use crate::config::jsonc::{parse_problem, read_value};
use crate::paths;
use crate::profiles::detect::agent_exe;
use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use marshell_protocol::agents::{AgentSetup, HookInstall, HookPlanRequest, ParseProblem, RemovePreview, SettingsState};
use marshell_protocol::config::ConfigPreview;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

    /// `--version` of `exe`, asked once per path while the app runs.
    fn version_of(&self, exe: &Path) -> Option<String> {
        if let Some(known) = self.0.versions.lock().unwrap().get(exe) {
            return known.clone();
        }
        let version = claude::run_version(exe, VERSION_TIMEOUT);
        self.0
            .versions
            .lock()
            .unwrap()
            .insert(exe.to_path_buf(), version.clone());
        version
    }
}

fn setup_of(st: &AppState) -> Result<AgentSetup, Fail> {
    let home = &st.0.home;
    let user_home = super::files::home_dir().ok_or_else(|| failed("the user's home folder is not known"))?;
    let settings_path = st.claude_settings_path(&user_home);

    // A profile that runs `claude` itself names the file; sessions otherwise start it from PATH.
    let exe = agent_exe(&st.0.profiles.lock().unwrap().profiles, AGENT).or_else(|| (st.0.agent_path)(AGENT));
    let version = exe.as_deref().and_then(|exe| st.version_of(exe));

    let mut setup = AgentSetup {
        agent: AGENT.to_string(),
        found: exe.is_some(),
        version,
        exe_path: exe.map(|p| p.to_string_lossy().into_owned()),
        settings_path: settings_path.to_string_lossy().into_owned(),
        settings_state: SettingsState::Ok,
        parse_error: None,
        hooks: HookInstall::None,
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

pub async fn plan(
    State(st): State<AppState>,
    UrlPath(agent): UrlPath<String>,
    Json(want): Json<HookPlanRequest>,
) -> Result<Json<ConfigPreview>, Fail> {
    only_claude(&agent)?;
    let preview = tokio::task::spawn_blocking(move || {
        let inputs = st.inputs()?;
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
