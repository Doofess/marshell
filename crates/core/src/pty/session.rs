use super::buffer::{MAX_UNACKED, OutputBuffer, SCROLLBACK_BYTES, peek_rows};
use super::dsr::{DSR_REPLY, DsrFilter};
use super::freeze::{self, FreezeError, Frozen};
use super::osc133::{Mark, Osc133Scanner};
use super::record::{CastHeader, Recorder};
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

/// Called with each OSC 133 mark the shell prints, from the reader thread, outside every session lock.
pub type MarkCallback = Box<dyn Fn(Mark) + Send + Sync>;

pub struct SpawnSpec {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    /// Variables the child must not inherit. Applied before `env`, so a name in both keeps its `env` value.
    pub env_remove: Vec<String>,
    pub cols: u16,
    pub rows: u16,
    /// Called once, from the wait thread, with the exit code. Runs outside every session lock.
    pub on_exit: Option<ExitCallback>,
    /// Called for each OSC 133 prompt or command mark in the output.
    pub on_mark: Option<MarkCallback>,
    /// Where to write the asciicast recording, and the SHELL value for its header. A path that cannot
    /// be written leaves the session running without a recording.
    pub record: Option<(PathBuf, Option<String>)>,
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
}

/// The longest command line that leaves the core, in chars.
const MAX_COMMAND_LINE: usize = 200;

/// The first line of `text`, trimmed and capped at 200 chars on a char boundary. `None` if empty.
fn shown_command_line(text: &str) -> Option<String> {
    let first = text.lines().next().unwrap_or("").trim();
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
    recorder: Option<Recorder>,
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
            command: Mutex::new(CommandTrack::default()),
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
            master: Mutex::new(Some(pair.master)),
            pid,
            exited: Arc::new(AtomicBool::new(false)),
            exit_callback: Mutex::new(spec.on_exit.take()),
            mark_callback: spec.on_mark.take(),
            recorder,
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
        self.track_command(&marks);
        self.notify.notify_one();
        // The recording gets what the terminal got, after the lock is dropped.
        if let Some(rec) = &self.recorder {
            if !pass.is_empty() {
                rec.output(&pass);
            }
            for mark in &marks {
                if *mark == Mark::PromptStart {
                    rec.marker("prompt");
                }
            }
        }
        if let Some(cb) = &self.mark_callback {
            for mark in marks {
                cb(mark);
            }
        }
        answered
    }

    /// Folds the shell's marks into the command tracking, before the mark callback sees them.
    fn track_command(&self, marks: &[Mark]) {
        if marks.is_empty() {
            return;
        }
        let mut track = self.command.lock().unwrap();
        for mark in marks {
            match mark {
                Mark::CommandLine(text) => track.line = shown_command_line(text),
                Mark::Executed => {
                    track.running = true;
                    track.ever_executed = true;
                    track.epoch += 1;
                }
                Mark::Finished(_) | Mark::PromptStart => {
                    track.running = false;
                    track.line = None;
                    track.epoch += 1;
                }
                Mark::CommandStart => {}
                Mark::Notify(_) | Mark::Bell => {}
                Mark::Cwd(path) => *self.cwd.lock().unwrap() = Some(PathBuf::from(path)),
            }
        }
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

    /// A renderer connected. Returns a generation number for `detach`.
    /// From now on the renderer (xterm) answers cursor queries itself.
    pub fn attach(&self) -> u64 {
        let mut out = self.output.lock().unwrap();
        let carry = out.dsr.take_carry();
        out.screen.process(&carry);
        out.buf.push(&carry);
        out.attached = true;
        out.generation += 1;
        out.generation
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

    /// The programs running in front of the shell, outermost first, as `detect::program_name` names them.
    /// Empty when the shell has no child. `None` when this cannot be read.
    pub fn foreground_chain(&self) -> Option<Vec<String>> {
        let pid = self.pid.filter(|_| !self.exited.load(Ordering::SeqCst))?;
        #[cfg(windows)]
        {
            super::proctree::foreground_chain(pid)
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
            Some(vec![crate::profiles::detect::program_name(comm.trim())])
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = pid;
            None
        }
    }
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
            cols: 80,
            rows: 24,
            on_exit: None,
            on_mark: None,
            record: Some((path.clone(), None)),
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
}
