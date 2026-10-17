//! Freezes a session's whole process tree and thaws it again.
//! Windows: suspends every thread of the session's process and its descendants.
//! Unix: SIGSTOP and SIGCONT to the session's process group.

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

/// What a thaw needs: the threads that were suspended on Windows, nothing on Unix.
pub(crate) struct Frozen {
    #[cfg(windows)]
    threads: Vec<u32>,
}

/// Freezes the tree rooted at `pid`. Either all of it is frozen or none of it is.
pub(crate) fn freeze_tree(pid: u32) -> Result<Frozen, ()> {
    #[cfg(windows)]
    {
        windows::suspend_tree(pid).map(|threads| Frozen { threads })
    }
    #[cfg(unix)]
    {
        // portable-pty starts the child in its own session, so its pid is the process group id.
        // SAFETY: killpg only sends a signal; a bad pid makes it fail, which is checked.
        if unsafe { libc::killpg(pid as i32, libc::SIGSTOP) } == 0 {
            Ok(Frozen {})
        } else {
            Err(())
        }
    }
}

/// Thaws what `freeze_tree` froze.
pub(crate) fn thaw_tree(pid: u32, frozen: &Frozen) {
    #[cfg(windows)]
    {
        let _ = pid;
        windows::resume_threads(&frozen.threads);
    }
    #[cfg(unix)]
    {
        let _ = frozen;
        // SAFETY: killpg only sends a signal; the caller knows the group is still ours.
        unsafe {
            libc::killpg(pid as i32, libc::SIGCONT);
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::collections::HashMap;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
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

    /// Suspends every thread of `root` and its descendants. Returns the thread ids it suspended,
    /// or rolls everything back and fails.
    pub fn suspend_tree(root: u32) -> Result<Vec<u32>, ()> {
        let mut done: Vec<u32> = Vec::new();
        for _ in 0..MAX_PASSES {
            let Some(pids) = descendants(root) else {
                resume_threads(&done);
                return Err(());
            };
            let Some(threads) = threads_of(&pids) else {
                resume_threads(&done);
                return Err(());
            };
            let fresh: Vec<u32> = threads.into_iter().filter(|t| !done.contains(t)).collect();
            if fresh.is_empty() {
                return Ok(done);
            }
            for tid in fresh {
                if !suspend(tid) {
                    resume_threads(&done);
                    return Err(());
                }
                done.push(tid);
            }
        }
        resume_threads(&done);
        Err(())
    }

    pub fn resume_threads(threads: &[u32]) {
        for &tid in threads {
            // SAFETY: the result is checked; the handle is closed by `Owned`.
            let handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, tid) };
            if !handle.is_null() {
                let handle = Owned(handle);
                // SAFETY: `handle` is a valid thread handle with the resume right.
                unsafe { ResumeThread(handle.0) };
            }
        }
    }

    fn suspend(tid: u32) -> bool {
        // SAFETY: the result is checked; the handle is closed by `Owned`.
        let handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, tid) };
        if handle.is_null() {
            return false;
        }
        let handle = Owned(handle);
        // SAFETY: `handle` is a valid thread handle with the suspend right.
        unsafe { SuspendThread(handle.0) != u32::MAX }
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

    /// When the process started, or `None` if it cannot be read (it ended, or it is not ours to query).
    fn creation_time(pid: u32) -> Option<u64> {
        // SAFETY: the result is checked; the handle is closed by `Owned`.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let handle = Owned(handle);
        let zero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        // SAFETY: `handle` is a valid process handle; the four pointers are valid FILETIMEs.
        let ok = unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } != 0;
        ok.then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }

    /// `root` and every process it started. A child counts only if it started after its parent, so a
    /// process whose recorded parent pid was reused by one of ours is left alone. A child whose start
    /// time cannot be read is left alone too. `None` if `root` itself cannot be read.
    fn descendants(root: u32) -> Option<Vec<u32>> {
        let procs = processes()?;
        let mut tree: HashMap<u32, u64> = HashMap::from([(root, creation_time(root)?)]);
        loop {
            let mut grew = false;
            for &(pid, parent) in &procs {
                if pid == parent || tree.contains_key(&pid) {
                    continue;
                }
                let Some(&parent_born) = tree.get(&parent) else {
                    continue;
                };
                if let Some(born) = creation_time(pid)
                    && born > parent_born
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
