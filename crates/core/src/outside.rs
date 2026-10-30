//! "Copy resume command" and "Open in system terminal": the two ways to continue a session outside Marshell.
//! Both are built from what the core holds (the tab, its history row, its profile), never from a request body.

use crate::profiles::detect::{posix_quote, powershell_quote};
use marshell_protocol::api::ShellFamily;
use marshell_protocol::brand;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// What a system terminal says off Windows.
pub const WINDOWS_ONLY: &str = "Open in system terminal works on Windows only for now.";

/// Answers "is the console still running?". Dropping it lets go of whatever it watches.
pub type ConsoleProbe = Box<dyn Fn() -> bool + Send + Sync>;

/// What starts the console: `argv`, the whole environment it gets, the PATH its program is looked up on, and the
/// folder. It answers with a probe of the console it started.
pub type ConsoleOpener = fn(&[String], &[(String, String)], Option<&OsStr>, &Path) -> io::Result<ConsoleProbe>;

/// One line that changes to `cwd` and then runs `line`, quoted for a shell of `family`, so pasting it runs nothing
/// until the person presses Enter. With no folder it is the line itself. `cwd` must hold no control character (the
/// caller checks); a Command Prompt path can't hold `"` (Windows refuses it in a name), but a `%` in one is
/// expanded by cmd, which this doesn't escape.
pub fn copy_line(family: ShellFamily, cwd: &str, line: &str) -> String {
    if cwd.is_empty() {
        return line.to_string();
    }
    match family {
        // `-ErrorAction Stop` ends the line when the folder is gone, so the agent never runs in the wrong one.
        ShellFamily::PowerShell => format!(
            "Set-Location -LiteralPath {} -ErrorAction Stop; {line}",
            powershell_quote(cwd)
        ),
        ShellFamily::Cmd => format!("cd /d \"{cwd}\" && {line}"),
        ShellFamily::Posix | ShellFamily::Wsl => format!("cd {} && {line}", posix_quote(cwd)),
    }
}

/// The variables Marshell sets on its own sessions. A terminal opened outside has none of them.
fn marshell_markers() -> [String; 3] {
    ["TAB_ID", "SESSION_NUMBER", "AGENT"].map(brand::env_var)
}

/// Whether two environment names are the same: Windows ignores case.
fn same_name(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

fn is_marker(name: &str) -> bool {
    marshell_markers().iter().any(|marker| same_name(name, marker))
}

/// The environment for the outside terminal: the profile's own variables without Marshell's session markers, and
/// the names the child must not inherit from the app (the markers again, in case the app was started from one of
/// its own sessions, and the Claude Code session it may have been started from, as a session drops them). Never
/// `PATH` with Marshell's `bin` folder in it: that is a session's environment only.
pub fn outside_env(profile_env: &[(String, String)]) -> (Vec<(String, String)>, Vec<String>) {
    let kept = profile_env
        .iter()
        .filter(|(name, _)| !is_marker(name))
        .cloned()
        .collect();
    let removed = marshell_markers()
        .into_iter()
        .chain(crate::agents::INHERITED_MARKERS.iter().map(|name| name.to_string()))
        .collect();
    (kept, removed)
}

/// `path` without the entries that are the folder `bin`.
fn without_folder(path: &str, bin: &Path) -> String {
    let bin = bin.to_string_lossy();
    let normal = |text: &str| {
        let trimmed = text.trim_end_matches(['/', '\\']);
        if cfg!(windows) {
            trimmed.replace('/', "\\").to_lowercase()
        } else {
            trimmed.to_string()
        }
    };
    let bin = normal(&bin);
    let kept: Vec<_> = std::env::split_paths(path)
        .filter(|entry| normal(&entry.to_string_lossy()) != bin)
        .collect();
    std::env::join_paths(kept).map_or_else(|_| path.to_string(), |joined| joined.to_string_lossy().into_owned())
}

/// The whole environment of the console: `base` (what a session starts from, see `detect::session_base_env`)
/// without Marshell's session markers, the profile's own variables over it as a session has them, and no `bin`
/// folder of Marshell's in any `PATH`.
pub fn console_env(
    base: impl IntoIterator<Item = (String, String)>,
    profile_env: &[(String, String)],
    bin: &Path,
) -> Vec<(String, String)> {
    let (profile, removed) = outside_env(profile_env);
    let mut env: Vec<(String, String)> = base
        .into_iter()
        .filter(|(name, _)| !removed.iter().any(|marker| same_name(name, marker)))
        .collect();
    for (name, value) in profile {
        env.retain(|(have, _)| !same_name(have, &name));
        env.push((name, value));
    }
    for (name, value) in &mut env {
        if same_name(name, "PATH") {
            *value = without_folder(value, bin);
        }
    }
    env
}

/// The PATH the console's program is looked up on: the profile's own when it sets one, else `tab_path` (the PATH a
/// session's program is looked up on), without Marshell's `bin` folder.
pub fn lookup_path(
    profile_env: &[(String, String)],
    tab_path: Option<std::ffi::OsString>,
    bin: &Path,
) -> Option<std::ffi::OsString> {
    let path = profile_env
        .iter()
        .rev()
        .find(|(name, _)| same_name(name, "PATH"))
        .map(|(_, value)| std::ffi::OsString::from(value))
        .or(tab_path)?;
    Some(without_folder(&path.to_string_lossy(), bin).into())
}

/// Which conversation a console holds. A resume by id names it; "continue the last conversation" names only the agent
/// and the folder it looks in, so a console started that way can't be told from one started by id for the same
/// conversation (see `Consoles`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConversationKey {
    Id { agent: String, id: String },
    LastIn { agent: String, folder: String },
}

