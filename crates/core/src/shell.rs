//! Shell integration: Marshell's own init scripts, which mark prompts and commands with OSC 133.
//! The scripts live in `~/.marshell/shell` and are loaded through the shell's command line or
//! environment. No user profile or rc file is ever written: each script loads the user's own
//! startup files itself, or runs after the shell has loaded them.

use crate::profiles::detect::{posix_quote, powershell_quote};
use marshell_protocol::api::{IntegrationInfo, Profile};
use std::path::{Path, PathBuf};

/// Every file in the script folder, as (path under the folder, text). Built into the binary.
const FILES: [(&str, &str); 8] = [
    ("marshell.ps1", include_str!("../shell/marshell.ps1")),
    ("marshell.bash", include_str!("../shell/marshell.bash")),
    ("marshell.zsh", include_str!("../shell/marshell.zsh")),
    ("marshell.fish", include_str!("../shell/marshell.fish")),
    ("zsh/.zshenv", include_str!("../shell/zsh/.zshenv")),
    ("zsh/.zprofile", include_str!("../shell/zsh/.zprofile")),
    ("zsh/.zshrc", include_str!("../shell/zsh/.zshrc")),
    ("zsh/.zlogin", include_str!("../shell/zsh/.zlogin")),
];

const OFF: &str = "Shell integration is off for this profile.";
const OWN_ARGS: &str = "This command has its own arguments, so Marshell runs it as written.";
const NOT_WRITTEN: &str = "The integration scripts could not be written.";
const WSL_BASH: &str =
    "This bash may be WSL's launcher, which can't read Marshell's script. Use a WSL profile, or Git Bash's full path.";

/// Writes the scripts to `<home>/shell`, touching only files whose text differs. Returns the folder.
pub fn install(home: &Path) -> anyhow::Result<PathBuf> {
    let dir = crate::paths::shell_dir(home);
    for (name, text) in FILES {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(text.as_bytes()) {
            crate::store::write_bytes_atomic(&path, text.as_bytes())?;
        }
    }
    Ok(dir)
}

/// What to add to a plain shell session's launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The full argv to run instead of the profile's command.
    pub argv: Vec<String>,
    /// Set after the profile's own env, so these win.
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    PowerShell,
    Cmd,
    Bash,
    Zsh,
    Fish,
    Wsl,
}

/// The shell a profile runs and whether Marshell can integrate it. Err is the reason it can't.
fn kind(profile: &Profile) -> Result<Kind, String> {
    if !profile.shell_integration {
        return Err(OFF.into());
    }
    let Some(exe) = profile.command.first() else {
        return Err("This profile has no command.".into());
    };
    let base = exe.rsplit(['\\', '/']).next().unwrap_or(exe).to_lowercase();
    let name = base.strip_suffix(".exe").unwrap_or(&base);
    let kind = match name {
        "pwsh" | "powershell" => Kind::PowerShell,
        "cmd" => Kind::Cmd,
        "bash" => Kind::Bash,
        "zsh" => Kind::Zsh,
        "fish" => Kind::Fish,
        "wsl" => Kind::Wsl,
        _ => return Err(format!("Marshell has no integration script for {name}.")),
    };
    let args = &profile.command[1..];
    let known = match kind {
        Kind::PowerShell => args
            .iter()
            .all(|a| a.eq_ignore_ascii_case("-NoLogo") || a.eq_ignore_ascii_case("-NoProfile")),
        Kind::Bash | Kind::Zsh | Kind::Fish => args.iter().all(|a| matches!(a.as_str(), "-l" | "--login" | "-i")),
        Kind::Cmd => args.is_empty(),
        Kind::Wsl => args.is_empty() || (args.len() == 2 && args[0].eq_ignore_ascii_case("-d")),
    };
    if !known {
        return Err(OWN_ARGS.into());
    }
    if kind == Kind::Bash && cfg!(windows) && may_be_wsl_launcher(exe) {
        return Err(WSL_BASH.into());
    }
    Ok(kind)
}

/// On Windows, whether `exe` may be WSL's `bash.exe` launcher, which can't read a Windows script path.
/// A bare `bash` usually is: System32 comes early on PATH. Git Bash is detected by its full path.
fn may_be_wsl_launcher(exe: &str) -> bool {
    let path = exe.replace('/', "\\").to_lowercase();
    let full = path.as_bytes().get(1..3) == Some(b":\\".as_slice()) || path.starts_with("\\\\");
    !full
        || ["\\windows\\system32\\", "\\windows\\sysnative\\", "\\windowsapps\\"]
            .iter()
            .any(|d| path.contains(d))
}

