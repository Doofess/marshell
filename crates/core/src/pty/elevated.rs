//! The core's side of an elevated session: launch `marshell-host` through UAC, connect to it over
//! two checked named pipes, and hand back the pieces a `Session` needs.

use marshell_protocol::host::SpawnRequest;
use marshell_protocol::host::{FromHost, MAX_FRAME, ToHost, read_from_host, write_to_host};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[cfg(windows)]
pub type ProcessHandle = std::os::windows::io::OwnedHandle;
#[cfg(not(windows))]
pub type ProcessHandle = std::os::fd::OwnedFd;

pub const NON_WINDOWS_COPY: &str = "Running as administrator is available on Windows only for now.";

/// The 409 for an API write (input, fill, paste, an approval) to a session that runs as administrator.
pub const API_WRITE_COPY: &str =
    "This session runs as administrator, so it takes input only from Marshell's own window.";

/// The 409 for an API create that asks to run as administrator.
pub const API_CREATE_COPY: &str = "Only Marshell's own window can start a session as administrator.";

/// The 409 for an API restart, resume or reopen of a session that runs as administrator.
pub const API_START_COPY: &str = "This session runs as administrator, so only Marshell's own window can start it.";

/// The 409 for an always-allow answer on a session that runs as administrator.
pub const ALWAYS_COPY: &str =
    "A session running as administrator can't keep an always-allow rule from Marshell. Allow this once instead.";

/// What the window's trusted door says to a session that does not run as administrator.
pub const NOT_ELEVATED_COPY: &str = "Not an administrator session.";

#[derive(Debug)]
pub enum ElevatedError {
    Declined,
    WrongPeer,
    Timeout,
    ExitedEarly(i32),
    Missing,
    Failed(String),
    Io(io::Error),
}

impl std::fmt::Display for ElevatedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Declined => f.write_str("Administrator rights were declined. Nothing was started."),
            Self::WrongPeer => f.write_str(
                "Another program connected to the administrator session's pipe, so it was refused. Nothing was started.",
            ),
            Self::Timeout => f.write_str("The administrator helper didn't connect within 10 seconds. Nothing was started."),
            Self::ExitedEarly(code) => write!(
                f,
                "The administrator helper stopped before it connected (exit code {code}). Nothing was started."
            ),
            Self::Missing => f.write_str(
                "The administrator helper (marshell-host) is missing next to Marshell. Reinstall Marshell.",
            ),
            Self::Failed(text) => f.write_str(text),
            Self::Io(e) => write!(f, "The administrator session could not be set up: {e}"),
        }
    }
}

impl std::error::Error for ElevatedError {}

/// A started helper. `process` stays open until the pipe peers are checked.
pub struct LaunchedHost {
    pub pid: u32,
    pub process: ProcessHandle,
}

pub trait HostLauncher: Send + Sync {
    fn launch(&self, exe: &Path, args: &str) -> Result<LaunchedHost, ElevatedError>;
}

pub trait PtyControl: Send + Sync {
    fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()>;
    /// Closes the pipe to the helper. The helper sees EOF, ends the shell and exits.
    fn close(&self);
    /// Unix: the terminal's foreground process group, for pausing. A pty with none says `None`.
    fn foreground_group(&self) -> Option<u32> {
        None
    }
}

pub struct RemotePty {
    pub pid: u32,
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub control: Box<dyn PtyControl>,
    pub wait: Box<dyn FnOnce() -> i32 + Send>,
}

/// The core's end of the pipe to the helper's input. One mutex so frames from different threads
/// never interleave; `None` once closed.
struct DownPipe {
    file: Mutex<Option<File>>,
    /// A second handle to the same pipe. A frame write that waits on a helper which stopped reading holds the `file`
    /// lock, so Close cancels that write through this handle instead of waiting for the lock.
    cancel: Mutex<Option<File>>,
    /// Set by Close: no new frame starts after it. An `Arc` so the demux thread can read it without owning the pipe:
    /// a handle held there would keep the helper from seeing EOF when the session is dropped.
    closing: Arc<AtomicBool>,
}

type Down = Arc<DownPipe>;

/// How long a resize waits for a frame write that is in progress. Past that the helper is not reading, and the resize is
/// dropped: the next one carries the size anyway.
const RESIZE_WAIT: Duration = Duration::from_secs(1);

impl DownPipe {
    fn new(file: File) -> Down {
        let cancel = file.try_clone().ok();
        Arc::new(Self {
            file: Mutex::new(Some(file)),
            cancel: Mutex::new(cancel),
            closing: Arc::new(AtomicBool::new(false)),
        })
    }