impl ConversationKey {
    pub fn by_id(agent: &str, id: &str) -> Self {
        ConversationKey::Id {
            agent: agent.to_string(),
            id: id.to_string(),
        }
    }

    /// The folder compares the way the file system does: Windows ignores case and the kind of slash, and a closing
    /// separator makes no difference.
    pub fn last_in(agent: &str, folder: &str) -> Self {
        let trimmed = folder.trim_end_matches(['/', '\\']);
        let folder = if cfg!(windows) {
            trimmed.replace('/', "\\").to_lowercase()
        } else {
            trimmed.to_string()
        };
        ConversationKey::LastIn {
            agent: agent.to_string(),
            folder,
        }
    }
}

enum Entry {
    /// A console is being started for it.
    Starting,
    Open(Arc<dyn Fn() -> bool + Send + Sync>),
}

/// The consoles Marshell opened, kept per conversation while they run, so a conversation is never resumed a second
/// time next to the console that holds it. In memory only: it is forgotten when Marshell quits. What it can't know:
/// a console it didn't open, and an agent ending while its window stays (the console is `pwsh -NoExit`, so the
/// probe follows the window, not the agent).
#[derive(Clone, Default)]
pub struct Consoles(Arc<Mutex<HashMap<ConversationKey, Entry>>>);

impl Consoles {
    /// Whether a console holds `key` or is starting for it. With `reserve` it also takes the key when none does.
    /// A dead console's entry is dropped on the way. A probe is called with no lock held.
    fn busy(&self, key: &ConversationKey, reserve: bool) -> bool {
        loop {
            let probe = {
                let mut map = self.0.lock().unwrap();
                match map.get(key) {
                    None => {
                        if reserve {
                            map.insert(key.clone(), Entry::Starting);
                        }
                        return false;
                    }
                    Some(Entry::Starting) => return true,
                    Some(Entry::Open(probe)) => probe.clone(),
                }
            };
            if probe() {
                return true;
            }
            let mut map = self.0.lock().unwrap();
            if matches!(map.get(key), Some(Entry::Open(have)) if Arc::ptr_eq(have, &probe)) {
                map.remove(key);
            }
        }
    }

    /// Whether a console still holds the conversation. A resume asks this.
    pub fn is_open(&self, key: &ConversationKey) -> bool {
        self.busy(key, false)
    }

    /// Takes `key` for a console about to be started: None while one holds it or is starting. Dropped without
    /// `finish`, it gives the key back.
    pub fn reserve(&self, key: ConversationKey) -> Option<Reservation> {
        if self.busy(&key, true) {
            return None;
        }
        Some(Reservation {
            consoles: self.clone(),
            key,
            done: false,
        })
    }
}

/// A key taken by `Consoles::reserve`.
pub struct Reservation {
    consoles: Consoles,
    key: ConversationKey,
    done: bool,
}

impl Reservation {
    /// The console started: keep its probe under the key.
    pub fn finish(mut self, probe: ConsoleProbe) {
        self.done = true;
        self.consoles
            .0
            .lock()
            .unwrap()
            .insert(self.key.clone(), Entry::Open(Arc::from(probe)));
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if !self.done {
            let mut map = self.consoles.0.lock().unwrap();
            if matches!(map.get(&self.key), Some(Entry::Starting)) {
                map.remove(&self.key);
            }
        }
    }
}

/// Starts `argv` in a new console window in `cwd`, and doesn't wait for it. Windows hands the console to the
/// terminal app the person chose, so Marshell picks none. `env` is the console's whole environment.
#[cfg(windows)]
pub fn open_console(
    argv: &[String],
    env: &[(String, String)],
    path: Option<&OsStr>,
    cwd: &Path,
) -> io::Result<ConsoleProbe> {
    console::start(
        argv,
        env,
        path,
        cwd,
        windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE,
    )
}

/// The console is started with `CreateProcessW` and not `std::process::Command`: std always passes the app's own
/// standard handles to the child. Under `pnpm tauri dev` those are pipes to log files, so a console started that
/// way writes to the log and sees redirected input and output (`claude --resume` then exits at once). Here the
/// child gets no handles, so its new console supplies them.
#[cfg(windows)]
mod console {
    use super::ConsoleProbe;
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{
        CREATE_BREAKAWAY_FROM_JOB, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
        WaitForSingleObject,
    };

    const ERROR_ACCESS_DENIED: i32 = 5;

