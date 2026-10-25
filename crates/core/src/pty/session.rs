use super::buffer::{MAX_UNACKED, OutputBuffer, SCROLLBACK_BYTES, peek_rows, tail_rows};
use super::dsr::{DSR_REPLY, DsrFilter};
use super::freeze::{self, FreezeError, Frozen};
use super::osc133::{Mark, Osc133Scanner};
use super::record::{CastHeader, Osc633Strip, Recorder};
use crate::tools::{Matcher, Profile, ToolEvent, Watch, secrets};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
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

/// Called with what a profiled tool's output shows (a prompt, its end, an announced port), from the reader thread or
/// from `set_watch`, outside every session lock.
pub type ToolCallback = Box<dyn Fn(ToolEvent) + Send + Sync>;

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
    /// Called for each event the matcher raises while a profiled tool runs.
    pub on_tool: Option<ToolCallback>,
    /// Where to write the asciicast recording, and the SHELL value for its header. A path that cannot
    /// be written leaves the session running without a recording.
    pub record: Option<(PathBuf, Option<String>)>,
    /// Classifies each command line the shell reports. Without it the session watches nothing.
    pub classify: Option<Classifier>,
    /// A command the shell was started with, classified at spawn and dropped at the first prompt mark.
    pub initial_command: Option<String>,
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
    /// Reads the output of the watched command while its watch has profiles and does not print secrets.
    tool: Option<ToolMatcher>,
    /// The `CommandTrack::generation` the matcher was last settled for. A settle for an older one is stale.
    tool_gen: u64,
}

/// A matcher beside the profiles it was built for, and whether it has raised a prompt that is still up.
struct ToolMatcher {
    matcher: Matcher,
    profiles: Vec<Arc<Profile>>,
    raised: bool,
}

/// What the matcher should do for the watch as it stands.
struct ToolView {
    generation: u64,
    /// The profiles to read with and whether the command's port lines make no chip. `None`: nothing is read.
    wanted: Option<(Vec<Arc<Profile>>, bool)>,
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
    /// Moves up whenever `watch` is replaced, so the matcher is never settled for an older watch than a newer one.
    generation: u64,
}

