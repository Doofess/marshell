mod events;
pub mod frames;
mod guard;
mod profiles;
mod projects;
mod pty_ws;
mod sessions;
mod settings;

use crate::profiles::detect::{Probe, detect_profiles};
use crate::registry::Registry;
use crate::{auth, paths};
use axum::http::{HeaderValue, Method, header};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use marshell_protocol::api::{Endpoint, Health, Platform, Profile, ProfilesDoc, Settings};
use marshell_protocol::brand;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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
    /// Open and recently closed sessions. Shared with each session's exit callback.
    registry: Arc<Registry>,
    // Kept here so every handler reaches it through one State.
    sessions: Mutex<HashMap<String, Arc<crate::pty::Session>>>,
    /// Whether an agent's command (`claude`, `codex`, `agy`) runs from PATH. Tests swap it.
    agent_on_path: fn(&str) -> bool,
}

impl AppState {
    pub fn new(token: String, port: u16, allowed_origins: Vec<String>, home: PathBuf, detected: Vec<Profile>) -> Self {
        let profiles = crate::profiles::store::load(&home, detected.clone());
        let settings = crate::settings::load(&home);
        let shell_dir = match crate::shell::install(&home) {
            Ok(dir) => Some(dir),
            Err(err) => {
                tracing::warn!("shell integration scripts were not written: {err:#}");
                None
            }
        };
        Self(Arc::new(Inner {
            token,
            host: format!("127.0.0.1:{port}"),
            allowed_origins,
            home,
            shell_dir,
            detected,
            profiles: Mutex::new(profiles),
            settings: Mutex::new(settings),
            registry: Arc::new(Registry::new()),
            sessions: Mutex::new(HashMap::new()),
            agent_on_path: crate::profiles::detect::command_on_path,
        }))
    }
    /// Replaces the PATH check for agent commands. Call it before the state is cloned.
    pub fn with_agent_check(mut self, check: fn(&str) -> bool) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("with_agent_check runs before the state is shared")
            .agent_on_path = check;
        self
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
    pub fn insert_session(&self, session: Arc<crate::pty::Session>) {
        self.0
            .sessions
            .lock()
            .unwrap()
            .insert(session.id().to_string(), session);
    }
    /// Forgets the session so no new socket can attach; the caller kills it.
    pub fn remove_session(&self, tab: &str) -> Option<Arc<crate::pty::Session>> {
        self.0.sessions.lock().unwrap().remove(tab)
    }
    pub fn origin_allowed(&self, origin: &str) -> bool {
        self.0.allowed_origins.iter().any(|o| o == origin)
    }
}

pub fn router(state: AppState) -> Router {
    let origins: Vec<HeaderValue> = state.0.allowed_origins.iter().filter_map(|o| o.parse().ok()).collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/profiles", get(profiles::get).put(profiles::put))
        .route("/v1/profiles/detected", get(profiles::detected))
        .route("/v1/profiles/integration", post(profiles::integration))
        .route("/v1/settings", get(settings::get).put(settings::put))
        .route("/v1/import/windows-terminal", get(settings::windows_terminal))
        .route("/v1/projects", get(projects::get))
        .route("/v1/sessions", get(sessions::list).post(sessions::create))
        .route("/v1/sessions/reorder", post(sessions::reorder))
        .route("/v1/sessions/reopen", post(sessions::reopen))
        .route("/v1/sessions/{tab}", patch(sessions::rename).delete(sessions::close))
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
    let endpoint_file = paths::endpoint_file(&cfg.home);
    let state = AppState::new(endpoint.token.clone(), port, cfg.allowed_origins, cfg.home, detected);
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
