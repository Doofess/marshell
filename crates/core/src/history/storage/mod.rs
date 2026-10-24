//! The storage thread: housekeeping for the files history keeps on disk.
//!
//! The thread ticks every minute and when nudged. A tick compresses the recordings that have finished
//! (the plain ones left by earlier runs too), then runs the retention cleanup when it is due. A later step
//! of the tick is the backup mirror. It owns one plain `std::thread`, as the importer does, and checks for a
//! halt between files.

pub mod backup;
pub mod compress;
pub mod retention;

use super::recorder::History;
use crate::paths;
use marshell_protocol::history::{BackupStatus, CleanupRun, RetentionLimits, RetentionPreview, StorageReport};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// How often the storage thread works.
#[derive(Clone, Copy, Debug)]
pub struct StorageTiming {
    /// How long the thread waits for a nudge before it ticks on its own.
    pub tick: Duration,
    /// How often the retention cleanup runs.
    pub cleanup_every: Duration,
    /// How long a recording must have gone unmodified before it is compressed or removed.
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

/// What is in use right now, so housekeeping leaves it alone.
#[derive(Clone, Debug, Default)]
pub struct Open {
    /// The recording files sessions are still writing.
    pub recordings: HashSet<PathBuf>,
    /// The tabs that are open: their images stay.
    pub tabs: HashSet<String>,
}

/// Where the thread learns what is in use. Each call is one snapshot; the thread takes it after it has listed
/// the folders, never before.
pub type OpenNow = Arc<dyn Fn() -> Open + Send + Sync>;

/// What the thread and the preview share.
struct Shared {
    home: PathBuf,
    history: Arc<History>,
    open: OpenNow,
    settle: Duration,
    /// The last cleanup that ran. Never held across file work.
    last_cleanup: Mutex<Option<CleanupRun>>,
    /// How the backup mirror stands. Never held across file work.
    backup: Mutex<BackupStatus>,
}

struct Worker {
    shared: Arc<Shared>,
    timing: StorageTiming,
    stopped: Arc<AtomicBool>,
    /// When the cleanup last ran. None until the first tick has.
    cleaned_at: Option<Instant>,
    /// The last backup error written to the log, so a folder that stays unreachable is logged once.
    backup_logged: Option<String>,
}

pub struct Storage {
    shared: Arc<Shared>,
    stopped: Arc<AtomicBool>,
    tx: Sender<Msg>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Storage {
    /// Starts the storage thread with the production timing (a tick every 60 s, 10 s to settle).
    pub fn start(home: PathBuf, history: Arc<History>, open: OpenNow) -> Arc<Storage> {
        Self::start_with(home, history, open, StorageTiming::default())
    }

    /// Starts the thread. `open` names what is in use; it is asked after a folder is listed, never before.
    /// The thread ticks once at once.
    pub fn start_with(home: PathBuf, history: Arc<History>, open: OpenNow, timing: StorageTiming) -> Arc<Storage> {
        let (tx, rx) = std::sync::mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Shared {
            home,
            history,
            open,
            settle: timing.settle,
            last_cleanup: Mutex::new(None),
            backup: Mutex::new(BackupStatus::default()),
        });
        let worker = Worker {
            shared: shared.clone(),
            timing,
            stopped: stopped.clone(),
            cleaned_at: None,
            backup_logged: None,
        };
        let thread = std::thread::Builder::new()
            .name("storage".into())
            .spawn(move || worker.run(rx))
            .map_err(|err| tracing::warn!("could not start the storage thread: {err}"))
            .ok();
        Arc::new(Storage {
            shared,
            stopped,
            tx,
            thread: Mutex::new(thread),
        })
    }

    /// Asks the thread to work now.
    pub fn nudge(&self, what: Nudge) {
        let _ = self.tx.send(Msg::Nudge(what));
    }

    /// What a cleanup with `limits` would remove now. It lists the files on the caller's thread, so a route
    /// runs it in `spawn_blocking`.
    pub fn preview(&self, limits: &RetentionLimits) -> RetentionPreview {
        let items = self.shared.collect();
        let chosen = retention::plan(&items, limits, retention::unix_ms(SystemTime::now()));
        let removed = || chosen.iter().filter_map(|&i| items.get(i));
        RetentionPreview {
            files: chosen.len() as u32,
            bytes: removed().map(|item| item.bytes).sum(),
            oldest_ms: removed().map(|item| item.at_ms).min(),
        }
    }

