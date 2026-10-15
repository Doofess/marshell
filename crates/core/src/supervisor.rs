//! Keeps the core running. When its server stops or a panic poisons its state, the supervisor
//! ends the old core's sessions and starts a new core on a new port and token. After three
//! restarts within a minute it gives up and reports `Failed`.

use crate::server::{self, CoreConfig, RunningCore};
use marshell_protocol::api::Endpoint;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const MAX_RESTARTS: usize = 3;
const RESTART_WINDOW: Duration = Duration::from_secs(60);

/// Serialised as the window's event payload: `{"generation": n}` for a restart, `null` for `Failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum CoreEvent {
    /// A new core is running; `generation` counts restarts since launch.
    Restarted { generation: u32 },
    /// Too many restarts in a minute, so the supervisor stopped trying.
    Failed,
}

pub struct Supervisor {
    cfg: CoreConfig,
    current: Mutex<Option<RunningCore>>,
    generation: AtomicU32,
    /// When each recent restart was attempted, for the three-per-minute limit.
    restarts: Mutex<Vec<Instant>>,
    /// Set by `Failed` or `shutdown`; nothing restarts after that.
    stopped: AtomicBool,
    on_event: Box<dyn Fn(CoreEvent) + Send + Sync>,
}

/// The supervisor's own locks are never held across a panic, but a poisoned one should not stop it either.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Supervisor {
    /// Starts the first core. `on_event` is called on every restart and on `Failed`.
    pub async fn start(
        cfg: CoreConfig,
        on_event: impl Fn(CoreEvent) + Send + Sync + 'static,
    ) -> anyhow::Result<Arc<Supervisor>> {
        let core = server::start(cfg.clone()).await?;
        Ok(Arc::new(Supervisor {
            cfg,
            current: Mutex::new(Some(core)),
            generation: AtomicU32::new(0),
            restarts: Mutex::new(Vec::new()),
            stopped: AtomicBool::new(false),
            on_event: Box::new(on_event),
        }))
    }

    /// The running core's endpoint; `None` after `Failed` or `shutdown`, or while a restart is pending.
    pub fn endpoint(&self) -> Option<Endpoint> {
        lock(&self.current).as_ref().map(|c| c.endpoint.clone())
    }

    /// 0 at launch, +1 per restart.
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Restarts the core if it is unhealthy. Returns true when a new core was started.
    /// A start that fails still counts toward the limit, and the next check tries again.
    pub async fn check_and_restart(&self) -> bool {
        if self.stopped.load(Ordering::SeqCst) {
            return false;
        }
        let old = {
            let mut current = lock(&self.current);
            if current.as_ref().is_some_and(|c| c.is_healthy()) {
                return false;
            }
            current.take()
        };
        if let Some(old) = old {
            tracing::error!("the core stopped working; ending its sessions and starting a new one");
            old.shutdown();
        }
        {
            let mut restarts = lock(&self.restarts);
            restarts.retain(|at| at.elapsed() < RESTART_WINDOW);
            if restarts.len() >= MAX_RESTARTS {
                tracing::error!("the core restarted {MAX_RESTARTS} times within a minute; giving up");
                self.stopped.store(true, Ordering::SeqCst);
                (self.on_event)(CoreEvent::Failed);
                return false;
            }
            restarts.push(Instant::now());
        }
        let core = match server::start(self.cfg.clone()).await {
            Ok(core) => core,
            Err(e) => {
                tracing::error!("the core could not restart: {e:#}");
                return false;
            }
        };
        if self.stopped.load(Ordering::SeqCst) {
            // `shutdown` was called while the new core was starting.
            core.shutdown();
            return false;
        }
        *lock(&self.current) = Some(core);
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        (self.on_event)(CoreEvent::Restarted { generation });
        true
    }

    /// Checks the core every `every` in a background task, until `Failed` or `shutdown`.
    /// Call it inside the tokio runtime.
    pub fn watch(self: &Arc<Self>, every: Duration) {
        let me = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(every);
            while !me.stopped.load(Ordering::SeqCst) {
                tick.tick().await;
                me.check_and_restart().await;
            }
        });
    }

    /// Ends the current core's sessions and stops it. Nothing restarts after this.
    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(core) = lock(&self.current).take() {
            core.shutdown();
        }
    }

    #[cfg(test)]
    fn current_for_test(&self) -> MutexGuard<'_, Option<RunningCore>> {
        lock(&self.current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::{Pull, Session, SpawnSpec};

    type Events = Arc<Mutex<Vec<CoreEvent>>>;

    async fn started() -> (tempfile::TempDir, Arc<Supervisor>, Events) {
        let home = tempfile::tempdir().unwrap();
        let cfg = CoreConfig {
            home: home.path().to_path_buf(),
            allowed_origins: vec![],
        };
        let events: Events = Arc::default();
        let sink = events.clone();
        let sup = Supervisor::start(cfg, move |e| sink.lock().unwrap().push(e))
            .await
            .unwrap();
        (home, sup, events)
    }

    async fn stop_server(sup: &Supervisor) {
        sup.current_for_test().as_ref().unwrap().stop_server_for_test();
        let deadline = Instant::now() + Duration::from_secs(2);
        while sup.current_for_test().as_ref().unwrap().is_healthy() {
            assert!(Instant::now() < deadline, "the server did not stop");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn long_running() -> SpawnSpec {
        let argv: &[&str] = if cfg!(windows) {
            &["cmd", "/c", "ping -n 30 127.0.0.1 > nul"]
        } else {
            &["sh", "-c", "sleep 30"]
        };
        SpawnSpec {
            argv: argv.iter().map(|s| s.to_string()).collect(),
            cwd: None,
            env: vec![],
            cols: 80,
            rows: 24,
            on_exit: None,
        }
    }

    async fn exits_within(session: &Session, timeout: Duration) -> bool {
        let generation = session.attach();
        session.resume(0);
        let deadline = Instant::now() + timeout;
        let mut cursor = 0;
        let exited = loop {
            match session.pull(cursor, 64 * 1024) {
                Pull::Data { seq, bytes } => cursor = seq + bytes.len() as u64,
                Pull::Reset { seq, .. } => cursor = seq,
                Pull::Exit(_) => break true,
                Pull::Idle if Instant::now() > deadline => break false,
                Pull::Idle => tokio::time::sleep(Duration::from_millis(10)).await,
            }
            session.ack(cursor);
        };
        session.detach(generation);
        exited
    }

    #[test]
    fn event_payloads_match_the_window_events() {
        let restarted = serde_json::to_value(CoreEvent::Restarted { generation: 2 }).unwrap();
        assert_eq!(restarted, serde_json::json!({ "generation": 2 }));
        assert_eq!(
            serde_json::to_value(CoreEvent::Failed).unwrap(),
            serde_json::Value::Null
        );
    }

    #[tokio::test]
    async fn healthy_core_is_left_alone() {
        let (_home, sup, events) = started().await;
        assert!(!sup.check_and_restart().await);
        assert_eq!(sup.generation(), 0);
        assert!(events.lock().unwrap().is_empty());
        sup.shutdown();
    }

    #[tokio::test]
    async fn restarts_after_server_stops() {
        let (home, sup, events) = started().await;
        let old = sup.endpoint().unwrap();
        stop_server(&sup).await;
        assert!(sup.check_and_restart().await);
        assert_eq!(sup.generation(), 1);
        let new = sup.endpoint().unwrap();
        assert_ne!(new.port, old.port);
        assert_ne!(new.token, old.token);
        let on_disk: serde_json::Value =
            serde_json::from_slice(&std::fs::read(crate::paths::endpoint_file(home.path())).unwrap()).unwrap();
        assert_eq!(on_disk["token"], new.token);
        assert_eq!(*events.lock().unwrap(), [CoreEvent::Restarted { generation: 1 }]);
        sup.shutdown();
    }

    #[tokio::test]
    async fn restarts_when_state_is_poisoned() {
        let (_home, sup, _events) = started().await;
        let session = Session::spawn("tpoison".into(), long_running()).unwrap();
        {
            let current = sup.current_for_test();
            let core = current.as_ref().unwrap();
            core.state.insert_session(session.clone());
            core.poison_for_test();
            assert!(!core.is_healthy());
        }
        assert!(sup.check_and_restart().await);
        assert!(
            exits_within(&session, Duration::from_secs(2)).await,
            "the session was not ended"
        );
        sup.shutdown();
    }

    #[tokio::test]
    async fn stops_after_three_restarts_in_a_minute() {
        let (_home, sup, events) = started().await;
        for _ in 0..3 {
            stop_server(&sup).await;
            assert!(sup.check_and_restart().await);
        }
        stop_server(&sup).await;
        assert!(!sup.check_and_restart().await);
        assert!(sup.endpoint().is_none());
        assert_eq!(events.lock().unwrap().last(), Some(&CoreEvent::Failed));
        assert!(!sup.check_and_restart().await, "a failed supervisor stays stopped");
    }
}
