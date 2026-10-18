//! A `notify` watcher over the folders of the files the service has previewed. Events are debounced
//! per path: `on_change` gets a path once, after `DEBOUNCE` with no further event for it.
//!
//! The debounce thread owns the receiving end of the event channel; the `notify` watcher owns the
//! sending end. Dropping the `Watcher` drops the sender, `recv_timeout` then returns `Disconnected`,
//! and the thread ends. The thread holds only `on_change`, so what it captures decides what it keeps
//! alive (the service captures a `Weak`).

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

pub const DEBOUNCE: Duration = Duration::from_millis(200);

/// How long the thread sleeps when nothing is pending; it wakes only to notice a disconnect.
const IDLE: Duration = Duration::from_secs(3600);

pub struct Watcher {
    inner: RecommendedWatcher,
    folders: HashSet<PathBuf>,
}

impl Watcher {
    /// `on_change` gets each changed path once, after 200 ms with no further event for it.
    pub fn start(on_change: impl Fn(PathBuf) + Send + 'static) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let inner = notify::recommended_watcher(tx)?;
        std::thread::Builder::new()
            .name("config-watch".into())
            .spawn(move || debounce(rx, on_change))
            .map_err(notify::Error::io)?;
        Ok(Self {
            inner,
            folders: HashSet::new(),
        })
    }

    /// Watches the file's folder (non-recursive) once; later calls for the same folder do nothing.
    /// A folder that doesn't exist yet is an error, and nothing is watched.
    pub fn watch_file(&mut self, file: &Path) -> notify::Result<()> {
        let Some(folder) = file.parent() else {
            return Ok(());
        };
        if self.folders.contains(folder) {
            return Ok(());
        }
        self.inner.watch(folder, RecursiveMode::NonRecursive)?;
        self.folders.insert(folder.to_path_buf());
        Ok(())
    }
}

fn debounce(rx: Receiver<notify::Result<Event>>, on_change: impl Fn(PathBuf)) {
    // The time of the latest event for each path.
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    loop {
        let wait = match pending.values().min() {
            Some(oldest) => (*oldest + DEBOUNCE).saturating_duration_since(Instant::now()),
            None => IDLE,
        };
        match rx.recv_timeout(wait) {
            Ok(Ok(event)) => {
                // Reading a file is not a change (and inotify reports it).
                if !matches!(event.kind, EventKind::Access(_)) {
                    let now = Instant::now();
                    for path in event.paths {
                        pending.insert(path, now);
                    }
                }
            }
            Ok(Err(e)) => tracing::warn!("config watcher: {e}"),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = Instant::now();
        let due: Vec<PathBuf> = pending
            .iter()
            .filter(|(_, at)| now.duration_since(**at) >= DEBOUNCE)
            .map(|(path, _)| path.clone())
            .collect();
        for path in due {
            pending.remove(&path);
            on_change(path);
        }
    }
}
