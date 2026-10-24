//! The storage thread: housekeeping for the files history keeps on disk.
//!
//! The thread ticks every minute and when nudged. A tick compresses the recordings that have finished
//! (the plain ones left by earlier runs too). Later steps of the tick, in order, are the retention cleanup
//! and the backup mirror. It owns one plain `std::thread`, as the importer does, and checks for a halt
//! between files.

pub mod compress;

use super::recorder::History;
use crate::paths;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// How often the storage thread works.
#[derive(Clone, Copy, Debug)]
pub struct StorageTiming {
    /// How long the thread waits for a nudge before it ticks on its own.
    pub tick: Duration,
    /// How often the retention cleanup runs.
    pub cleanup_every: Duration,
    /// How long a recording must have gone unmodified before it is compressed.
    pub settle: Duration,
}

impl Default for StorageTiming {
    fn default() -> Self {
        StorageTiming {
            tick: Duration::from_secs(60),
            cleanup_every: Duration::from_secs(24 * 60 * 60),
            settle: Duration::from_secs(10),
        }
    }
}

/// What a caller asks the thread to do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nudge {
    /// Run a tick now.
    Tick,
    /// Run the retention cleanup now, then a tick.
    Cleanup,
}

enum Msg {
    Nudge(Nudge),
    Stop,
}

/// Where the thread learns which recordings are still being written.
pub type OpenRecordings = Arc<dyn Fn() -> HashSet<PathBuf> + Send + Sync>;

struct Worker {
    home: PathBuf,
    /// For the retention cleanup step.
    #[allow(dead_code)]
    history: Arc<History>,
    open: OpenRecordings,
    timing: StorageTiming,
    stopped: Arc<AtomicBool>,
}

pub struct Storage {
    stopped: Arc<AtomicBool>,
    tx: Sender<Msg>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Storage {
    /// Starts the storage thread with the production timing (a tick every 60 s, 10 s to settle).
    pub fn start(home: PathBuf, history: Arc<History>, open: OpenRecordings) -> Arc<Storage> {
        Self::start_with(home, history, open, StorageTiming::default())
    }

    /// Starts the thread. `open` names the recordings still being written; it is asked after the recordings
    /// folder is listed, never before. The thread ticks once at once.
    pub fn start_with(
        home: PathBuf,
        history: Arc<History>,
        open: OpenRecordings,
        timing: StorageTiming,
    ) -> Arc<Storage> {
        let (tx, rx) = std::sync::mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            home,
            history,
            open,
            timing,
            stopped: stopped.clone(),
        };
        let thread = std::thread::Builder::new()
            .name("storage".into())
            .spawn(move || worker.run(rx))
            .map_err(|err| tracing::warn!("could not start the storage thread: {err}"))
            .ok();
        Arc::new(Storage {
            stopped,
            tx,
            thread: Mutex::new(thread),
        })
    }

    /// Asks the thread to work now.
    pub fn nudge(&self, what: Nudge) {
        let _ = self.tx.send(Msg::Nudge(what));
    }

    /// Tells the thread to end and returns at once. It stops between files.
    pub fn halt(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = self.tx.send(Msg::Stop);
    }

    /// Ends the thread (after the file it is compressing) and waits for it.
    pub fn stop(&self) {
        self.halt();
        let handle = self.thread.lock().unwrap().take();
        if let Some(handle) = handle
            // The last holder of the app state can be the thread itself, which can't wait for itself.
            && handle.thread().id() != std::thread::current().id()
        {
            let _ = handle.join();
        }
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Worker {
    fn run(self, rx: Receiver<Msg>) {
        self.tick();
        loop {
            match rx.recv_timeout(self.timing.tick) {
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                // The cleanup step comes in a later task; until then a cleanup nudge is a tick.
                Ok(Msg::Nudge(Nudge::Tick | Nudge::Cleanup)) | Err(RecvTimeoutError::Timeout) => {}
            }
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }
            self.tick();
        }
    }

    /// One pass, in order: compress, then (in later tasks) cleanup and mirror. A step that fails is logged and
    /// the next still runs.
    fn tick(&self) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        self.compress_finished();
    }

    /// Compresses the finished plain recordings, one at a time, oldest first.
    fn compress_finished(&self) {
        let dir = paths::recordings_dir(&self.home);
        // The folder first, the open set second: a recording that finishes in between reads as open or as
        // finished, and one that starts in between isn't listed.
        let listed = compress::list_plain(&dir);
        if listed.is_empty() {
            return;
        }
        let open = (self.open)();
        for path in compress::select_finished(listed, &open, SystemTime::now(), self.timing.settle) {
            if self.stopped.load(Ordering::SeqCst) {
                return;
            }
            if let Err(err) = compress::compress(&path) {
                tracing::warn!("could not compress {}: {err:#}", path.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::time::Instant;

    fn timing() -> StorageTiming {
        StorageTiming {
            tick: Duration::from_secs(3600),
            cleanup_every: Duration::from_secs(3600),
            settle: Duration::ZERO,
        }
    }

    fn wait_for(what: impl Fn() -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs(2);
        while Instant::now() < end {
            if what() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        what()
    }

    #[test]
    fn a_tick_compresses_a_finished_recording_and_skips_an_open_one() {
        let home = tempfile::tempdir().unwrap();
        let dir = paths::recordings_dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        let open = dir.join("t1-1000.cast");
        std::fs::write(&open, b"open").unwrap();
        std::fs::write(dir.join("t2-2000.cast"), b"finished").unwrap();
        let open_set: HashSet<PathBuf> = [open.clone()].into();
        let storage = Storage::start_with(
            home.path().to_path_buf(),
            Arc::new(History::open(home.path())),
            Arc::new(move || open_set.clone()),
            timing(),
        );
        storage.nudge(Nudge::Tick);
        assert!(wait_for(
            || dir.join("t2-2000.cast.zst").exists() && !dir.join("t2-2000.cast").exists()
        ));
        assert_eq!(
            zstd::decode_all(File::open(dir.join("t2-2000.cast.zst")).unwrap()).unwrap(),
            b"finished"
        );
        // The open file is still there after a second pass, which the nudge asked for.
        storage.nudge(Nudge::Tick);
        std::thread::sleep(Duration::from_millis(200));
        assert!(open.exists());
        assert!(!dir.join("t1-1000.cast.zst").exists());
        storage.stop();
    }

    #[test]
    fn halt_stops_between_files() {
        let home = tempfile::tempdir().unwrap();
        let dir = paths::recordings_dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        let body = "[0.5,\"o\",\"abc\"]\n".repeat(20_000);
        for i in 0..20 {
            std::fs::write(dir.join(format!("t{i}-{}.cast", 1000 + i)), &body).unwrap();
        }
        let storage = Storage::start_with(
            home.path().to_path_buf(),
            Arc::new(History::open(home.path())),
            Arc::new(HashSet::new),
            timing(),
        );
        storage.nudge(Nudge::Tick);
        storage.halt();
        let started = Instant::now();
        storage.stop();
        assert!(started.elapsed() < Duration::from_secs(2));
        for i in 0..20 {
            let plain = dir.join(format!("t{i}-{}.cast", 1000 + i));
            let zst = dir.join(format!("t{i}-{}.cast.zst", 1000 + i));
            assert!(plain.exists() || zst.exists(), "file {i} was lost");
            assert!(!dir.join(format!("t{i}-{}.cast.zst.tmp", 1000 + i)).exists());
        }
    }
}