    /// How many bytes each kind of file takes now, with `limits` (the saved settings) and the last cleanup
    /// and backup state. It walks the folders on the caller's thread, so a route runs it in `spawn_blocking`.
    /// Links are skipped.
    pub fn usage(&self, limits: RetentionLimits) -> StorageReport {
        let home = &self.shared.home;
        let index = paths::index_file(home);
        let with = |suffix: &str| {
            let mut name = index.clone().into_os_string();
            name.push(suffix);
            PathBuf::from(name)
        };
        StorageReport {
            transcripts: folder_bytes(&paths::archive_root(home)),
            recordings: folder_bytes(&paths::recordings_dir(home)),
            attachments: folder_bytes(&paths::attachments_dir(home)),
            snapshots: folder_bytes(&paths::snapshots_dir(home)),
            logs: folder_bytes(&paths::logs_dir(home)),
            index: [index.clone(), with("-wal"), with("-shm")]
                .iter()
                .map(|f| file_bytes(f))
                .sum(),
            limits,
            last_cleanup: self.last_cleanup(),
            backup: self.backup_status(),
        }
    }

    /// The last cleanup that ran. None until one has, and while no limit has been set.
    pub fn last_cleanup(&self) -> Option<CleanupRun> {
        *self.shared.last_cleanup.lock().unwrap()
    }

    /// How the backup mirror stands: the folder, when it last finished, and why it could not.
    pub fn backup_status(&self) -> BackupStatus {
        self.shared.backup.lock().unwrap().clone()
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

/// The size of a regular file; 0 for a link, a folder or a missing file.
fn file_bytes(path: &std::path::Path) -> u64 {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => meta.len(),
        _ => 0,
    }
}

/// The bytes of every regular file under `dir`. A link (or junction) is not followed or counted.
fn folder_bytes(dir: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            // `DirEntry::metadata` does not follow links.
            Ok(meta) if meta.is_file() => meta.len(),
            Ok(meta) if meta.is_dir() && !is_reparse(&meta) => folder_bytes(&entry.path()),
            _ => 0,
        })
        .sum()
}

