//! "Copy resume command" and "Open in system terminal": the two ways to continue a session outside Marshell.
//! Both are built from what the core holds (the tab, its history row, its profile), never from a request body.

use crate::profiles::detect::{posix_quote, powershell_quote};
use marshell_protocol::api::ShellFamily;
use marshell_protocol::brand;
use std::io;
use std::path::Path;

/// What a system terminal says off Windows.
pub const WINDOWS_ONLY: &str = "Open in system terminal works on Windows only for now.";

/// What starts the console: `argv`, the variables to add and to remove, and the folder.
pub type ConsoleOpener = fn(&[String], &[(String, String)], &[String], &Path) -> io::Result<()>;

/// One line that changes to `cwd` and then runs `line`, quoted for a shell of `family`, so pasting it runs nothing
/// until the person presses Enter. With no folder it is the line itself. `cwd` must hold no control character (the
/// caller checks); a Command Prompt path can't hold `"` (Windows refuses it in a name), but a `%` in one is
/// expanded by cmd, which this doesn't escape.
pub fn copy_line(family: ShellFamily, cwd: &str, line: &str) -> String {
    if cwd.is_empty() {
        return line.to_string();
    }
    match family {
        ShellFamily::PowerShell => format!("Set-Location -LiteralPath {}; {line}", powershell_quote(cwd)),
        ShellFamily::Cmd => format!("cd /d \"{cwd}\" && {line}"),
        ShellFamily::Posix | ShellFamily::Wsl => format!("cd {} && {line}", posix_quote(cwd)),
    }
}

/// The variables Marshell sets on its own sessions. A terminal opened outside has none of them.
fn marshell_markers() -> [String; 3] {
    ["TAB_ID", "SESSION_NUMBER", "AGENT"].map(brand::env_var)
}

fn is_marker(name: &str) -> bool {
    marshell_markers().iter().any(|marker| {
        if cfg!(windows) {
            name.eq_ignore_ascii_case(marker)
        } else {
            name == marker
        }
    })
}

/// The environment for the outside terminal: the profile's own variables without Marshell's session markers, and
/// the names the child must not inherit from the app (the markers again, in case the app was started from one of
/// its own sessions). Never `PATH` with Marshell's `bin` folder in it: that is a session's environment only.
pub fn outside_env(profile_env: &[(String, String)]) -> (Vec<(String, String)>, Vec<String>) {
    let kept = profile_env
        .iter()
        .filter(|(name, _)| !is_marker(name))
        .cloned()
        .collect();
    (kept, marshell_markers().to_vec())
}

/// Starts `argv` in a new console window in `cwd`, and doesn't wait for it. Windows hands the console to the
/// terminal app the person chose, so Marshell picks none.
#[cfg(windows)]
pub fn open_console(argv: &[String], env: &[(String, String)], env_remove: &[String], cwd: &Path) -> io::Result<()> {
    console::start(
        argv,
        env,
        env_remove,
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
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CREATE_BREAKAWAY_FROM_JOB, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
    };

    const ERROR_ACCESS_DENIED: i32 = 5;

    fn invalid(message: impl Into<String>) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, message.into())
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

    /// The file `program` names, never looked up in the current directory. A name with a path separator is used
    /// as given; any other is searched for in the absolute folders of `path` (a relative one, `.` included, is
    /// skipped). A name with no extension also tries each of `pathext`.
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
        let found = if program.contains(['/', '\\']) {
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
    /// from the app's job when the job allows it.
    pub(super) fn start(
        argv: &[String],
        env: &[(String, String)],
        env_remove: &[String],
        cwd: &Path,
        console: u32,
    ) -> io::Result<()> {
        let Some((program, args)) = argv.split_first() else {
            return Err(invalid("There is nothing to run."));
        };
        let file = resolve_program(
            program,
            std::env::var_os("PATH").as_deref(),
            std::env::var_os("PATHEXT").as_deref(),
        )?;
        let extension = file.extension().map(|ext| ext.to_string_lossy().to_ascii_lowercase());
        if matches!(extension.as_deref(), Some("bat" | "cmd")) {
            // cmd.exe reads a batch file's arguments by rules of its own, and they need the escaping std does for
            // them (and a refusal of what can't be escaped). That isn't ported yet.
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("Can't open `{program}` in a system terminal: batch files aren't supported."),
            ));
        }
        let application = wide(file.as_os_str());
        let line = command_line(program, args)?;
        let block = env_block(std::env::vars_os(), env, env_remove)?;
        let cwd = wide(cwd.as_os_str());
        let launch = |breakaway: bool| -> io::Result<()> {
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
            // SAFETY: both handles came from the successful call and aren't used again.
            unsafe {
                CloseHandle(info.hProcess);
                CloseHandle(info.hThread);
            }
            Ok(())
        };
        match launch(true) {
            // A job that doesn't allow breakaway refuses it. The console then stays in Marshell's job, which is
            // better than no console: it opens either way.
            Err(err) if err.raw_os_error() == Some(ERROR_ACCESS_DENIED) => launch(false),
            other => other,
        }
    }
}

/// Windows only for now.
#[cfg(not(windows))]
pub fn open_console(_: &[String], _: &[(String, String)], _: &[String], _: &Path) -> io::Result<()> {
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
        assert_eq!(ps, r"Set-Location -LiteralPath 'C:\My ''x'''; claude --resume abc");
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

    #[cfg(windows)]
    mod console_tests {
        use super::super::console::{command_line, env_block, flags, resolve_program, start};
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
            let err = start(&[], &[], &[], Path::new("."), CREATE_NO_WINDOW).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput);
            let argv = strings(&["zz-marshell-no-such-program"]);
            let err = start(&argv, &[], &[], Path::new("."), CREATE_NO_WINDOW).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
        }

        #[test]
        fn start_does_not_run_a_batch_file_it_cannot_escape_yet() {
            let dir = tempfile::tempdir().unwrap();
            let bat = dir.path().join("zz-marshell-probe.BAT");
            std::fs::write(&bat, b"@echo off\r\n").unwrap();
            let argv = vec![bat.to_str().unwrap().to_string()];
            let err = start(&argv, &[], &[], dir.path(), CREATE_NO_WINDOW).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::Unsupported);
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
            start(&argv, &[], &[], dir.path(), CREATE_NO_WINDOW).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            let seen = loop {
                if let Ok(content) = std::fs::read_to_string(&out)
                    && !content.trim().is_empty()
                {
                    break content.trim().to_string();
                }
                assert!(std::time::Instant::now() < deadline, "the child wrote nothing in 20 s");
                std::thread::sleep(std::time::Duration::from_millis(100));
            };
            assert_eq!(seen, "False");
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
