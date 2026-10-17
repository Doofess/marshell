//! Freezes a session's whole process tree and thaws it again.
//! Windows: suspends every thread of the session's process and its descendants.
//! Unix: SIGSTOP and SIGCONT to the session's process group and the terminal's foreground group.

use std::fmt;

/// Why a session could not be frozen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezeError {
    NotRunning,
    Partial,
}

impl fmt::Display for FreezeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FreezeError::NotRunning => "Only a running session can be paused.",
            FreezeError::Partial => "Part of it can't be paused, such as a program running as administrator.",
        })
    }
}

impl std::error::Error for FreezeError {}

/// What a thaw needs: on Windows the suspended threads, kept open so their ids cannot be reused;
/// on Unix the process groups that were stopped, in the order they were stopped.
pub(crate) struct Frozen {
    #[cfg(windows)]
    threads: Vec<windows::Suspended>,
    #[cfg(unix)]
    groups: Vec<i32>,
}

/// Freezes the tree rooted at `pid`. Either all of it is frozen or none of it.
/// `foreground` is the terminal's foreground process group, if known. On Unix an interactive shell
/// puts its foreground job in a group of its own, which the session's group does not include.
/// A background job (`&`) is in yet another group and stays running. Windows ignores `foreground`.
pub(crate) fn freeze_tree(pid: u32, foreground: Option<u32>) -> Result<Frozen, ()> {
    #[cfg(windows)]
    {
        let _ = foreground;
        windows::suspend_tree(pid).map(|threads| Frozen { threads })
    }
    #[cfg(unix)]
    {
        // portable-pty starts the child in its own session, so its pid is the process group id.
        // The foreground group goes first so a shell never runs on to wait for a job still running.
        let mut order = Vec::new();
        if let Some(fg) = foreground.map(|g| g as i32).filter(|&g| g != pid as i32) {
            order.push(fg);
        }
        order.push(pid as i32);
        let mut groups = Vec::new();
        for &group in &order {
            match signal_group(group, libc::SIGSTOP) {
                Ok(()) => groups.push(group),
                // The foreground job ended since it was read; there is nothing left to stop.
                Err(libc::ESRCH) if group != pid as i32 => {}
                // Not allowed (such as a setuid program) or the session's own group is gone:
                // undo what was stopped and refuse.
                Err(_) => {
                    for &stopped in groups.iter().rev() {
                        let _ = signal_group(stopped, libc::SIGCONT);
                    }
                    return Err(());
                }
            }
        }
        Ok(Frozen { groups })
    }
}

/// Thaws what `freeze_tree` froze.
pub(crate) fn thaw_tree(frozen: &Frozen) {
    #[cfg(windows)]
    {
        windows::resume_threads(&frozen.threads);
    }
    #[cfg(unix)]
    {
        for &group in frozen.groups.iter().rev() {
            let _ = signal_group(group, libc::SIGCONT);
        }
    }
}

/// Sends `signal` to a process group. `Err` holds the errno.
#[cfg(unix)]
fn signal_group(group: i32, signal: i32) -> Result<(), i32> {
    // SAFETY: killpg only sends a signal; a bad group makes it fail, which is returned.
    if unsafe { libc::killpg(group, signal) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EPERM))
    }
}