impl CommandTrack {
    fn starting(classify: Option<&Classifier>, initial_command: Option<&str>) -> Self {
        let mut track = CommandTrack::default();
        if let (Some(classify), Some(line)) = (classify, initial_command) {
            track.watch = classify(line);
            track.generation += 1;
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
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    pid: Option<u32>,
    /// Set when the child has exited, so a late kill cannot hit a reused pid.
    exited: Arc<AtomicBool>,
    /// Taken and called once when the child exits.
    exit_callback: Mutex<Option<ExitCallback>>,
    mark_callback: Option<MarkCallback>,
    tool_callback: Option<ToolCallback>,
    recorder: Option<Recorder>,
    classify: Option<Classifier>,
    rec: Mutex<RecState>,
    /// Set while the process tree is frozen. Held for the whole freeze, so `on_exit` waits for it.
    frozen: Mutex<Option<Frozen>>,
}

impl Session {
    pub fn spawn(id: String, mut spec: SpawnSpec) -> anyhow::Result<Arc<Session>> {
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
        let mut child = pair.slave.spawn_command(cmd)?;
        // Our copy of the slave must close, or the reader never sees EOF on Unix.
        drop(pair.slave);
        let pid = child.process_id();
        let reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
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
                tool: None,
                tool_gen: 0,
            }),
            space: Condvar::new(),
            notify: tokio::sync::Notify::new(),
            input: input_tx,
            master: Mutex::new(Some(pair.master)),
            pid,
            exited: Arc::new(AtomicBool::new(false)),
            exit_callback: Mutex::new(spec.on_exit.take()),
            mark_callback: spec.on_mark.take(),
            tool_callback: spec.on_tool.take(),
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
                let code = child.wait().map(|st| st.exit_code() as i32).unwrap_or(-1);
                s.on_exit(code);
            })?;

        Ok(session)
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
        // The matcher reads the chunk against the watch as the marks left it, and never a chunk that may hold secrets.
        let view = Self::tool_view(&self.command.lock().unwrap(), hide.is_some());
        let tool_events = self.settle_tools(Some(&pass), view);
        // The recording gets what the terminal got, after the lock is dropped, but never a secret command's output.
        if let Some(rec) = &self.recorder {
            self.record_chunk(rec, &pass, &marks, hide);
        }
        // No session lock is held now: the callback takes the registry's lock.
        self.emit_tool_events(tool_events);
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

    /// What the matcher should read for the watch as it stands. Nothing while the command prints secrets, or when
    /// `hide` says the chunk may hold some.
    fn tool_view(track: &CommandTrack, hide: bool) -> ToolView {
        let watch = &track.watch;
        let wanted =
            (!hide && !watch.secret && !watch.profiles.is_empty()).then(|| (watch.profiles.clone(), watch.quiet_ports));
        ToolView {
            generation: track.generation,
            wanted,
        }
    }

    /// Brings the matcher in line with `view` and, for a chunk (`pass`: what the terminal got), reads it. A matcher
    /// is built when the watch has profiles and none matches them, and dropped when the watch loses its profiles or
    /// turns secret; a raised prompt it held then ends with a `Cleared`. The screen's tail is taken while the output
    /// lock is held, and the lock is never held across a callback: the events come back for the caller to emit.
    fn settle_tools(&self, pass: Option<&[u8]>, view: ToolView) -> Vec<ToolEvent> {
        let mut events = Vec::new();
        let mut out = self.output.lock().unwrap();
        // A newer watch has been settled already (the foreground set it while this chunk was being read).
        if view.generation < out.tool_gen {
            return events;
        }
        out.tool_gen = view.generation;
        let same = match (&view.wanted, &out.tool) {
            (Some((wanted, _)), Some(tool)) => {
                wanted.len() == tool.profiles.len() && wanted.iter().zip(&tool.profiles).all(|(a, b)| Arc::ptr_eq(a, b))
            }
            _ => false,
        };
        if !same {
            if let Some(old) = out.tool.take()
                && old.raised
            {
                events.push(ToolEvent::Cleared);
            }
            if let (Some((profiles, _)), Some(_)) = (&view.wanted, pass) {
                out.tool = Some(ToolMatcher {
                    matcher: Matcher::new(profiles.clone()),
                    profiles: profiles.clone(),
                    raised: false,
                });
            }
        }
        if let (Some(pass), Some((_, quiet_ports))) = (pass, &view.wanted) {
            let tail = tail_rows(out.screen.screen());
            if let Some(tool) = out.tool.as_mut() {
                for event in tool.matcher.feed(pass, &tail) {
                    match &event {
                        ToolEvent::NeedsYou(_) => tool.raised = true,
                        ToolEvent::Cleared => tool.raised = false,
                        // A command that lists its port lines as transient makes no chip.
                        ToolEvent::Port(_) if *quiet_ports => continue,
                        ToolEvent::Port(_) => {}
                    }
                    events.push(event);
                }
            }
        }
        events
    }

    /// Hands events to `on_tool`. The caller holds no session lock.
    fn emit_tool_events(&self, events: Vec<ToolEvent>) {
        if let Some(cb) = &self.tool_callback {
            for event in events {
                cb(event);
            }
        }
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
                        track.generation += 1;
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
                    track.generation += 1;
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
    /// still tell more: an alias hides `gh auth token` from the line, so a reading may turn `secret` on (it never turns
    /// it off), and may fill in the tool and profiles when the line named none. A raised prompt whose profiles go
    /// is cleared through `on_tool` at once.
    pub fn set_watch(&self, epoch: u64, watch: Watch) -> bool {
        let (changed, view) = {
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
                if next.tool.is_none() && watch.tool.is_some() {
                    next.tool = watch.tool;
                    next.profiles = watch.profiles;
                    next.quiet_ports = watch.quiet_ports;
                }
                next
            } else {
                watch
            };
            let changed = track.watch.differs(&next);
            if changed {
                track.watch = next;
                track.generation += 1;
            }
            (changed, Self::tool_view(&track, false))
        };
        if changed {
            let events = self.settle_tools(None, view);
            self.emit_tool_events(events);
        }
        changed
    }

    /// The id of the tool the watched command is, if a profile matched it.
    pub fn watch_tool(&self) -> Option<String> {
        self.command.lock().unwrap().watch.tool.clone()
    }

    /// Whether the watched command's profile marks its port lines as transient (`vercel login`'s callback): the port
    /// watcher makes no chip for a listener while it is set.
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
        let master = self.master.lock().unwrap().take();
        drop(master);
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
        if let Some(m) = self.master.lock().unwrap().as_ref() {
            m.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
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
        if let Some(pid) = self.pid {
            kill_tree(pid, self.exited.clone());
        }
    }

    /// Kills the whole process tree right now and waits for it. Use when the process is about to exit.
    pub fn force_kill(&self) {
        if self.exited.load(Ordering::SeqCst) {
            return;
        }
        self.thaw();
        if let Some(pid) = self.pid {
            force_kill_tree(pid);
        }
    }

    /// Freezes the whole process tree, or none of it. Freezing a frozen session does nothing.
    pub fn freeze(&self) -> Result<(), FreezeError> {
        let mut frozen = self.frozen.lock().unwrap();
        if frozen.is_some() {
            return Ok(());
        }
        let Some(pid) = self.pid.filter(|_| !self.exited.load(Ordering::SeqCst)) else {
            return Err(FreezeError::NotRunning);
        };
        #[cfg(unix)]
        let foreground = self
            .master
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|m| m.process_group_leader())
            .and_then(|g| u32::try_from(g).ok());
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
                .master
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|m| m.process_group_leader())
                .and_then(|g| u32::try_from(g).ok())?;
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
fn force_kill_tree(pid: u32) {
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
fn force_kill_tree(pid: u32) {
    unsafe {
        libc::killpg(pid as i32, libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            on_tool: None,
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
            on_tool: None,
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
                tool: None,
                tool_gen: 0,
            }),
            space: Condvar::new(),
            notify: tokio::sync::Notify::new(),
            input: mpsc::channel().0,
            master: Mutex::new(None),
            pid: None,
            exited: Arc::new(AtomicBool::new(false)),
            exit_callback: Mutex::new(None),
            mark_callback: None,
            tool_callback: None,
            recorder: Some(Recorder::start(path.to_path_buf(), header).unwrap()),
            classify,
            rec: Mutex::new(RecState::default()),
            frozen: Mutex::new(None),
        }
    }

    fn gh_classifier() -> Classifier {
        let tools = crate::tools::Tools::shipped();
        Arc::new(move |line| crate::tools::watch_for(&tools, line))
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
        assert_eq!(s.watch_tool().as_deref(), Some("gh"));
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
        let w = crate::tools::watch_for(&crate::tools::Tools::shipped(), "gh auth token");
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
        assert_eq!(s.watch_tool().as_deref(), Some("docker"));
    }

    #[test]
    fn a_watch_ended_between_chunks_writes_its_marker_before_the_next_output() {
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("g.cast"), None, None);
        let secret = crate::tools::watch_for(&crate::tools::Tools::shipped(), "gh auth token");
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
        let tools = crate::tools::Tools::shipped();
        let watch = |line: &str| crate::tools::watch_for(&tools, line);
        let epoch = s.command_epoch();
        // A reading taken at an older epoch is stale.
        s.ingest(C);
        assert!(!s.set_watch(epoch, watch("vercel dev")));
        assert_eq!(s.watch_tool(), None);
        // At the current epoch, with no command line seen, it applies; the same watch again changes nothing.
        let epoch = s.command_epoch();
        assert!(s.set_watch(epoch, watch("vercel dev")));
        assert_eq!(s.watch_tool().as_deref(), Some("vercel"));
        assert!(!s.set_watch(epoch, watch("vercel dev")));
        // The next command's line sets its own watch, and the foreground reading of that command does not replace it.
        s.ingest(&cat(&[D_A, b"\x1b]633;E;docker ps\x1b\\"]));
        assert_eq!(s.watch_tool().as_deref(), Some("docker"));
        s.ingest(C);
        assert!(!s.set_watch(s.command_epoch(), watch("vercel dev")));
        assert_eq!(s.watch_tool().as_deref(), Some("docker"));
        // The command ends: the watch is dropped.
        s.ingest(D_A);
        assert_eq!(s.watch_tool(), None);
        assert!(s.set_watch(s.command_epoch(), watch("vercel dev")));
    }

    /// What a session's `on_tool` hears, in order.
    type Heard = Arc<Mutex<Vec<crate::tools::ToolEvent>>>;

    fn listening(s: &mut Session) -> Heard {
        let heard: Heard = Arc::default();
        let sink = heard.clone();
        s.tool_callback = Some(Box::new(move |event| sink.lock().unwrap().push(event)));
        heard
    }

    fn heard(h: &Heard) -> Vec<crate::tools::ToolEvent> {
        std::mem::take(&mut *h.lock().unwrap())
    }

    const E_VERCEL_LOGIN: &[u8] = b"\x1b]633;E;vercel login\x1b\\";
    const WAITING: &str = "Waiting for sign-in in the browser";

    fn raised() -> Vec<crate::tools::ToolEvent> {
        vec![crate::tools::ToolEvent::NeedsYou(WAITING.into())]
    }

    #[test]
    fn a_profiled_commands_prompt_raises_and_a_new_row_clears_it() {
        use crate::tools::ToolEvent;
        let dir = tempfile::tempdir().unwrap();
        let mut s = bare(&dir.path().join("t1.cast"), Some(gh_classifier()), None);
        let h = listening(&mut s);
        s.ingest(b"$ ");
        s.ingest(&cat(&[E_VERCEL_LOGIN, C, b"Waiting for authentication..."]));
        assert_eq!(heard(&h), raised());
        // A spinner redrawing its row is not new output.
        for _ in 0..3 {
            s.ingest("\r\x1b[2K\u{280b} Waiting for authentication...".as_bytes());
        }
        assert_eq!(heard(&h), []);
        // One new row is not enough: the matcher reads the last 3 non-empty rows, and the prompt is still among them.
        s.ingest(b"\r\nSuccess\r\n");
        assert_eq!(heard(&h), []);
        // Three rows of other text push it out, and the line breaks say it is new output.
        s.ingest(b"one\r\ntwo\r\nthree\r\n");
        assert_eq!(heard(&h), [ToolEvent::Cleared]);
    }

    #[test]
    fn a_command_that_ends_while_its_prompt_shows_clears_it() {
        use crate::tools::ToolEvent;
        let dir = tempfile::tempdir().unwrap();
        let mut s = bare(&dir.path().join("t2.cast"), Some(gh_classifier()), None);
        let h = listening(&mut s);
        s.ingest(&cat(&[E_VERCEL_LOGIN, C, b"Waiting for authentication..."]));
        assert_eq!(heard(&h), raised());
        // D and A with no line break: the watch drops, and the prompt with it.
        s.ingest(D_A);
        assert_eq!(heard(&h), [ToolEvent::Cleared]);
        // Nothing is matched at the shell's own prompt.
        s.ingest(b"Waiting for authentication...");
        assert_eq!(heard(&h), []);
    }

    #[test]
    fn a_shell_with_no_watch_matches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = bare(&dir.path().join("t3.cast"), Some(gh_classifier()), None);
        let h = listening(&mut s);
        s.ingest(&cat(&[
            b"\x1b]633;E;echo hi\x1b\\",
            C,
            b"Waiting for authentication...",
        ]));
        assert_eq!(heard(&h), []);
        assert!(s.output.lock().unwrap().tool.is_none());
    }

    #[test]
    fn a_secret_command_is_neither_matched_nor_announced() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = bare(&dir.path().join("t4.cast"), None, None);
        let h = listening(&mut s);
        let mut watch = crate::tools::watch_for(&crate::tools::Tools::shipped(), "vercel login");
        watch.secret = true;
        watch.label = "vercel login".into();
        assert!(s.set_watch(s.command_epoch(), watch));
        s.ingest(b"Waiting for authentication...\r\nReady! Available at http://localhost:3000\r\n");
        assert_eq!(heard(&h), []);
    }

    #[test]
    fn a_watch_that_loses_its_profiles_clears_a_raised_prompt_at_once() {
        use crate::tools::ToolEvent;
        let dir = tempfile::tempdir().unwrap();
        let mut s = bare(&dir.path().join("t5.cast"), None, None);
        let h = listening(&mut s);
        let tools = crate::tools::Tools::shipped();
        assert!(s.set_watch(s.command_epoch(), crate::tools::watch_for(&tools, "vercel login")));
        s.ingest(b"Waiting for authentication...");
        assert_eq!(heard(&h), raised());
        // No output is needed for the prompt to go.
        assert!(s.set_watch(s.command_epoch(), Watch::default()));
        assert_eq!(heard(&h), [ToolEvent::Cleared]);
        assert!(s.output.lock().unwrap().tool.is_none());
    }

    #[test]
    fn an_announced_port_reaches_on_tool_once_unless_the_command_is_quiet() {
        use crate::tools::ToolEvent;
        let dir = tempfile::tempdir().unwrap();
        let mut s = bare(&dir.path().join("t7.cast"), Some(gh_classifier()), None);
        let h = listening(&mut s);
        s.ingest(&cat(&[
            b"\x1b]633;E;vercel dev\x1b\\",
            C,
            b"Ready! Available at http://localhost:3000\r\n",
        ]));
        let got = heard(&h);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(matches!(&got[0], ToolEvent::Port(chip) if chip.port == 3000 && chip.announced));
        s.ingest(b"Ready! Available at http://localhost:3000\r\n");
        assert_eq!(heard(&h), []);
        // `vercel login` lists "login" in quiet_ports: its callback port makes no chip.
        s.ingest(D_A);
        s.ingest(&cat(&[
            E_VERCEL_LOGIN,
            C,
            b"Ready! Available at http://localhost:4000\r\n",
        ]));
        assert_eq!(heard(&h), []);
    }

    #[test]
    fn a_callback_runs_with_no_session_lock_held() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = bare(&dir.path().join("t8.cast"), Some(gh_classifier()), None);
        let seen = Arc::new(AtomicBool::new(false));
        let flag = seen.clone();
        let probe: Arc<Mutex<Option<Arc<Session>>>> = Arc::default();
        let from_callback = probe.clone();
        s.tool_callback = Some(Box::new(move |_| {
            let session = from_callback.lock().unwrap().clone().unwrap();
            // Each of these takes a lock that the matcher's pass held.
            let _ = (
                session.peek(1),
                session.command_epoch(),
                session.secret_now(),
                session.counters(),
            );
            flag.store(true, Ordering::SeqCst);
        }));
        let s = Arc::new(s);
        *probe.lock().unwrap() = Some(s.clone());
        let worker = s.clone();
        let t = std::thread::spawn(move || {
            worker.ingest(&cat(&[E_VERCEL_LOGIN, C, b"Waiting for authentication..."]));
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !seen.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            seen.load(Ordering::SeqCst),
            "the callback did not run, or it waited on a lock"
        );
        t.join().unwrap();
        *probe.lock().unwrap() = None;
    }

    #[test]
    fn a_command_line_watch_beats_a_late_watcher_reading() {
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("t9.cast"), Some(gh_classifier()), None);
        let tools = crate::tools::Tools::shipped();
        s.ingest(&cat(&[b"\x1b]633;E;docker ps\x1b\\", C]));
        assert_eq!(s.watch_tool().as_deref(), Some("docker"));
        // The foreground reads the same command as another tool: the line's tool stays.
        assert!(!s.set_watch(s.command_epoch(), crate::tools::watch_for(&tools, "vercel dev")));
        assert_eq!(s.watch_tool().as_deref(), Some("docker"));
    }

    #[test]
    fn a_foreground_reading_can_turn_a_line_watch_secret() {
        let dir = tempfile::tempdir().unwrap();
        let tools = crate::tools::Tools::shipped();
        let argv = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        // An alias the line does not name: the line watch has no tool, and the process is `gh auth token`.
        let s = bare(&dir.path().join("t10.cast"), Some(gh_classifier()), None);
        s.ingest(&cat(&[b"\x1b]633;E;tok\x1b\\", C]));
        assert!(!s.secret_now());
        assert_eq!(s.watch_tool(), None);
        let reading = crate::tools::watch_for_argv(&tools, &argv(&["gh.exe", "auth", "token"]));
        assert!(s.set_watch(s.command_epoch(), reading));
        assert!(s.secret_now());
        assert_eq!(s.watch_tool().as_deref(), Some("gh"));
        // The same reading again changes nothing.
        let reading = crate::tools::watch_for_argv(&tools, &argv(&["gh.exe", "auth", "token"]));
        assert!(!s.set_watch(s.command_epoch(), reading));
        // A line watch with a tool keeps its tool, and still turns secret on.
        let s = bare(&dir.path().join("t11.cast"), Some(gh_classifier()), None);
        s.ingest(&cat(&[b"\x1b]633;E;docker\x1b\\", C]));
        assert_eq!(s.watch_tool().as_deref(), Some("docker"));
        let reading = crate::tools::watch_for_argv(&tools, &argv(&["gh.exe", "auth", "token"]));
        assert!(s.set_watch(s.command_epoch(), reading));
        assert!(s.secret_now());
        assert_eq!(s.watch_tool().as_deref(), Some("docker"));
        // It never turns secret off.
        let reading = crate::tools::watch_for_argv(&tools, &argv(&["docker.exe", "ps"]));
        s.set_watch(s.command_epoch(), reading);
        assert!(s.secret_now());
    }

    #[test]
    fn an_initial_command_is_watched_until_the_first_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let s = bare(&dir.path().join("f.cast"), Some(gh_classifier()), Some("gh auth token"));
        assert_eq!(s.watch_tool().as_deref(), Some("gh"));
        assert!(s.secret_now());
        s.ingest(b"gho_SECRET\r\n\x1b]133;A\x1b\\");
        assert!(!s.secret_now());
        assert_eq!(s.watch_tool(), None);
        let (output, _) = cast_of(&s);
        assert!(!output.contains("gho_SECRET"), "{output}");
    }
}