/// The value of `name` in the profile's env, else in Marshell's own environment.
fn env_or_outer(
    profile: &Profile,
    name: &str,
    ignore_case: bool,
    outer_env: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    profile
        .env
        .iter()
        .rev()
        .find(|(k, _)| {
            if ignore_case {
                k.eq_ignore_ascii_case(name)
            } else {
                k == name
            }
        })
        .map(|(_, v)| v.clone())
        .or_else(|| outer_env(name))
}

/// `dir` with forward slashes, for POSIX shells (Git Bash reads `C:/...` paths).
fn slashed(dir: &Path) -> String {
    dir.to_string_lossy().replace('\\', "/")
}

/// The cmd `PROMPT`: marks before the user's own prompt, unless it already has them.
fn cmd_prompt(profile: &Profile, outer_env: &dyn Fn(&str) -> Option<String>) -> String {
    let inner = env_or_outer(profile, "PROMPT", true, outer_env).unwrap_or_else(|| "$P$G".into());
    if inner.contains("]133;") {
        return inner;
    }
    format!(r"$e]133;D$e\$e]133;A$e\$e]9;9;$P$e\{inner}")
}

/// A `/bin/sh` script, run inside the distro, that starts the user's shell with the matching script.
fn wsl_dispatcher(dir: &Path) -> String {
    let win_dir = posix_quote(&dir.to_string_lossy());
    [
        r#"s=${SHELL:-$(getent passwd "$(id -un)" | cut -d: -f7)}; s=${s:-/bin/sh}"#.to_string(),
        format!("d=$(wslpath -u {win_dir} 2>/dev/null) || d="),
        r#"[ -n "$d" ] || exec "$s" -l"#.to_string(),
        r#"case "${s##*/}" in"#.to_string(),
        r#"bash) MARSHELL_BASH_LOGIN=1 exec "$s" --init-file "$d/marshell.bash" -i ;;"#.to_string(),
        r#"zsh) MARSHELL_USER_ZDOTDIR="${ZDOTDIR:-}" ZDOTDIR="$d/zsh" exec "$s" -l ;;"#.to_string(),
        r#"fish) MARSHELL_FISH_SCRIPT="$d/marshell.fish" exec "$s" -l -C 'source $MARSHELL_FISH_SCRIPT' ;;"#
            .to_string(),
        r#"*) exec "$s" -l ;;"#.to_string(),
        "esac".to_string(),
    ]
    .join("\n")
}

fn build(kind: Kind, profile: &Profile, dir: &Path, outer_env: &dyn Fn(&str) -> Option<String>) -> Applied {
    let command = &profile.command;
    let mut argv = command.clone();
    let mut env = Vec::new();
    match kind {
        Kind::PowerShell => {
            // Dot-sourcing a scriptblock made from the file's text works under a Restricted
            // ExecutionPolicy, which only blocks script files.
            let path = powershell_quote(&dir.join("marshell.ps1").to_string_lossy());
            argv.extend([
                "-NoExit".into(),
                "-Command".into(),
                format!(". ([scriptblock]::Create([IO.File]::ReadAllText({path})))"),
            ]);
        }
        Kind::Cmd => env.push(("PROMPT".into(), cmd_prompt(profile, outer_env))),
        Kind::Bash => {
            if command[1..].iter().any(|a| a == "-l" || a == "--login") {
                env.push(("MARSHELL_BASH_LOGIN".into(), "1".into()));
            }
            argv = vec![
                command[0].clone(),
                "--init-file".into(),
                format!("{}/marshell.bash", slashed(dir)),
                "-i".into(),
            ];
        }
        Kind::Zsh => {
            let user = env_or_outer(profile, "ZDOTDIR", false, outer_env).unwrap_or_default();
            env.push(("ZDOTDIR".into(), format!("{}/zsh", slashed(dir))));
            env.push(("MARSHELL_USER_ZDOTDIR".into(), user));
        }
        Kind::Fish => {
            let path = format!("{}/marshell.fish", slashed(dir));
            argv.extend(["-C".into(), format!("source {}", posix_quote(&path))]);
        }
        Kind::Wsl => argv.extend(["--exec".into(), "/bin/sh".into(), "-c".into(), wsl_dispatcher(dir)]),
    }
    Applied { argv, env }
}