#[cfg(windows)]
mod windows {
    use std::collections::HashMap;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, FILETIME, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
        TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, OpenThread, PROCESS_QUERY_LIMITED_INFORMATION, ResumeThread, SuspendThread,
        THREAD_SUSPEND_RESUME,
    };

    /// A pass that finds no new thread ends the walk. A tree still growing after this many is refused.
    const MAX_PASSES: usize = 5;

    /// A handle that closes itself.
    struct Owned(HANDLE);

    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: the handle is valid and nothing else closes it.
            unsafe { CloseHandle(self.0) };
        }
    }

    // SAFETY: a kernel handle is only a number the kernel looks up; any thread may use or close it.
    unsafe impl Send for Owned {}
    // SAFETY: as above.
    unsafe impl Sync for Owned {}

    /// A thread this module suspended. The open handle keeps its id from being reused.
    pub struct Suspended {
        id: u32,
        handle: Owned,
    }

    /// Suspends every thread of `root` and its descendants. Returns the threads it suspended,
    /// or rolls everything back and fails.
    pub fn suspend_tree(root: u32) -> Result<Vec<Suspended>, ()> {
        let mut done: Vec<Suspended> = Vec::new();
        for _ in 0..MAX_PASSES {
            let Some(pids) = descendants(root) else {
                resume_threads(&done);
                return Err(());
            };
            let Some(threads) = threads_of(&pids) else {
                resume_threads(&done);
                return Err(());
            };
            let fresh: Vec<u32> = threads
                .into_iter()
                .filter(|t| done.iter().all(|d| d.id != *t))
                .collect();
            if fresh.is_empty() {
                return Ok(done);
            }
            for tid in fresh {
                match suspend(tid) {
                    Ok(Some(thread)) => done.push(thread),
                    Ok(None) => {}
                    Err(()) => {
                        resume_threads(&done);
                        return Err(());
                    }
                }
            }
        }
        resume_threads(&done);
        Err(())
    }

    pub fn resume_threads(threads: &[Suspended]) {
        for thread in threads {
            // SAFETY: the handle is a valid thread handle with the resume right.
            unsafe { ResumeThread(thread.handle.0) };
        }
    }

    /// `Ok(None)` if the thread ended since the snapshot.
    fn suspend(tid: u32) -> Result<Option<Suspended>, ()> {
        // SAFETY: the result is checked; the handle is closed by `Owned`.
        let handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, tid) };
        if handle.is_null() {
            return if std::io::Error::last_os_error().raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                Ok(None)
            } else {
                Err(())
            };
        }
        let handle = Owned(handle);
        // SAFETY: `handle` is a valid thread handle with the suspend right.
        if unsafe { SuspendThread(handle.0) } == u32::MAX {
            return Err(());
        }
        Ok(Some(Suspended { id: tid, handle }))
    }

    fn snapshot(flags: u32) -> Option<Owned> {
        // SAFETY: process id 0 means every process; the result is checked.
        let handle = unsafe { CreateToolhelp32Snapshot(flags, 0) };
        (handle != INVALID_HANDLE_VALUE).then_some(Owned(handle))
    }

    /// (pid, parent pid) of every process. `None` if the snapshot cannot be taken.
    fn processes() -> Option<Vec<(u32, u32)>> {
        let snap = snapshot(TH32CS_SNAPPROCESS)?;
        // SAFETY: an all-zero PROCESSENTRY32W is valid (plain integers and arrays).
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        // SAFETY: `snap` is a valid snapshot and `entry` has its size set.
        let mut more = unsafe { Process32FirstW(snap.0, &mut entry) } != 0;
        if !more {
            return None;
        }
        let mut out = Vec::new();
        while more {
            out.push((entry.th32ProcessID, entry.th32ParentProcessID));
            // SAFETY: as above.
            more = unsafe { Process32NextW(snap.0, &mut entry) } != 0;
        }
        Some(out)
    }

    /// When the process started. `Ok(None)` if it is gone. `Err` if it exists but cannot be queried,
    /// such as a process running as administrator.
    fn creation_time(pid: u32) -> Result<Option<u64>, ()> {
        // SAFETY: the result is checked; the handle is closed by `Owned`.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return match std::io::Error::last_os_error().raw_os_error() {
                Some(code) if code == ERROR_ACCESS_DENIED as i32 => Err(()),
                _ => Ok(None),
            };
        }
        let handle = Owned(handle);
        let zero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        // SAFETY: `handle` is a valid process handle; the four pointers are valid FILETIMEs.
        let ok = unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } != 0;
        // The process exists but cannot be timed, so it cannot be told apart from a reused pid.
        if !ok {
            return Err(());
        }
        Ok(Some(
            (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime),
        ))
    }

    /// `root` and every process it started. A child counts only if it did not start before its parent (they can share a clock tick), so a
    /// process whose recorded parent pid was reused by one of ours is left alone. A child that is gone
    /// is skipped. `None` if `root` or a child cannot be queried, so the tree is never frozen in part.
    fn descendants(root: u32) -> Option<Vec<u32>> {
        let procs = processes()?;
        let mut tree: HashMap<u32, u64> = HashMap::from([(root, creation_time(root).ok()??)]);
        loop {
            let mut grew = false;
            for &(pid, parent) in &procs {
                if pid == parent || tree.contains_key(&pid) {
                    continue;
                }
                let Some(&parent_born) = tree.get(&parent) else {
                    continue;
                };
                if let Some(born) = creation_time(pid).ok()?
                    && born >= parent_born
                {
                    tree.insert(pid, born);
                    grew = true;
                }
            }
            if !grew {
                return Some(tree.into_keys().collect());
            }
        }
    }

    /// Ids of every thread owned by one of `pids`. `None` if the snapshot cannot be taken.
    fn threads_of(pids: &[u32]) -> Option<Vec<u32>> {
        let snap = snapshot(TH32CS_SNAPTHREAD)?;
        // SAFETY: an all-zero THREADENTRY32 is valid (plain integers).
        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        // SAFETY: `snap` is a valid snapshot and `entry` has its size set.
        let mut more = unsafe { Thread32First(snap.0, &mut entry) } != 0;
        if !more {
            return None;
        }
        let mut out = Vec::new();
        while more {
            if pids.contains(&entry.th32OwnerProcessID) {
                out.push(entry.th32ThreadID);
            }
            // SAFETY: as above.
            more = unsafe { Thread32Next(snap.0, &mut entry) } != 0;
        }
        Some(out)
    }
}
