mod agents;
mod attachments;
mod config;
mod events;
mod files;
mod folders;
mod foreground;
pub mod frames;
mod guard;
mod hook;
mod layout;
mod permissions;
mod profiles;
mod projects;
mod pty_ws;
mod reports;
mod sessions;
mod settings;
mod shell_panel;
mod tasks;

use crate::profiles::detect::{Probe, detect_profiles};
use crate::registry::{Registry, Relaunch};
use crate::{auth, paths, private};
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, Method, header};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use marshell_protocol::api::{Endpoint, Health, LayoutDoc, Platform, Profile, ProfilesDoc, Settings};
use marshell_protocol::brand;
use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Clone)]
pub struct CoreConfig {
    /// Usually `paths::home_dir()`; a temp dir in tests.
    pub home: PathBuf,
    /// Webview origins allowed to call the API, e.g. `http://tauri.localhost`.
    pub allowed_origins: Vec<String>,
}

/// Shared server state. Cloning is cheap (one `Arc`).
#[derive(Clone)]
pub struct AppState(Arc<Inner>);

struct Inner {
    token: String,
    host: String,
    allowed_origins: Vec<String>,
    /// Where `profiles.json` lives.
    home: PathBuf,
    /// Where the shell integration scripts were written. None when writing them failed,
    /// so shells start without integration.
    shell_dir: Option<PathBuf>,
    /// The shells found at startup. Detection runs once, not per request.
    detected: Vec<Profile>,
    /// Detected profiles merged with `profiles.json`.
    profiles: Mutex<ProfilesDoc>,
    /// The contents of `settings.json`, or the defaults.
    settings: Mutex<Settings>,
    /// The contents of `layout.json`, or the defaults.
    layout: Mutex<LayoutDoc>,
    /// Open and recently closed sessions. Shared with each session's exit callback.
    registry: Arc<Registry>,
    // Kept here so every handler reaches it through one State.
    sessions: Mutex<HashMap<String, Arc<crate::pty::Session>>>,
    /// Tabs a Resume or a Restart is starting a process for. One start per tab at a time, so two
    /// cannot both put a process in `sessions` and leave the first running unseen.
    starting: Mutex<HashSet<String>>,
    /// The sidebar's shell, while there is one. It is in `sessions` but has no registry row.
    shell_panel: Mutex<Option<shell_panel::PanelShell>>,
    /// Wakes the foreground watcher when a command starts, so it does not wait out its second.
    /// An `Arc` so a session's mark callback can hold it without holding the state.
    foreground_wake: Arc<tokio::sync::Notify>,
    /// Whether an agent's command (`claude`, `codex`, `agy`) runs from PATH. Tests swap it.
    agent_on_path: fn(&str) -> bool,
    /// The file an agent's command runs from PATH, for its setup and version. Tests swap it.
    agent_path: fn(&str) -> Option<PathBuf>,
    /// The last events the bridge sent, per live tab.
    hook_log: crate::hooks::log::HookLog,
    /// The permission requests being held for a decision.
    holds: crate::hooks::holds::Holds,
    /// How long the core holds a permission request before it lets the agent ask.
    permission_hold: Duration,
    /// Previews and applies changes to the CLIs' config files.
    config: crate::config::service::ConfigService,
    /// Claude Code's settings file when a test or a caller names one; None means
    /// `MARSHELL_CLAUDE_SETTINGS`, else `~/.claude/settings.json`.
    claude_settings: Option<PathBuf>,
    /// `--version` of each agent executable, asked once per path for the app's lifetime.
    versions: Mutex<HashMap<PathBuf, Option<String>>>,
}

/// How long a permission request is held when `MARSHELL_PERMISSION_HOLD_SECS` is not set.
pub const DEFAULT_PERMISSION_HOLD: Duration = Duration::from_secs(120);

