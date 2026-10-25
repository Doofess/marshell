//! Copies the conversations Claude Code and Codex CLI ran outside Marshell into the history archive.
//!
//! The importer owns one thread. It scans `<claude root>/<folder>/<uuid>.jsonl` and
//! `<codex root>/YYYY/MM/DD/rollout-*.jsonl[.zst]`, watches the roots with `notify`, and rescans on a timer.
//! A file is copied once its size differs from the size last copied and has stayed the same for `settle`;
//! the size, not the modification time, is the signal. Files are copied one at a time, newest first.
//! The source folders are only ever read, and links in them are not followed.

use super::recorder::History;
use super::{SessionMeta, archive, valid_session_id};
use crate::agents::codex::rollout;
use crate::agents::common::rfc3339_ms;
use crate::{paths, store};
use marshell_protocol::history::{Client, ImportStatus};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

const CLAUDE: &str = "claude";
const CODEX: &str = "codex";

/// How much of a file's start is read to learn when it began.
const HEAD_BYTES: u64 = 64 * 1024;

/// The folders the CLIs keep their conversations in. A folder that is None is not looked at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportRoots {
    pub claude: Option<PathBuf>,
    pub codex: Option<PathBuf>,
}

/// Claude Code's `projects` folder is `$CLAUDE_CONFIG_DIR/projects`, else `<home>/.claude/projects`; Codex's
/// `sessions` folder is `$CODEX_HOME/sessions`, else `<home>/.codex/sessions`.
pub fn import_roots(env: impl Fn(&str) -> Option<String>, user_home: &Path) -> ImportRoots {
    let from = |var: &str, default: &str, leaf: &str| {
        match env(var).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty()) {
            Some(dir) => dir,
            None => user_home.join(default),
        }
        .join(leaf)
    };
    ImportRoots {
        claude: Some(from("CLAUDE_CONFIG_DIR", ".claude", "projects")),
        codex: Some(from("CODEX_HOME", ".codex", "sessions")),
    }
}

/// The roots for this machine, from the environment and the user's home folder. None when the home folder
/// can't be found.
pub fn system_roots() -> Option<ImportRoots> {
    Some(import_roots(|k| std::env::var(k).ok(), &dirs::home_dir()?))
}

/// How long a file must stay the same size before it is copied, and how often everything is scanned again.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub settle: Duration,
    pub rescan: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            settle: Duration::from_secs(3),
            rescan: Duration::from_secs(60),
        }
    }
}

/// What the state file remembers of one source file.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Copied {
    /// The file's size when it was last read.
    size: u64,
    agent: String,
    /// Empty for a file that is never copied (a Codex sub-agent thread, a Claude file with no usable id).
    session_id: String,
}

/// `<home>/history/import.json`.
#[derive(Default, Serialize, Deserialize)]
struct StateDoc {
    #[serde(default)]
    files: BTreeMap<String, Copied>,
}

impl store::Versioned for StateDoc {
    const VERSION: u32 = 1;
}

struct Shared {
    on: AtomicBool,
    stopped: AtomicBool,
    copying: AtomicBool,
    done: AtomicU32,
    total: AtomicU32,
    /// The sessions copied, in order, for tests.
    #[cfg(test)]
    order: Mutex<Vec<String>>,
    /// For tests: the worker waits on this lock before it copies, and sets `at_gate` when it gets there.
    #[cfg(test)]
    gate: Mutex<()>,
    #[cfg(test)]
    at_gate: AtomicBool,
}

enum Msg {
    /// Paths `notify` reported something about.
    Event(Vec<PathBuf>),
    /// Scan everything now.
    Rescan,
    Stop,
}

pub struct Importer {
    shared: Arc<Shared>,
    tx: Sender<Msg>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Importer {
    /// Starts the importer thread with the production timing (3 s to settle, a rescan every 60 s).
    pub fn start(history: Arc<History>, roots: ImportRoots, on: bool) -> Arc<Importer> {
        Self::start_with(history, roots, on, Timing::default())
    }

    pub fn start_with(history: Arc<History>, roots: ImportRoots, on: bool, timing: Timing) -> Arc<Importer> {
        Self::start_capped(history, roots, on, timing, archive::MAX_DECODED_BYTES)
    }

    /// Like `start_with`, with a different most a source file may be, plain or decoded, before it is skipped.
    fn start_capped(
        history: Arc<History>,
        roots: ImportRoots,
        on: bool,
        timing: Timing,
        max_source: u64,
    ) -> Arc<Importer> {
        let (tx, rx) = std::sync::mpsc::channel();
        let shared = Arc::new(Shared {
            on: AtomicBool::new(on),
            stopped: AtomicBool::new(false),
            copying: AtomicBool::new(false),
            done: AtomicU32::new(0),
            total: AtomicU32::new(0),
            #[cfg(test)]
            order: Mutex::new(Vec::new()),
            #[cfg(test)]
            gate: Mutex::new(()),
            #[cfg(test)]
            at_gate: AtomicBool::new(false),
        });
        let state_path = paths::history_dir(history.home()).join("import.json");
        let (known, first_run) = match store::read::<StateDoc>(&state_path) {
            Ok(Some(doc)) => (doc.files, false),
            Ok(None) => (BTreeMap::new(), true),
            Err(err) => {
                tracing::warn!("history import state could not be read, starting over: {err:#}");
                (BTreeMap::new(), true)
            }
        };
        let worker = Worker {
            history,
            roots,
            timing,
            shared: shared.clone(),
            tx: tx.clone(),
            state_path,
            known,
            seen: HashMap::new(),
            watched: HashSet::new(),
            watchers: Vec::new(),
            missing_logged: HashSet::new(),
            first_run,
            max_source,
        };
        let thread = std::thread::Builder::new()
            .name("history-import".into())
            .spawn(move || worker.run(rx))
            .map_err(|err| tracing::warn!("could not start the history importer: {err}"))
            .ok();
        Arc::new(Importer {
            shared,
            tx,
            thread: Mutex::new(thread),
        })
    }

    /// Turns the copying on or off. Off stops after the file being copied; on scans everything again.
    pub fn set_on(&self, on: bool) {
        self.shared.on.store(on, Ordering::SeqCst);
        if on {
            let _ = self.tx.send(Msg::Rescan);
        }
    }

    pub fn status(&self) -> ImportStatus {
        ImportStatus {
            on: self.shared.on.load(Ordering::SeqCst),
            copying: self.shared.copying.load(Ordering::SeqCst),
            done: self.shared.done.load(Ordering::SeqCst),
            total: self.shared.total.load(Ordering::SeqCst),
        }
    }

    /// Tells the thread to end and returns at once. For shutdown: the file being copied is registered with
    /// `History`, whose `flush` waits for it up to its own cap, so a large file can't hold shutdown up.
    pub fn halt(&self) {
        self.shared.stopped.store(true, Ordering::SeqCst);
        let _ = self.tx.send(Msg::Stop);
    }

    /// Ends the thread (after the file it is copying) and its watchers. Waits for it.
    pub fn stop(&self) {
        self.halt();
        let handle = self.thread.lock().unwrap().take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

impl Drop for Importer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Claude,
    Codex,
}

impl Kind {
    fn agent(self) -> &'static str {
        match self {
            Kind::Claude => CLAUDE,
            Kind::Codex => CODEX,
        }
    }
}

/// True when the error comes from the operating system (a file that can't be opened or read), as opposed to
/// a file whose content is the problem: too large, or not a zstd frame.
fn failed_in_the_system(err: &anyhow::Error) -> bool {
    err.chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|io| io.raw_os_error().is_some())
}

