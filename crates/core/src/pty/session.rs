use super::buffer::{MAX_UNACKED, OutputBuffer, SCROLLBACK_BYTES, peek_rows};
use super::dsr::{DSR_REPLY, DsrFilter};
use super::elevated::{self, HostLauncher, PtyControl};
use super::freeze::{self, FreezeError, Frozen};
use super::osc133::{Mark, Osc133Scanner};
use super::record::{CastHeader, Osc633Strip, Recorder};
use crate::secrets;
use crate::watch::Watch;
use marshell_protocol::host::SpawnRequest;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Runs once with the exit code when the process ends.
pub type ExitCallback = Box<dyn FnOnce(i32) + Send>;

/// Tells what a typed command line is. Called with the real words, from the reader thread, outside every session
/// lock; what it returns never holds the line.
pub type Classifier = Arc<dyn Fn(&str) -> Watch + Send + Sync>;

/// Called with each OSC 133 mark the shell prints, from the reader thread, outside every session lock.
pub type MarkCallback = Box<dyn Fn(Mark) + Send + Sync>;

pub struct SpawnSpec {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    /// Variables the child must not inherit. Applied before `env`, so a name in both keeps its `env` value.
    pub env_remove: Vec<String>,
    /// Also drops every inherited variable whose name this accepts. portable-pty fills a child's environment from
    /// Marshell's own and from the registry's `Environment` values, so a list of Marshell's own names would miss a
    /// variable that only the registry holds. Decided at spawn, before `env` is applied.
    pub env_strip: Option<fn(&str) -> bool>,
    pub cols: u16,
    pub rows: u16,
    /// Called once, from the wait thread, with the exit code. Runs outside every session lock.
    pub on_exit: Option<ExitCallback>,
    /// Called for each OSC 133 prompt or command mark in the output.
    pub on_mark: Option<MarkCallback>,
    /// Where to write the asciicast recording, and the SHELL value for its header. A path that cannot
    /// be written leaves the session running without a recording.
    pub record: Option<(PathBuf, Option<String>)>,
    /// Classifies each command line the shell reports. Without it the session watches nothing.
    pub classify: Option<Classifier>,
    /// A command the shell was started with, classified at spawn and dropped at the first prompt mark.
    pub initial_command: Option<String>,
}

/// What `Session::start` needs of a pty, wherever it lives: the local ConPTY or the elevated helper's pipes.
pub struct PtyParts {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub control: Box<dyn PtyControl>,
    /// Blocks until the child ends and gives its exit code.
    pub wait: Box<dyn FnOnce() -> i32 + Send>,
    pub pid: Option<u32>,
    pub elevated: bool,
}

/// The local pty's control: the portable-pty master. Closing it drops the master.
struct LocalControl {
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
}

impl PtyControl for LocalControl {
    fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        if let Some(master) = self.master.lock().unwrap().as_ref() {
            master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        }
        Ok(())
    }

    fn close(&self) {
        // Take it out of the mutex first: closing can block, and `resize` must not wait on it.
        let master = self.master.lock().unwrap().take();
        drop(master);
    }

    #[cfg(unix)]
    fn foreground_group(&self) -> Option<u32> {
        let group = self.master.lock().unwrap().as_ref()?.process_group_leader()?;
        u32::try_from(group).ok()
    }
}

/// Applies the spec's environment steps to `cmd`, in this order: `env_remove`, then the inherited names `env_strip`
/// accepts, then `env`.
fn apply_env(cmd: &mut CommandBuilder, spec: &SpawnSpec) {
    for k in &spec.env_remove {
        cmd.env_remove(k);
    }
    if let Some(strip) = spec.env_strip {
        // The names only: a value is never read here.
        let inherited: Vec<String> = cmd
            .iter_full_env_as_str()
            .filter(|(name, _)| strip(name))
            .map(|(name, _)| name.to_string())
            .collect();
        for name in inherited {
            cmd.env_remove(name);
        }
    }
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
}