impl AppState {
    pub fn new(token: String, port: u16, allowed_origins: Vec<String>, home: PathBuf, detected: Vec<Profile>) -> Self {
        let profiles = crate::profiles::store::load(&home, detected.clone());
        let settings = crate::settings::load(&home);
        let layout = crate::layout::load(&home);
        // The tabs of the last run come back as Ended rows. Nothing is started, and this runs before
        // the saver subscribes, so seeding never writes the file.
        let registry = Registry::new();
        registry.seed_ended(&layout.tabs, |tab| Relaunch {
            profile_id: tab.profile_id.clone(),
            agent: tab.agent.clone(),
            cwd: tab.cwd.clone(),
            mode: tab.mode,
            name: tab.name.clone(),
        });
        let shell_dir = match crate::shell::install(&home) {
            Ok(dir) => Some(dir),
            Err(err) => {
                tracing::warn!("shell integration scripts were not written: {err:#}");
                None
            }
        };
        let config = crate::config::service::ConfigService::new(home.clone());
        Self(Arc::new(Inner {
            token,
            host: format!("127.0.0.1:{port}"),
            allowed_origins,
            home,
            shell_dir,
            detected,
            profiles: Mutex::new(profiles),
            settings: Mutex::new(settings),
            layout: Mutex::new(layout),
            registry: Arc::new(registry),
            sessions: Mutex::new(HashMap::new()),
            starting: Mutex::new(HashSet::new()),
            shell_panel: Mutex::new(None),
            foreground_wake: Arc::new(tokio::sync::Notify::new()),
            agent_on_path: crate::profiles::detect::command_on_path,
            agent_path: crate::profiles::detect::command_path,
            hook_log: crate::hooks::log::HookLog::default(),
            holds: crate::hooks::holds::Holds::default(),
            permission_hold: DEFAULT_PERMISSION_HOLD,
            config,
            claude_settings: None,
            versions: Mutex::new(HashMap::new()),
        }))
    }
    /// Names Claude Code's settings file, in place of the environment. Call it before the state is cloned.
    pub fn with_claude_settings(mut self, path: PathBuf) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_claude_settings runs before the state is shared")
            .claude_settings = Some(path);
        self
    }
    /// Sets how long a permission request is held. Call it before the state is cloned.
    pub fn with_permission_hold(mut self, hold: Duration) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_permission_hold runs before the state is shared")
            .permission_hold = hold;
        self
    }
    pub fn permission_hold(&self) -> Duration {
        self.0.permission_hold
    }
    pub fn holds(&self) -> &crate::hooks::holds::Holds {
        &self.0.holds
    }
    /// Replaces the PATH check for agent commands. Call it before the state is cloned.
    pub fn with_agent_check(mut self, check: fn(&str) -> bool) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_agent_check runs before the state is shared")
            .agent_on_path = check;
        self
    }
    /// Replaces the PATH lookup of an agent's file. Call it before the state is cloned.
    pub fn with_agent_path(mut self, find: fn(&str) -> Option<PathBuf>) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_agent_path runs before the state is shared")
            .agent_path = find;
        self
    }
    /// The folder holding `profiles.json`, `tasks.json` and the other files Marshell keeps.
    pub fn home(&self) -> &std::path::Path {
        &self.0.home
    }
    /// The open and recently closed sessions.
    pub fn registry(&self) -> &Registry {
        &self.0.registry
    }
    pub fn config(&self) -> &crate::config::service::ConfigService {
        &self.0.config
    }
    pub fn hook_log(&self) -> &crate::hooks::log::HookLog {
        &self.0.hook_log
    }
    pub fn token(&self) -> &str {
        &self.0.token
    }
    pub fn host(&self) -> &str {
        &self.0.host
    }
    pub fn session(&self, tab: &str) -> Option<Arc<crate::pty::Session>> {
        self.0.sessions.lock().unwrap().get(tab).cloned()
    }
    /// How many sessions have a running process. Ended tabs have none.
    pub fn live_session_count(&self) -> usize {
        self.0.sessions.lock().unwrap().len()
    }
    pub fn insert_session(&self, session: Arc<crate::pty::Session>) {
        self.0
            .sessions
            .lock()
            .unwrap()
            .insert(session.id().to_string(), session);
    }
    /// Claims the tab for one Resume or Restart. None while another one is starting it.
    /// The claim ends when the returned value is dropped.
    pub fn claim_start(&self, tab: &str) -> Option<StartClaim> {
        if !self.0.starting.lock().unwrap().insert(tab.to_string()) {
            return None;
        }
        Some(StartClaim {
            state: self.clone(),
            tab: tab.to_string(),
        })
    }
    /// Forgets the session so no new socket can attach; the caller kills it. Its held permission
    /// requests and hook events go too.
    pub fn remove_session(&self, tab: &str) -> Option<Arc<crate::pty::Session>> {
        let removed = self.0.sessions.lock().unwrap().remove(tab);
        self.holds().release_tab(tab);
        self.hook_log().forget_tab(tab);
        removed
    }
    pub fn origin_allowed(&self, origin: &str) -> bool {
        self.0.allowed_origins.iter().any(|o| o == origin)
    }
}