    fn invalid(message: impl Into<String>) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, message.into())
    }

    /// A process handle that is closed when this is dropped.
    struct Process(HANDLE);

    // SAFETY: a process handle is valid on any thread, and `WaitForSingleObject` may be called on it from several.
    unsafe impl Send for Process {}
    unsafe impl Sync for Process {}

    impl Process {
        /// Whether the process is still running.
        fn alive(&self) -> bool {
            // SAFETY: the handle is open until `self` is dropped; a zero timeout only polls it.
            unsafe { WaitForSingleObject(self.0, 0) == WAIT_TIMEOUT }
        }
    }

    impl Drop for Process {
        fn drop(&mut self) {
            // SAFETY: the handle came from `CreateProcessW` and is closed only here.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn wide(text: &OsStr) -> Vec<u16> {
        text.encode_wide().chain(Some(0)).collect()
    }

    /// The creation flags of the console: `console` (its own window), a Unicode environment, and out of the app's
    /// job so closing Marshell doesn't close it.
    pub(super) fn flags(console: u32, breakaway: bool) -> u32 {
        let flags = console | CREATE_UNICODE_ENVIRONMENT;
        if breakaway {
            flags | CREATE_BREAKAWAY_FROM_JOB
        } else {
            flags
        }
    }

    /// The file `program` names, never looked up in the current directory. A name with a path separator or a `:` must be
    /// absolute and is used as given; any other is searched for in the absolute folders of `path` (a relative one,
    /// `.` included, is skipped). A name with no extension also tries each of `pathext`.
    pub(super) fn resolve_program(program: &str, path: Option<&OsStr>, pathext: Option<&OsStr>) -> io::Result<PathBuf> {
        let not_found = || io::Error::new(io::ErrorKind::NotFound, format!("Couldn't find `{program}` to run."));
        if program.is_empty() || program.contains('\0') {
            return Err(invalid("There is nothing to run."));
        }
        let has_extension = Path::new(program).extension().is_some();
        let pathext = pathext
            .map(|ext| ext.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_string());
        let candidates = |dir: &Path| -> Vec<PathBuf> {
            let file = dir.join(program);
            if has_extension {
                return vec![file];
            }
            pathext
                .split(';')
                .filter(|ext| ext.len() > 1 && ext.starts_with('.'))
                .map(|ext| {
                    let mut name = file.clone().into_os_string();
                    name.push(ext);
                    PathBuf::from(name)
                })
                .collect()
        };
        // A `:` makes a name a path too: `C:tool.exe` is a file in drive C's current folder, which `join` would pick.
        let found = if program.contains(['/', '\\', ':']) {
            if !Path::new(program).is_absolute() {
                return Err(invalid(format!(
                    "`{program}` is a relative path, and the console only runs a program by its full path."
                )));
            }
            candidates(Path::new("")).into_iter().find(|file| file.is_file())
        } else {
            let dirs: Vec<PathBuf> = path
                .map(|path| std::env::split_paths(path).collect())
                .unwrap_or_default();
            dirs.iter()
                .filter(|dir| dir.is_absolute())
                .flat_map(|dir| candidates(dir))
                .find(|file| file.is_file())
        };
        found.ok_or_else(not_found)
    }

    /// Whether Windows runs `file` as a batch file. It ignores trailing dots and spaces in a name (`x.bat.` and
    /// `x.bat ` both run as `x.bat`), so those are cut before the extension is read.
    pub(super) fn is_batch(file: &Path) -> bool {
        let name = file.file_name().map(|name| name.to_string_lossy().into_owned());
        let name = name.as_deref().unwrap_or_default().trim_end_matches(['.', ' ']);
        name.rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd"))
    }

    /// Adds `arg` to `line` the way Rust's `std::process::Command` does for an ordinary program (the rules of the
    /// MSVC runtime's argv parser): quoted when empty or holding a space or tab, a run of backslashes doubled in
    /// front of the closing quote, and each `"` escaped as `\"` with the backslashes before it doubled.
    fn push_arg(line: &mut Vec<u16>, arg: &str) -> io::Result<()> {
        if arg.contains('\0') {
            return Err(invalid("An argument can't contain a NUL character."));
        }
        let quote = arg.is_empty() || arg.contains([' ', '\t']);
        if quote {
            line.push(u16::from(b'"'));
        }
        let mut backslashes = 0usize;
        for unit in arg.encode_utf16() {
            if unit == u16::from(b'\\') {
                backslashes += 1;
            } else {
                if unit == u16::from(b'"') {
                    line.extend((0..=backslashes).map(|_| u16::from(b'\\')));
                }
                backslashes = 0;
            }
            line.push(unit);
        }
        if quote {
            line.extend((0..backslashes).map(|_| u16::from(b'\\')));
            line.push(u16::from(b'"'));
        }
        Ok(())
    }

    /// The NUL-terminated command line: `program` always in quotes (as std does; a program name can't hold a
    /// `"`), then `args`.
    pub(super) fn command_line(program: &str, args: &[String]) -> io::Result<Vec<u16>> {
        if program.contains(['"', '\0']) {
            return Err(invalid("The program name can't contain a quote or a NUL character."));
        }
        let mut line = vec![u16::from(b'"')];
        line.extend(program.encode_utf16());
        line.push(u16::from(b'"'));
        for arg in args {
            line.push(u16::from(b' '));
            push_arg(&mut line, arg)?;
        }
        line.push(0);
        Ok(line)
    }

    /// What a name compares as: its upper case, as Windows compares environment names.
    fn key(name: &OsStr) -> Vec<u16> {
        let mut key = Vec::new();
        for unit in char::decode_utf16(name.encode_wide()) {
            match unit {
                Ok(c) => {
                    for upper in c.to_uppercase() {
                        key.extend_from_slice(upper.encode_utf16(&mut [0; 2]));
                    }
                }
                Err(err) => key.push(err.unpaired_surrogate()),
            }
        }
        key
    }

    fn check_name(name: &str) -> io::Result<()> {
        // A name can start with `=` (cmd keeps its drive folders in `=C:`) and holds no other.
        if name.is_empty() || name.contains('\0') || name.chars().skip(1).any(|c| c == '=') {
            return Err(invalid(format!("`{name}` isn't a valid environment variable name.")));
        }
        Ok(())
    }

    /// The environment block: `base` without `env_remove`, with `env` set over it, sorted by name, as
    /// `NAME=VALUE\0...\0\0` in UTF-16. Names compare without regard to case.
    pub(super) fn env_block(
        base: impl IntoIterator<Item = (OsString, OsString)>,
        env: &[(String, String)],
        env_remove: &[String],
    ) -> io::Result<Vec<u16>> {
        let mut vars: BTreeMap<Vec<u16>, (OsString, OsString)> = base
            .into_iter()
            .map(|(name, value)| (key(&name), (name, value)))
            .collect();
        for name in env_remove {
            check_name(name)?;
            vars.remove(&key(OsStr::new(name)));
        }
        for (name, value) in env {
            check_name(name)?;
            if value.contains('\0') {
                return Err(invalid(format!("The value of `{name}` can't contain a NUL character.")));
            }
            vars.insert(key(OsStr::new(name)), (name.into(), value.into()));
        }
        let mut block = Vec::new();
        for (name, value) in vars.values() {
            block.extend(name.encode_wide());
            block.push(u16::from(b'='));
            block.extend(value.encode_wide());
            block.push(0);
        }
        // An empty block still has its two NULs.
        if block.is_empty() {
            block.push(0);
        }
        block.push(0);
        Ok(block)
    }

    /// Starts `argv` with `console` among its creation flags (a test passes `CREATE_NO_WINDOW`), breaking away
    /// from the app's job when the job allows it. `env` is the whole environment, and the program is looked up on
    /// `path`. The probe answers whether the process still runs, and holds its handle until it is dropped.
    pub(super) fn start(
        argv: &[String],
        env: &[(String, String)],
        path: Option<&OsStr>,
        cwd: &Path,
        console: u32,
    ) -> io::Result<ConsoleProbe> {
        let Some((program, args)) = argv.split_first() else {
            return Err(invalid("There is nothing to run."));
        };
        let pathext = env
            .iter()
            .rev()
            .find(|(name, _)| name.eq_ignore_ascii_case("PATHEXT"))
            .map(|(_, value)| OsString::from(value));
        let file = resolve_program(program, path, pathext.as_deref())?;
        if is_batch(&file) {
            // cmd.exe reads a batch file's arguments by rules of its own, and they need the escaping std does for
            // them (and a refusal of what can't be escaped). That isn't ported yet.
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("Can't open `{program}` in a system terminal: batch files aren't supported."),
            ));
        }
        let application = wide(file.as_os_str());
        let line = command_line(program, args)?;
        let block = env_block([], env, &[])?;
        let cwd = wide(cwd.as_os_str());
        let launch = |breakaway: bool| -> io::Result<Process> {
            // CreateProcessW may write to its command line, so each try gets its own copy.
            let mut line = line.clone();
            // SAFETY: both are plain C structs for which all zeros is valid.
            let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
            startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
            let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
            // SAFETY: the strings and the block are NUL-terminated and outlive the call; no handle is inherited
            // and no standard handle is passed, so the new console supplies them.
            let ok = unsafe {
                CreateProcessW(
                    application.as_ptr(),
                    line.as_mut_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    flags(console, breakaway),
                    block.as_ptr().cast(),
                    cwd.as_ptr(),
                    &startup,
                    &mut info,
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: the thread handle came from the successful call and isn't used again. The process handle
            // is kept, for the probe.
            unsafe {
                CloseHandle(info.hThread);
            }
            Ok(Process(info.hProcess))
        };
        let process = match launch(true) {
            // A job that doesn't allow breakaway refuses it. The console then stays in Marshell's job, which is
            // better than no console: it opens either way.
            Err(err) if err.raw_os_error() == Some(ERROR_ACCESS_DENIED) => launch(false)?,
            other => other?,
        };
        Ok(Box::new(move || process.alive()))
    }
}

