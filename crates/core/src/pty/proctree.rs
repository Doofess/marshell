//! Windows process tree helpers over Toolhelp: who a session's processes are, and what runs in front.

use crate::profiles::detect::program_name;
use std::collections::HashMap;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

/// A chain longer than this is a loop made by reused pids, not a real tree.
const MAX_DEPTH: usize = 32;

/// A handle that closes itself.
pub(super) struct Owned(pub(super) HANDLE);

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

pub(super) fn snapshot(flags: u32) -> Option<Owned> {
    // SAFETY: process id 0 means every process; the result is checked.
    let handle = unsafe { CreateToolhelp32Snapshot(flags, 0) };
    (handle != INVALID_HANDLE_VALUE).then_some(Owned(handle))
}

/// (pid, parent pid, exe file name) of every process. `None` if the snapshot cannot be taken.
fn processes() -> Option<Vec<(u32, u32, String)>> {
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
        let len = entry
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(entry.szExeFile.len());
        out.push((
            entry.th32ProcessID,
            entry.th32ParentProcessID,
            String::from_utf16_lossy(&entry.szExeFile[..len]),
        ));
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
pub(super) fn descendants(root: u32) -> Option<Vec<u32>> {
    let procs = processes()?;
    let mut tree: HashMap<u32, u64> = HashMap::from([(root, creation_time(root).ok()??)]);
    loop {
        let mut grew = false;
        for &(pid, parent, _) in &procs {
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

/// True for the console host a pseudoconsole starts beside its shell. It is never what a user runs.
fn is_console_host(exe: &str) -> bool {
    let name = exe.to_lowercase();
    name == "conhost.exe" || name == "openconsole.exe"
}

/// The programs running under `root`, outermost first, root excluded: at each step the newest child
/// (by the same rule as `descendants`) that is not a console host. Empty when `root` has no child.
/// `None` if the snapshot cannot be taken or `root` is gone.
pub(super) fn foreground_chain(root: u32) -> Option<Vec<String>> {
    let procs = processes()?;
    let mut current = root;
    let mut born = creation_time(root).ok()??;
    let mut chain = Vec::new();
    while chain.len() < MAX_DEPTH {
        let newest = procs
            .iter()
            .filter(|(pid, parent, exe)| *parent == current && *pid != current && !is_console_host(exe))
            .filter_map(|(pid, _, exe)| match creation_time(*pid) {
                Ok(Some(t)) if t >= born => Some((t, *pid, exe)),
                _ => None,
            })
            .max_by_key(|(t, _, _)| *t);
        let Some((t, pid, exe)) = newest else {
            break;
        };
        chain.push(program_name(exe));
        (current, born) = (pid, t);
    }
    Some(chain)
}
