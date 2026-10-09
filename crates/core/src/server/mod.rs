mod guard;

use crate::{auth, paths};
use axum::http::{header, HeaderValue, Method};
use axum::routing::get;
use axum::{Json, Router};
use marshell_protocol::api::{Endpoint, Health, Platform};
use marshell_protocol::brand;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower_http::cors::{AllowOrigin, CorsLayer};

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
    // Filled in Task 6. Kept here so every handler reaches it through one State.
    #[allow(dead_code)]
    sessions: Mutex<HashMap<String, Arc<crate::pty::Session>>>,
}

impl AppState {
    pub fn new(token: String, port: u16, allowed_origins: Vec<String>) -> Self {
        Self(Arc::new(Inner {
            token,
            host: format!("127.0.0.1:{port}"),
            allowed_origins,
            sessions: Mutex::new(HashMap::new()),
        }))
    }
    pub fn token(&self) -> &str {
        &self.0.token
    }
    pub fn host(&self) -> &str {
        &self.0.host
    }
    pub fn origin_allowed(&self, origin: &str) -> bool {
        self.0.allowed_origins.iter().any(|o| o == origin)
    }
}

pub fn router(state: AppState) -> Router {
    let origins: Vec<HeaderValue> = state.0.allowed_origins.iter().filter_map(|o| o.parse().ok()).collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    Router::new()
        .route("/v1/health", get(health))
        .layer(axum::middleware::from_fn_with_state(state.clone(), guard::auth_guard))
        .layer(cors)
        // Added last = outermost: Host is checked before CORS can answer a preflight.
        .layer(axum::middleware::from_fn_with_state(state.clone(), guard::host_guard))
        .with_state(state)
}

async fn health() -> Json<Health> {
    Json(Health { app: brand::APP_NAME.into(), version: env!("CARGO_PKG_VERSION").into() })
}

pub struct RunningCore {
    pub endpoint: Endpoint,
    pub state: AppState,
    shutdown: tokio::sync::oneshot::Sender<()>,
}

impl RunningCore {
    /// Kills every session's process tree, then stops the server.
    pub fn shutdown(self) {
        let sessions: Vec<_> = self.state.0.sessions.lock().unwrap().values().cloned().collect();
        for s in sessions {
            s.force_kill();
        }
        let _ = self.shutdown.send(());
    }
}

/// Binds 127.0.0.1 on a random port, writes `run/endpoint.json`, serves in the background.
pub async fn start(cfg: CoreConfig) -> anyhow::Result<RunningCore> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let endpoint = Endpoint { port, token: auth::new_token(), platform: Platform::current() };
    auth::write_endpoint_file(&paths::endpoint_file(&cfg.home), &endpoint)?;
    let state = AppState::new(endpoint.token.clone(), port, cfg.allowed_origins);
    let app = router(state.clone());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let serve = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = rx.await;
        });
        if let Err(e) = serve.await {
            tracing::error!("core server stopped: {e}");
        }
    });
    Ok(RunningCore { endpoint, state, shutdown: tx })
}