/// The whole environment the child gets: Marshell's own, with `apply_env`'s steps. The elevated helper starts with
/// a fresh environment block, so the core sends it this.
pub(crate) fn child_env(spec: &SpawnSpec) -> Vec<(String, String)> {
    let mut cmd = CommandBuilder::new("");
    apply_env(&mut cmd, spec);
    cmd.iter_full_env_as_str()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// Opens a local pty and starts the spec's program in it.
fn local_parts(spec: &SpawnSpec) -> anyhow::Result<PtyParts> {
    let size = PtySize {
        rows: spec.rows.max(1),
        cols: spec.cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = native_pty_system().openpty(size)?;
    let mut cmd = CommandBuilder::from_argv(spec.argv.iter().map(OsString::from).collect());
    if let Some(cwd) = &spec.cwd {
        cmd.cwd(cwd);
    }
    apply_env(&mut cmd, spec);
    let mut child = pair.slave.spawn_command(cmd)?;
    // Our copy of the slave must close, or the reader never sees EOF on Unix.
    drop(pair.slave);
    let pid = child.process_id();
    let reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;
    Ok(PtyParts {
        reader,
        writer,
        control: Box::new(LocalControl {
            master: Mutex::new(Some(pair.master)),
        }),
        wait: Box::new(move || child.wait().map(|st| st.exit_code() as i32).unwrap_or(-1)),
        pid,
        elevated: false,
    })
}

/// What the WebSocket task should send next.
pub enum Pull {
    Data {
        seq: u64,
        bytes: Vec<u8>,
    },
    /// The cursor fell out of the ring: send the screen, then continue from `seq`.
    Reset {
        seq: u64,
        screen: Vec<u8>,
    },
    Exit(i32),
    Idle,
}

/// Everything about the output stream, behind one std Mutex.
/// Rule: lock, change, drop. Never hold it across `.await` or I/O.
struct Output {
    buf: OutputBuffer,
    screen: vt100::Parser,
    dsr: DsrFilter,
    marks: Osc133Scanner,
    attached: bool,
    generation: u64,
    acked: u64,
    exit: Option<(i32, Instant)>,
    reader_done: bool,
    /// Windows: ConPTY's startup cursor query has been answered by the core.
    startup_dsr_answered: bool,
}

/// What the shell's marks say about the command it is running.
#[derive(Default)]
struct CommandTrack {
    running: bool,
    ever_executed: bool,
    line: Option<String>,
    /// Counts the C, D and A marks, so a reading taken before one can tell it is stale.
    epoch: u64,
    /// What the running command is: set from its command line, or from the foreground (`Session::set_watch`).
    watch: Watch,
    /// The epoch the command line set `watch` at. A C mark moves it up with the epoch, so the foreground's later
    /// reading of the same command does not replace it.
    line_watch_epoch: Option<u64>,
}

impl CommandTrack {
    fn starting(classify: Option<&Classifier>, initial_command: Option<&str>) -> Self {
        let mut track = CommandTrack::default();
        if let (Some(classify), Some(line)) = (classify, initial_command) {
            track.watch = classify(line);
            track.line_watch_epoch = Some(track.epoch);
        }
        track
    }
}

/// What the recording has been told about hiding output. Only the reader thread uses it.
#[derive(Default)]
struct RecState {
    strip: Osc633Strip,
    /// A "hidden" marker is the last of its kind written.
    hiding: bool,
}

/// The longest command line that leaves the core, in chars.
const MAX_COMMAND_LINE: usize = 200;

/// The first line of `text`, trimmed, scrubbed of secret flags and arguments, and capped at 200 chars on a char
/// boundary. `None` if empty.
fn shown_command_line(text: &str) -> Option<String> {
    let first = text.lines().next().unwrap_or("").trim();
    let first = secrets::scrub(first);
    let first = first.as_ref();
    let capped = match first.char_indices().nth(MAX_COMMAND_LINE) {
        Some((end, _)) => &first[..end],
        None => first,
    };
    (!capped.is_empty()).then(|| capped.to_string())
}

pub struct Session {
    id: String,
    command: Mutex<CommandTrack>,
    /// The folder the shell last reported (OSC 9;9 or OSC 7).
    cwd: Mutex<Option<PathBuf>>,
    output: Mutex<Output>,
    /// The reader thread waits here while the client is more than MAX_UNACKED behind.
    space: Condvar,
    /// Wakes the async WebSocket task when there is something new to pull.
    notify: tokio::sync::Notify,
    /// Input goes through a channel to a writer thread, so async code never blocks on the pty.
    input: mpsc::Sender<Vec<u8>>,
    /// The pty's control, local or through the elevated helper. Taken out when the session ends.
    control: Mutex<Option<Arc<dyn PtyControl>>>,
    pid: Option<u32>,
    /// The session runs through the elevated helper: it takes no input from the API, cannot be paused, and is
    /// ended by closing its pipes, never by `taskkill`.
    elevated: bool,
    /// Set when the child has exited, so a late kill cannot hit a reused pid.
    exited: Arc<AtomicBool>,
    /// Taken and called once when the child exits.
    exit_callback: Mutex<Option<ExitCallback>>,
    mark_callback: Option<MarkCallback>,
    recorder: Option<Recorder>,
    classify: Option<Classifier>,
    rec: Mutex<RecState>,
    /// Set while the process tree is frozen. Held for the whole freeze, so `on_exit` waits for it.
    frozen: Mutex<Option<Frozen>>,
}

impl Session {
    pub fn spawn(id: String, spec: SpawnSpec) -> anyhow::Result<Arc<Session>> {
        let parts = local_parts(&spec)?;
        Session::start(id, spec, parts)
    }

    /// Starts the session in the elevated helper (`launcher` starts `host_exe`). The returned error is an
    /// `ElevatedError` underneath, so a caller can tell a declined prompt from the rest.
    pub fn spawn_elevated(
        id: String,
        spec: SpawnSpec,
        launcher: &dyn HostLauncher,
        host_exe: &Path,
    ) -> anyhow::Result<Arc<Session>> {
        let request = SpawnRequest {
            argv: spec.argv.clone(),
            cwd: spec.cwd.as_ref().map(|c| c.to_string_lossy().into_owned()),
            env: child_env(&spec),
            cols: spec.cols.max(1),
            rows: spec.rows.max(1),
        };
        let remote = elevated::connect(launcher, host_exe, request).map_err(anyhow::Error::new)?;
        let parts = PtyParts {
            reader: remote.reader,
            writer: remote.writer,
            control: remote.control,
            wait: remote.wait,
            pid: Some(remote.pid),
            elevated: true,
        };
        Session::start(id, spec, parts)
    }

    /// Runs the session on the local backend but marks it elevated, so the API's guards can be tested without the
    /// helper binary. Production never calls this.
    #[doc(hidden)]
    pub fn spawn_marked_elevated_for_tests(id: String, spec: SpawnSpec) -> anyhow::Result<Arc<Session>> {
        let mut parts = local_parts(&spec)?;
        parts.elevated = true;
        Session::start(id, spec, parts)
    }

    /// The part of starting a session that does not depend on where its pty lives.
    fn start(id: String, mut spec: SpawnSpec, parts: PtyParts) -> anyhow::Result<Arc<Session>> {
        let size = PtySize {
            rows: spec.rows.max(1),
            cols: spec.cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        };
        let PtyParts {
            reader,
            mut writer,
            control,
            wait,
            pid,
            elevated,
        } = parts;
        let (input_tx, input_rx) = mpsc::channel::<Vec<u8>>();
        let recorder = spec.record.take().and_then(|(path, shell)| {
            let header = CastHeader {
                cols: size.cols,
                rows: size.rows,
                started_unix: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs()),
                shell,
            };
            Recorder::start(path.clone(), header)
                .inspect_err(|e| tracing::warn!("session {id} is not recorded to {}: {e}", path.display()))
                .ok()
        });

        let session = Arc::new(Session {
            id: id.clone(),
            command: Mutex::new(CommandTrack::starting(
                spec.classify.as_ref(),
                spec.initial_command.as_deref(),
            )),
            cwd: Mutex::new(None),
            output: Mutex::new(Output {
                buf: OutputBuffer::new(SCROLLBACK_BYTES),
                screen: vt100::Parser::new(size.rows, size.cols, 0),
                dsr: DsrFilter::default(),
                marks: Osc133Scanner::default(),
                attached: false,
                generation: 0,
                acked: 0,
                exit: None,
                reader_done: false,
                startup_dsr_answered: false,
            }),
            space: Condvar::new(),
            notify: tokio::sync::Notify::new(),
            input: input_tx,
            control: Mutex::new(Some(Arc::from(control))),
            pid,
            elevated,
            exited: Arc::new(AtomicBool::new(false)),
            exit_callback: Mutex::new(spec.on_exit.take()),
            mark_callback: spec.on_mark.take(),
            recorder,
            classify: spec.classify.take(),
            rec: Mutex::new(RecState::default()),
            frozen: Mutex::new(None),
        });

        std::thread::Builder::new()
            .name(format!("pty-write-{id}"))
            .spawn(move || {
                for bytes in input_rx {
                    if writer.write_all(&bytes).and_then(|_| writer.flush()).is_err() {
                        break;
                    }
                }
            })?;

        let s = session.clone();
        std::thread::Builder::new()
            .name(format!("pty-read-{id}"))
            .spawn(move || s.read_loop(reader))?;

        let s = session.clone();
        std::thread::Builder::new()
            .name(format!("pty-wait-{id}"))
            .spawn(move || {
                let code = wait();
                s.on_exit(code);
            })?;

        Ok(session)
    }

    /// True when the session runs as administrator through the helper.
    pub fn is_elevated(&self) -> bool {
        self.elevated
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// The child's process id, if it had one.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// True once the child process has exited.
    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    /// The exit code once the child has exited. It is set before the exit callback runs,
    /// so `None` means the callback has not run yet.
    pub fn exit_code(&self) -> Option<i32> {
        self.output.lock().unwrap().exit.map(|(code, _)| code)
    }

    fn read_loop(&self, mut reader: Box<dyn Read + Send>) {
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            let n = match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let answered = self.ingest(&chunk[..n]);
            for _ in 0..answered {
                self.write_input(DSR_REPLY.to_vec());
            }
            self.wait_for_space();
        }
        if let Some(rec) = &self.recorder {
            rec.close();
        }
        self.output.lock().unwrap().reader_done = true;
        self.notify.notify_one();
    }

    /// Appends output to the ring and the screen. Returns how many ESC[6n queries we must answer.
    fn ingest(&self, bytes: &[u8]) -> usize {
        let mut out = self.output.lock().unwrap();
        // ConPTY holds back all output until its startup query is answered, and a client can attach
        // before that query arrives, so the core answers the first one even when attached.
        let startup_pending = cfg!(windows) && !out.startup_dsr_answered;
        let (mut pass, answered) = if out.attached && !startup_pending {
            (bytes.to_vec(), 0)
        } else {
            out.dsr.filter(bytes)
        };
        if answered > 0 && !out.startup_dsr_answered {
            out.startup_dsr_answered = true;
            if out.attached {
                // The filter stops here, so release any bytes it was holding back.
                let carry = out.dsr.take_carry();
                pass.extend(carry);
            }
        }
        // Scan the raw bytes: the DSR filter may hold some back, but marks never contain ESC[6n.
        let marks = out.marks.scan(bytes);
        out.screen.process(&pass);
        out.buf.push(&pass);
        drop(out);
        // The marks are folded before the chunk is recorded: a command line in this chunk can make the rest of it
        // secret, and the foreground may already have said so before it.
        let secret_before = self.secret_label();
        let secret_started = self.track_command(&marks);
        let hide = secret_before.or(secret_started);
        self.notify.notify_one();
        // The recording gets what the terminal got, after the lock is dropped, but never a secret command's output.
        if let Some(rec) = &self.recorder {
            self.record_chunk(rec, &pass, &marks, hide);
        }
        // No session lock is held now: the callback takes the registry's lock.
        if let Some(cb) = &self.mark_callback {
            for mark in marks {
                cb(mark);
            }
        }
        answered
    }

    /// Writes one chunk to the recording. `hide` names the secret command whose output the chunk may hold: the watch
    /// was on before its marks were folded, or a command line in it turned the watch on. The chunk is then not
    /// recorded, even when the watch is off again by its end.
    fn record_chunk(&self, rec: &Recorder, pass: &[u8], marks: &[Mark], hide: Option<String>) {
        let mut state = self.rec.lock().unwrap();
        // The filter sees every chunk, hidden or not, to follow a sequence cut between two reads.
        let clean = state.strip.filter(pass);
        match &hide {
            Some(label) => {
                if !state.hiding {
                    state.hiding = true;
                    rec.marker(&format!("Output hidden: {label} prints secrets"));
                }
            }
            None => {
                // The watch ended between chunks (the foreground said so): the marker goes before this output.
                if state.hiding {
                    state.hiding = false;
                    rec.marker("Output recorded again");
                }
                if !clean.is_empty() {
                    rec.output(&clean);
                }
            }
        }
        for mark in marks {
            if *mark == Mark::PromptStart {
                rec.marker("prompt");
            }
        }
        if state.hiding && !self.secret_now() {
            state.hiding = false;
            rec.marker("Output recorded again");
        }
    }

    /// The label of the running command if it prints secrets.
    fn secret_label(&self) -> Option<String> {
        let track = self.command.lock().unwrap();
        track.watch.secret.then(|| track.watch.label.clone())
    }

    /// True while the watched command prints secrets.
    fn secret_now(&self) -> bool {
        self.command.lock().unwrap().watch.secret
    }

    /// Folds the shell's marks into the command tracking, before the mark callback sees them. Returns the label of
    /// a secret command that a command line among them put on watch, even if a later mark dropped it again.
    fn track_command(&self, marks: &[Mark]) -> Option<String> {
        if marks.is_empty() {
            return None;
        }
        // The classifier runs before the lock is taken.
        let watches: Vec<Option<Watch>> = marks
            .iter()
            .map(|mark| match (mark, &self.classify) {
                (Mark::CommandLine(text), Some(classify)) => Some(classify(text)),
                _ => None,
            })
            .collect();
        let mut started = None;
        let mut track = self.command.lock().unwrap();
        for (mark, watch) in marks.iter().zip(watches) {
            match mark {
                Mark::CommandLine(text) => {
                    track.line = shown_command_line(text);
                    if let Some(watch) = watch {
                        if watch.secret {
                            started = Some(watch.label.clone());
                        }
                        track.watch = watch;
                        track.line_watch_epoch = Some(track.epoch);
                    }
                }
                Mark::Executed => {
                    track.running = true;
                    track.ever_executed = true;
                    track.epoch += 1;
                    track.line_watch_epoch = track.line_watch_epoch.map(|_| track.epoch);
                }
                Mark::Finished(_) | Mark::PromptStart => {
                    track.running = false;
                    track.line = None;
                    track.epoch += 1;
                    track.watch = Watch::default();
                    track.line_watch_epoch = None;
                }
                Mark::CommandStart => {}
                Mark::Notify(_) | Mark::Progress(_) | Mark::Bell => {}
                Mark::Cwd(path) => *self.cwd.lock().unwrap() = Some(PathBuf::from(path)),
            }
        }
        started
    }

    /// Sets the watch from the foreground process of the command that `epoch` was read at. Refused when a C, D or A
    /// mark came since. True when the watch changed.
    ///
    /// The command's own line is the better source of what the command is, so a watch it set stays. The process can
    /// still tell more: an alias hides `gh auth token` or `vercel login` from the line, so a reading may turn `secret`
    /// or `quiet_ports` on (it never turns either off).
    pub fn set_watch(&self, epoch: u64, watch: Watch) -> bool {
        let mut track = self.command.lock().unwrap();
        if track.epoch != epoch {
            return false;
        }
        let next = if track.line_watch_epoch == Some(epoch) {
            let mut next = track.watch.clone();
            if watch.secret && !next.secret {
                next.secret = true;
                next.label = watch.label;
            }
            next.quiet_ports |= watch.quiet_ports;
            next
        } else {
            watch
        };
        let changed = track.watch.differs(&next);
        if changed {
            track.watch = next;
        }
        changed
    }

    /// Whether the watched command is a login (`vercel login`'s callback): the port watcher makes no chip for a
    /// listener while it is set.
    pub fn quiet_ports(&self) -> bool {
        self.command.lock().unwrap().watch.quiet_ports
    }

    /// The last folder the shell reported, or `None` if it never reported one.
    pub fn current_cwd(&self) -> Option<PathBuf> {
        self.cwd.lock().unwrap().clone()
    }

    /// True between the shell's C mark and the next D or A mark.
    pub fn command_running(&self) -> bool {
        self.command.lock().unwrap().running
    }

    /// True once the shell has sent a C mark. A shell that never does has no integration.
    pub fn ever_executed(&self) -> bool {
        self.command.lock().unwrap().ever_executed
    }

    /// How many C, D and A marks the shell has sent. A reading taken at one value is stale at another.
    pub fn command_epoch(&self) -> u64 {
        self.command.lock().unwrap().epoch
    }

    /// The command line the shell reported for the running command, as shown: its first line,
    /// trimmed and capped at 200 chars. `None` when none was reported or the command has finished.
    pub fn command_line(&self) -> Option<String> {
        self.command.lock().unwrap().line.clone()
    }

    fn wait_for_space(&self) {
        let mut out = self.output.lock().unwrap();
        while out.attached && out.exit.is_none() && out.buf.end().saturating_sub(out.acked) > MAX_UNACKED {
            out = self.space.wait(out).unwrap();
        }
    }

    fn on_exit(self: &Arc<Self>, code: i32) {
        self.exited.store(true, Ordering::SeqCst);
        // The root's own threads are gone, but a descendant it left behind stays suspended unless
        // it is resumed through the held handles. (On Unix the kernel continues an orphaned
        // stopped group itself.)
        let frozen = self.frozen.lock().unwrap().take();
        #[cfg(windows)]
        if let Some(frozen) = frozen {
            freeze::thaw_tree(&frozen);
        }
        #[cfg(not(windows))]
        drop(frozen);
        self.output.lock().unwrap().exit = Some((code, Instant::now()));
        self.space.notify_all();
        // Windows: the reader only gets EOF once the pseudoconsole is closed.
        // Take it out of the mutex first: closing can block, and `resize` must not wait on it.
        let control = self.control.lock().unwrap().take();
        if let Some(control) = control {
            control.close();
        }
        self.notify.notify_one();
        // `pull` gives up waiting for EOF after 500 ms; wake the async task so it re-pulls then.
        let s = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(550));
            s.notify.notify_one();
        });
        // Take the callback out of its mutex, then call it with no session lock held:
        // the registry's callback takes its own lock.
        let callback = self.exit_callback.lock().unwrap().take();
        if let Some(callback) = callback {
            callback(code);
        }
    }

    pub fn write_input(&self, bytes: Vec<u8>) {
        let _ = self.input.send(bytes);
    }

    /// Whether the program in the session asked for bracketed paste (DECSET 2004).
    pub fn bracketed_paste(&self) -> bool {
        self.output.lock().unwrap().screen.screen().bracketed_paste()
    }

    /// Copies the last rendered rows as plain text while holding the output lock.
    pub fn peek(&self, lines: usize) -> Vec<String> {
        let out = self.output.lock().unwrap();
        peek_rows(out.screen.screen(), lines)
    }

    /// Ignores 0x0 (a minimized window reports it).
    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        if cols == 0 || rows == 0 {
            return Ok(());
        }
        // The lock is let go before the call: through the administrator helper it can wait on a pipe write, and
        // Close needs this lock to end the session.
        let control = self.control.lock().unwrap().clone();
        if let Some(control) = control {
            control.resize(cols, rows)?;
        }
        // vt100 0.16; on 0.15 this is `parser.set_size(rows, cols)`.
        let changed = {
            let mut out = self.output.lock().unwrap();
            let changed = out.screen.screen().size() != (rows, cols);
            out.screen.screen_mut().set_size(rows, cols);
            changed
        };
        if changed && let Some(rec) = &self.recorder {
            rec.resize(cols, rows);
        }
        Ok(())
    }

    /// The file this session is recorded to. `None` when recording never started or a disk error stopped it.
    pub fn recording_path(&self) -> Option<PathBuf> {
        self.recorder
            .as_ref()
            .filter(|r| r.is_recording())
            .map(|r| r.path().to_path_buf())
    }

    /// The recording while its writer still holds the file, including a pending close.
    pub fn open_recording(&self) -> Option<PathBuf> {
        self.recorder
            .as_ref()
            .filter(|r| r.is_recording() && !r.is_closed())
            .map(|r| r.path().to_path_buf())
    }

    /// A renderer connected. Returns a generation number for `detach`, and the offset where live
    /// output starts. From that exact byte the renderer (xterm) answers queries itself and the core
    /// stops answering cursor queries, so no query falls between the two.
    pub fn attach(&self) -> (u64, u64) {
        let mut out = self.output.lock().unwrap();
        let carry = out.dsr.take_carry();
        out.screen.process(&carry);
        out.buf.push(&carry);
        out.attached = true;
        out.generation += 1;
        (out.generation, out.buf.end())
    }

    /// Only the latest connection may detach (a reconnect replaces the old socket).
    pub fn detach(&self, generation: u64) {
        let mut out = self.output.lock().unwrap();
        if out.generation == generation {
            out.attached = false;
        }
        drop(out);
        self.space.notify_all();
    }

    /// The client has everything before `from`.
    pub fn resume(&self, from: u64) {
        let mut out = self.output.lock().unwrap();
        out.acked = from.min(out.buf.end());
        drop(out);
        self.space.notify_all();
        self.notify.notify_one();
    }

    pub fn ack(&self, upto: u64) {
        let mut out = self.output.lock().unwrap();
        let upto = upto.min(out.buf.end());
        if upto > out.acked {
            out.acked = upto;
        }
        drop(out);
        self.space.notify_all();
    }

    pub fn pull(&self, cursor: u64, max: usize) -> Pull {
        let out = self.output.lock().unwrap();
        match out.buf.read_from(cursor, max) {
            None => Pull::Reset {
                seq: out.buf.end(),
                screen: out.screen.screen().state_formatted(),
            },
            Some(bytes) if !bytes.is_empty() => Pull::Data { seq: cursor, bytes },
            Some(_) => match out.exit {
                // All output is out once the reader is done; on Windows EOF can be late, so give up after 500 ms.
                Some((code, at)) if out.reader_done || at.elapsed() > Duration::from_millis(500) => Pull::Exit(code),
                _ => Pull::Idle,
            },
        }
    }

    /// Bytes waiting after `cursor` (used for the 64 KiB flush rule).
    pub fn available(&self, cursor: u64) -> u64 {
        self.output.lock().unwrap().buf.end().saturating_sub(cursor)
    }

    pub fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.notify.notified()
    }

    /// (end of stream, acked). For tests and the perf panel.
    pub fn counters(&self) -> (u64, u64) {
        let out = self.output.lock().unwrap();
        (out.buf.end(), out.acked)
    }

    /// Politely kills the whole process tree. Never blocks the caller.
    /// Unix: SIGHUP now, SIGKILL after 2 s if the child has not exited by then.
    /// Windows: taskkill /T /F on a helper thread.
    pub fn kill(&self) {
        if self.exited.load(Ordering::SeqCst) {
            return;
        }
        self.thaw();
        if self.elevated {
            self.close_control();
        } else if let Some(pid) = self.pid {
            kill_tree(pid, self.exited.clone());
        }
    }

    /// Kills the whole process tree right now and waits for it. Use when the process is about to exit.
    pub fn force_kill(&self) {
        if self.exited.load(Ordering::SeqCst) {
            return;
        }
        self.thaw();
        if self.elevated {
            self.close_control();
        } else if let Some(pid) = self.pid {
            force_kill_tree(pid);
        }
    }

    /// Closes the pipes to the helper, which ends the shell and its tree. A core that is not elevated could not
    /// `taskkill` an elevated process anyway.
    fn close_control(&self) {
        let control = self.control.lock().unwrap().take();
        if let Some(control) = control {
            control.close();
        }
    }

    /// Freezes the whole process tree, or none of it. Freezing a frozen session does nothing.
    pub fn freeze(&self) -> Result<(), FreezeError> {
        if self.elevated {
            return Err(FreezeError::Elevated);
        }
        let mut frozen = self.frozen.lock().unwrap();
        if frozen.is_some() {
            return Ok(());
        }
        let Some(pid) = self.pid.filter(|_| !self.exited.load(Ordering::SeqCst)) else {
            return Err(FreezeError::NotRunning);
        };
        #[cfg(unix)]
        let foreground = self.control.lock().unwrap().as_ref().and_then(|c| c.foreground_group());
        #[cfg(not(unix))]
        let foreground = None;
        *frozen = Some(freeze::freeze_tree(pid, foreground).map_err(|()| FreezeError::Partial)?);
        Ok(())
    }

    /// Thaws a frozen session. Does nothing if it is not frozen.
    pub fn thaw(&self) {
        let frozen = self.frozen.lock().unwrap().take();
        if let Some(frozen) = frozen {
            freeze::thaw_tree(&frozen);
        }
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen.lock().unwrap().is_some()
    }

    /// The programs running in front of the shell as `(pid, name)`, outermost first, the names as
    /// `detect::program_name` gives them. Empty when the shell has no child. `None` when this cannot be read.
    /// Blocking.
    pub fn foreground_chain_pids(&self) -> Option<Vec<(u32, String)>> {
        let pid = self.pid.filter(|_| !self.exited.load(Ordering::SeqCst))?;
        #[cfg(windows)]
        {
            super::proctree::foreground_chain_pids(pid)
        }
        #[cfg(target_os = "linux")]
        {
            let group = self
                .control
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|c| c.foreground_group())?;
            if group == pid {
                return Some(Vec::new());
            }
            let comm = std::fs::read_to_string(format!("/proc/{group}/comm")).ok()?;
            Some(vec![(group, crate::profiles::detect::program_name(comm.trim()))])
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = pid;
            None
        }
    }

    /// The programs running in front of the shell, outermost first, as `detect::program_name` names them.
    /// Empty when the shell has no child. `None` when this cannot be read.
    pub fn foreground_chain(&self) -> Option<Vec<String>> {
        Some(names_of(&self.foreground_chain_pids()?))
    }

    /// The words of the deepest foreground process's command line. `None` when nothing runs or it cannot be read.
    /// Blocking.
    pub fn foreground_argv(&self) -> Option<Vec<String>> {
        argv_of(&self.foreground_chain_pids()?)
    }

    /// The chain's names and the deepest process's words from one read of the process list. Blocking.
    pub fn foreground_reading(&self) -> (Option<Vec<String>>, Option<Vec<String>>) {
        match self.foreground_chain_pids() {
            Some(chain) => (Some(names_of(&chain)), argv_of(&chain)),
            None => (None, None),
        }
    }
}

