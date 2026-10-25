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
    descendants_in(&processes()?, root)
}

/// One read of the process list, to ask for several trees without listing the processes again.
pub(super) struct Processes(Vec<(u32, u32, String)>);

impl Processes {
    pub(super) fn read() -> Option<Processes> {
        processes().map(Processes)
    }

    /// `descendants` of `root` as `(pid, exe file name)`.
    pub(super) fn tree(&self, root: u32) -> Option<Vec<(u32, String)>> {
        let pids = descendants_in(&self.0, root)?;
        Some(
            pids.into_iter()
                .map(|pid| {
                    let name = self
                        .0
                        .iter()
                        .find(|(p, _, _)| *p == pid)
                        .map_or_else(String::new, |(_, _, n)| n.clone());
                    (pid, name)
                })
                .collect(),
        )
    }

    /// The first program of `foreground_chain_pids`: what `root` itself runs.
    pub(super) fn outermost(&self, root: u32) -> Option<u32> {
        chain_in(&self.0, root)?.first().map(|(pid, _)| *pid)
    }
}

fn descendants_in(procs: &[(u32, u32, String)], root: u32) -> Option<Vec<u32>> {
    let mut tree: HashMap<u32, u64> = HashMap::from([(root, creation_time(root).ok()??)]);
    loop {
        let mut grew = false;
        for &(pid, parent, _) in procs {
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

/// The programs running under `root` as `(pid, name)`, outermost first, root excluded: at each step the newest child
/// (by the same rule as `descendants`) that is not a console host. Empty when `root` has no child.
/// `None` if the snapshot cannot be taken or `root` is gone.
pub(super) fn foreground_chain_pids(root: u32) -> Option<Vec<(u32, String)>> {
    chain_in(&processes()?, root)
}

fn chain_in(procs: &[(u32, u32, String)], root: u32) -> Option<Vec<(u32, String)>> {
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
        chain.push((pid, program_name(exe)));
        (current, born) = (pid, t);
    }
    Some(chain)
}

/// The command line of process `pid`, or `None` if it is gone, cannot be opened (another user's, or elevated) or the
/// query fails. `NtQueryInformationProcess` with `ProcessCommandLineInformation`, which needs only
/// `PROCESS_QUERY_LIMITED_INFORMATION`. Blocking: the watcher calls it inside its `spawn_blocking`.
pub(super) fn command_line(pid: u32) -> Option<String> {
    use windows_sys::Wdk::System::Threading::{NtQueryInformationProcess, ProcessCommandLineInformation};
    use windows_sys::Win32::Foundation::UNICODE_STRING;

    // SAFETY: the result is checked; the handle is closed by `Owned` on every path.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let handle = Owned(handle);
    // A first call with no room reports the length needed. Plain words, so the buffer is aligned for the
    // `UNICODE_STRING` at its start (it holds a pointer).
    let mut needed: u32 = 0;
    // SAFETY: a null buffer of length 0 is the documented way to ask for the length; `needed` is a valid u32.
    unsafe {
        NtQueryInformationProcess(
            handle.0,
            ProcessCommandLineInformation,
            std::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    let header = size_of::<UNICODE_STRING>();
    let bytes = (needed as usize).max(header);
    // The command line is at most about 32k wide chars; a larger answer is not a command line.
    if bytes > 1 << 20 {
        return None;
    }
    let mut buffer = vec![0usize; bytes.div_ceil(size_of::<usize>())];
    let capacity = buffer.len() * size_of::<usize>();
    let mut returned: u32 = 0;
    // SAFETY: `buffer` is writable for `capacity` bytes (at most 1 MiB, so it fits u32); `returned` is a valid u32.
    let status = unsafe {
        NtQueryInformationProcess(
            handle.0,
            ProcessCommandLineInformation,
            buffer.as_mut_ptr().cast(),
            capacity as u32,
            &mut returned,
        )
    };
    // A negative NTSTATUS is a failure, including a length mismatch if the process changed between the calls.
    if status < 0 {
        return None;
    }
    let base = buffer.as_ptr() as usize;
    // SAFETY: the buffer is aligned for a `UNICODE_STRING` and holds at least its size; the kernel filled it.
    let unicode = unsafe { std::ptr::read(buffer.as_ptr().cast::<UNICODE_STRING>()) };
    let length = usize::from(unicode.Length);
    let start = unicode.Buffer as usize;
    let end = start.checked_add(length)?;
    // The text lies inside this allocation, after the header, and is whole wide chars.
    if length == 0
        || length % 2 != 0
        || !start.is_multiple_of(2)
        || start < base + header
        || end > base + capacity
        || length > usize::from(unicode.MaximumLength)
    {
        return None;
    }
    // SAFETY: `start..end` was checked to lie inside `buffer`, is 2-aligned and a whole number of u16s.
    let wide = unsafe { std::slice::from_raw_parts(start as *const u16, length / 2) };
    Some(String::from_utf16_lossy(wide))
}

/// The words of a Windows command line, by the `CommandLineToArgvW` rules: whitespace separates words outside quotes;
/// backslashes are literal unless a quote follows them; `2n` backslashes and a quote give `n` backslashes and toggle
/// quoting, `2n+1` give `n` backslashes and a literal quote; inside quotes a doubled quote is a literal quote.
pub(super) fn split_args(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut words = Vec::new();
    let mut i = 0;
    loop {
        while i < chars.len() && matches!(chars[i], ' ' | '\t') {
            i += 1;
        }
        if i >= chars.len() {
            return words;
        }
        let mut word = String::new();
        let mut quoted = false;
        while i < chars.len() {
            match chars[i] {
                ' ' | '\t' if !quoted => break,
                '\\' => {
                    let mut slashes = 0;
                    while i < chars.len() && chars[i] == '\\' {
                        slashes += 1;
                        i += 1;
                    }
                    if chars.get(i) == Some(&'"') {
                        word.extend(std::iter::repeat_n('\\', slashes / 2));
                        if slashes % 2 == 1 {
                            word.push('"');
                            i += 1;
                        }
                        // An even run leaves the quote to toggle on the next turn.
                    } else {
                        word.extend(std::iter::repeat_n('\\', slashes));
                    }
                }
                '"' => {
                    if quoted && chars.get(i + 1) == Some(&'"') {
                        word.push('"');
                        i += 2;
                    } else {
                        quoted = !quoted;
                        i += 1;
                    }
                }
                c => {
                    word.push(c);
                    i += 1;
                }
            }
        }
        words.push(word);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn command_line_of_this_process_names_the_test_binary() {
        let line = command_line(std::process::id()).expect("the command line of this process");
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_stem().unwrap().to_string_lossy().to_lowercase();
        assert!(line.to_lowercase().contains(&name), "{line}");
        // And it splits into words that start with that binary.
        let argv = split_args(&line);
        assert!(
            argv[0].to_lowercase().contains(&name),
            "{argv:?} does not start with {name}"
        );
        assert_eq!(command_line(u32::MAX - 1), None);
    }

    #[test]
    fn split_args_follows_the_windows_quoting_rules() {
        assert_eq!(split_args(""), Vec::<String>::new());
        assert_eq!(split_args("  a   b "), words(&["a", "b"]));
        // A quoted path with a space is one word.
        assert_eq!(
            split_args(r#""C:\Program Files\nodejs\node.exe" C:\x\main.js login"#),
            words(&[r"C:\Program Files\nodejs\node.exe", r"C:\x\main.js", "login"])
        );
        // Backslashes are literal unless they come before a quote.
        assert_eq!(split_args(r"a\b\\c d"), words(&[r"a\b\\c", "d"]));
        // 2n backslashes then a quote: n backslashes, and the quote toggles.
        assert_eq!(split_args(r#""a\\" b"#), words(&[r"a\", "b"]));
        // 2n+1 backslashes then a quote: n backslashes and a literal quote.
        assert_eq!(split_args(r#"a\"b c"#), words(&[r#"a"b"#, "c"]));
        assert_eq!(split_args(r#""say \"hi\" now" x"#), words(&[r#"say "hi" now"#, "x"]));
        assert_eq!(split_args(r#"a\\\"b"#), words(&[r#"a\"b"#]));
        // A doubled quote inside quotes is a literal quote.
        assert_eq!(split_args(r#""a ""b"" c" d"#), words(&[r#"a "b" c"#, "d"]));
        // An empty quoted word stays a word; a missing closing quote runs to the end.
        assert_eq!(split_args(r#"a "" b"#), words(&["a", "", "b"]));
        assert_eq!(split_args(r#"a "b c"#), words(&["a", "b c"]));
    }
}
