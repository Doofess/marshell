//! "Copy resume command" and "Open in system terminal": the two ways to continue a session outside Marshell.
//! Both are built from what the core holds (the tab, its history row, its profile), never from a request body.

use crate::profiles::detect::{posix_quote, powershell_quote};
use marshell_protocol::api::ShellFamily;
use marshell_protocol::brand;
use std::io;
use std::path::Path;
use std::process::Command;

/// What a system terminal says off Windows.
pub const WINDOWS_ONLY: &str = "Open in system terminal works on Windows only for now.";

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

/// The process to start: `argv` run in `cwd` with `env` added and `env_remove` taken away.
pub fn console_command(
    argv: &[String],
    env: &[(String, String)],
    env_remove: &[String],
    cwd: &Path,
) -> io::Result<Command> {
    let Some((program, args)) = argv.split_first() else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "There is nothing to run."));
    };
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd);
    for name in env_remove {
        command.env_remove(name);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    Ok(command)
}

/// The creation flags of the console: its own window, and out of the app's job so closing Marshell doesn't close it.
#[cfg(windows)]
fn console_flags(breakaway: bool) -> u32 {
    use windows_sys::Win32::System::Threading::{CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_CONSOLE};
    if breakaway {
        CREATE_NEW_CONSOLE | CREATE_BREAKAWAY_FROM_JOB
    } else {
        CREATE_NEW_CONSOLE
    }
}

/// Starts `argv` in a new console window in `cwd`, and doesn't wait for it. Windows hands the console to the
/// terminal app the person chose, so Marshell picks none.
#[cfg(windows)]
pub fn open_console(argv: &[String], env: &[(String, String)], env_remove: &[String], cwd: &Path) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    const ERROR_ACCESS_DENIED: i32 = 5;
    let start = |breakaway: bool| -> io::Result<()> {
        let mut command = console_command(argv, env, env_remove, cwd)?;
        command.creation_flags(console_flags(breakaway)).spawn().map(drop)
    };
    match start(true) {
        // A job that doesn't allow breakaway refuses it. The console then stays in Marshell's job, which is
        // better than no console: it opens either way.
        Err(err) if err.raw_os_error() == Some(ERROR_ACCESS_DENIED) => start(false),
        other => other,
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
    use std::path::Path;

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

    #[test]
    fn console_command_carries_argv_env_and_folder() {
        let argv = vec![
            "pwsh.exe".to_string(),
            "-NoExit".into(),
            "-Command".into(),
            "& claude".into(),
        ];
        let (kept, removed) = outside_env(&env(&[("EDITOR", "vim"), ("MARSHELL_TAB_ID", "t")]));
        let command = console_command(&argv, &kept, &removed, Path::new("work")).unwrap();
        assert_eq!(command.get_program(), "pwsh.exe");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, ["-NoExit", "-Command", "& claude"]);
        assert_eq!(command.get_current_dir(), Some(Path::new("work")));
        let envs: Vec<_> = command.get_envs().collect();
        assert!(envs.contains(&(std::ffi::OsStr::new("EDITOR"), Some(std::ffi::OsStr::new("vim")))));
        // A removed variable is `None`: the child doesn't inherit it.
        assert!(envs.contains(&(std::ffi::OsStr::new("MARSHELL_TAB_ID"), None)));
    }

    #[test]
    fn console_command_refuses_an_empty_argv() {
        assert!(console_command(&[], &[], &[], Path::new(".")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn console_flags_open_a_new_console_and_break_away() {
        const NEW_CONSOLE: u32 = 0x10;
        const BREAKAWAY: u32 = 0x0100_0000;
        assert_eq!(console_flags(true), NEW_CONSOLE | BREAKAWAY);
        assert_eq!(console_flags(false), NEW_CONSOLE);
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
