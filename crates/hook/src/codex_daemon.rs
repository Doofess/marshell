//! Whether a Codex hook runs inside Codex's shared app-server daemon (slice 6a decision 12). Codex
//! 0.163 runs hooks in the daemon unless it was started with `--no-daemon`, and the daemon keeps the
//! environment of the Codex that started it, so its `MARSHELL_TAB_ID` names that first tab whatever
//! session the hook is for. The bridge then reports to no tab rather than to the wrong one.

/// How far up the parent chain to look for Codex: the hook's shell, then Codex, with room to spare.
const MAX_DEPTH: usize = 6;

/// True when the nearest `codex` among `ancestors` (image paths, parent first) is the daemon's: its
/// binary lives under `app-server-daemon` in `CODEX_HOME`. A chain with no Codex in it is false.
pub fn runs_in_daemon(ancestors: &[String]) -> bool {
    ancestors
        .iter()
        .take(MAX_DEPTH)
        .find(|path| is_codex(path))
        .is_some_and(|path| components(path).any(|c| c.eq_ignore_ascii_case("app-server-daemon")))
}

fn components(path: &str) -> impl DoubleEndedIterator<Item = &str> {
    path.split(['\\', '/']).filter(|c| !c.is_empty())
}

fn is_codex(path: &str) -> bool {
    components(path)
        .next_back()
        .is_some_and(|name| name.eq_ignore_ascii_case("codex.exe") || name == "codex")
}

/// This process's ancestors' image paths, parent first, up to `MAX_DEPTH`. An ancestor whose path
/// can't be read ends the chain.
#[cfg(windows)]
pub fn ancestors() -> Vec<String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };

    // SAFETY: a process snapshot takes no pointers; the handle is checked and closed below.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return Vec::new();
    }
    let mut parents = std::collections::HashMap::new();
    // SAFETY: an all-zero PROCESSENTRY32W is valid once dwSize is set.
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    // SAFETY: `snap` is a valid snapshot and `entry` a sized PROCESSENTRY32W that outlives each call.
    let mut more = unsafe { Process32FirstW(snap, &mut entry) } != 0;
    while more {
        parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
        // SAFETY: as above.
        more = unsafe { Process32NextW(snap, &mut entry) } != 0;
    }
    // SAFETY: `snap` is a valid handle that nothing else uses.
    unsafe { CloseHandle(snap) };

    let image = |pid: u32| -> Option<String> {
        // SAFETY: OpenProcess takes no pointers; a null result is checked.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: `handle` is valid; `buf` and `len` describe a writable buffer that outlives the call.
        let ok = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) };
        // SAFETY: `handle` is a valid handle that nothing else uses.
        unsafe { CloseHandle(handle) };
        (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
    };

    let mut chain = Vec::new();
    let mut pid = std::process::id();
    while chain.len() < MAX_DEPTH {
        match parents.get(&pid) {
            Some(&parent) if parent != 0 && parent != pid => pid = parent,
            _ => break,
        }
        match image(pid) {
            Some(path) => chain.push(path),
            None => break,
        }
    }
    chain
}

/// Not verified off Windows: no chain, so the tab id stands.
#[cfg(not(windows))]
pub fn ancestors() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAEMON: &str = r"C:\Users\me\AppData\Local\Temp\cx\packages\app-server-daemon\releases\0.163.0-x86_64-pc-windows-msvc\bin\codex.exe";
    const TUI: &str = r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe";
    const SHELL: &str = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    const PWSH: &str = r"C:\Program Files\PowerShell\7\pwsh.exe";

    fn chain(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn a_hook_run_by_the_daemon_is_in_the_daemon() {
        // The daemon is a child of the Codex that started it, in another tab.
        assert!(runs_in_daemon(&chain(&[SHELL, DAEMON, TUI, PWSH])));
        let extended = DAEMON
            .replace(r"C:\", r"\\?\C:\")
            .replace("app-server-daemon", "App-Server-Daemon");
        assert!(runs_in_daemon(&chain(&[SHELL, &extended])));
    }

    #[test]
    fn a_hook_run_by_codex_without_the_daemon_is_not() {
        assert!(!runs_in_daemon(&chain(&[SHELL, TUI, PWSH])));
        assert!(!runs_in_daemon(&chain(&[SHELL, "/usr/local/bin/codex", "/bin/bash"])));
    }

    #[test]
    fn only_the_nearest_codex_counts() {
        // A Codex without the daemon, started from a shell the daemon ran, is not the daemon's.
        assert!(!runs_in_daemon(&chain(&[SHELL, TUI, PWSH, DAEMON])));
    }

    #[test]
    fn a_chain_without_codex_or_past_the_depth_is_not() {
        assert!(!runs_in_daemon(&[]));
        assert!(!runs_in_daemon(&chain(&[SHELL, PWSH])));
        assert!(!runs_in_daemon(&chain(&[
            SHELL, SHELL, SHELL, SHELL, SHELL, SHELL, DAEMON
        ])));
        // A folder named like the daemon's, above a binary that isn't Codex, is not it.
        assert!(!runs_in_daemon(&chain(&[r"C:\x\app-server-daemon\bin\node.exe"])));
    }

    #[cfg(windows)]
    #[test]
    fn ancestors_reads_this_process_chain() {
        // The test runner has a parent (cargo or a shell) whose image path can be read.
        let chain = ancestors();
        assert!(!chain.is_empty());
        assert!(chain[0].to_ascii_lowercase().ends_with(".exe"), "{chain:?}");
    }
}