fn names_of(chain: &[(u32, String)]) -> Vec<String> {
    chain.iter().map(|(_, name)| name.clone()).collect()
}

fn argv_of(chain: &[(u32, String)]) -> Option<Vec<String>> {
    let (pid, _) = chain.last()?;
    let argv = super::split_args(&super::command_line(*pid)?);
    (!argv.is_empty()).then_some(argv)
}

#[cfg(windows)]
fn taskkill(pid: u32) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[cfg(windows)]
fn kill_tree(pid: u32, _exited: Arc<AtomicBool>) {
    std::thread::spawn(move || taskkill(pid));
}

#[cfg(windows)]
pub(crate) fn force_kill_tree(pid: u32) {
    taskkill(pid);
}

#[cfg(unix)]
fn kill_tree(pid: u32, exited: Arc<AtomicBool>) {
    // portable-pty starts the child in its own session, so its pid is the process group id.
    unsafe {
        libc::killpg(pid as i32, libc::SIGHUP);
        // A stopped process acts on the hang-up only once it continues.
        libc::killpg(pid as i32, libc::SIGCONT);
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        // After the child exits its pid may be reused, so only escalate while it is still ours.
        if !exited.load(Ordering::SeqCst) {
            unsafe {
                libc::killpg(pid as i32, libc::SIGKILL);
            }
        }
    });
}

