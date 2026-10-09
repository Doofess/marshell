use super::buffer::{MAX_UNACKED, OutputBuffer, SCROLLBACK_BYTES};
use super::dsr::{DSR_REPLY, DsrFilter};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub struct SpawnSpec {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub cols: u16,
    pub rows: u16,
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
    attached: bool,
    generation: u64,
    acked: u64,
    exit: Option<(i32, Instant)>,
    reader_done: bool,
    /// Windows: ConPTY's startup cursor query has been answered by the core.
    startup_dsr_answered: bool,
}

pub struct Session {
    id: String,
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
}

impl Session {
    pub fn spawn(id: String, spec: SpawnSpec) -> anyhow::Result<Arc<Session>> {
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

        let session = Arc::new(Session {
            id: id.clone(),
            output: Mutex::new(Output {
                buf: OutputBuffer::new(SCROLLBACK_BYTES),
                screen: vt100::Parser::new(size.rows, size.cols, 0),
                dsr: DsrFilter::default(),
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
        out.screen.process(&pass);
        out.buf.push(&pass);
        drop(out);
        self.notify.notify_one();
        answered
    }

    fn wait_for_space(&self) {
        let mut out = self.output.lock().unwrap();
        while out.attached && out.exit.is_none() && out.buf.end().saturating_sub(out.acked) > MAX_UNACKED {
            out = self.space.wait(out).unwrap();
        }
    }

    fn on_exit(self: &Arc<Self>, code: i32) {
        self.exited.store(true, Ordering::SeqCst);
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
    }

    pub fn write_input(&self, bytes: Vec<u8>) {
        let _ = self.input.send(bytes);
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
        self.output.lock().unwrap().screen.screen_mut().set_size(rows, cols);
        Ok(())
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
        if let Some(pid) = self.pid {
            kill_tree(pid, self.exited.clone());
        }
    }

    /// Kills the whole process tree right now and waits for it. Use when the process is about to exit.
    pub fn force_kill(&self) {
        if self.exited.load(Ordering::SeqCst) {
            return;
        }
        if let Some(pid) = self.pid {
            force_kill_tree(pid);
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