/// What reading a source file's text decided.
enum Prepared {
    /// Copy it with this meta.
    Copy(SessionMeta),
    /// Record it and never copy it (until it grows).
    Skip(&'static str),
}

struct Worker {
    history: Arc<History>,
    roots: ImportRoots,
    timing: Timing,
    shared: Arc<Shared>,
    tx: Sender<Msg>,
    state_path: PathBuf,
    /// By source path: what was last copied. Mirrors the state file.
    known: BTreeMap<String, Copied>,
    /// By source path: the size last seen and since when it has been that size.
    seen: HashMap<PathBuf, (u64, Instant)>,
    /// Roots a watcher was tried on.
    watched: HashSet<PathBuf>,
    watchers: Vec<RecommendedWatcher>,
    missing_logged: HashSet<PathBuf>,
    first_run: bool,
    /// The most a source file may be, plain or decoded; a larger one is skipped.
    max_source: u64,
}

impl Worker {
    fn run(mut self, rx: Receiver<Msg>) {
        let tick = self
            .timing
            .settle
            .min(Duration::from_millis(500))
            .max(Duration::from_millis(10));
        let mut last_scan: Option<Instant> = None;
        let mut rescan_now = true;
        loop {
            match rx.recv_timeout(tick) {
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Msg::Event(paths)) => paths.iter().for_each(|path| self.observe(path)),
                Ok(Msg::Rescan) => rescan_now = true,
                Err(RecvTimeoutError::Timeout) => {}
            }
            if self.shared.stopped.load(Ordering::SeqCst) {
                break;
            }
            if rescan_now || last_scan.is_none_or(|at| at.elapsed() >= self.timing.rescan) {
                // Only the first scan of a first run takes files at the size it finds them; any later one waits.
                let backdate = last_scan.is_none() && self.first_run;
                self.scan(backdate);
                rescan_now = false;
                last_scan = Some(Instant::now());
            }
            self.work();
        }
    }

    fn roots(&self) -> [(Kind, Option<&Path>); 2] {
        [
            (Kind::Claude, self.roots.claude.as_deref()),
            (Kind::Codex, self.roots.codex.as_deref()),
        ]
    }

    /// Lists every candidate under the roots that exist, and starts a watcher on each root once.
    fn scan(&mut self, backdate: bool) {
        let roots: Vec<(Kind, PathBuf)> = self
            .roots()
            .into_iter()
            .filter_map(|(kind, root)| Some((kind, root?.to_path_buf())))
            .collect();
        for (kind, root) in roots {
            if !root.is_dir() {
                if self.missing_logged.insert(root.clone()) {
                    tracing::info!(
                        "history import: {} does not exist; it is checked again at each rescan",
                        root.display()
                    );
                }
                continue;
            }
            self.missing_logged.remove(&root);
            self.watch(&root);
            for path in candidates(kind, &root) {
                self.track(&path, backdate);
            }
        }
    }