/// Windows only for now.
#[cfg(not(windows))]
pub fn open_console(_: &[String], _: &[(String, String)], _: Option<&OsStr>, _: &Path) -> io::Result<ConsoleProbe> {
    Err(io::Error::new(io::ErrorKind::Unsupported, WINDOWS_ONLY))
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::ShellFamily;

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn copy_line_quotes_per_shell() {
        let line = "claude --resume abc";
        let ps = copy_line(ShellFamily::PowerShell, r"C:\My 'x'", line);
        assert_eq!(
            ps,
            r"Set-Location -LiteralPath 'C:\My ''x''' -ErrorAction Stop; claude --resume abc"
        );
        let cmd = copy_line(ShellFamily::Cmd, r"C:\My x", line);
        assert_eq!(cmd, r#"cd /d "C:\My x" && claude --resume abc"#);
        let posix = copy_line(ShellFamily::Posix, "/home/a b", line);
        assert_eq!(posix, "cd '/home/a b' && claude --resume abc");
        let quote = copy_line(ShellFamily::Posix, "/home/it's", line);
        assert_eq!(quote, r"cd '/home/it'\''s' && claude --resume abc");
        for text in [&ps, &cmd, &posix, &quote] {
            assert!(!text.contains(['\r', '\n']), "{text:?}");
        }
    }

    #[test]
    fn copy_line_with_no_folder_is_the_line() {
        assert_eq!(copy_line(ShellFamily::Posix, "", "codex resume x"), "codex resume x");
    }

    #[test]
    fn open_outside_env_drops_marshell_vars() {
        let profile = env(&[
            ("MARSHELL_TAB_ID", "t1"),
            ("marshell_session_number", "3"),
            ("MARSHELL_AGENT", "claude"),
            ("EDITOR", "vim"),
        ]);
        let (kept, removed) = outside_env(&profile);
        assert_eq!(kept, env(&[("EDITOR", "vim")]));
        for name in ["MARSHELL_TAB_ID", "MARSHELL_SESSION_NUMBER", "MARSHELL_AGENT"] {
            assert!(removed.iter().any(|r| r == name), "{name} not removed: {removed:?}");
        }
    }

    #[test]
    fn open_outside_env_never_adds_the_bin_folder_to_path() {
        let (kept, removed) = outside_env(&env(&[("PATH", "/usr/bin")]));
        assert_eq!(kept, env(&[("PATH", "/usr/bin")]));
        assert!(!removed.iter().any(|r| r.eq_ignore_ascii_case("PATH")));
    }

    fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        env(items)
    }

    fn value<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
        env.iter().find(|(n, _)| same_name(n, name)).map(|(_, v)| v.as_str())
    }

    #[test]
    fn console_env_keeps_what_only_the_app_has_and_drops_the_markers() {
        let base = pairs(&[
            ("CLAUDE_CONFIG_DIR", "C:\\scratch"),
            ("MARSHELL_TAB_ID", "t1"),
            ("marshell_agent", "claude"),
            ("MARSHELL_SESSION_NUMBER", "3"),
            ("CLAUDECODE", "1"),
            ("CLAUDE_CODE_SESSION_ID", "s1"),
            ("CLAUDE_CODE_GIT_BASH_PATH", "C:\\git\\bash.exe"),
            ("EDITOR", "nano"),
        ]);
        let profile = pairs(&[("EDITOR", "vim"), ("MARSHELL_AGENT", "codex")]);
        let env = console_env(base, &profile, Path::new("C:\\m\\bin"));
        assert_eq!(value(&env, "CLAUDE_CONFIG_DIR"), Some("C:\\scratch"));
        assert_eq!(value(&env, "EDITOR"), Some("vim"), "the profile's value wins");
        assert_eq!(env.iter().filter(|(n, _)| same_name(n, "EDITOR")).count(), 1);
        for marker in [
            "MARSHELL_TAB_ID",
            "MARSHELL_AGENT",
            "MARSHELL_SESSION_NUMBER",
            "CLAUDECODE",
            "CLAUDE_CODE_SESSION_ID",
        ] {
            assert_eq!(value(&env, marker), None, "{marker}");
        }
        // A user setting, not the identity of the Claude Code session the app was started from.
        assert_eq!(value(&env, "CLAUDE_CODE_GIT_BASH_PATH"), Some("C:\\git\\bash.exe"));
    }

    #[test]
    fn console_env_never_has_the_bin_folder_in_path() {
        let bin = Path::new("/m/bin");
        let path = |parts: &[&str]| std::env::join_paths(parts).unwrap().to_string_lossy().into_owned();
        // The app's own PATH (started from a Marshell session) holds it; the profile's PATH may too.
        let base = pairs(&[("PATH", &path(&["/m/bin", "/usr/bin"]))]);
        let env = console_env(base.clone(), &[], bin);
        assert_eq!(value(&env, "PATH"), Some(path(&["/usr/bin"]).as_str()));
        let profile = pairs(&[("PATH", &path(&["/a", "/m/bin/", "/b"]))]);
        let env = console_env(base, &profile, bin);
        assert_eq!(value(&env, "PATH"), Some(path(&["/a", "/b"]).as_str()));
        assert_eq!(env.iter().filter(|(n, _)| same_name(n, "PATH")).count(), 1);
    }

    #[test]
    fn lookup_path_is_the_profiles_else_the_tab_path_without_bin() {
        let bin = Path::new("/m/bin");
        let path = |parts: &[&str]| std::env::join_paths(parts).unwrap();
        let tab = Some(path(&["/m/bin", "/tab"]));
        assert_eq!(lookup_path(&[], tab.clone(), bin), Some(path(&["/tab"])));
        let own = path(&["/own"]).to_string_lossy().into_owned();
        let profile = pairs(&[("Path", own.as_str())]);
        let found = lookup_path(&profile, tab, bin);
        if cfg!(windows) {
            assert_eq!(found, Some(path(&["/own"])));
        } else {
            // Another case is another variable off Windows.
            assert_eq!(found, Some(path(&["/tab"])));
        }
        assert_eq!(lookup_path(&[], None, bin), None);
    }

    fn alive_until(flag: &Arc<std::sync::atomic::AtomicBool>) -> ConsoleProbe {
        let flag = flag.clone();
        Box::new(move || flag.load(std::sync::atomic::Ordering::SeqCst))
    }

    #[test]
    fn a_conversation_is_held_while_its_console_runs() {
        let consoles = Consoles::default();
        let key = ConversationKey::by_id("claude", "abc");
        assert!(!consoles.is_open(&key));
        let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));
        consoles.reserve(key.clone()).unwrap().finish(alive_until(&alive));
        assert!(consoles.is_open(&key));
        assert!(consoles.reserve(key.clone()).is_none(), "refused while it runs");
        assert!(!consoles.is_open(&ConversationKey::by_id("claude", "other")));
        assert!(!consoles.is_open(&ConversationKey::by_id("codex", "abc")));
        alive.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(!consoles.is_open(&key), "free once it exits");
        assert!(consoles.reserve(key).is_some());
    }

    #[test]
    fn a_console_that_is_starting_holds_the_conversation_and_a_failed_start_gives_it_back() {
        let consoles = Consoles::default();
        let key = ConversationKey::by_id("codex", "x");
        let first = consoles.reserve(key.clone()).unwrap();
        assert!(consoles.is_open(&key));
        assert!(consoles.reserve(key.clone()).is_none());
        drop(first);
        assert!(!consoles.is_open(&key));
        assert!(consoles.reserve(key).is_some());
    }

    #[test]
    fn continue_last_is_keyed_by_agent_and_folder() {
        let a = ConversationKey::last_in("claude", "C:\\Work\\App\\");
        if cfg!(windows) {
            assert_eq!(a, ConversationKey::last_in("claude", "c:/work/app"));
        }
        assert_ne!(a, ConversationKey::last_in("codex", "C:\\Work\\App\\"));
        assert_ne!(a, ConversationKey::last_in("claude", "C:\\Work\\Other"));
        assert_ne!(a, ConversationKey::by_id("claude", "C:\\Work\\App"));
    }

    #[cfg(windows)]
    mod console_tests {
        use super::super::console::{command_line, env_block, flags, is_batch, resolve_program, start};
        use std::ffi::{OsStr, OsString};
        use std::io::ErrorKind;
        use std::path::Path;
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::System::Threading::{CREATE_NEW_CONSOLE, CREATE_NO_WINDOW};
        use windows_sys::Win32::UI::Shell::CommandLineToArgvW;

        fn text(units: &[u16]) -> String {
            String::from_utf16(units).unwrap()
        }

        fn strings(items: &[&str]) -> Vec<String> {
            items.iter().map(|s| s.to_string()).collect()
        }

        /// What `CommandLineToArgvW` makes of `line`: the way the child sees its arguments.
        fn parse(line: &[u16]) -> Vec<String> {
            let mut count = 0;
            // SAFETY: `line` is NUL-terminated; the array is read for `count` items and freed once.
            unsafe {
                let argv = CommandLineToArgvW(line.as_ptr(), &mut count);
                assert!(!argv.is_null());
                let out = (0..count as usize)
                    .map(|i| {
                        let arg = *argv.add(i);
                        let len = (0..).take_while(|&n| *arg.add(n) != 0).count();
                        String::from_utf16(std::slice::from_raw_parts(arg, len)).unwrap()
                    })
                    .collect();
                LocalFree(argv.cast());
                out
            }
        }

        #[test]
        fn command_line_quotes_like_std() {
            let args = strings(&[
                "-NoExit",
                "-Command",
                "& 'x y'",
                "",
                "a\"b",
                "a b\"c",
                r"C:\x y\",
                r"C:\x\",
                r#"a\\\"b"#,
            ]);
            let line = command_line("pwsh.exe", &args).unwrap();
            assert_eq!(
                text(&line[..line.len() - 1]),
                r#""pwsh.exe" -NoExit -Command "& 'x y'" "" a\"b "a b\"c" "C:\x y\\" C:\x\ a\\\\\\\"b"#
            );
            assert_eq!(*line.last().unwrap(), 0);
        }

        #[test]
        fn command_line_round_trips_through_the_argv_parser() {
            let args = strings(&[
                "-NoExit",
                "-Command",
                "& 'C:\\My Folder\\claude.exe' --resume \"abc def\"",
                "",
                "tab\there",
                "say \"hi\"",
                "a\"b",
                "a b\"c",
                r"C:\x y\",
                r"C:\x\",
                r#"a\\\"b"#,
                r"a b\\",
                r"\\server\share name\",
                "ünï ♥ 😀",
            ]);
            let line = command_line(r"C:\Program Files\PowerShell\7\pwsh.exe", &args).unwrap();
            let mut expected = vec![r"C:\Program Files\PowerShell\7\pwsh.exe".to_string()];
            expected.extend(args);
            assert_eq!(parse(&line), expected);
        }

        #[test]
        fn command_line_refuses_nul_and_a_quoted_program() {
            assert_eq!(
                command_line("a", &strings(&["x\0y"])).unwrap_err().kind(),
                ErrorKind::InvalidInput
            );
            assert_eq!(command_line("a\"b", &[]).unwrap_err().kind(), ErrorKind::InvalidInput);
        }

        fn pair(name: &str, value: &str) -> (OsString, OsString) {
            (name.into(), value.into())
        }

        fn block_items(block: &[u16]) -> Vec<String> {
            assert_eq!(&block[block.len() - 2..], [0, 0], "ends with two NULs");
            block[..block.len() - 2]
                .split(|&unit| unit == 0)
                .map(|item| String::from_utf16(item).unwrap())
                .collect()
        }

        #[test]
        fn env_block_removes_and_sets_without_regard_to_case_and_sorts() {
            let base = vec![
                pair("Path", "C:\\bin"),
                pair("zeta", "1"),
                pair("MARSHELL_TAB_ID", "t"),
                pair("Alpha", "a"),
            ];
            let env = vec![
                ("PATH".to_string(), "C:\\other".to_string()),
                ("beta".to_string(), "b".to_string()),
            ];
            let block = env_block(base, &env, &strings(&["marshell_tab_id"])).unwrap();
            assert_eq!(block_items(&block), ["Alpha=a", "beta=b", "PATH=C:\\other", "zeta=1"]);
        }

        #[test]
        fn env_block_is_encoded_with_both_nuls_even_when_empty() {
            assert_eq!(env_block([], &[], &[]).unwrap(), [0, 0]);
            let block = env_block([pair("A", "b")], &[], &[]).unwrap();
            assert_eq!(block, "A=b\0\0".encode_utf16().collect::<Vec<_>>());
        }

        #[test]
        fn env_block_keeps_a_base_name_that_starts_with_equals() {
            let block = env_block([pair("=C:", "C:\\work")], &[], &[]).unwrap();
            assert_eq!(block_items(&block), ["=C:=C:\\work"]);
        }

        #[test]
        fn env_block_refuses_bad_names_and_values() {
            let bad = |env: &[(&str, &str)], remove: &[&str]| {
                let env: Vec<_> = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
                env_block([], &env, &strings(remove)).unwrap_err().kind()
            };
            assert_eq!(bad(&[("A=B", "x")], &[]), ErrorKind::InvalidInput);
            assert_eq!(bad(&[("A\0", "x")], &[]), ErrorKind::InvalidInput);
            assert_eq!(bad(&[("", "x")], &[]), ErrorKind::InvalidInput);
            assert_eq!(bad(&[("A", "x\0y")], &[]), ErrorKind::InvalidInput);
            assert_eq!(bad(&[], &["A=B"]), ErrorKind::InvalidInput);
        }

        #[test]
        fn resolve_program_never_looks_in_the_current_directory() {
            let here = tempfile::tempdir().unwrap();
            let elsewhere = tempfile::tempdir().unwrap();
            std::fs::write(here.path().join("zz-marshell-probe.exe"), b"").unwrap();
            // A relative entry (the current directory) and an empty one are skipped; the folder with the file
            // isn't on the path.
            let path = std::env::join_paths([Path::new("."), Path::new(""), elsewhere.path()]).unwrap();
            let err = resolve_program("zz-marshell-probe", Some(&path), Some(OsStr::new(".EXE"))).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
            let err = resolve_program("zz-marshell-probe.exe", Some(&path), None).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
            let err = resolve_program("zz-marshell-probe", None, None).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
        }

        #[test]
        fn resolve_program_searches_the_path_with_the_extensions() {
            let dir = tempfile::tempdir().unwrap();
            let other = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("zz-marshell-probe.exe"), b"").unwrap();
            std::fs::write(dir.path().join("zz-marshell-plain"), b"").unwrap();
            let path = std::env::join_paths([other.path(), dir.path()]).unwrap();
            let exts = OsStr::new(".COM;.EXE;.CMD");
            // The extension is the one PATHEXT spells, and the file system ignores its case.
            let found = resolve_program("zz-marshell-probe", Some(&path), Some(exts)).unwrap();
            assert_eq!(found, dir.path().join("zz-marshell-probe.EXE"));
            // With an extension the name is used as it is.
            let found = resolve_program("zz-marshell-probe.exe", Some(&path), Some(exts)).unwrap();
            assert_eq!(found, dir.path().join("zz-marshell-probe.exe"));
            // Without one, a file with none is not a program.
            let err = resolve_program("zz-marshell-plain", Some(&path), Some(exts)).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
            // The default extensions apply when PATHEXT is unset.
            assert!(resolve_program("zz-marshell-probe", Some(&path), None).is_ok());
        }

        #[test]
        fn resolve_program_uses_a_path_as_given() {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("zz-marshell-probe.exe");
            std::fs::write(&file, b"").unwrap();
            assert_eq!(resolve_program(file.to_str().unwrap(), None, None).unwrap(), file);
            let no_ext = dir.path().join("zz-marshell-probe");
            let found = resolve_program(no_ext.to_str().unwrap(), None, None).unwrap();
            assert_eq!(found, dir.path().join("zz-marshell-probe.EXE"));
            let missing = dir.path().join("zz-marshell-missing.exe");
            let err = resolve_program(missing.to_str().unwrap(), None, None).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
        }

        #[test]
        fn resolve_program_refuses_a_relative_path_with_a_separator() {
            let here = tempfile::tempdir().unwrap();
            std::fs::write(here.path().join("zz-marshell-probe.exe"), b"").unwrap();
            for name in [
                ".\\zz-marshell-probe.exe",
                "sub\\zz-marshell-probe.exe",
                "./zz-marshell-probe.exe",
                "..\\zz-marshell-probe",
                "\\zz-marshell-probe.exe",
                "C:zz-marshell-probe.exe",
            ] {
                let err = resolve_program(name, None, None).unwrap_err();
                assert_eq!(err.kind(), ErrorKind::InvalidInput, "{name}");
            }
        }

        /// The probe follows the process: alive while it runs, and false once it has exited.
        #[test]
        fn the_probe_answers_whether_the_console_still_runs() {
            let dir = tempfile::tempdir().unwrap();
            let argv = strings(&[
                "powershell.exe",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Milliseconds 1500",
            ]);
            let path = std::env::var_os("PATH");
            let env: Vec<_> = std::env::vars().collect();
            let probe = start(&argv, &env, path.as_deref(), dir.path(), CREATE_NO_WINDOW).unwrap();
            assert!(probe(), "alive just after it started");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while probe() {
                assert!(std::time::Instant::now() < deadline, "the process never ended");
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            assert!(!probe(), "and stays ended");
        }

        #[test]
        fn flags_open_a_new_console_and_break_away() {
            const NEW_CONSOLE: u32 = 0x10;
            const BREAKAWAY: u32 = 0x0100_0000;
            const UNICODE_ENVIRONMENT: u32 = 0x400;
            assert_eq!(
                flags(CREATE_NEW_CONSOLE, true),
                NEW_CONSOLE | BREAKAWAY | UNICODE_ENVIRONMENT
            );
            assert_eq!(flags(CREATE_NEW_CONSOLE, false), NEW_CONSOLE | UNICODE_ENVIRONMENT);
        }

        #[test]
        fn start_refuses_nothing_to_run_and_an_unknown_program() {
            let err = start(&[], &[], None, Path::new("."), CREATE_NO_WINDOW)
                .err()
                .expect("refused");
            assert_eq!(err.kind(), ErrorKind::InvalidInput);
            let argv = strings(&["zz-marshell-no-such-program"]);
            let path = std::env::var_os("PATH");
            let err = start(&argv, &[], path.as_deref(), Path::new("."), CREATE_NO_WINDOW)
                .err()
                .expect("refused");
            assert_eq!(err.kind(), ErrorKind::NotFound);
        }

        #[test]
        fn start_finds_the_program_on_the_path_it_is_given() {
            let dir = tempfile::tempdir().unwrap();
            let bat = dir.path().join("zz-marshell-probe.cmd");
            std::fs::write(&bat, b"@echo off\r\n").unwrap();
            // Found on the given PATH (and, being a batch file, refused for that reason, not as missing).
            let path = std::env::join_paths([dir.path()]).unwrap();
            let argv = strings(&["zz-marshell-probe"]);
            let pathext = [("PATHEXT".to_string(), ".EXE;.CMD".to_string())];
            let err = start(&argv, &pathext, Some(&path), dir.path(), CREATE_NO_WINDOW)
                .err()
                .expect("refused");
            assert_eq!(err.kind(), ErrorKind::Unsupported);
            // The app's own PATH is not consulted.
            let err = start(&argv, &pathext, None, dir.path(), CREATE_NO_WINDOW)
                .err()
                .expect("refused");
            assert_eq!(err.kind(), ErrorKind::NotFound);
        }

        #[test]
        fn start_does_not_run_a_batch_file_it_cannot_escape_yet() {
            let dir = tempfile::tempdir().unwrap();
            let bat = dir.path().join("zz-marshell-probe.BAT");
            std::fs::write(&bat, b"@echo off\r\n").unwrap();
            let argv = vec![bat.to_str().unwrap().to_string()];
            let err = start(&argv, &[], None, dir.path(), CREATE_NO_WINDOW)
                .err()
                .expect("refused");
            assert_eq!(err.kind(), ErrorKind::Unsupported);
            // Windows runs `x.BAT.` and `x.BAT ` as the same batch file.
            for tail in [".", " ", ". .", " ."] {
                let argv = vec![format!("{}{tail}", bat.to_str().unwrap())];
                let err = start(&argv, &[], None, dir.path(), CREATE_NO_WINDOW)
                    .err()
                    .expect("refused");
                assert_ne!(err.kind(), ErrorKind::NotFound, "{argv:?}");
                assert_eq!(err.kind(), ErrorKind::Unsupported, "{argv:?}");
            }
        }

        #[test]
        fn is_batch_ignores_trailing_dots_and_spaces() {
            for name in ["a.bat", "a.CMD", "C:\\x\\a.bat.", "a.bat ", "a.cmd. .", "a.b.bat"] {
                assert!(is_batch(Path::new(name)), "{name}");
            }
            for name in ["a.exe", "a.bat.exe", "a", "bat", "a.batx", "a.exe.", ""] {
                assert!(!is_batch(Path::new(name)), "{name}");
            }
        }

        /// The bug: a console started with std's `Command` got the app's own (piped) standard handles, so the
        /// shell in it saw redirected output. Started with `CreateProcessW` and no handles it sees a console.
        /// `CREATE_NO_WINDOW` stands in for `CREATE_NEW_CONSOLE`, so no window opens.
        #[test]
        fn start_gives_the_child_a_console_not_the_apps_handles() {
            let dir = tempfile::tempdir().unwrap();
            let out = dir.path().join("redirected.txt");
            let script = format!(
                "[Console]::IsOutputRedirected | Out-File -Encoding ascii '{}'",
                out.display()
            );
            let argv = strings(&["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", &script]);
            let path = std::env::var_os("PATH");
            let env: Vec<_> = std::env::vars().collect();
            let probe = start(&argv, &env, path.as_deref(), dir.path(), CREATE_NO_WINDOW).unwrap();
            // Until the child exits, not a fixed time: a cold PowerShell on a loaded CI runner took over 20 s.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            while probe() {
                assert!(std::time::Instant::now() < deadline, "the child ran for over 120 s");
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            let seen = std::fs::read_to_string(&out).unwrap_or_default();
            assert_eq!(seen.trim(), "False", "the child exited having written {seen:?}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn open_outside_refuses_off_windows() {
        let err = open_console(&["sh".into()], &[], &[], Path::new(".")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
        assert_eq!(
            err.to_string(),
            "Open in system terminal works on Windows only for now."
        );
    }
}