/// The argv and env that start a plain shell session from `profile` with integration.
/// Err is the reason shown on the profile page; the session then runs the profile as written.
pub fn integrate(profile: &Profile, dir: &Path, outer_env: &dyn Fn(&str) -> Option<String>) -> Result<Applied, String> {
    Ok(build(kind(profile)?, profile, dir, outer_env))
}

/// What integration does for `profile`, for the profile page. `dir` is None when install failed.
pub fn describe(profile: &Profile, dir: Option<&Path>, outer_env: &dyn Fn(&str) -> Option<String>) -> IntegrationInfo {
    let not_applied = |reason: &str| IntegrationInfo {
        applies: false,
        reason: Some(reason.into()),
        script_path: None,
        script: None,
    };
    let kind = match kind(profile) {
        Ok(kind) => kind,
        Err(reason) => return not_applied(&reason),
    };
    let Some(dir) = dir else {
        return not_applied(NOT_WRITTEN);
    };
    let file = |name: &str| {
        let text = FILES.iter().find(|(n, _)| *n == name).map(|(_, t)| t.to_string());
        (Some(dir.join(name).to_string_lossy().into_owned()), text)
    };
    let (script_path, script) = match kind {
        Kind::PowerShell => file("marshell.ps1"),
        Kind::Bash => file("marshell.bash"),
        Kind::Zsh => file("marshell.zsh"),
        Kind::Fish => file("marshell.fish"),
        Kind::Cmd => (None, Some(cmd_prompt(profile, outer_env))),
        Kind::Wsl => (None, Some(wsl_dispatcher(dir))),
    };
    IntegrationInfo {
        applies: true,
        reason: None,
        script_path,
        script,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::{Profile, ShellFamily};
    use std::path::Path;

    fn profile(command: &[&str], family: ShellFamily) -> Profile {
        Profile {
            id: "p".into(),
            name: "P".into(),
            command: command.iter().map(|s| s.to_string()).collect(),
            family,
            cwd: None,
            env: Vec::new(),
            shell_integration: true,
            terminal_theme: None,
            detected: true,
        }
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn env_value<'a>(applied: &'a Applied, name: &str) -> Option<&'a str> {
        applied.env.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    const DIR: &str = r"C:\Users\O'Brien Home\.marshell\shell";

    #[test]
    fn windows_bash_that_may_be_wsl_launcher_is_refused() {
        for exe in [
            "bash",
            "bash.exe",
            r"C:\Windows\System32\bash.exe",
            r"C:\WINDOWS\system32\bash.exe",
            r"C:\Windows\Sysnative\bash.exe",
            r"C:\Users\me\AppData\Local\Microsoft\WindowsApps\bash.exe",
        ] {
            assert!(may_be_wsl_launcher(exe), "{exe}");
        }
        for exe in [
            r"C:\Program Files\Git\bin\bash.exe",
            "C:/msys64/usr/bin/bash.exe",
            r"D:\tools\Git\usr\bin\bash.exe",
        ] {
            assert!(!may_be_wsl_launcher(exe), "{exe}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_system32_bash_runs_as_written_with_a_reason() {
        let p = profile(&[r"C:\Windows\System32\bash.exe"], ShellFamily::Posix);
        let reason = integrate(&p, Path::new(DIR), &no_env).err().unwrap();
        assert!(reason.contains("WSL"), "{reason}");
        let git = profile(&[r"C:\Program Files\Git\bin\bash.exe"], ShellFamily::Posix);
        assert!(integrate(&git, Path::new(DIR), &no_env).is_ok());
    }

    #[test]
    fn pwsh_appends_scriptblock_load() {
        let p = profile(&["pwsh.exe", "-NoLogo"], ShellFamily::PowerShell);
        let applied = integrate(&p, Path::new(DIR), &no_env).unwrap();
        assert_eq!(applied.argv[..4], ["pwsh.exe", "-NoLogo", "-NoExit", "-Command"]);
        let last = applied.argv.last().unwrap();
        assert!(last.contains("ReadAllText("), "{last}");
        assert!(last.contains(r"'C:\Users\O''Brien Home\.marshell\shell"), "{last}");
        assert!(last.contains("marshell.ps1'"), "{last}");
        assert!(applied.env.is_empty());
    }

    #[test]
    fn powershell_with_unknown_args_runs_as_written() {
        let p = profile(&["powershell.exe", "-File", "x.ps1"], ShellFamily::PowerShell);
        assert_eq!(
            integrate(&p, Path::new(DIR), &no_env).unwrap_err(),
            "This command has its own arguments, so Marshell runs it as written."
        );
        let p = profile(&["PowerShell.EXE", "-noprofile", "-NOLOGO"], ShellFamily::PowerShell);
        assert!(integrate(&p, Path::new(DIR), &no_env).is_ok());
    }

    #[test]
    fn cmd_wraps_existing_prompt() {
        let p = profile(&["cmd.exe"], ShellFamily::Cmd);
        let outer = |name: &str| (name == "PROMPT").then(|| "$T $P$G".to_string());
        let applied = integrate(&p, Path::new(DIR), &outer).unwrap();
        assert_eq!(applied.argv, ["cmd.exe"]);
        assert_eq!(
            env_value(&applied, "PROMPT"),
            Some(r"$e]133;D$e\$e]133;A$e\$e]9;9;$P$e\$T $P$G")
        );
        // The profile's own PROMPT wins over the outer one.
        let mut p = p;
        p.env.push(("prompt".into(), "> ".into()));
        let applied = integrate(&p, Path::new(DIR), &outer).unwrap();
        assert_eq!(
            env_value(&applied, "PROMPT"),
            Some(r"$e]133;D$e\$e]133;A$e\$e]9;9;$P$e\> ")
        );
    }

    #[test]
    fn cmd_default_prompt() {
        let p = profile(&["cmd.exe"], ShellFamily::Cmd);
        let applied = integrate(&p, Path::new(DIR), &no_env).unwrap();
        assert_eq!(
            env_value(&applied, "PROMPT"),
            Some(r"$e]133;D$e\$e]133;A$e\$e]9;9;$P$e\$P$G")
        );
    }

    #[test]
    fn cmd_does_not_double_wrap() {
        let p = profile(&["cmd.exe"], ShellFamily::Cmd);
        let wrapped = r"$e]133;D$e\$e]133;A$e\$e]9;9;$P$e\$P$G".to_string();
        let outer = move |name: &str| (name == "PROMPT").then(|| wrapped.clone());
        let applied = integrate(&p, Path::new(DIR), &outer).unwrap();
        assert_eq!(
            env_value(&applied, "PROMPT"),
            Some(r"$e]133;D$e\$e]133;A$e\$e]9;9;$P$e\$P$G")
        );
    }

    #[test]
    fn bash_login_becomes_init_file() {
        let p = profile(&[r"C:\Program Files\Git\bin\bash.exe", "-l"], ShellFamily::Posix);
        let applied = integrate(&p, Path::new(DIR), &no_env).unwrap();
        assert_eq!(
            applied.argv,
            [
                r"C:\Program Files\Git\bin\bash.exe",
                "--init-file",
                "C:/Users/O'Brien Home/.marshell/shell/marshell.bash",
                "-i"
            ]
        );
        assert_eq!(env_value(&applied, "MARSHELL_BASH_LOGIN"), Some("1"));
        let p = profile(&["C:/Program Files/Git/bin/bash.exe"], ShellFamily::Posix);
        let applied = integrate(&p, Path::new(DIR), &no_env).unwrap();
        assert_eq!(env_value(&applied, "MARSHELL_BASH_LOGIN"), None);
    }

    #[test]
    fn zsh_sets_zdotdir_and_keeps_user_one() {
        let p = profile(&["/bin/zsh", "-l"], ShellFamily::Posix);
        let outer = |name: &str| (name == "ZDOTDIR").then(|| "/home/me/.config/zsh".to_string());
        let applied = integrate(&p, Path::new("/home/me/.marshell/shell"), &outer).unwrap();
        assert_eq!(applied.argv, ["/bin/zsh", "-l"]);
        assert_eq!(env_value(&applied, "ZDOTDIR"), Some("/home/me/.marshell/shell/zsh"));
        assert_eq!(
            env_value(&applied, "MARSHELL_USER_ZDOTDIR"),
            Some("/home/me/.config/zsh")
        );
        let applied = integrate(&p, Path::new("/home/me/.marshell/shell"), &no_env).unwrap();
        assert_eq!(env_value(&applied, "MARSHELL_USER_ZDOTDIR"), Some(""));
    }

    #[test]
    fn fish_sources_with_c() {
        let p = profile(&["/usr/bin/fish", "-l"], ShellFamily::Posix);
        let applied = integrate(&p, Path::new("/home/o'b/.marshell/shell"), &no_env).unwrap();
        assert_eq!(
            applied.argv,
            [
                "/usr/bin/fish",
                "-l",
                "-C",
                r"source '/home/o'\''b/.marshell/shell/marshell.fish'"
            ]
        );
    }

    #[test]
    fn wsl_dispatcher_quotes_space_path() {
        let p = profile(&["wsl.exe", "-d", "Ubuntu"], ShellFamily::Wsl);
        let dir = r"C:\Users\Home Office\.marshell\shell";
        let applied = integrate(&p, Path::new(dir), &no_env).unwrap();
        assert_eq!(
            applied.argv[..6],
            ["wsl.exe", "-d", "Ubuntu", "--exec", "/bin/sh", "-c"]
        );
        let dispatcher = &applied.argv[6];
        assert!(
            dispatcher.contains(r"wslpath -u 'C:\Users\Home Office\.marshell\shell'"),
            "{dispatcher}"
        );
        let p = profile(&["wsl.exe", "--cd", "~"], ShellFamily::Wsl);
        assert!(integrate(&p, Path::new(dir), &no_env).is_err());
    }

    #[test]
    fn unknown_shell_reason() {
        let p = profile(&["nu"], ShellFamily::Posix);
        assert_eq!(
            integrate(&p, Path::new(DIR), &no_env).unwrap_err(),
            "Marshell has no integration script for nu."
        );
    }

    #[test]
    fn switch_off_reason() {
        let mut p = profile(&["pwsh.exe"], ShellFamily::PowerShell);
        p.shell_integration = false;
        assert_eq!(
            integrate(&p, Path::new(DIR), &no_env).unwrap_err(),
            "Shell integration is off for this profile."
        );
        let info = describe(&p, Some(Path::new(DIR)), &no_env);
        assert!(!info.applies);
        assert_eq!(
            info.reason.as_deref(),
            Some("Shell integration is off for this profile.")
        );
    }

    #[test]
    fn describe_without_dir_and_with_script() {
        let p = profile(&["pwsh.exe"], ShellFamily::PowerShell);
        let info = describe(&p, None, &no_env);
        assert_eq!(
            info.reason.as_deref(),
            Some("The integration scripts could not be written.")
        );
        let info = describe(&p, Some(Path::new(DIR)), &no_env);
        assert!(info.applies && info.reason.is_none());
        assert!(info.script.unwrap().contains("133;A"));
        assert!(info.script_path.unwrap().ends_with("marshell.ps1"));
    }

    #[test]
    fn ps1_reports_the_command_line_before_c() {
        let ps1 = FILES.iter().find(|(name, _)| *name == "marshell.ps1").unwrap().1;
        let line = ps1.find("]633;E;").expect("ps1 reports the command line");
        let c = ps1.find("]133;C").expect("ps1 marks C");
        assert!(line < c);
    }

    #[test]
    fn posix_scripts_have_no_cr() {
        for (name, text) in FILES {
            if !name.ends_with(".ps1") {
                assert!(!text.contains('\r'), "{name} has a CR");
            }
        }
    }

    #[test]
    fn install_writes_once() {
        let home = tempfile::tempdir().unwrap();
        let dir = install(home.path()).unwrap();
        assert_eq!(dir, crate::paths::shell_dir(home.path()));
        let mtimes = |dir: &Path| -> Vec<std::time::SystemTime> {
            FILES
                .iter()
                .map(|(name, _)| std::fs::metadata(dir.join(name)).unwrap().modified().unwrap())
                .collect()
        };
        let first = mtimes(&dir);
        std::thread::sleep(std::time::Duration::from_millis(50));
        install(home.path()).unwrap();
        assert_eq!(mtimes(&dir), first);
        let zshrc = FILES.iter().find(|(name, _)| *name == "zsh/.zshrc").unwrap().1;
        assert_eq!(std::fs::read_to_string(dir.join("zsh/.zshrc")).unwrap(), zshrc);
    }
}