#[cfg(unix)]
pub(crate) fn force_kill_tree(pid: u32) {
    unsafe {
        libc::killpg(pid as i32, libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_spec(env: Vec<(String, String)>, env_remove: Vec<String>, env_strip: Option<fn(&str) -> bool>) -> SpawnSpec {
        SpawnSpec {
            argv: vec![],
            cwd: None,
            env,
            env_remove,
            env_strip,
            cols: 80,
            rows: 24,
            on_exit: None,
            on_mark: None,
            record: None,
            classify: None,
            initial_command: None,
        }
    }

    /// The test process's variables are never changed, so the strip step is tried on names the process already has
    /// (`SystemRoot` or `HOME`) and on a prefix no inherited name has.
    #[test]
    fn child_env_applies_remove_strip_and_env_in_order() {
        fn strip(name: &str) -> bool {
            name.starts_with("ZZ_STRIP_") || name.eq_ignore_ascii_case("SystemRoot") || name == "HOME"
        }
        let inherited = |name: &str| std::env::vars_os().any(|(k, _)| k.to_string_lossy().eq_ignore_ascii_case(name));
        let spec = env_spec(
            vec![
                ("PATH".into(), "/set-by-env".into()),
                ("ZZ_STRIP_AFTER".into(), "kept".into()),
            ],
            vec!["PATH".into()],
            Some(strip),
        );
        let env = child_env(&spec);
        let get = |name: &str| {
            env.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        };
        // Removed, then set again by `env`.
        assert_eq!(get("PATH"), Some("/set-by-env"));
        // Stripped, though inherited.
        assert!(
            inherited("SystemRoot") || inherited("HOME"),
            "no inherited name to strip"
        );
        assert_eq!(get("SystemRoot"), None);
        assert_eq!(get("HOME"), None);
        // `env` comes after the strip, so a name the strip accepts still keeps the value `env` gives it.
        assert_eq!(get("ZZ_STRIP_AFTER"), Some("kept"));
        // Everything else is inherited as it is.
        assert!(env.len() > 3);
        // Without a remove or strip, an inherited name stays.
        let plain = child_env(&env_spec(vec![], vec![], None));
        assert!(plain.iter().any(|(k, _)| k.eq_ignore_ascii_case("PATH")));
    }

    #[test]
    fn the_recording_closes_when_the_output_ends() {
        // Run this test executable as the fake program, as the session integration tests do
        // for their child-process probes. Only the child receives this variable.
        const CHILD: &str = "MARSHELL_TEST_RECORDING_CHILD";
        if std::env::var_os(CHILD).is_some() {
            println!("last recording output");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t-end.cast");
        let spec = SpawnSpec {
            argv: vec![
                std::env::current_exe().unwrap().to_string_lossy().into_owned(),
                "--exact".into(),
                "pty::session::tests::the_recording_closes_when_the_output_ends".into(),
                "--nocapture".into(),
            ],
            cwd: Some(dir.path().to_path_buf()),
            env: vec![(CHILD.into(), "1".into())],
            env_remove: vec![],
            env_strip: None,
            cols: 80,
            rows: 24,
            on_exit: None,
            on_mark: None,
            record: Some((path.clone(), None)),
            classify: None,
            initial_command: None,
        };
        let session = Session::spawn("t-end".into(), spec).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !session.output.lock().unwrap().reader_done && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let reader_done = session.output.lock().unwrap().reader_done;
        if !reader_done {
            session.force_kill();
        }
        assert!(reader_done, "the fake program's output did not end");
        let deadline = Instant::now() + Duration::from_secs(2);
        while session.open_recording().is_some() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(session.open_recording(), None);
        assert_eq!(session.recording_path(), Some(path.clone()));
        assert!(std::fs::read_to_string(path).unwrap().contains("last recording output"));
    }

    #[test]
    fn attach_returns_the_buffer_end_as_the_live_offset() {
        let dir = tempfile::tempdir().unwrap();
        let spec = SpawnSpec {
            argv: vec![
                std::env::current_exe().unwrap().to_string_lossy().into_owned(),
                "--exact".into(),
                "pty::session::tests::attach_returns_the_buffer_end_as_the_live_offset".into(),
                "--nocapture".into(),
            ],
            cwd: Some(dir.path().to_path_buf()),
            env: vec![("MARSHELL_TEST_LIVE_CHILD".into(), "1".into())],
            env_remove: vec![],
            env_strip: None,
            cols: 80,
            rows: 24,
            on_exit: None,
            on_mark: None,
            record: None,
            classify: None,
            initial_command: None,
        };
        if std::env::var_os("MARSHELL_TEST_LIVE_CHILD").is_some() {
            // The child: output, then a query cut short, which the filter holds as its carry.
            print!("some output\x1b[6");
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            return;
        }
        let session = Session::spawn("t-live".into(), spec).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !session.output.lock().unwrap().reader_done && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let before = session.counters().0;
        assert!(before > 0, "the child printed nothing");
        let (_generation, live_from) = session.attach();
        assert_eq!(live_from, session.counters().0);
        assert!(live_from >= before);
    }

    /// A session over no pty, so a test can hand `ingest` chunks of its own choosing.
    fn bare(path: &std::path::Path, classify: Option<Classifier>, initial: Option<&str>) -> Session {
        let header = CastHeader {
            cols: 80,
            rows: 24,
            started_unix: 0,
            shell: None,
        };
        Session {
            id: "t-bare".into(),
            command: Mutex::new(CommandTrack::starting(classify.as_ref(), initial)),
            cwd: Mutex::new(None),
            output: Mutex::new(Output {
                buf: OutputBuffer::new(SCROLLBACK_BYTES),
                screen: vt100::Parser::new(24, 80, 0),
                dsr: DsrFilter::default(),
                marks: Osc133Scanner::default(),
                attached: false,
                generation: 0,
                acked: 0,
                exit: None,
                reader_done: false,
                startup_dsr_answered: true,
            }),
            space: Condvar::new(),
            notify: tokio::sync::Notify::new(),
            input: mpsc::channel().0,
            control: Mutex::new(None),
            pid: None,
            elevated: false,
            exited: Arc::new(AtomicBool::new(false)),
            exit_callback: Mutex::new(None),
            mark_callback: None,
            recorder: Some(Recorder::start(path.to_path_buf(), header).unwrap()),
            classify,
            rec: Mutex::new(RecState::default()),
            frozen: Mutex::new(None),
        }
    }

    fn gh_classifier() -> Classifier {
        Arc::new(crate::watch::watch_for)
    }

    /// The recording's output text and marker labels, once the writer has closed the file.
    fn cast_of(s: &Session) -> (String, Vec<String>) {
        let rec = s.recorder.as_ref().unwrap();
        rec.close();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !rec.is_closed() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(rec.is_closed(), "the recording did not close");
        let text = std::fs::read_to_string(rec.path()).unwrap();
        let (mut output, mut markers) = (String::new(), Vec::new());
        for line in text.lines().skip(1) {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            match v[1].as_str() {
                Some("o") => output.push_str(v[2].as_str().unwrap()),
                Some("m") => markers.push(v[2].as_str().unwrap().to_owned()),
                _ => {}
            }
        }
        (output, markers)
    }

    const E_GH_AUTH_TOKEN: &[u8] = b"\x1b]633;E;gh auth token\x1b\\";
    const C: &[u8] = b"\x1b]133;C\x1b\\";
    const D_A: &[u8] = b"\x1b]133;D;0\x1b\\\x1b]133;A\x1b\\";

    fn cat(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    #[test]
    fn a_secret_command_line_in_the_same_chunk_hides_that_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("a.cast"), Some(gh_classifier()), None);
        s.ingest(b"$ ");
        s.ingest(&cat(&[E_GH_AUTH_TOKEN, C, b"gho_SECRET\r\n"]));
        assert!(s.secret_now());
        s.ingest(b"gho_MORE_SECRET\r\n");
        s.ingest(&cat(&[D_A, b"PS> "]));
        s.ingest(b"visible\r\n");
        let (output, markers) = cast_of(&s);
        assert!(!output.contains("gho_"), "{output}");
        assert!(output.contains("$ ") && output.contains("visible"), "{output}");
        let marks: Vec<&str> = markers.iter().map(String::as_str).filter(|m| *m != "prompt").collect();
        assert_eq!(
            marks,
            ["Output hidden: gh auth token … prints secrets", "Output recorded again"]
        );
        // The terminal still gets everything.
        let screen = s.peek(24).join("\n");
        assert!(screen.contains("gho_SECRET"), "{screen}");
    }

    #[test]
    fn the_chunk_that_ends_a_secret_command_is_hidden_whole() {
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("b.cast"), Some(gh_classifier()), None);
        let w = crate::watch::watch_for("gh auth token");
        assert!(w.secret);
        assert!(s.set_watch(s.command_epoch(), w));
        // The tail of the output, then D and A, in one read: the watch is off by the end of it.
        s.ingest(&cat(&[b"gho_SECRET\r\n", D_A]));
        assert!(!s.secret_now());
        s.ingest(b"after\r\n");
        let (output, markers) = cast_of(&s);
        assert!(!output.contains("gho_SECRET"), "{output}");
        assert!(output.contains("after"), "{output}");
        let hidden = markers.iter().filter(|m| m.starts_with("Output hidden")).count();
        let again = markers.iter().filter(|m| *m == "Output recorded again").count();
        assert_eq!((hidden, again), (1, 1), "{markers:?}");
    }

    #[test]
    fn osc633_is_stripped_from_the_recording_and_not_from_the_screen() {
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("c.cast"), None, None);
        s.ingest(b"one \x1b]633;E;docker login -p hunter2\x1b");
        s.ingest(b"\\ two\r\n");
        let (output, _) = cast_of(&s);
        assert_eq!(output, "one  two\r\n");
        // The ring, which feeds the terminal, still holds the sequence whole.
        let Pull::Data { bytes, .. } = s.pull(0, 4096) else {
            panic!("the ring is empty")
        };
        assert!(String::from_utf8_lossy(&bytes).contains("\x1b]633;E;docker login -p hunter2"));
    }

    #[test]
    fn the_foreground_phrase_is_scrubbed() {
        assert_eq!(
            shown_command_line("docker login -u me -p hunter2").as_deref(),
            Some("docker login …")
        );
        assert_eq!(shown_command_line("git status\nsecond").as_deref(), Some("git status"));
        assert_eq!(shown_command_line("   ").as_deref(), None);
        let long = format!("echo {}", "x".repeat(300));
        assert_eq!(shown_command_line(&long).unwrap().chars().count(), 200);
        // Through the marks: the session keeps the scrubbed line, and classifies the real one.
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("d.cast"), Some(gh_classifier()), None);
        s.ingest(&cat(&[b"\x1b]633;E;docker login -u me -p hunter2\x1b\\", C]));
        assert_eq!(s.command_line().as_deref(), Some("docker login …"));
        assert!(s.quiet_ports(), "docker login is a login");
    }

    #[test]
    fn a_watch_ended_between_chunks_writes_its_marker_before_the_next_output() {
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("g.cast"), None, None);
        let secret = crate::watch::watch_for("gh auth token");
        assert!(s.set_watch(s.command_epoch(), secret));
        s.ingest(b"gho_SECRET\r\n");
        assert!(s.set_watch(s.command_epoch(), Watch::default()));
        s.ingest(b"plain\r\n");
        let (output, markers) = cast_of(&s);
        assert_eq!(output, "plain\r\n");
        assert_eq!(
            markers,
            ["Output hidden: gh auth token … prints secrets", "Output recorded again"]
        );
    }

    #[test]
    fn a_watch_from_the_foreground_follows_the_epoch_and_yields_to_the_command_line() {
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("e.cast"), Some(gh_classifier()), None);
        let watch = crate::watch::watch_for;
        let epoch = s.command_epoch();
        // A reading taken at an older epoch is stale.
        s.ingest(C);
        assert!(!s.set_watch(epoch, watch("vercel login")));
        assert!(!s.quiet_ports());
        // At the current epoch, with no command line seen, it applies; the same watch again changes nothing.
        let epoch = s.command_epoch();
        assert!(s.set_watch(epoch, watch("vercel login")));
        assert!(s.quiet_ports());
        assert!(!s.set_watch(epoch, watch("vercel login")));
        // The next command's line sets its own watch, and a reading of that command that says the same changes nothing.
        s.ingest(&cat(&[D_A, b"\x1b]633;E;docker ps\x1b\\"]));
        assert!(!s.quiet_ports());
        s.ingest(C);
        assert!(!s.set_watch(s.command_epoch(), watch("docker ps")));
        // A reading that finds a login behind the line's alias turns the quiet watch on, and the command's end drops it.
        assert!(s.set_watch(s.command_epoch(), watch("vercel login")));
        assert!(s.quiet_ports());
        s.ingest(D_A);
        assert!(!s.quiet_ports());
        assert!(s.set_watch(s.command_epoch(), watch("vercel login")));
    }

    #[test]
    fn a_foreground_reading_can_turn_a_line_watch_secret() {
        let dir = tempfile::tempdir().unwrap();
        let argv = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        // An alias the line does not name: the line watch is plain, and the process is `gh auth token`.
        let s = bare(&dir.path().join("t10.cast"), Some(gh_classifier()), None);
        s.ingest(&cat(&[b"\x1b]633;E;tok\x1b\\", C]));
        assert!(!s.secret_now());
        let reading = crate::watch::watch_for_argv(&argv(&["gh.exe", "auth", "token"]));
        assert!(s.set_watch(s.command_epoch(), reading));
        assert!(s.secret_now());
        // The same reading again changes nothing.
        let reading = crate::watch::watch_for_argv(&argv(&["gh.exe", "auth", "token"]));
        assert!(!s.set_watch(s.command_epoch(), reading));
        // It never turns secret off.
        let reading = crate::watch::watch_for_argv(&argv(&["docker.exe", "ps"]));
        s.set_watch(s.command_epoch(), reading);
        assert!(s.secret_now());
    }

    #[test]
    fn an_initial_command_is_watched_until_the_first_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("f.cast"), Some(gh_classifier()), Some("gh auth token"));
        assert!(s.secret_now());
        s.ingest(b"gho_SECRET\r\n\x1b]133;A\x1b\\");
        assert!(!s.secret_now());
        let (output, _) = cast_of(&s);
        assert!(!output.contains("gho_SECRET"), "{output}");
    }

    /// An administrator session whose helper stopped draining `-down` while a large input frame is being written: a
    /// resize cannot freeze its caller, and Close and the kill after it do not wait behind the frame.
    #[cfg(windows)]
    #[test]
    fn a_stuck_helper_does_not_hold_resize_or_kill() {
        use super::super::elevated::stuck::{stuck_remote, within_two_seconds};
        let (remote, _helper) = stuck_remote();
        let parts = PtyParts {
            reader: remote.reader,
            writer: remote.writer,
            control: remote.control,
            wait: remote.wait,
            pid: Some(remote.pid),
            elevated: true,
        };
        let session = Session::start("t-stuck".into(), env_spec(vec![], vec![], None), parts).unwrap();
        session.write_input(vec![b'x'; 60_000]);
        std::thread::sleep(Duration::from_millis(300));
        // The resize may fail (the frame never went out), but it returns.
        let s = Arc::clone(&session);
        assert!(
            within_two_seconds(move || {
                let _ = s.resize(100, 30);
            })
            .is_some(),
            "resize did not return"
        );
        // A second resize stuck behind the first would hold the control lock the kill needs.
        let s = Arc::clone(&session);
        let resizing = std::thread::spawn(move || {
            let _ = s.resize(90, 30);
        });
        std::thread::sleep(Duration::from_millis(100));
        let s = Arc::clone(&session);
        assert!(within_two_seconds(move || s.kill()).is_some(), "kill did not return");
        let s = Arc::clone(&session);
        assert!(
            within_two_seconds(move || s.force_kill()).is_some(),
            "force_kill did not return"
        );
        let _ = resizing.join();
    }
}