/// A tab's claim on starting its process, from `AppState::claim_start`. Dropping it ends the claim.
pub struct StartClaim {
    state: AppState,
    tab: String,
}

impl Drop for StartClaim {
    fn drop(&mut self) {
        self.state.0.starting.lock().unwrap().remove(&self.tab);
    }
}

pub fn router(state: AppState) -> Router {
    // Here and not in `AppState::new`, which `with_agent_check` needs to be the only owner of.
    layout::spawn_layout_saver(&state);
    foreground::spawn_watcher(&state);
    let origins: Vec<HeaderValue> = state.0.allowed_origins.iter().filter_map(|o| o.parse().ok()).collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        // The webview is another origin and reads this from a content reply.
        .expose_headers([HeaderName::from_static("x-modified-ms")]);
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/profiles", get(profiles::get).put(profiles::put))
        .route("/v1/profiles/detected", get(profiles::detected))
        .route("/v1/profiles/integration", post(profiles::integration))
        .route("/v1/settings", get(settings::get).put(settings::put))
        .route("/v1/layout", get(layout::get))
        .route("/v1/layout/view", put(layout::put_view))
        .route("/v1/import/windows-terminal", get(settings::windows_terminal))
        .route("/v1/projects", get(projects::get))
        .route("/v1/folders/check", get(folders::check))
        .route("/v1/tasks", get(tasks::list).post(tasks::create))
        .route("/v1/tasks/plan", post(tasks::plan))
        .route("/v1/tasks/{id}/changes", get(tasks::changes))
        .route("/v1/tasks/{id}/merge/plan", post(tasks::merge_plan))
        .route("/v1/tasks/{id}/merge", post(tasks::merge))
        .route("/v1/tasks/{id}/remove/plan", post(tasks::remove_plan))
        .route("/v1/tasks/{id}/remove", post(tasks::remove))
        .route("/v1/sessions", get(sessions::list).post(sessions::create))
        .route("/v1/sessions/reorder", post(sessions::reorder))
        .route("/v1/sessions/reopen", post(sessions::reopen))
        .route("/v1/sessions/{tab}/resume", post(sessions::resume))
        .route("/v1/sessions/{tab}/restart", post(sessions::restart))
        .route("/v1/sessions/{tab}/pause", post(sessions::pause))
        .route("/v1/sessions/{tab}/continue", post(sessions::cont))
        .route("/v1/sessions/{tab}/input", post(sessions::input))
        .route("/v1/sessions/{tab}/recording", get(sessions::recording))
        .route("/v1/sessions/{tab}/resolve", post(files::resolve))
        .route("/v1/sessions/{tab}/files", get(files::list))
        .route(
            "/v1/sessions/{tab}/attachments",
            post(attachments::save).layer(DefaultBodyLimit::max(files::IMAGE_CAP as usize + 1)),
        )
        .route("/v1/sessions/{tab}/paste", post(attachments::paste))
        .route("/v1/sessions/{tab}/paste-paths", post(attachments::paste_paths))
        .route("/v1/files/stat", get(files::stat))
        .route("/v1/files/content", get(files::content))
        .route("/v1/sessions/{tab}", patch(sessions::rename).delete(sessions::close))
        .route("/v1/shell-panel", post(shell_panel::open))
        .route("/v1/shell-panel/adopt", post(shell_panel::adopt))
        .route(
            "/v1/hook",
            post(hook::post).layer(DefaultBodyLimit::max(marshell_protocol::hook::HOOK_BODY_LIMIT)),
        )
        .route("/v1/hook/events", get(hook::events))
        .route("/v1/permissions", get(permissions::list))
        .route("/v1/permissions/{rid}/decision", post(permissions::decision))
        .route("/v1/permissions/{rid}/release", post(permissions::release))
        .route("/v1/config/file", get(config::file))
        .route("/v1/config/preview", post(config::preview))
        .route("/v1/config/preview/{id}", get(config::get_preview))
        .route("/v1/config/apply", post(config::apply))
        .route("/v1/snapshots", get(config::snapshots))
        .route("/v1/snapshots/{id}/restore", post(config::restore))
        .route("/v1/agents/{agent}/setup", get(agents::setup))
        .route("/v1/agents/{agent}/hooks/plan", post(agents::plan))
        .route("/v1/agents/{agent}/hooks/remove-plan", post(agents::remove_plan))
        .route("/v1/events", get(events::handler))
        .route("/v1/pty/{tab}", get(pty_ws::handler))
        .layer(axum::middleware::from_fn_with_state(state.clone(), guard::auth_guard))
        .layer(cors)
        // Added last = outermost: Host is checked before CORS can answer a preflight.
        .layer(axum::middleware::from_fn_with_state(state.clone(), guard::host_guard))
        .with_state(state)
}