/// A Windows junction or other reparse point, which `is_dir` can report as a folder.
fn is_reparse(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Shared {
    /// The files retention may remove now. The folders are listed before the open set is taken.
    fn collect(&self) -> Vec<retention::Item> {
        retention::collect_with(&self.home, &*self.open, SystemTime::now(), self.settle)
    }
}

impl Worker {
    fn run(mut self, rx: Receiver<Msg>) {
        self.tick(false);
        loop {
            let cleanup_now = match rx.recv_timeout(self.timing.tick) {
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Msg::Nudge(Nudge::Cleanup)) => true,
                Ok(Msg::Nudge(Nudge::Tick)) | Err(RecvTimeoutError::Timeout) => false,
            };
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }
            self.tick(cleanup_now);
        }
    }

    /// One pass, in order: compress, cleanup when it is due or was asked for, then mirror.
    /// A step that fails is logged and the next still runs.
    fn tick(&mut self, cleanup_now: bool) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        self.compress_finished();
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let due = self
            .cleaned_at
            .is_none_or(|at| at.elapsed() >= self.timing.cleanup_every);
        if cleanup_now || due {
            self.cleaned_at = Some(Instant::now());
            self.cleanup();
        }
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        self.mirror();
    }

    /// Copies the history into the backup folder when one is set. A folder that can't be reached, or is refused,
    /// sets the error and is tried again on the next tick; the other steps of the tick don't wait for it.
    fn mirror(&mut self) {
        let settings = crate::settings::load(&self.shared.home);
        let Some(dir) = settings.history_backup_dir else {
            *self.shared.backup.lock().unwrap() = BackupStatus::default();
            self.backup_logged = None;
            return;
        };
        if !self.shared.history.is_on() {
            return;
        }
        let stopped = self.stopped.clone();
        let outcome = backup::mirror_with(
            &self.shared.home,
            std::path::Path::new(&dir),
            &*self.shared.open,
            self.timing.settle,
            &|| stopped.load(Ordering::SeqCst),
        );
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let mut status = self.shared.backup.lock().unwrap();
        if status.dir.as_deref() != Some(dir.as_str()) {
            *status = BackupStatus {
                dir: Some(dir),
                ..BackupStatus::default()
            };
        }
        match outcome {
            Ok(report) => {
                status.last_copy_ms = Some(retention::unix_ms(SystemTime::now()));
                status.error = None;
                self.backup_logged = None;
                if report.copied + report.removed > 0 {
                    tracing::info!(
                        "backup copied {} files ({} bytes) and removed {}",
                        report.copied,
                        report.bytes,
                        report.removed
                    );
                }
            }
            Err(err) => {
                let message = format!("{err:#}");
                if self.backup_logged.as_deref() != Some(message.as_str()) {
                    tracing::warn!("backup failed: {message}");
                    self.backup_logged = Some(message.clone());
                }
                status.error = Some(message);
            }
        }
    }

    /// Runs the retention cleanup with the limits in the settings file as they are now, not as they were when
    /// the nudge was sent and not as a preview listed them. With no limit set nothing happens.
    fn cleanup(&self) {
        let settings = crate::settings::load(&self.shared.home);
        let limits = RetentionLimits {
            limit_gb: settings.history_limit_gb,
            limit_days: settings.history_limit_days,
        };
        if (limits.limit_gb.is_none() && limits.limit_days.is_none()) || !self.shared.history.is_on() {
            return;
        }
        let items = self.shared.collect();
        let chosen = retention::plan(&items, &limits, retention::unix_ms(SystemTime::now()));
        let run = retention::apply(&items, &chosen);
        *self.shared.last_cleanup.lock().unwrap() = Some(run);
    }

    /// Compresses the finished plain recordings, one at a time, oldest first.
    fn compress_finished(&self) {
        let dir = paths::recordings_dir(&self.shared.home);
        // The folder first, the open set second: a recording that finishes in between reads as open or as
        // finished, and one that starts in between isn't listed.
        let listed = compress::list_plain(&dir);
        if listed.is_empty() {
            return;
        }
        let open = (self.shared.open)().recordings;
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

    fn start(home: &std::path::Path, open: Open) -> Arc<Storage> {
        Storage::start_with(
            home.to_path_buf(),
            Arc::new(History::open(home)),
            Arc::new(move || open.clone()),
            timing(),
        )
    }

    #[test]
    fn usage_counts_each_folder() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        let put = |path: PathBuf, n: usize| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, vec![b'x'; n]).unwrap();
        };
        put(paths::archive_dir(h, "claude").join("a.jsonl.zst"), 10);
        put(paths::archive_dir(h, "codex").join("b.jsonl.zst"), 5);
        put(paths::recordings_dir(h).join("t-1.cast.zst"), 20);
        put(paths::attachments_dir(h).join("t1").join("i.png"), 30);
        put(paths::snapshots_dir(h).join("s1").join("f"), 40);
        put(paths::logs_dir(h).join("marshell.log"), 50);
        let storage = start(h, Open::default());
        // Opening the history made the index; the side files are added after.
        put(h.join("history").join("index.sqlite-wal"), 7);
        put(h.join("history").join("index.sqlite-shm"), 3);
        let index_len = std::fs::metadata(paths::index_file(h)).unwrap().len();
        let limits = RetentionLimits {
            limit_gb: Some(5),
            limit_days: None,
        };
        let report = storage.usage(limits);
        assert_eq!(
            (
                report.transcripts,
                report.recordings,
                report.attachments,
                report.snapshots,
                report.logs,
                report.index
            ),
            (15, 20, 30, 40, 50, index_len + 10)
        );
        assert_eq!(report.limits, limits);
        assert_eq!(report.last_cleanup, None);
        storage.stop();
    }

    #[test]
    fn a_tick_compresses_a_finished_recording_and_skips_an_open_one() {
        let home = tempfile::tempdir().unwrap();
        let dir = paths::recordings_dir(home.path());
        std::fs::create_dir_all(&dir).unwrap();
        let open = dir.join("t1-1000.cast");
        std::fs::write(&open, b"open").unwrap();
        std::fs::write(dir.join("t2-2000.cast"), b"finished").unwrap();
        let storage = start(
            home.path(),
            Open {
                recordings: [open.clone()].into(),
                tabs: HashSet::new(),
            },
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
    fn an_unreachable_backup_sets_the_error_and_recovers() {
        let home = tempfile::tempdir().unwrap();
        let outer = tempfile::tempdir().unwrap();
        let dir = outer.path().join("usb");
        let recordings = paths::recordings_dir(home.path());
        std::fs::create_dir_all(&recordings).unwrap();
        std::fs::write(recordings.join("t1-1000.cast.zst"), b"recording").unwrap();
        let settings = marshell_protocol::api::Settings {
            history_backup_dir: Some(dir.to_string_lossy().into_owned()),
            ..Default::default()
        };
        crate::settings::save(home.path(), &settings).unwrap();
        // A recording to compress still gets compressed while the backup fails.
        std::fs::write(recordings.join("t2-2000.cast"), b"finished").unwrap();
        let storage = start(home.path(), Open::default());
        assert!(wait_for(|| storage.backup_status().error.is_some()));
        let status = storage.backup_status();
        assert_eq!(status.dir.as_deref(), Some(dir.to_string_lossy().as_ref()));
        assert_eq!(status.last_copy_ms, None);
        assert_eq!(status.error.as_deref(), Some("That folder doesn't exist."));
        assert!(wait_for(|| recordings.join("t2-2000.cast.zst").exists()));

        std::fs::create_dir(&dir).unwrap();
        storage.nudge(Nudge::Tick);
        assert!(wait_for(|| storage.backup_status().last_copy_ms.is_some()));
        let status = storage.backup_status();
        assert_eq!(status.error, None);
        let copy = dir
            .join(backup::TARGET_NAME)
            .join("recordings")
            .join("t1-1000.cast.zst");
        assert_eq!(std::fs::read(copy).unwrap(), b"recording");

        // Stopping the backup clears the status and leaves the copy where it is.
        crate::settings::save(home.path(), &marshell_protocol::api::Settings::default()).unwrap();
        storage.nudge(Nudge::Tick);
        assert!(wait_for(|| storage.backup_status() == BackupStatus::default()));
        assert!(dir.join(backup::TARGET_NAME).exists());
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
        let storage = start(home.path(), Open::default());
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

    /// Writes an old recording and a new one, and saves `limits` in the settings file.
    fn seed_retention(home: &std::path::Path, limits: (Option<u32>, Option<u32>)) -> (PathBuf, PathBuf) {
        let dir = paths::recordings_dir(home);
        std::fs::create_dir_all(&dir).unwrap();
        let (old, new) = (dir.join("t1-1000.cast.zst"), dir.join("t2-2000.cast.zst"));
        for (path, age_days) in [(&old, 40), (&new, 1)] {
            std::fs::write(path, b"recording").unwrap();
            File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(SystemTime::now() - Duration::from_secs(age_days * 86_400))
                .unwrap();
        }
        let settings = marshell_protocol::api::Settings {
            history_limit_gb: limits.0,
            history_limit_days: limits.1,
            ..Default::default()
        };
        crate::settings::save(home, &settings).unwrap();
        (old, new)
    }

    #[test]
    fn the_first_tick_runs_the_cleanup_with_the_saved_limits() {
        let home = tempfile::tempdir().unwrap();
        let (old, new) = seed_retention(home.path(), (None, Some(30)));
        let storage = start(home.path(), Open::default());
        assert!(wait_for(|| storage.last_cleanup().is_some()));
        let run = storage.last_cleanup().unwrap();
        assert_eq!((run.files, run.bytes), (1, 9));
        assert!(!old.exists());
        assert!(new.exists());
        storage.stop();
    }

    #[test]
    fn no_limit_means_no_cleanup_and_no_last_run() {
        let home = tempfile::tempdir().unwrap();
        let (old, _) = seed_retention(home.path(), (None, None));
        let storage = start(home.path(), Open::default());
        storage.nudge(Nudge::Cleanup);
        std::thread::sleep(Duration::from_millis(300));
        assert!(old.exists());
        assert_eq!(storage.last_cleanup(), None);
        // Saving a limit and nudging runs it at once, ahead of the next daily run.
        let settings = marshell_protocol::api::Settings {
            history_limit_days: Some(30),
            ..Default::default()
        };
        crate::settings::save(home.path(), &settings).unwrap();
        storage.nudge(Nudge::Cleanup);
        assert!(wait_for(|| storage.last_cleanup().is_some()));
        assert!(!old.exists());
        storage.stop();
    }

    #[test]
    fn a_cleanup_never_removes_an_open_recording_or_an_open_tabs_images() {
        let home = tempfile::tempdir().unwrap();
        let dir = paths::recordings_dir(home.path());
        let (old, _) = seed_retention(home.path(), (None, Some(30)));
        let image = paths::attachments_dir(home.path()).join("t9").join("a.png");
        std::fs::create_dir_all(image.parent().unwrap()).unwrap();
        std::fs::write(&image, b"png").unwrap();
        File::options()
            .write(true)
            .open(&image)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(90 * 86_400))
            .unwrap();
        let storage = start(
            home.path(),
            Open {
                recordings: [dir.join("t1-1000.cast")].into(),
                tabs: ["t9".to_owned()].into(),
            },
        );
        assert!(wait_for(|| storage.last_cleanup().is_some()));
        assert_eq!(storage.last_cleanup().unwrap().files, 0);
        assert!(old.exists());
        assert!(image.exists());
        storage.stop();
    }
}