    fn watch(&mut self, root: &Path) {
        if !self.watched.insert(root.to_path_buf()) {
            return;
        }
        let tx = self.tx.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if let Ok(event) = event
                && !event.kind.is_access()
            {
                let _ = tx.send(Msg::Event(event.paths));
            }
        });
        match watcher {
            Ok(mut watcher) => match watcher.watch(root, RecursiveMode::Recursive) {
                Ok(()) => self.watchers.push(watcher),
                Err(err) => tracing::warn!(
                    "history import: not watching {}, the rescan still finds changes: {err}",
                    root.display()
                ),
            },
            Err(err) => tracing::warn!("history import: no file watcher, the rescan still finds changes: {err}"),
        }
    }

    /// A `notify` event: look at the path if it is a candidate.
    fn observe(&mut self, path: &Path) {
        if classify(&self.roots, path).is_some() {
            self.track(path, false);
        }
    }

    /// Notes the size of a candidate.
    fn track(&mut self, path: &Path, backdate: bool) {
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            self.seen.remove(path);
            return;
        };
        if !is_plain_file(&meta) {
            self.seen.remove(path);
            return;
        }
        let size = meta.len();
        if self.known.get(&key(path)).is_some_and(|copied| copied.size == size) {
            self.seen.remove(path);
            return;
        }
        if self.seen.get(path).is_some_and(|(seen, _)| *seen == size) {
            return;
        }
        let since = if backdate {
            Instant::now()
                .checked_sub(self.timing.settle)
                .unwrap_or_else(Instant::now)
        } else {
            Instant::now()
        };
        self.seen.insert(path.to_path_buf(), (size, since));
    }

    /// Copies the files whose size has settled, newest first, one at a time.
    fn work(&mut self) {
        if !self.can_start() {
            return;
        }
        let now = Instant::now();
        let mut due: Vec<(u64, PathBuf, u64)> = self
            .seen
            .iter()
            .filter(|(_, (_, since))| now.saturating_duration_since(*since) >= self.timing.settle)
            .map(|(path, (size, _))| (head_time(path), path.clone(), *size))
            .collect();
        if due.is_empty() {
            return;
        }
        due.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        self.shared.done.store(0, Ordering::SeqCst);
        self.shared.total.store(due.len() as u32, Ordering::SeqCst);
        self.shared.copying.store(true, Ordering::SeqCst);
        for (_, path, size) in due {
            if !self.can_start() {
                break;
            }
            self.process(&path, size);
            self.shared.done.fetch_add(1, Ordering::SeqCst);
        }
        self.shared.copying.store(false, Ordering::SeqCst);
    }

    /// False when the switch is off, the importer is stopping or Marshell is shutting down.
    fn can_start(&self) -> bool {
        self.shared.on.load(Ordering::SeqCst)
            && !self.shared.stopped.load(Ordering::SeqCst)
            && !self.history.is_hurrying()
    }

    /// Reads one source file and copies it, or records that it is not copied. `queued` is the size it had
    /// when it was queued; a file that has changed since goes back to settling.
    fn process(&mut self, path: &Path, queued: u64) {
        let Ok(md) = std::fs::symlink_metadata(path) else {
            self.seen.remove(path);
            return;
        };
        if !is_plain_file(&md) {
            self.seen.remove(path);
            return;
        }
        let size = md.len();
        if size != queued {
            self.seen.insert(path.to_path_buf(), (size, Instant::now()));
            return;
        }
        let Some(kind) = classify(&self.roots, path) else {
            self.seen.remove(path);
            return;
        };
        #[cfg(test)]
        {
            self.shared.at_gate.store(true, Ordering::SeqCst);
            drop(self.shared.gate.lock().unwrap());
        }
        let bytes = match archive::read_source_capped(path, self.max_source) {
            Ok(bytes) => bytes,
            // The system could not open or read it (a lock, a vanished file): try again after the next rescan.
            Err(err) if failed_in_the_system(&err) => {
                tracing::warn!("history import: could not read {}: {err:#}", path.display());
                self.retry_later(path, size);
                return;
            }
            // Over the cap, or it does not decode: reading it again gives the same result. Record it at this
            // size, so it is looked at again only when its size changes, and say so once.
            Err(err) => {
                tracing::warn!(
                    "history import: skipping {}, which is looked at again only when its size changes: {err:#}",
                    path.display()
                );
                self.record(path, size, kind.agent(), "");
                return;
            }
        };
        let prepared = prepare(kind, path, &String::from_utf8_lossy(&bytes), &md);
        // Shutdown began while the file was being read: the copy would start after the flush stopped waiting.
        if self.shared.stopped.load(Ordering::SeqCst) || self.history.is_hurrying() {
            return;
        }
        match prepared {
            Prepared::Skip(agent) => self.record(path, size, agent, ""),
            // The user deleted this conversation's copy and the file has not changed since: it stays deleted.
            // A file written after the delete is a conversation that went on, and is copied.
            Prepared::Copy(meta)
                if archive::deleted_at(self.history.home(), &meta.agent, &meta.session_id)
                    .is_some_and(|deleted_ms| modified_ms(&md) <= deleted_ms) =>
            {
                self.record(path, size, &meta.agent, &meta.session_id);
            }
            Prepared::Copy(meta) => match self.history.import_bytes(&meta, &bytes) {
                Ok(true) => {
                    #[cfg(test)]
                    self.shared.order.lock().unwrap().push(meta.session_id.clone());
                    self.record(path, size, &meta.agent, &meta.session_id);
                }
                // The hook path is copying this session right now: look again after it settles.
                Ok(false) => {
                    self.seen.insert(path.to_path_buf(), (size, Instant::now()));
                }
                Err(err) => {
                    tracing::warn!("history import: could not copy {}: {err:#}", path.display());
                    self.retry_later(path, size);
                }
            },
        }
    }

    /// A file that failed is tried again after the next rescan, not on every tick.
    fn retry_later(&mut self, path: &Path, size: u64) {
        self.seen
            .insert(path.to_path_buf(), (size, Instant::now() + self.timing.rescan));
    }

    fn record(&mut self, path: &Path, size: u64, agent: &str, session_id: &str) {
        self.seen.remove(path);
        self.known.insert(
            key(path),
            Copied {
                size,
                agent: agent.to_string(),
                session_id: session_id.to_string(),
            },
        );
        let doc = StateDoc {
            files: self.known.clone(),
        };
        if let Err(err) = store::write(&self.state_path, &doc) {
            tracing::warn!("history import: could not save its state: {err:#}");
        }
    }
}

fn key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A regular file that is not a link of any kind.
fn is_plain_file(md: &std::fs::Metadata) -> bool {
    md.is_file() && !is_link(md)
}

/// A symlink, or a Windows junction or other reparse point.
fn is_link(md: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        if md.file_attributes() & REPARSE_POINT != 0 {
            return true;
        }
    }
    md.file_type().is_symlink()
}

fn is_plain_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|md| md.is_dir() && !is_link(&md))
}