async fn health() -> Json<Health> {
    Json(Health {
        app: brand::APP_NAME.into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

pub struct RunningCore {
    pub endpoint: Endpoint,
    pub state: AppState,
    shutdown: tokio::sync::oneshot::Sender<()>,
    serve: tokio::task::JoinHandle<()>,
}

impl RunningCore {
    /// False once the server task has stopped, or when a panic left the sessions lock poisoned.
    pub fn is_healthy(&self) -> bool {
        !self.serve.is_finished() && !self.state.0.sessions.is_poisoned()
    }

    /// Kills every session's process tree, then stops the server. Works on a poisoned lock too.
    pub fn shutdown(self) {
        let sessions: Vec<_> = self
            .state
            .0
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        for s in sessions {
            s.force_kill();
        }
        let _ = self.shutdown.send(());
    }

    /// Panics in another thread while it holds the sessions lock, so the lock is poisoned.
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        std::thread::scope(|s| {
            let _ = s
                .spawn(|| {
                    let _held = self.state.0.sessions.lock();
                    panic!("poisoning the sessions lock for a test");
                })
                .join();
        });
    }

    /// Stops the server task as a crash would; the sessions keep running.
    #[cfg(test)]
    pub(crate) fn stop_server_for_test(&self) {
        self.serve.abort();
    }
}

/// `MARSHELL_PERMISSION_HOLD_SECS` clamped to 1..=590 s, or the default when unset or not a number.
fn permission_hold_from_env(value: Option<&str>) -> Duration {
    match value.and_then(|v| v.trim().parse::<u64>().ok()) {
        Some(secs) => Duration::from_secs(secs.clamp(1, 590)),
        None => DEFAULT_PERMISSION_HOLD,
    }
}

/// Binds 127.0.0.1 on a random port, writes `run/endpoint.json`, serves in the background.
pub async fn start(cfg: CoreConfig) -> anyhow::Result<RunningCore> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let endpoint = Endpoint {
        port,
        token: auth::new_token(),
        platform: Platform::current(),
    };
    // Looking for shells runs programs (wsl.exe can take up to 2 s): keep it off the async threads.
    let detected = tokio::task::spawn_blocking(|| detect_profiles(&Probe::system())).await?;
    // Before anything is written under the home. A failure is not fatal: the default home sits under the
    // user's profile, which is already private to the user. The first fix may walk the whole home, so it
    // runs off the async threads.
    let home = cfg.home.clone();
    match tokio::task::spawn_blocking(move || private::secure_home(&home)).await? {
        Ok(private::HomeAcl::Fixed) => tracing::info!("made the Marshell home owner-only"),
        Ok(private::HomeAcl::AlreadyPrivate) => {}
        Err(e) => tracing::warn!("could not make the Marshell home owner-only: {e:#}"),
    }
    let endpoint_file = paths::endpoint_file(&cfg.home);
    let state = AppState::new(endpoint.token.clone(), port, cfg.allowed_origins, cfg.home, detected)
        .with_permission_hold(permission_hold_from_env(
            std::env::var("MARSHELL_PERMISSION_HOLD_SECS").ok().as_deref(),
        ));
    // Last, so the endpoint is only advertised once the state is ready to serve.
    auth::write_endpoint_file(&endpoint_file, &endpoint)?;
    let app = router(state.clone());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let serve = tokio::spawn(async move {
        let serve = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = rx.await;
        });
        if let Err(e) = serve.await {
            tracing::error!("core server stopped: {e}");
        }
    });
    Ok(RunningCore {
        endpoint,
        state,
        shutdown: tx,
        serve,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_hold_env_is_clamped() {
        assert_eq!(permission_hold_from_env(None), DEFAULT_PERMISSION_HOLD);
        assert_eq!(permission_hold_from_env(Some("abc")), DEFAULT_PERMISSION_HOLD);
        assert_eq!(permission_hold_from_env(Some("0")), Duration::from_secs(1));
        assert_eq!(permission_hold_from_env(Some("45")), Duration::from_secs(45));
        assert_eq!(permission_hold_from_env(Some("9999")), Duration::from_secs(590));
    }
}
