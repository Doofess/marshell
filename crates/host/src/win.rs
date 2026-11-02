//! The Windows parts of the helper: elevation, the job, and the two pipes to the core.

use std::fs::File;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, OPEN_EXISTING, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows_sys::Win32::System::Pipes::{GetNamedPipeServerProcessId, WaitNamedPipeW};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const CONNECT_DEADLINE: Duration = Duration::from_secs(10);

pub fn is_elevated() -> bool {
    // SAFETY: the token handle is checked before use and closed afterwards; the out-pointers
    // describe locals that outlive the calls.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            (&raw mut elevation).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// A job that kills every process in it when its handle closes (this process ends, however it ends).
/// There is no breakaway, so a process in it takes its later children with it. Keep the value alive
/// for the life of `main`.
pub struct Job(HANDLE);

impl Job {
    /// Creates the job and puts this process in it. Call before spawning anything.
    pub fn tie_this_process() -> anyhow::Result<Job> {
        // SAFETY: GetCurrentProcess is a pseudo-handle that needs no closing.
        Self::holding(unsafe { GetCurrentProcess() })
            .map_err(|e| anyhow::anyhow!("could not put the helper in a job object: {e}"))
    }

    /// Puts the started program in a job of its own, right after it starts.
    ///
    /// The helper's own job does not always reach it: PowerShell 7 from the Microsoft Store, started
    /// in a ConPTY from the bundled OpenConsole, is not in its creator's job. A killed core then took
    /// the helper and OpenConsole down and left the shell and everything it ran (live check
    /// 2026-11-01). A new, empty job takes any process, and its children from then on.
    pub fn tie_child(process: RawHandle) -> anyhow::Result<Job> {
        Self::holding(process as HANDLE).map_err(|e| anyhow::anyhow!("could not put the program in a job object: {e}"))
    }

    fn holding(process: HANDLE) -> std::io::Result<Job> {
        // SAFETY: both arguments may be null (default security, no name); the result is checked.
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: an all-zero JOBOBJECT_EXTENDED_LIMIT_INFORMATION is valid (plain integers and sizes).
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `job` and `process` are valid; the pointer and size describe `info`, which outlives the call.
        let done = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) != 0
                && AssignProcessToJobObject(job, process) != 0
        };
        if !done {
            let err = std::io::Error::last_os_error();
            // SAFETY: `job` is a valid handle that nothing else uses.
            unsafe { CloseHandle(job) };
            return Err(err);
        }
        Ok(Job(job))
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle is valid and owned here. Closing it kills the job's processes.
        unsafe { CloseHandle(self.0) };
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Opens one end of a core pipe, retrying until the deadline while the core has not created it yet
/// or its instance is busy. The pipe is opened for identification only, so the server can't
/// impersonate this elevated token.
fn open_pipe(name: &str, access: u32, deadline: Instant) -> anyhow::Result<File> {
    let wname = wide(name);
    loop {
        // SAFETY: `wname` is a NUL-terminated UTF-16 string; other pointers are null (defaults).
        let h = unsafe {
            CreateFileW(
                wname.as_ptr(),
                access,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                std::ptr::null_mut(),
            )
        };
        if h != INVALID_HANDLE_VALUE {
            // SAFETY: `h` is a valid handle that nothing else owns; File takes it.
            return Ok(unsafe { File::from_raw_handle(h as RawHandle) });
        }
        let err = std::io::Error::last_os_error();
        let code = err.raw_os_error().unwrap_or(0) as u32;
        if (code != ERROR_FILE_NOT_FOUND && code != ERROR_PIPE_BUSY) || Instant::now() >= deadline {
            anyhow::bail!("could not open {name}: {err}");
        }
        if code == ERROR_PIPE_BUSY {
            // SAFETY: `wname` is NUL-terminated.
            unsafe { WaitNamedPipeW(wname.as_ptr(), 200) };
        } else {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn server_pid(pipe: &File) -> anyhow::Result<u32> {
    use std::os::windows::io::AsRawHandle;
    let mut pid = 0u32;
    // SAFETY: the handle is a valid open pipe; `pid` outlives the call.
    let ok = unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle() as HANDLE, &mut pid) };
    anyhow::ensure!(
        ok != 0,
        "could not read the pipe server: {}",
        std::io::Error::last_os_error()
    );
    Ok(pid)
}

/// The two pipes: `down` (core to helper, read) and `up` (helper to core, write).
pub struct Pipes {
    pub down: File,
    pub up: File,
}

/// Connects to `\\.\pipe\<base>-down` and `-up` within 10 s and checks that `core_pid` serves both.
pub fn connect(base: &str, core_pid: u32) -> anyhow::Result<Pipes> {
    let deadline = Instant::now() + CONNECT_DEADLINE;
    let down = open_pipe(&format!(r"\\.\pipe\{base}-down"), GENERIC_READ, deadline)?;
    let up = open_pipe(&format!(r"\\.\pipe\{base}-up"), GENERIC_WRITE, deadline)?;
    anyhow::ensure!(
        server_pid(&down)? == core_pid && server_pid(&up)? == core_pid,
        "the pipe is served by another process"
    );
    Ok(Pipes { down, up })
}