fn is_uuid(text: &str) -> bool {
    let groups: Vec<&str> = text.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(group, len)| group.len() == len && group.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn claude_file(name: &str) -> bool {
    name.strip_suffix(".jsonl").is_some_and(is_uuid)
}

fn codex_file(name: &str) -> bool {
    name.starts_with("rollout-") && (name.ends_with(".jsonl") || name.ends_with(".jsonl.zst"))
}

fn digits(name: &str, len: usize) -> bool {
    name.len() == len && name.bytes().all(|b| b.is_ascii_digit())
}

/// The files under `root` that may hold a conversation. Links are not followed, in folders or files.
fn candidates(kind: Kind, root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    match kind {
        Kind::Claude => {
            for project in plain_dirs(root, |_| true) {
                found.extend(plain_files(&project, claude_file));
            }
        }
        Kind::Codex => {
            for year in plain_dirs(root, |n| digits(n, 4)) {
                for month in plain_dirs(&year, |n| digits(n, 2)) {
                    for day in plain_dirs(&month, |n| digits(n, 2)) {
                        found.extend(plain_files(&day, codex_file));
                    }
                }
            }
        }
    }
    found
}

fn entries(dir: &Path, want_dir: bool, name_ok: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .filter(|entry| name_ok(&entry.file_name().to_string_lossy()))
        .filter(|entry| {
            entry.metadata().is_ok_and(|md| {
                !is_link(&md) && !entry.file_type().is_ok_and(|t| t.is_symlink()) && md.is_dir() == want_dir
            })
        })
        .map(|entry| entry.path())
        .collect()
}

fn plain_dirs(dir: &Path, name_ok: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    entries(dir, true, name_ok)
}

fn plain_files(dir: &Path, name_ok: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    entries(dir, false, name_ok)
}

/// Which kind of conversation file `path` is, if it sits where one does under a root and no folder between
/// the root and the file is a link.
fn classify(roots: &ImportRoots, path: &Path) -> Option<Kind> {
    let under = |root: &Option<PathBuf>| -> Option<Vec<String>> {
        let rel = path.strip_prefix(root.as_ref()?).ok()?;
        rel.components()
            .map(|c| match c {
                Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect()
    };
    let shaped = |root: &Option<PathBuf>, kind: Kind| -> bool {
        let Some(parts) = under(root) else {
            return false;
        };
        let ok = match (kind, parts.as_slice()) {
            (Kind::Claude, [_, file]) => claude_file(file),
            (Kind::Codex, [y, m, d, file]) => digits(y, 4) && digits(m, 2) && digits(d, 2) && codex_file(file),
            _ => false,
        };
        let Some(root) = root else {
            return false;
        };
        ok && {
            let mut dir = root.clone();
            parts[..parts.len() - 1].iter().all(|part| {
                dir.push(part);
                is_plain_dir(&dir)
            })
        }
    };
    if shaped(&roots.claude, Kind::Claude) {
        Some(Kind::Claude)
    } else if shaped(&roots.codex, Kind::Codex) {
        Some(Kind::Codex)
    } else {
        None
    }
}

fn modified_ms(md: &std::fs::Metadata) -> u64 {
    md.modified()
        .ok()
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

/// When the file's first line with a time says it began, read from its first bytes; the file's modification
/// time when there is none. Only for ordering.
fn head_time(path: &Path) -> u64 {
    use std::io::Read;
    let fallback = || std::fs::metadata(path).map(|md| modified_ms(&md)).unwrap_or_default();
    let Ok(file) = std::fs::File::open(path) else {
        return 0;
    };
    let mut head = Vec::new();
    let read = if path.extension().is_some_and(|ext| ext == "zst") {
        zstd::Decoder::new(file).and_then(|decoder| decoder.take(HEAD_BYTES).read_to_end(&mut head))
    } else {
        file.take(HEAD_BYTES).read_to_end(&mut head)
    };
    // A decoder that stops early on a cut frame still leaves a usable head.
    let _ = read;
    String::from_utf8_lossy(&head)
        .lines()
        .find_map(line_time)
        .unwrap_or_else(fallback)
}

/// The `timestamp` of a JSON line, in ms.
fn line_time(line: &str) -> Option<u64> {
    let value: Value = serde_json::from_str(line).ok()?;
    value.get("timestamp")?.as_str().and_then(rfc3339_ms)
}

fn prepare(kind: Kind, path: &Path, text: &str, md: &std::fs::Metadata) -> Prepared {
    match kind {
        Kind::Claude => claude_meta(path, text, md),
        Kind::Codex => codex_meta(path, text, md),
    }
}

fn claude_client(entrypoint: &str) -> Client {
    match entrypoint {
        "claude-vscode" => Client::Vscode,
        "claude-desktop" => Client::Desktop,
        e if e.starts_with("sdk-") => Client::Headless,
        _ => Client::Terminal,
    }
}

fn claude_meta(path: &Path, text: &str, md: &std::fs::Metadata) -> Prepared {
    let Some(id) = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|id| is_uuid(id) && valid_session_id(id))
    else {
        return Prepared::Skip(CLAUDE);
    };
    let (mut cwd, mut started, mut client) = (None, None, None);
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if cwd.is_none() {
            cwd = value
                .get("cwd")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from);
        }
        if started.is_none() {
            started = value.get("timestamp").and_then(Value::as_str).and_then(rfc3339_ms);
        }
        if client.is_none() {
            client = value.get("entrypoint").and_then(Value::as_str).map(claude_client);
        }
        if cwd.is_some() && started.is_some() && client.is_some() {
            break;
        }
    }
    let started_ms = started.unwrap_or_else(|| modified_ms(md));
    // The last line may be half written; the last one that parses gives the time.
    let updated_ms = text.lines().rev().find_map(line_time).unwrap_or(started_ms);
    Prepared::Copy(SessionMeta {
        agent: CLAUDE.into(),
        session_id: id.to_string(),
        tab_id: String::new(),
        cwd: cwd.unwrap_or_default(),
        transcript_path: path.to_string_lossy().into_owned(),
        started_ms,
        updated_ms: updated_ms.max(started_ms),
        previous_in_tab: None,
        recordings: Vec::new(),
        client: client.unwrap_or(Client::Terminal),
        preset: None,
    })
}

fn codex_meta(path: &Path, text: &str, md: &std::fs::Metadata) -> Prepared {
    let Some(meta) = text.lines().next().and_then(rollout::meta) else {
        return Prepared::Skip(CODEX);
    };
    if !meta.listed || !valid_session_id(&meta.id) {
        return Prepared::Skip(CODEX);
    }
    let started_ms = if meta.started_ms == 0 {
        modified_ms(md)
    } else {
        meta.started_ms
    };
    let updated_ms = text.lines().rev().find_map(line_time).unwrap_or(started_ms);
    Prepared::Copy(SessionMeta {
        agent: CODEX.into(),
        session_id: meta.id,
        tab_id: String::new(),
        cwd: meta.cwd,
        transcript_path: path.to_string_lossy().into_owned(),
        started_ms,
        updated_ms: updated_ms.max(started_ms),
        previous_in_tab: None,
        recordings: Vec::new(),
        client: meta.client,
        preset: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::RecordingLink;
    use serde_json::json;
    use std::time::SystemTime;

    const CLAUDE_A: &str = "00000000-0000-4000-8000-0000000000a1";
    const CLAUDE_B: &str = "00000000-0000-4000-8000-0000000000a2";
    const CLAUDE_C: &str = "00000000-0000-4000-8000-0000000000a3";
    const CLAUDE_D: &str = "00000000-0000-4000-8000-0000000000a4";
    const CODEX_A: &str = "00000000-0000-4000-8000-0000000000c1";
    const CODEX_SUB: &str = "00000000-0000-4000-8000-0000000000c4";

    const FAST: Timing = Timing {
        settle: Duration::from_millis(100),
        rescan: Duration::from_millis(300),
    };

    struct Rig {
        home: tempfile::TempDir,
        claude: tempfile::TempDir,
        codex: tempfile::TempDir,
        history: Arc<History>,
    }

    fn rig() -> Rig {
        let home = tempfile::tempdir().unwrap();
        let history = Arc::new(History::open(home.path()));
        assert!(history.is_on());
        Rig {
            home,
            claude: tempfile::tempdir().unwrap(),
            codex: tempfile::tempdir().unwrap(),
            history,
        }
    }

    impl Rig {
        fn roots(&self) -> ImportRoots {
            ImportRoots {
                claude: Some(self.claude.path().to_path_buf()),
                codex: Some(self.codex.path().to_path_buf()),
            }
        }

        fn start(&self, on: bool, timing: Timing) -> Arc<Importer> {
            Importer::start_with(self.history.clone(), self.roots(), on, timing)
        }

        /// Like `start`, with a small cap on the size of a source file.
        fn start_capped(&self, timing: Timing, max_source: u64) -> Arc<Importer> {
            Importer::start_capped(self.history.clone(), self.roots(), true, timing, max_source)
        }

        /// What the importer has recorded of the source file at `path`.
        fn recorded(&self, path: &Path) -> Option<Copied> {
            let state: Option<StateDoc> = store::read(&self.home.path().join("history").join("import.json")).unwrap();
            state?.files.get(&key(path)).cloned()
        }

        /// The decoded archive copy, or None when there is none.
        fn archived(&self, agent: &str, id: &str) -> Option<String> {
            archive::read_archive(self.home.path(), agent, id).unwrap()
        }

        fn sidecar(&self, agent: &str, id: &str) -> Option<SessionMeta> {
            archive::read_meta(self.home.path(), agent, id).unwrap()
        }

        fn claude_path(&self, id: &str) -> PathBuf {
            self.claude.path().join("project-one").join(format!("{id}.jsonl"))
        }
    }

    fn claude_user_line(entrypoint: &str, ts: &str, text: &str) -> String {
        json!({
            "isSidechain": false, "entrypoint": entrypoint, "cwd": "C:/work/demo", "version": "2.1.309",
            "type": "user", "message": { "role": "user", "content": text },
            "uuid": "00000000-0000-4000-8000-000000000003", "timestamp": ts,
        })
        .to_string()
    }

    fn write_claude(rig: &Rig, id: &str, entrypoint: &str, ts: &str) -> PathBuf {
        let path = rig.claude_path(id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{}\n", claude_user_line(entrypoint, ts, "hello there"))).unwrap();
        path
    }

    fn codex_lines(id: &str, originator: &str, thread_source: &str, ts: &str) -> String {
        let meta = json!({
            "timestamp": ts, "ordinal": 0, "type": "session_meta",
            "payload": { "id": id, "timestamp": ts, "cwd": "/work/demo", "cli_version": "0.163.0",
                "originator": originator, "source": "cli", "thread_source": thread_source },
        });
        let started = json!({
            "timestamp": ts, "ordinal": 1, "type": "event_msg",
            "payload": { "type": "task_started", "turn_id": "00000000-0000-4000-8000-0000000000b1" },
        });
        let message = json!({
            "timestamp": ts, "ordinal": 2, "type": "event_msg",
            "payload": { "type": "item_completed", "turn_id": "00000000-0000-4000-8000-0000000000b1",
                "item": { "type": "UserMessage", "id": "u1", "content": [{ "type": "text", "text": "Run the tests" }] } },
        });
        format!("{meta}\n{started}\n{message}\n")
    }

    fn write_codex(rig: &Rig, id: &str, thread_source: &str, ts: &str, zst: bool) -> PathBuf {
        let dir = rig.codex.path().join("2026").join("10").join("21");
        std::fs::create_dir_all(&dir).unwrap();
        let text = codex_lines(id, "codex-tui", thread_source, ts);
        if zst {
            let path = dir.join(format!("rollout-2026-10-21T09-00-00-{id}.jsonl.zst"));
            std::fs::write(&path, zstd::encode_all(text.as_bytes(), 3).unwrap()).unwrap();
            path
        } else {
            let path = dir.join(format!("rollout-2026-10-21T09-00-00-{id}.jsonl"));
            std::fs::write(&path, text).unwrap();
            path
        }
    }

    fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !cond() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Waits until a batch has run and nothing is being copied.
    fn wait_idle(importer: &Importer, total: u32) {
        wait_for("the batch to finish", || {
            let status = importer.status();
            !status.copying && status.total >= total && status.done == status.total
        });
    }

    #[test]
    fn roots_follow_claude_config_dir_and_codex_home() {
        let home = Path::new("home");
        let none = import_roots(|_| None, home);
        assert_eq!(none.claude, Some(home.join(".claude").join("projects")));
        assert_eq!(none.codex, Some(home.join(".codex").join("sessions")));
        let set = import_roots(
            |k| match k {
                "CLAUDE_CONFIG_DIR" => Some("cfg".into()),
                "CODEX_HOME" => Some("cx".into()),
                _ => None,
            },
            home,
        );
        assert_eq!(set.claude, Some(Path::new("cfg").join("projects")));
        assert_eq!(set.codex, Some(Path::new("cx").join("sessions")));
        let empty = import_roots(|_| Some(String::new()), home);
        assert_eq!(empty.claude, none.claude);
    }

    #[test]
    fn the_first_run_copies_every_transcript_newest_first() {
        let rig = rig();
        write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        write_claude(&rig, CLAUDE_B, "cli", "2026-10-22T09:00:00.000Z");
        write_codex(&rig, CODEX_A, "user", "2026-10-21T09:00:00.000Z", true);
        // Slow enough that the first run could not have waited for settle: it takes files as it finds them.
        let importer = rig.start(
            true,
            Timing {
                settle: Duration::from_secs(30),
                rescan: Duration::from_secs(60),
            },
        );
        wait_idle(&importer, 3);
        let status = importer.status();
        assert_eq!((status.done, status.total, status.copying), (3, 3, false));
        for (agent, id) in [("claude", CLAUDE_A), ("claude", CLAUDE_B), ("codex", CODEX_A)] {
            assert!(rig.archived(agent, id).is_some(), "{agent} {id}");
            assert!(
                rig.history.get(agent, id).unwrap().is_some(),
                "{agent} {id} in the index"
            );
        }
        assert_eq!(
            *importer.shared.order.lock().unwrap(),
            [CLAUDE_B, CODEX_A, CLAUDE_A],
            "newest first by first-line time"
        );
        // A .jsonl.zst source is archived as the decoded text.
        let codex = rig.archived("codex", CODEX_A).unwrap();
        assert!(codex.starts_with("{\"timestamp\""), "{codex}");
        assert_eq!(rig.sidecar("codex", CODEX_A).unwrap().client, Client::Terminal);
        assert!(importer.status().on);
    }

    #[test]
    fn a_file_is_copied_again_when_it_grows_and_its_size_settles() {
        let rig = rig();
        let path = write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        let importer = rig.start(
            true,
            Timing {
                settle: Duration::from_millis(500),
                rescan: Duration::from_millis(150),
            },
        );
        wait_for("the first copy", || rig.archived("claude", CLAUDE_A).is_some());
        let first = rig.archived("claude", CLAUDE_A).unwrap();

        // Growing faster than the settle time: no copy until it stops.
        let mut text = std::fs::read_to_string(&path).unwrap();
        for n in 0..12 {
            text.push_str(&claude_user_line(
                "cli",
                "2026-10-20T09:05:00.000Z",
                &format!("more {n}"),
            ));
            text.push('\n');
            std::fs::write(&path, &text).unwrap();
            std::thread::sleep(Duration::from_millis(40));
            assert_eq!(
                rig.archived("claude", CLAUDE_A).unwrap(),
                first,
                "copied while still growing"
            );
        }
        wait_for("the second copy", || rig.archived("claude", CLAUDE_A).unwrap() == text);
        drop(importer);
    }

    #[test]
    fn a_half_written_line_is_copied_and_completed_later() {
        let rig = rig();
        let path = write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        let whole = claude_user_line("cli", "2026-10-20T09:01:00.000Z", "second prompt");
        let (head, tail) = whole.split_at(whole.len() / 2);
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(head);
        std::fs::write(&path, &text).unwrap();
        let importer = rig.start(true, FAST);
        wait_for("the copy of the half line", || {
            rig.archived("claude", CLAUDE_A).is_some_and(|a| a == text)
        });
        wait_idle(&importer, 1);
        let meta = rig.sidecar("claude", CLAUDE_A).unwrap();
        assert_eq!(meta.updated_ms, meta.started_ms, "the half line gives no time");
        // The CLI finishes the line.
        text.push_str(tail);
        text.push('\n');
        std::fs::write(&path, &text).unwrap();
        wait_for("the completed line", || {
            rig.archived("claude", CLAUDE_A).is_some_and(|a| a == text)
        });
        // The sidecar is written just after the archive.
        wait_for("the sidecar", || {
            rig.sidecar("claude", CLAUDE_A)
                .is_some_and(|m| m.updated_ms > meta.started_ms)
        });
        drop(importer);
    }

    #[test]
    fn headless_and_vscode_runs_get_their_client() {
        let rig = rig();
        write_claude(&rig, CLAUDE_A, "sdk-cli", "2026-10-20T09:00:00.000Z");
        write_claude(&rig, CLAUDE_B, "claude-vscode", "2026-10-20T09:00:00.000Z");
        write_claude(&rig, CLAUDE_C, "cli", "2026-10-20T09:00:00.000Z");
        write_claude(&rig, CLAUDE_D, "sdk-py", "2026-10-20T09:00:00.000Z");
        let importer = rig.start(true, FAST);
        wait_idle(&importer, 4);
        let client = |id| rig.sidecar("claude", id).unwrap().client;
        assert_eq!(client(CLAUDE_A), Client::Headless);
        assert_eq!(client(CLAUDE_B), Client::Vscode);
        assert_eq!(client(CLAUDE_C), Client::Terminal);
        assert_eq!(client(CLAUDE_D), Client::Headless);
        let row = rig.history.get("claude", CLAUDE_B).unwrap().unwrap();
        assert_eq!((row.tab_id.as_str(), row.client), ("", Client::Vscode));
    }

    #[test]
    fn a_codex_subagent_thread_is_never_copied() {
        let rig = rig();
        write_codex(&rig, CODEX_A, "user", "2026-10-21T09:00:00.000Z", false);
        write_codex(&rig, CODEX_SUB, "subagent", "2026-10-21T09:05:00.000Z", false);
        let importer = rig.start(true, FAST);
        wait_idle(&importer, 2);
        assert!(rig.archived("codex", CODEX_A).is_some());
        assert!(rig.archived("codex", CODEX_SUB).is_none());
        assert!(rig.sidecar("codex", CODEX_SUB).is_none());
        assert!(rig.history.get("codex", CODEX_SUB).unwrap().is_none());
        // It is remembered, so a rescan does not read it into a copy either.
        std::thread::sleep(FAST.rescan * 2);
        assert!(rig.archived("codex", CODEX_SUB).is_none());
        let state: StateDoc = store::read(&rig.home.path().join("history").join("import.json"))
            .unwrap()
            .unwrap();
        assert_eq!(state.files.len(), 2);
        assert!(state.files.values().any(|c| c.session_id.is_empty()));
    }

    #[test]
    fn a_marshell_session_seen_by_the_importer_stays_marshell() {
        let rig = rig();
        let path = write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        // The hook path saw it first, in tab t1, with a recording.
        let hook = json!({
            "hook_event_name": "SessionStart", "session_id": CLAUDE_A, "cwd": "C:/work/demo",
            "transcript_path": path.to_string_lossy(),
        });
        rig.history
            .note_event("claude", "t1", Some("t1-1000.cast"), &hook, 1000);
        let before = rig.sidecar("claude", CLAUDE_A).unwrap();
        assert_eq!(before.tab_id, "t1");
        assert_eq!(before.recordings.len(), 1);

        let importer = rig.start(true, FAST);
        wait_for("the import", || rig.archived("claude", CLAUDE_A).is_some());
        wait_idle(&importer, 1);
        let meta = rig.sidecar("claude", CLAUDE_A).unwrap();
        assert_eq!(meta.tab_id, "t1");
        assert_eq!(meta.client, Client::Marshell);
        assert_eq!(
            meta.recordings,
            vec![RecordingLink {
                file: "t1-1000.cast".into(),
                first_ms: 1000,
                last_ms: 1000
            }]
        );
        let row = rig.history.get("claude", CLAUDE_A).unwrap().unwrap();
        assert_eq!((row.tab_id.as_str(), row.client), ("t1", Client::Marshell));
    }

    #[test]
    fn the_importer_keeps_a_codex_tab_session_marshells() {
        for note_first in [true, false] {
            let rig = rig();
            let path = write_codex(&rig, CODEX_A, "user", "2026-10-21T09:00:00.000Z", false);
            let hook = json!({
                "hook_event_name": "SessionStart", "session_id": CODEX_A, "cwd": "C:/work/demo",
                "transcript_path": path.to_string_lossy(),
            });
            let note = || {
                assert!(rig.history.note_event("codex", "t1", None, &hook, 1000).is_none());
            };
            if note_first {
                note();
            }
            let importer = rig.start(true, FAST);
            wait_for("the import", || rig.archived("codex", CODEX_A).is_some());
            wait_idle(&importer, 1);
            if !note_first {
                note();
            }
            let meta = rig.sidecar("codex", CODEX_A).unwrap();
            assert_eq!((meta.tab_id.as_str(), meta.client), ("t1", Client::Marshell));
            let row = rig.history.get("codex", CODEX_A).unwrap().unwrap();
            assert_eq!((row.tab_id.as_str(), row.client), ("t1", Client::Marshell));
        }
    }

    #[test]
    fn a_deleted_copy_stays_deleted_until_its_file_grows() {
        let rig = rig();
        let path = write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        let importer = rig.start(true, FAST);
        wait_idle(&importer, 1);
        rig.history.delete_copy("claude", CLAUDE_A).unwrap();
        assert!(rig.archived("claude", CLAUDE_A).is_none());
        // Several rescans and settle times pass with the file unchanged.
        std::thread::sleep(FAST.rescan * 4);
        assert!(rig.archived("claude", CLAUDE_A).is_none());
        assert!(rig.history.get("claude", CLAUDE_A).unwrap().is_none());
        // It grows: it is copied again.
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(&claude_user_line("cli", "2026-10-20T10:00:00.000Z", "back again"));
        text.push('\n');
        std::fs::write(&path, &text).unwrap();
        wait_for("the copy after growth", || {
            rig.archived("claude", CLAUDE_A) == Some(text.clone())
        });
        assert!(rig.history.get("claude", CLAUDE_A).unwrap().is_some());
    }

    /// Copies the Claude file the way a hook would, with the importer never having seen it.
    fn copy_without_the_importer(rig: &Rig, path: &Path) {
        let text = std::fs::read_to_string(path).unwrap();
        let md = std::fs::metadata(path).unwrap();
        let Prepared::Copy(meta) = prepare(Kind::Claude, path, &text, &md) else {
            panic!("the file should be copyable");
        };
        assert!(rig.history.import(&meta, path).unwrap());
    }

    #[test]
    fn a_copy_deleted_while_the_switch_was_off_stays_deleted() {
        let rig = rig();
        let path = write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        copy_without_the_importer(&rig, &path);
        rig.history.delete_copy("claude", CLAUDE_A).unwrap();
        assert!(rig.archived("claude", CLAUDE_A).is_none());
        assert!(archive::deleted_at(rig.home.path(), "claude", CLAUDE_A).is_some());
        // The importer has never recorded this file, so only the delete remembers it.
        let importer = rig.start(true, FAST);
        wait_idle(&importer, 1);
        std::thread::sleep(FAST.rescan * 2);
        assert!(rig.archived("claude", CLAUDE_A).is_none());
        assert!(rig.history.get("claude", CLAUDE_A).unwrap().is_none());
        let state: StateDoc = store::read(&rig.home.path().join("history").join("import.json"))
            .unwrap()
            .unwrap();
        let known = state.files.get(&key(&path)).expect("the file is recorded");
        assert_eq!(known.size, std::fs::metadata(&path).unwrap().len());
        // The delete is still remembered: the file is skipped again after a restart.
        assert!(archive::deleted_at(rig.home.path(), "claude", CLAUDE_A).is_some());
    }

    #[test]
    fn a_deleted_conversation_that_goes_on_is_copied_again() {
        let rig = rig();
        let path = write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        copy_without_the_importer(&rig, &path);
        rig.history.delete_copy("claude", CLAUDE_A).unwrap();
        assert!(archive::deleted_at(rig.home.path(), "claude", CLAUDE_A).is_some());
        // The conversation goes on after the delete: the file's modification time is later.
        std::thread::sleep(Duration::from_millis(50));
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(&claude_user_line("cli", "2026-10-20T10:00:00.000Z", "back again"));
        text.push('\n');
        std::fs::write(&path, &text).unwrap();
        let importer = rig.start(true, FAST);
        wait_for("the copy of the conversation that went on", || {
            rig.archived("claude", CLAUDE_A) == Some(text.clone())
        });
        wait_idle(&importer, 1);
        assert!(rig.history.get("claude", CLAUDE_A).unwrap().is_some());
        assert!(archive::deleted_at(rig.home.path(), "claude", CLAUDE_A).is_none());
    }

    #[test]
    fn a_plain_file_over_the_cap_is_skipped_until_its_size_changes() {
        let rig = rig();
        let path = write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        let small = std::fs::read_to_string(&path).unwrap();
        let cap = small.len() as u64 + 50;
        let mut big = small.clone();
        for n in 0..2 {
            big.push_str(&claude_user_line(
                "cli",
                "2026-10-20T09:05:00.000Z",
                &format!("more {n}"),
            ));
            big.push('\n');
        }
        assert!(big.len() as u64 > cap);
        std::fs::write(&path, &big).unwrap();
        let importer = rig.start_capped(FAST, cap);
        wait_for("the big file to be recorded", || rig.recorded(&path).is_some());
        let recorded = rig.recorded(&path).unwrap();
        assert_eq!((recorded.size, recorded.session_id.as_str()), (big.len() as u64, ""));
        // Rescans pass and it is still not copied.
        std::thread::sleep(FAST.rescan * 3);
        assert!(rig.archived("claude", CLAUDE_A).is_none());
        assert!(rig.history.get("claude", CLAUDE_A).unwrap().is_none());
        // Its size changes to one under the cap: it is looked at again and copied.
        let smaller = small.replace("hello there", "hello");
        assert!(smaller.len() as u64 <= cap && smaller.len() != big.len());
        std::fs::write(&path, &smaller).unwrap();
        wait_for("the copy after the size changed", || {
            rig.archived("claude", CLAUDE_A) == Some(smaller.clone())
        });
        drop(importer);
    }

    #[test]
    fn a_corrupt_zst_is_not_read_again_until_its_size_changes() {
        let rig = rig();
        let path = write_codex(&rig, CODEX_A, "user", "2026-10-21T09:00:00.000Z", true);
        std::fs::write(&path, b"this is not a zstd frame, whatever its name says").unwrap();
        let importer = rig.start(true, FAST);
        wait_for("the corrupt file to be recorded", || rig.recorded(&path).is_some());
        let recorded = rig.recorded(&path).unwrap();
        assert_eq!(
            (recorded.size, recorded.session_id.as_str()),
            (std::fs::metadata(&path).unwrap().len(), "")
        );
        std::thread::sleep(FAST.rescan * 3);
        assert!(rig.archived("codex", CODEX_A).is_none());
        // The CLI rewrites it whole: the size changes and it is copied.
        write_codex(&rig, CODEX_A, "user", "2026-10-21T09:00:00.000Z", true);
        wait_for("the copy after the file was rewritten", || {
            rig.archived("codex", CODEX_A).is_some()
        });
        drop(importer);
    }

    #[test]
    fn the_shutdown_stop_returns_at_once_while_a_file_is_being_copied() {
        let rig = rig();
        write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        // Off at the start, so the file is seen but the worker is not yet in the copy.
        let importer = rig.start(false, FAST);
        std::thread::sleep(FAST.rescan);
        // Holding the gate keeps the worker busy with that file for as long as the test wants.
        let busy = importer.shared.gate.lock().unwrap();
        importer.set_on(true);
        wait_for("the worker to reach the file", || {
            importer.shared.at_gate.load(Ordering::SeqCst)
        });
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let halting = importer.clone();
        let halter = std::thread::spawn(move || {
            halting.halt();
            let _ = done_tx.send(());
        });
        // The worker cannot finish until the gate is released, so a stop that returns in time did not wait for it.
        let returned = done_rx.recv_timeout(Duration::from_secs(5)).is_ok();
        drop(busy);
        halter.join().unwrap();
        assert!(returned, "halt waited for the file being copied");
        // The worker, once let go, stops without starting another file.
        importer.stop();
        assert!(importer.shared.stopped.load(Ordering::SeqCst));
    }

    #[test]
    fn a_folder_that_appears_later_is_picked_up_by_the_rescan() {
        let rig = rig();
        let missing = rig.claude.path().join("not-yet");
        let roots = ImportRoots {
            claude: Some(missing.clone()),
            codex: None,
        };
        let importer = Importer::start_with(rig.history.clone(), roots, true, FAST);
        std::thread::sleep(FAST.rescan * 2);
        assert_eq!(importer.status().total, 0);
        let dir = missing.join("proj");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{CLAUDE_A}.jsonl")),
            format!("{}\n", claude_user_line("cli", "2026-10-20T09:00:00.000Z", "hi")),
        )
        .unwrap();
        wait_for("the copy after the folder appeared", || {
            rig.archived("claude", CLAUDE_A).is_some()
        });
    }

    #[test]
    fn turning_the_switch_off_stops_the_queue() {
        let rig = rig();
        write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z");
        // Off from the start: files are seen and settle, but nothing is copied.
        let importer = rig.start(false, FAST);
        std::thread::sleep(FAST.rescan * 3);
        assert!(rig.archived("claude", CLAUDE_A).is_none());
        assert!(!importer.status().on);
        assert!(!importer.status().copying);
        // On: it scans again and copies.
        importer.set_on(true);
        wait_for("the copy after turning on", || {
            rig.archived("claude", CLAUDE_A).is_some()
        });
        wait_idle(&importer, 1);
        // Off again: a new file is not copied, and it is after turning on.
        importer.set_on(false);
        write_claude(&rig, CLAUDE_B, "cli", "2026-10-21T09:00:00.000Z");
        std::thread::sleep(FAST.rescan * 3);
        assert!(rig.archived("claude", CLAUDE_B).is_none());
        importer.set_on(true);
        wait_for("the copy after turning on again", || {
            rig.archived("claude", CLAUDE_B).is_some()
        });
        assert!(importer.status().on);
    }

    #[test]
    fn nothing_under_the_roots_is_written() {
        let rig = rig();
        let files = [
            write_claude(&rig, CLAUDE_A, "cli", "2026-10-20T09:00:00.000Z"),
            write_claude(&rig, CLAUDE_B, "sdk-cli", "2026-10-22T09:00:00.000Z"),
            write_codex(&rig, CODEX_A, "user", "2026-10-21T09:00:00.000Z", true),
            write_codex(&rig, CODEX_SUB, "subagent", "2026-10-21T09:05:00.000Z", false),
        ];
        type Entry = (PathBuf, Option<(Vec<u8>, SystemTime)>);
        let snapshot = |dir: &Path| -> Vec<Entry> {
            let mut all = Vec::new();
            let mut stack = vec![dir.to_path_buf()];
            while let Some(d) = stack.pop() {
                for entry in std::fs::read_dir(&d).unwrap().flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        all.push((p.clone(), None));
                        stack.push(p);
                    } else {
                        let modified = std::fs::metadata(&p).unwrap().modified().unwrap();
                        all.push((p.clone(), Some((std::fs::read(&p).unwrap(), modified))));
                    }
                }
            }
            all.sort_by(|a, b| a.0.cmp(&b.0));
            all
        };
        let before = (snapshot(rig.claude.path()), snapshot(rig.codex.path()));
        let importer = rig.start(true, FAST);
        wait_idle(&importer, 4);
        std::thread::sleep(FAST.rescan * 2);
        let after = (snapshot(rig.claude.path()), snapshot(rig.codex.path()));
        assert!(before == after, "a file under a root changed");
        assert_eq!(files.len(), 4);
    }
}