    #[cfg(test)]
    fn closed() -> Down {
        Arc::new(Self {
            file: Mutex::new(None),
            cancel: Mutex::new(None),
            closing: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Waits at most `wait` for the pipe, so a stuck frame write cannot hold a caller for good.
    fn lock_within(&self, wait: Duration) -> anyhow::Result<std::sync::MutexGuard<'_, Option<File>>> {
        let deadline = Instant::now() + wait;
        loop {
            match self.file.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(std::sync::TryLockError::Poisoned(_)) => return Err(closed().into()),
                Err(std::sync::TryLockError::WouldBlock) => {
                    anyhow::ensure!(
                        Instant::now() < deadline,
                        "The administrator session is not taking input right now."
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }

    /// Ends a write that is stuck in the pipe.
    fn cancel_pending_write(&self) {
        let guard = self.cancel.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(file) = guard.as_ref() {
            cancel_io(file);
        }
    }
}

#[cfg(windows)]
fn cancel_io(file: &File) {
    use std::os::windows::io::AsRawHandle;
    // SAFETY: a valid pipe handle that `file` owns; no OVERLAPPED cancels every pending operation on it. Nothing
    // pending is not an error worth reporting.
    unsafe {
        windows_sys::Win32::System::IO::CancelIoEx(file.as_raw_handle() as _, std::ptr::null());
    }
}

#[cfg(not(windows))]
fn cancel_io(_file: &File) {}

fn closed() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "The administrator session is closed.")
}

/// Wraps bytes as `Input` frames, at most `MAX_FRAME` bytes each.
struct HostWriter {
    down: Down,
}

impl Write for HostWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let n = buf.len().min(MAX_FRAME);
        let mut guard = self.down.file.lock().map_err(|_| closed())?;
        if self.down.closing.load(Ordering::SeqCst) {
            return Err(closed());
        }
        let file = guard.as_mut().ok_or_else(closed)?;
        write_to_host(file, &ToHost::Input(buf[..n].to_vec()))?;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct HostControl {
    down: Down,
}

impl PtyControl for HostControl {
    fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        // The helper closes on a size outside this range, so it is refused here instead.
        anyhow::ensure!(
            (1..=1000).contains(&cols) && (1..=1000).contains(&rows),
            "A terminal size of {cols}x{rows} is out of range."
        );
        let mut guard = self.down.lock_within(RESIZE_WAIT)?;
        if self.down.closing.load(Ordering::SeqCst) {
            return Err(closed().into());
        }
        let file = guard.as_mut().ok_or_else(closed)?;
        write_to_host(file, &ToHost::Resize { cols, rows })?;
        Ok(())
    }

    fn close(&self) {
        // Dropping the handle is the helper's EOF. The up pipe is closed by the demux thread when
        // the helper has gone.
        self.down.closing.store(true, Ordering::SeqCst);
        // A frame write waiting on a helper that stopped reading holds the lock: cancel it until the lock is free,
        // which the failed write does. No frame starts after `closing`, so this ends.
        loop {
            match self.down.file.try_lock() {
                Ok(mut guard) => {
                    guard.take();
                    break;
                }
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    poisoned.into_inner().take();
                    break;
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    self.down.cancel_pending_write();
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        // The helper sees EOF once both handles are gone.
        self.down.cancel.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}

/// The bytes of `Output` frames. `Ok(0)` once the helper is gone.
struct ChannelReader {
    rx: mpsc::Receiver<Vec<u8>>,
    pending: Vec<u8>,
    at: usize,
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.at >= self.pending.len() {
            match self.rx.recv() {
                Ok(chunk) => {
                    self.pending = chunk;
                    self.at = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.pending.len() - self.at);
        buf[..n].copy_from_slice(&self.pending[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// Reads the up pipe until the helper is gone: `Output` goes to `tx`, `Exit` is the result, EOF without one is -1.
/// A full channel holds this loop back, so the helper's output pipe fills and the admin process waits, as a local
/// session's does. Once `closing` is set, or the reader is gone, a chunk is dropped instead, so Close never waits
/// on a reader that doesn't drain, and the `Exit` frame still ends the loop.
#[cfg_attr(not(windows), allow(dead_code))]
fn demux(mut up: impl Read, tx: mpsc::SyncSender<Vec<u8>>, closing: &AtomicBool) -> i32 {
    let mut code = -1;
    loop {
        match read_from_host(&mut up) {
            Ok(Some(FromHost::Output(mut bytes))) => loop {
                if closing.load(Ordering::SeqCst) {
                    break;
                }
                match tx.try_send(bytes) {
                    Ok(()) => break,
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                    Err(mpsc::TrySendError::Full(back)) => {
                        bytes = back;
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            },
            Ok(Some(FromHost::Exit { code: c })) => {
                code = c;
                break;
            }
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }
    code
}

/// Builds the session's pieces once the helper has said `Started`. One demux thread reads the up
/// pipe: `Output` feeds the reader, `Exit` is what `wait` returns, EOF without one makes it -1.
#[cfg_attr(not(windows), allow(dead_code))]
fn remote_pty(down: File, up: File, pid: u32) -> RemotePty {
    let down: Down = DownPipe::new(down);
    // 64 frames of at most 16 KiB is about the unacknowledged limit a local session holds back at.
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(64);
    // Only the flag goes to the thread, not the pipe: see `DownPipe::closing`.
    let closing = Arc::clone(&down.closing);
    let demux = std::thread::Builder::new()
        .name("marshell-host-demux".into())
        .spawn(move || demux(up, tx, &closing))
        .expect("a thread can be started");
    RemotePty {
        pid,
        reader: Box::new(ChannelReader {
            rx,
            pending: Vec::new(),
            at: 0,
        }),
        writer: Box::new(HostWriter {
            down: Arc::clone(&down),
        }),
        control: Box::new(HostControl { down }),
        wait: Box::new(move || demux.join().unwrap_or(-1)),
    }
}

#[cfg(not(windows))]
pub fn connect(_launcher: &dyn HostLauncher, _exe: &Path, _req: SpawnRequest) -> Result<RemotePty, ElevatedError> {
    Err(ElevatedError::Failed(NON_WINDOWS_COPY.to_string()))
}

#[cfg(windows)]
pub use win::{PlainLaunch, RunAs, connect};

#[cfg(windows)]
mod win {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{
        ERROR_CANCELLED, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
        STILL_ACTIVE, WAIT_FAILED, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetExitCodeProcess, GetProcessId, OpenProcessToken, WaitForSingleObject,
    };
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };

    const CONNECT_DEADLINE: Duration = Duration::from_secs(10);
    const PIPE_BUFFER: u32 = 64 * 1024;
    /// `SW_HIDE`, kept here so the `Win32_UI_WindowsAndMessaging` feature is not needed for one number.
    const SW_HIDE: i32 = 0;

    fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
        s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
    }

    /// What `ShellExecuteExW` failing with `code` means.
    pub(super) fn map_shell_error(code: u32) -> ElevatedError {
        if code == ERROR_CANCELLED {
            ElevatedError::Declined
        } else {
            ElevatedError::Io(io::Error::from_raw_os_error(code as i32))
        }
    }

    fn new_base() -> Result<String, ElevatedError> {
        let mut raw = [0u8; 16];
        getrandom::fill(&mut raw)
            .map_err(|e| ElevatedError::Failed(format!("The OS random number generator failed: {e}")))?;
        Ok(format!("marshell-host-{}", hex::encode(raw)))
    }

    /// The current user's SID as text, from the process token.
    fn current_user_sid() -> io::Result<String> {
        // SAFETY: every out-pointer describes a local that outlives its call; the token handle is
        // owned (and closed) by `OwnedHandle`; the buffer is 8-aligned and sized by the first call.
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(io::Error::last_os_error());
            }
            let token = OwnedHandle::from_raw_handle(token as RawHandle);
            let mut needed = 0u32;
            GetTokenInformation(
                token.as_raw_handle() as HANDLE,
                TokenUser,
                std::ptr::null_mut(),
                0,
                &mut needed,
            );
            if needed == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
            if GetTokenInformation(
                token.as_raw_handle() as HANDLE,
                TokenUser,
                buf.as_mut_ptr().cast(),
                needed,
                &mut needed,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let user = &*(buf.as_ptr().cast::<TOKEN_USER>());
            let mut text: *mut u16 = std::ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut len = 0;
            while *text.add(len) != 0 {
                len += 1;
            }
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            Ok(sid)
        }
    }

    /// A security descriptor that grants `GENERIC_ALL` to the current user and nobody else.
    struct OwnerOnly {
        attributes: SECURITY_ATTRIBUTES,
        descriptor: PSECURITY_DESCRIPTOR,
    }

    impl OwnerOnly {
        fn new() -> io::Result<OwnerOnly> {
            let sddl = wide(format!("D:P(A;;GA;;;{})", current_user_sid()?));
            let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            // SAFETY: `sddl` is NUL-terminated; revision 1 is SDDL_REVISION_1; the out-pointer is a local.
            let ok = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    1,
                    &mut descriptor,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(OwnerOnly {
                attributes: SECURITY_ATTRIBUTES {
                    nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                    lpSecurityDescriptor: descriptor,
                    bInheritHandle: 0,
                },
                descriptor,
            })
        }
    }

    impl Drop for OwnerOnly {
        fn drop(&mut self) {
            // SAFETY: the descriptor was allocated by the Convert call with LocalAlloc.
            unsafe { LocalFree(self.descriptor) };
        }
    }

    fn create_pipe(name: &str, direction: u32, owner: &OwnerOnly) -> io::Result<OwnedHandle> {
        let wname = wide(name);
        // SAFETY: `wname` is NUL-terminated; the attributes outlive the call; the result is checked.
        let h = unsafe {
            CreateNamedPipeW(
                wname.as_ptr(),
                direction | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                PIPE_BUFFER,
                PIPE_BUFFER,
                0,
                &owner.attributes,
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a valid handle nothing else owns.
        Ok(unsafe { OwnedHandle::from_raw_handle(h as RawHandle) })
    }

    /// Opens a client end, the way the helper does and the way this side unblocks a waiting connect.
    fn open_client(name: &str, access: u32) -> io::Result<OwnedHandle> {
        let wname = wide(name);
        // SAFETY: `wname` is NUL-terminated; the other pointers are null (defaults).
        let h = unsafe {
            CreateFileW(
                wname.as_ptr(),
                access,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a valid handle nothing else owns.
        Ok(unsafe { OwnedHandle::from_raw_handle(h as RawHandle) })
    }

    /// `Some(code)` once the process has exited.
    fn exited(process: &OwnedHandle) -> Option<i32> {
        let h = process.as_raw_handle() as HANDLE;
        let mut code = 0u32;
        // SAFETY: `h` is a valid handle owned by the caller; `code` is a local.
        let (waited, have_code) = unsafe { (WaitForSingleObject(h, 0), GetExitCodeProcess(h, &mut code) != 0) };
        let code = if have_code { code as i32 } else { -1 };
        match waited {
            WAIT_OBJECT_0 => Some(code),
            // A handle without SYNCHRONIZE access cannot be waited on; its exit code still tells.
            WAIT_FAILED if have_code && code != STILL_ACTIVE => Some(code),
            _ => None,
        }
    }

    fn client_pid(pipe: &OwnedHandle) -> io::Result<u32> {
        let mut pid = 0u32;
        // SAFETY: a valid pipe handle; `pid` is a local.
        let ok = unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle() as HANDLE, &mut pid) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(pid)
    }

    /// The two server ends: `down` (core to helper) and `up` (helper to core).
    pub(super) struct Pipes {
        pub(super) down: OwnedHandle,
        pub(super) up: OwnedHandle,
        pub(super) down_name: String,
        pub(super) up_name: String,
    }

    impl Pipes {
        pub(super) fn create(base: &str) -> io::Result<Pipes> {
            let owner = OwnerOnly::new()?;
            let down_name = format!(r"\\.\pipe\{base}-down");
            let up_name = format!(r"\\.\pipe\{base}-up");
            let down = create_pipe(&down_name, PIPE_ACCESS_OUTBOUND, &owner)?;
            let up = create_pipe(&up_name, PIPE_ACCESS_INBOUND, &owner)?;
            Ok(Pipes {
                down,
                up,
                down_name,
                up_name,
            })
        }

        /// Waits for a client on both pipes, the helper's process ending, or the deadline.
        /// On failure nothing stays blocked and every handle is closed.
        pub(super) fn wait_connected(self, process: &OwnedHandle) -> Result<Pipes, ElevatedError> {
            let (tx, rx) = mpsc::channel::<(usize, io::Result<()>)>();
            for (i, pipe) in [&self.down, &self.up].into_iter().enumerate() {
                let raw = pipe.as_raw_handle() as usize;
                let tx = tx.clone();
                let started = std::thread::Builder::new()
                    .name("marshell-host-connect".into())
                    .spawn(move || {
                        // SAFETY: the handle stays open until this thread has reported (or is leaked).
                        let ok = unsafe { ConnectNamedPipe(raw as HANDLE, std::ptr::null_mut()) } != 0;
                        let err = io::Error::last_os_error();
                        let connected = ok || err.raw_os_error() == Some(ERROR_PIPE_CONNECTED as i32);
                        let _ = tx.send((i, if connected { Ok(()) } else { Err(err) }));
                    });
                if let Err(e) = started {
                    return Err(ElevatedError::Io(e));
                }
            }
            drop(tx);

            let deadline = Instant::now() + CONNECT_DEADLINE;
            let mut reported = [false; 2];
            let mut failure = None;
            while !(reported[0] && reported[1]) {
                if let Some(code) = exited(process) {
                    failure = Some(ElevatedError::ExitedEarly(code));
                    break;
                }
                if Instant::now() >= deadline {
                    failure = Some(ElevatedError::Timeout);
                    break;
                }
                match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok((i, Ok(()))) => reported[i] = true,
                    Ok((i, Err(e))) => {
                        reported[i] = true;
                        failure = Some(ElevatedError::Io(e));
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            let Some(failure) = failure else {
                return Ok(self);
            };

            // Unblock a connect that is still waiting by connecting to it ourselves.
            let mut keep = Vec::new();
            for (i, (name, access)) in [(&self.down_name, GENERIC_READ), (&self.up_name, GENERIC_WRITE)]
                .into_iter()
                .enumerate()
            {
                if !reported[i]
                    && let Ok(h) = open_client(name, access)
                {
                    keep.push(h);
                }
            }
            let give_up = Instant::now() + Duration::from_secs(2);
            while !(reported[0] && reported[1]) {
                match rx.recv_timeout(give_up.saturating_duration_since(Instant::now())) {
                    Ok((i, _)) => reported[i] = true,
                    Err(_) => break,
                }
            }
            drop(keep);
            if !(reported[0] && reported[1]) {
                // A thread is still inside ConnectNamedPipe; its handle must not be closed under it.
                std::mem::forget(self);
            }
            Err(failure)
        }
    }

    /// The helper's exit code, allowing the OS a moment to record it.
    fn exit_code_soon(process: &OwnedHandle) -> i32 {
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            if let Some(code) = exited(process) {
                return code;
            }
            if Instant::now() >= deadline {
                return -1;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn connect(launcher: &dyn HostLauncher, exe: &Path, req: SpawnRequest) -> Result<RemotePty, ElevatedError> {
        if !exe.exists() {
            return Err(ElevatedError::Missing);
        }
        let base = new_base()?;
        let pipes = Pipes::create(&base).map_err(ElevatedError::Io)?;
        let args = format!("--pipe {base} --core-pid {}", std::process::id());
        let host = launcher.launch(exe, &args)?;
        let pipes = pipes.wait_connected(&host.process)?;

        let peers_ok =
            client_pid(&pipes.down).is_ok_and(|p| p == host.pid) && client_pid(&pipes.up).is_ok_and(|p| p == host.pid);
        if !peers_ok {
            return Err(ElevatedError::WrongPeer);
        }

        let Pipes { down, up, .. } = pipes;
        let (mut down, mut up) = (File::from(down), File::from(up));
        let early = |host: &LaunchedHost| ElevatedError::ExitedEarly(exit_code_soon(&host.process));
        if write_to_host(&mut down, &ToHost::Spawn(req)).is_err() {
            return Err(early(&host));
        }
        match read_from_host(&mut up) {
            Ok(Some(FromHost::Started { pid })) => Ok(remote_pty(down, up, pid)),
            Ok(Some(FromHost::Failed(text))) => Err(ElevatedError::Failed(text)),
            Ok(Some(_)) => Err(ElevatedError::Failed(
                "The administrator helper sent something unexpected. Nothing was started.".into(),
            )),
            Ok(None) | Err(_) => Err(early(&host)),
        }
    }

    /// The production launcher: UAC through `ShellExecuteExW` with `runas`.
    pub struct RunAs;

    impl HostLauncher for RunAs {
        fn launch(&self, exe: &Path, args: &str) -> Result<LaunchedHost, ElevatedError> {
            let file = wide(exe);
            let params = wide(args);
            let verb = wide("runas");
            // SAFETY: an all-zero SHELLEXECUTEINFOW is valid (null pointers and zeros); the strings
            // it points to outlive the call.
            let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
            info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
            info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
            info.lpVerb = verb.as_ptr();
            info.lpFile = file.as_ptr();
            info.lpParameters = params.as_ptr();
            info.nShow = SW_HIDE;
            // SAFETY: `info` is fully initialised above. This blocks while the UAC prompt is up.
            if unsafe { ShellExecuteExW(&mut info) } == 0 {
                let code = io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32;
                return Err(map_shell_error(code));
            }
            if info.hProcess.is_null() {
                return Err(ElevatedError::Failed(
                    "Windows did not return the administrator helper's process. Nothing was started.".into(),
                ));
            }
            // SAFETY: a valid process handle (SEE_MASK_NOCLOSEPROCESS) that only we own.
            let (pid, process) = unsafe {
                (
                    GetProcessId(info.hProcess),
                    OwnedHandle::from_raw_handle(info.hProcess as RawHandle),
                )
            };
            Ok(LaunchedHost { pid, process })
        }
    }

    /// Test-only: starts the helper unelevated, with `env` added to the inherited environment.
    #[doc(hidden)]
    pub struct PlainLaunch {
        pub env: Vec<(String, String)>,
    }

    impl HostLauncher for PlainLaunch {
        fn launch(&self, exe: &Path, args: &str) -> Result<LaunchedHost, ElevatedError> {
            let child = std::process::Command::new(exe)
                .args(args.split_whitespace())
                .envs(self.env.iter().map(|(k, v)| (k, v)))
                .stdin(std::process::Stdio::null())
                .spawn()
                .map_err(ElevatedError::Io)?;
            let pid = child.id();
            Ok(LaunchedHost {
                pid,
                process: OwnedHandle::from(child),
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
        use windows_sys::Win32::System::Threading::CreateEventW;

        #[test]
        fn a_second_client_cannot_open_the_pipe() {
            let pipes = Pipes::create(&new_base().unwrap()).unwrap();
            let _first = open_client(&pipes.down_name, GENERIC_READ).expect("the first client opens");
            let second = open_client(&pipes.down_name, GENERIC_READ).expect_err("the second is refused");
            assert_eq!(second.raw_os_error(), Some(ERROR_PIPE_BUSY as i32));
            let _first_up = open_client(&pipes.up_name, GENERIC_WRITE).expect("the first client opens");
            let second = open_client(&pipes.up_name, GENERIC_WRITE).expect_err("the second is refused");
            assert_eq!(second.raw_os_error(), Some(ERROR_PIPE_BUSY as i32));
        }

        #[test]
        fn a_pipe_name_already_taken_fails_the_create() {
            let base = new_base().unwrap();
            let _first = Pipes::create(&base).unwrap();
            assert!(Pipes::create(&base).is_err());
        }

        /// Opens both pipes from this process, claiming to be pid 1.
        struct Impostor(Mutex<Vec<OwnedHandle>>);

        impl HostLauncher for Impostor {
            fn launch(&self, _exe: &Path, args: &str) -> Result<LaunchedHost, ElevatedError> {
                let base = args.split_whitespace().nth(1).expect("--pipe <base>");
                let mut held = self.0.lock().unwrap();
                held.push(open_client(&format!(r"\\.\pipe\{base}-down"), GENERIC_READ).unwrap());
                held.push(open_client(&format!(r"\\.\pipe\{base}-up"), GENERIC_WRITE).unwrap());
                // SAFETY: a plain unnamed, unsignalled event; the result is checked.
                let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
                assert!(!event.is_null());
                // SAFETY: a valid handle that only we own.
                let process = unsafe { OwnedHandle::from_raw_handle(event as RawHandle) };
                Ok(LaunchedHost { pid: 1, process })
            }
        }

        #[test]
        fn a_client_that_is_not_the_launched_pid_is_refused() {
            let req = SpawnRequest {
                argv: vec!["cmd".into()],
                cwd: None,
                env: vec![],
                cols: 80,
                rows: 24,
            };
            let exe = std::env::current_exe().unwrap();
            let err = connect(&Impostor(Mutex::new(Vec::new())), &exe, req)
                .err()
                .expect("an error");
            assert!(matches!(err, ElevatedError::WrongPeer), "got {err:?}");
        }

        #[test]
        fn cancelling_the_uac_prompt_is_a_decline() {
            assert!(matches!(map_shell_error(1223), ElevatedError::Declined));
            assert!(matches!(map_shell_error(5), ElevatedError::Io(e) if e.raw_os_error() == Some(5)));
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn display_is_the_copy() {
        assert_eq!(
            ElevatedError::ExitedEarly(3).to_string(),
            "The administrator helper stopped before it connected (exit code 3). Nothing was started."
        );
        assert_eq!(
            ElevatedError::Declined.to_string(),
            "Administrator rights were declined. Nothing was started."
        );
        assert_eq!(
            ElevatedError::Timeout.to_string(),
            "The administrator helper didn't connect within 10 seconds. Nothing was started."
        );
        assert_eq!(
            ElevatedError::Missing.to_string(),
            "The administrator helper (marshell-host) is missing next to Marshell. Reinstall Marshell."
        );
        assert_eq!(
            ElevatedError::WrongPeer.to_string(),
            "Another program connected to the administrator session's pipe, so it was refused. Nothing was started."
        );
    }

    #[test]
    fn a_large_write_is_split_into_frames_of_at_most_max_frame() {
        use marshell_protocol::host::{ToHost, read_to_host};
        use std::io::Seek;
        let file = tempfile::tempfile().unwrap();
        let mut reader = file.try_clone().unwrap();
        let down: Down = DownPipe::new(file);
        let mut writer = HostWriter { down };
        let data: Vec<u8> = (0..(5 * MAX_FRAME / 2)).map(|i| (i % 251) as u8).collect();
        writer.write_all(&data).unwrap();
        reader.rewind().unwrap();
        let (mut got, mut frames) = (Vec::new(), 0);
        while let Some(m) = read_to_host(&mut reader).unwrap() {
            let ToHost::Input(bytes) = m else {
                panic!("only Input frames")
            };
            assert!(bytes.len() <= MAX_FRAME);
            got.extend_from_slice(&bytes);
            frames += 1;
        }
        assert_eq!(frames, 3);
        assert!(got == data);
    }

    #[test]
    fn a_closed_session_refuses_writes_and_resizes() {
        let control = HostControl {
            down: DownPipe::closed(),
        };
        assert!(control.resize(80, 24).is_err());
        let mut writer = HostWriter {
            down: Arc::clone(&control.down),
        };
        assert_eq!(writer.write(b"x").unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        // Out of range is refused before anything is written.
        let live = HostControl {
            down: DownPipe::new(tempfile::tempfile().unwrap()),
        };
        assert!(live.resize(0, 24).is_err());
        assert!(live.resize(80, 1001).is_err());
        assert!(live.resize(80, 24).is_ok());
    }

    #[test]
    fn a_missing_helper_is_reported() {
        struct Never;
        impl HostLauncher for Never {
            fn launch(&self, _: &Path, _: &str) -> Result<LaunchedHost, ElevatedError> {
                panic!("must not launch");
            }
        }
        let req = SpawnRequest {
            argv: vec!["cmd".into()],
            cwd: None,
            env: vec![],
            cols: 80,
            rows: 24,
        };
        let err = connect(&Never, Path::new(r"C:\no-such-dir\marshell-host.exe"), req)
            .err()
            .expect("an error");
        assert!(matches!(err, ElevatedError::Missing));
    }
}

/// A helper that never drains `-down`, for the tests that a stuck frame write does not freeze the rest.
#[cfg(all(test, windows))]
pub(crate) mod stuck {
    use super::*;
    use std::os::windows::io::{FromRawHandle, RawHandle};
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Pipes::CreatePipe;

    /// An anonymous pipe with a small buffer: `(read end, write end)`.
    fn pipe() -> (File, File) {
        let (mut read, mut write): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: both out-pointers are valid locals; the result is checked.
        let ok = unsafe { CreatePipe(&mut read, &mut write, std::ptr::null(), 4096) };
        assert!(ok != 0, "CreatePipe failed");
        // SAFETY: two fresh handles that only these Files own.
        unsafe {
            (
                File::from_raw_handle(read as RawHandle),
                File::from_raw_handle(write as RawHandle),
            )
        }
    }

    /// The core's pieces for a helper that reads nothing from `-down` and sends nothing on `-up`. Keep the returned
    /// files alive: they are the helper's ends of the pipes.
    pub(crate) fn stuck_remote() -> (RemotePty, Vec<File>) {
        let (down_read, down_write) = pipe();
        let (up_read, up_write) = pipe();
        (remote_pty(down_write, up_read, 1), vec![down_read, up_write])
    }

    /// Runs `f` on its own thread and gives its result, or None when it has not returned within two seconds.
    pub(crate) fn within_two_seconds<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(std::time::Duration::from_secs(2)).ok()
    }

    #[test]
    fn resize_and_close_return_while_a_frame_write_is_stuck() {
        let (remote, _helper) = stuck_remote();
        let RemotePty {
            mut writer, control, ..
        } = remote;
        let control: Arc<dyn PtyControl> = Arc::from(control);
        // More than the pipe holds: the write waits for a helper that never reads.
        std::thread::spawn(move || {
            let _ = writer.write(&vec![b'x'; 60_000]);
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
        let c = Arc::clone(&control);
        assert!(
            within_two_seconds(move || c.resize(100, 30)).is_some(),
            "resize waited behind the stuck frame"
        );
        let c = Arc::clone(&control);
        assert!(
            within_two_seconds(move || c.close()).is_some(),
            "close waited behind the stuck frame"
        );
    }
}

#[cfg(test)]
mod demux_tests {
    use super::*;
    use marshell_protocol::host::write_from_host;
    use std::io::Cursor;
    use std::sync::atomic::AtomicUsize;

    /// 15 bytes per `Output` frame: the 5-byte header and a 10-byte payload.
    const FRAME_BYTES: usize = 15;

    /// 200 `Output` frames and an `Exit { code: 7 }`.
    fn frames() -> Vec<u8> {
        let mut bytes = Vec::new();
        for i in 0..200u8 {
            write_from_host(&mut bytes, &FromHost::Output(vec![i; 10])).unwrap();
        }
        write_from_host(&mut bytes, &FromHost::Exit { code: 7 }).unwrap();
        bytes
    }

    /// A reader that counts the bytes taken from it.
    struct Counting {
        inner: Cursor<Vec<u8>>,
        taken: Arc<AtomicUsize>,
    }

    impl Read for Counting {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.taken.fetch_add(n, Ordering::SeqCst);
            Ok(n)
        }
    }

    struct Stalled {
        done: mpsc::Receiver<i32>,
        rx: mpsc::Receiver<Vec<u8>>,
        taken: Arc<AtomicUsize>,
        closing: Arc<AtomicBool>,
    }

    /// Runs `demux` on a thread with a channel of four and nobody reading, and waits until it has stalled.
    fn stalled() -> Stalled {
        let taken = Arc::new(AtomicUsize::new(0));
        let up = Counting {
            inner: Cursor::new(frames()),
            taken: Arc::clone(&taken),
        };
        let closing = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(4);
        let (done_tx, done) = mpsc::channel();
        let flag = Arc::clone(&closing);
        std::thread::spawn(move || {
            let _ = done_tx.send(demux(up, tx, &flag));
        });
        std::thread::sleep(Duration::from_millis(300));
        Stalled {
            done,
            rx,
            taken,
            closing,
        }
    }

    #[test]
    fn a_reader_that_does_not_drain_holds_the_demux_back_at_the_channel_capacity() {
        let s = stalled();
        let frames_read = s.taken.load(Ordering::SeqCst) / FRAME_BYTES;
        assert!(
            (4..=6).contains(&frames_read),
            "the demux read {frames_read} frames past a channel of four"
        );
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            s.taken.load(Ordering::SeqCst) / FRAME_BYTES,
            frames_read,
            "the demux kept reading"
        );
        assert!(s.done.try_recv().is_err(), "the demux returned while stalled");
        // Draining lets it finish, and every chunk arrives in order.
        let mut got = 0usize;
        let deadline = Instant::now() + Duration::from_secs(5);
        let code = loop {
            while let Ok(chunk) = s.rx.try_recv() {
                assert_eq!(chunk, vec![got as u8; 10]);
                got += 1;
            }
            if let Ok(code) = s.done.try_recv() {
                break code;
            }
            assert!(Instant::now() < deadline, "the demux did not finish after a drain");
            std::thread::sleep(Duration::from_millis(5));
        };
        while let Ok(chunk) = s.rx.try_recv() {
            assert_eq!(chunk, vec![got as u8; 10]);
            got += 1;
        }
        assert_eq!(code, 7);
        assert_eq!(got, 200);
    }

    #[test]
    fn closing_ends_a_stalled_demux_within_a_second() {
        let s = stalled();
        assert!(s.done.try_recv().is_err(), "the demux returned while stalled");
        s.closing.store(true, Ordering::SeqCst);
        let code = s
            .done
            .recv_timeout(Duration::from_secs(1))
            .expect("the demux ended after closing");
        assert_eq!(code, 7);
    }

    #[test]
    fn a_dropped_reader_does_not_stop_the_demux_before_the_exit_frame() {
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(4);
        drop(rx);
        let closing = AtomicBool::new(false);
        assert_eq!(demux(Cursor::new(frames()), tx, &closing), 7);
    }
}
