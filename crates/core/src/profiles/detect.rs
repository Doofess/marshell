use marshell_protocol::api::{Profile, ShellFamily};
use std::path::PathBuf;

/// The shell a new tab runs when the request names none.
/// Windows: PowerShell 7 if installed, else Windows PowerShell. Unix: `$SHELL -l`.
pub fn default_shell() -> Vec<String> {
    #[cfg(windows)]
    {
        let exe = if on_path("pwsh.exe") {
            "pwsh.exe"
        } else {
            "powershell.exe"
        };
        vec![exe.to_string(), "-NoLogo".to_string()]
    }
    #[cfg(unix)]
    {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        vec![sh, "-l".to_string()]
    }
}

/// True when `exe` is a file in one of the PATH directories.
pub fn on_path(exe: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|dir| dir.join(exe).is_file()))
        .unwrap_or(false)
}

/// True when `name` runs from one of the PATH directories. On Windows that includes the
/// PATHEXT endings, so `claude` finds `claude.cmd` and `agy` finds `agy.exe`.
pub fn command_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    let exts: Vec<&str> = if cfg!(windows) {
        pathext.split(';').filter(|e| !e.is_empty()).collect()
    } else {
        Vec::new()
    };
    found_in(&dirs, name, &exts)
}

/// True when a directory holds `name` itself or `name` plus one of the endings.
fn found_in(dirs: &[PathBuf], name: &str, exts: &[&str]) -> bool {
    dirs.iter()
        .any(|dir| dir.join(name).is_file() || exts.iter().any(|ext| dir.join(format!("{name}{ext}")).is_file()))
}

/// What detection asks the machine. Tests pass fakes; `Probe::system()` is the real thing.
pub struct Probe {
    pub on_path: fn(&str) -> bool,
    /// Raw stdout of `wsl.exe -l -q`. Empty when WSL is missing or fails.
    pub wsl_list: fn() -> Vec<u8>,
    /// Folders that may hold `Git\bin\bash.exe`.
    pub program_roots: Vec<PathBuf>,
    /// The unix login shell, for example `/bin/zsh`.
    pub login_shell: Option<String>,
}

impl Probe {
    pub fn system() -> Self {
        let mut program_roots = Vec::new();
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(dir) = std::env::var_os(var) {
                program_roots.push(PathBuf::from(dir));
            }
        }
        if let Some(dir) = std::env::var_os("LOCALAPPDATA") {
            program_roots.push(PathBuf::from(dir).join("Programs"));
        }
        let login_shell = if cfg!(windows) {
            None
        } else {
            Some(std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()))
        };
        Probe {
            on_path,
            wsl_list: system_wsl_list,
            program_roots,
            login_shell,
        }
    }
}

/// Runs `wsl.exe -l -q` on a thread and gives up after 2 seconds.
fn system_wsl_list() -> Vec<u8> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut cmd = std::process::Command::new("wsl.exe");
        cmd.args(["-l", "-q"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let out = match cmd.output() {
            Ok(o) if o.status.success() => o.stdout,
            _ => Vec::new(),
        };
        let _ = tx.send(out);
    });
    rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or_default()
}

/// Distro names from `wsl.exe -l -q` output. WSL prints UTF-16LE, usually with a BOM.
pub fn parse_wsl_list(stdout: &[u8]) -> Vec<String> {
    let has_bom = stdout.starts_with(&[0xFF, 0xFE]);
    // ASCII text in UTF-16LE has a zero in every odd byte.
    let looks_utf16 =
        stdout.len() >= 2 && stdout.len().is_multiple_of(2) && stdout.iter().skip(1).step_by(2).all(|b| *b == 0);
    let text = if has_bom || looks_utf16 {
        let units: Vec<u16> = stdout
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        char::decode_utf16(units)
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect::<String>()
    } else {
        String::from_utf8_lossy(stdout).into_owned()
    };
    text.lines()
        .map(|line| line.trim_matches(|c: char| c == '\0' || c == '\u{feff}' || c.is_whitespace()))
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect()
}

/// The first `<root>\Git\bin\bash.exe` that exists.
pub fn git_bash_path(roots: &[PathBuf]) -> Option<PathBuf> {
    roots
        .iter()
        .map(|root| root.join("Git").join("bin").join("bash.exe"))
        .find(|path| path.is_file())
}

fn detected(id: &str, name: &str, command: Vec<String>, family: ShellFamily) -> Profile {
    Profile {
        id: id.to_string(),
        name: name.to_string(),
        command,
        family,
        cwd: None,
        env: Vec::new(),
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    }
}

/// The shells found on this machine, in the order the launcher lists them.
pub fn detect_profiles(probe: &Probe) -> Vec<Profile> {
    let mut found = Vec::new();
    if cfg!(windows) {
        if (probe.on_path)("pwsh.exe") {
            found.push(detected(
                "pwsh",
                "PowerShell 7",
                vec!["pwsh.exe".into(), "-NoLogo".into()],
                ShellFamily::PowerShell,
            ));
        }
        if (probe.on_path)("powershell.exe") {
            found.push(detected(
                "powershell",
                "Windows PowerShell",
                vec!["powershell.exe".into(), "-NoLogo".into()],
                ShellFamily::PowerShell,
            ));
        }
        if (probe.on_path)("cmd.exe") {
            found.push(detected(
                "cmd",
                "Command Prompt",
                vec!["cmd.exe".into()],
                ShellFamily::Cmd,
            ));
        }
        if let Some(bash) = git_bash_path(&probe.program_roots) {
            found.push(detected(
                "git-bash",
                "Git Bash",
                vec![bash.to_string_lossy().into_owned(), "-l".into()],
                ShellFamily::Posix,
            ));
        }
        for distro in parse_wsl_list(&(probe.wsl_list)()) {
            // Docker Desktop's internal distros are not shells to launch in.
            if distro == "docker-desktop" || distro == "docker-desktop-data" {
                continue;
            }
            found.push(detected(
                &format!("wsl-{}", distro.to_lowercase()),
                &distro,
                vec!["wsl.exe".into(), "-d".into(), distro.clone()],
                ShellFamily::Wsl,
            ));
        }
    } else if let Some(shell) = &probe.login_shell {
        let name = std::path::Path::new(shell)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| shell.clone());
        found.push(detected(
            "login-shell",
            &name,
            vec![shell.clone(), "-l".into()],
            ShellFamily::Posix,
        ));
    }
    found
}

/// The program an agent id runs. Other agents stay hidden until their adapters exist.
pub fn agent_command(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        "antigravity" => Some("agy"),
        _ => None,
    }
}

/// The arguments that make an agent continue its last conversation in the current folder.
/// Antigravity has none that is verified, so it resumes as a fresh start.
pub fn resume_args(agent: &str) -> &'static [&'static str] {
    match agent {
        "claude" => &["--continue"],
        "codex" => &["resume", "--last"],
        _ => &[],
    }
}

/// `'text'` for PowerShell, where a quote inside is doubled. PowerShell also treats the
/// curly single quotes (U+2018 to U+201B) as quotes, so those are doubled too.
pub(crate) fn powershell_quote(text: &str) -> String {
    let mut quoted = String::from("'");
    for c in text.chars() {
        quoted.push(c);
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            quoted.push(c);
        }
    }
    quoted.push('\'');
    quoted
}

/// `'text'` for POSIX shells, where a quote inside becomes `'\''`.
pub(crate) fn posix_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// True when the profile runs `powershell.exe` (Windows PowerShell 5, not `pwsh.exe`).
fn is_windows_powershell_5(profile: &Profile) -> bool {
    profile
        .command
        .first()
        .and_then(|exe| exe.rsplit(['\\', '/']).next())
        .is_some_and(|name| name.eq_ignore_ascii_case("powershell.exe"))
}

/// The argv that starts `agent` (with an optional first prompt) inside `profile`'s shell.
/// With no agent it is the profile's own command.
pub fn launch_argv(profile: &Profile, agent: Option<&str>, prompt: Option<&str>) -> Result<Vec<String>, String> {
    build_argv(profile, agent, prompt, &[])
}

/// Like `launch_argv` with no prompt, but the agent is told to continue its last conversation.
/// With no agent it is the profile's own command.
pub fn launch_argv_resume(profile: &Profile, agent: Option<&str>) -> Result<Vec<String>, String> {
    let extra = agent.map_or(&[][..], resume_args);
    build_argv(profile, agent, None, extra)
}

/// `extra` goes right after the agent's program name, before any prompt.
fn build_argv(
    profile: &Profile,
    agent: Option<&str>,
    prompt: Option<&str>,
    extra: &[&str],
) -> Result<Vec<String>, String> {
    let Some(agent) = agent else {
        return Ok(profile.command.clone());
    };
    let program = agent_command(agent).ok_or_else(|| format!("Unknown agent: {agent}."))?;
    let mut program_line = program.to_string();
    for arg in extra {
        program_line.push(' ');
        program_line.push_str(arg);
    }
    let prompt = prompt.filter(|p| !p.is_empty());
    // agy takes the first prompt after -i; Claude and Codex take it as one positional argument.
    let flag = if program == "agy" { "-i " } else { "" };
    let mut argv = profile.command.clone();
    match profile.family {
        ShellFamily::PowerShell => {
            let mut line = format!("& {program_line}");
            if let Some(p) = prompt {
                // Windows PowerShell 5 mangles double quotes when it calls a native command.
                if p.contains('"') && is_windows_powershell_5(profile) {
                    return Err(
                        "Windows PowerShell 5 can't pass double quotes to the agent. Use PowerShell 7 or remove the quotes."
                            .into(),
                    );
                }
                line.push_str(&format!(" {flag}{}", powershell_quote(p)));
            }
            argv.extend(["-NoExit".into(), "-Command".into(), line]);
        }
        ShellFamily::Cmd => {
            // The `/k` string gets quoted on its way to cmd, which splits a prompt into
            // broken arguments. So cmd starts agents only without a prompt.
            if prompt.is_some() {
                return Err(
                    "Command Prompt can't pass a prompt safely. Use PowerShell or start without a prompt.".into(),
                );
            }
            argv.extend(["/k".into(), program_line]);
        }
        ShellFamily::Posix | ShellFamily::Wsl => {
            let mut line = program_line;
            if let Some(p) = prompt {
                line.push_str(&format!(" {flag}{}", posix_quote(p)));
            }
            if profile.family == ShellFamily::Wsl {
                line.push_str("; exec bash -l");
                // --exec starts bash directly; plain `--` would run the line through the
                // distro's default shell first, which expands $(...) inside the prompt.
                argv.extend(["--exec".into(), "bash".into(), "-lc".into(), line]);
            } else {
                // Git Bash returns to itself by path; other login shells use $SHELL.
                if profile.id == "git-bash" {
                    let bash = profile.command.first().map(String::as_str).unwrap_or("bash");
                    line.push_str(&format!("; exec {} -l", posix_quote(bash)));
                } else {
                    line.push_str("; exec \"$SHELL\" -l");
                }
                argv.extend(["-c".into(), line]);
            }
        }
    }
    Ok(argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn found_in_tries_the_endings() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("claude.cmd"), "").unwrap();
        std::fs::write(dir.path().join("plain"), "").unwrap();
        let dirs = vec![dir.path().to_path_buf()];
        assert!(found_in(&dirs, "claude", &[".EXE", ".cmd"]));
        assert!(found_in(&dirs, "plain", &[]));
        assert!(!found_in(&dirs, "claude", &[".EXE"]));
        assert!(!found_in(&dirs, "agy", &[".EXE", ".cmd"]));
    }

    fn utf16le(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
    }

    fn profile(id: &str, command: &[&str], family: ShellFamily) -> Profile {
        Profile {
            id: id.into(),
            name: id.into(),
            command: command.iter().map(|s| s.to_string()).collect(),
            family,
            cwd: None,
            env: Vec::new(),
            shell_integration: true,
            terminal_theme: None,
            detected: true,
        }
    }

    #[test]
    fn parses_utf16_wsl_list_with_bom() {
        let bytes = utf16le("\u{feff}Ubuntu\r\nDebian\r\n");
        assert_eq!(parse_wsl_list(&bytes), ["Ubuntu", "Debian"]);
    }

    #[test]
    fn parses_empty_and_utf8_wsl_list() {
        assert!(parse_wsl_list(b"").is_empty());
        assert_eq!(parse_wsl_list(b"Ubuntu\n"), ["Ubuntu"]);
    }

    #[test]
    fn parses_utf16_wsl_list_without_bom() {
        assert_eq!(
            parse_wsl_list(&utf16le("Ubuntu\r\n\r\nDebian\r\n")),
            ["Ubuntu", "Debian"]
        );
    }

    #[cfg(windows)]
    fn all_on_path(_: &str) -> bool {
        true
    }

    #[cfg(windows)]
    fn ubuntu_only() -> Vec<u8> {
        utf16le("\u{feff}Ubuntu\r\n")
    }

    #[cfg(windows)]
    #[test]
    fn detects_windows_set() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("Git").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("bash.exe"), b"").unwrap();
        let probe = Probe {
            on_path: all_on_path,
            wsl_list: ubuntu_only,
            program_roots: vec![root.path().to_path_buf()],
            login_shell: None,
        };
        let found = detect_profiles(&probe);
        let ids: Vec<&str> = found.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["pwsh", "powershell", "cmd", "git-bash", "wsl-ubuntu"]);
        assert!(found.iter().all(|p| p.detected));
        assert_eq!(found[4].name, "Ubuntu");
        assert_eq!(found[4].command, ["wsl.exe", "-d", "Ubuntu"]);
    }

    #[cfg(windows)]
    #[test]
    fn missing_pwsh_is_skipped() {
        fn no_pwsh(exe: &str) -> bool {
            exe != "pwsh.exe"
        }
        let probe = Probe {
            on_path: no_pwsh,
            wsl_list: Vec::new,
            program_roots: Vec::new(),
            login_shell: None,
        };
        let ids: Vec<String> = detect_profiles(&probe).into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["powershell", "cmd"]);
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_detects_only_the_login_shell() {
        let probe = Probe {
            on_path: |_| true,
            wsl_list: Vec::new,
            program_roots: Vec::new(),
            login_shell: Some("/bin/zsh".into()),
        };
        let found = detect_profiles(&probe);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "login-shell");
        assert_eq!(found[0].name, "zsh");
        assert_eq!(found[0].command, ["/bin/zsh", "-l"]);
    }

    #[test]
    fn git_bash_path_finds_the_first_existing_root() {
        let empty = tempfile::tempdir().unwrap();
        let with_git = tempfile::tempdir().unwrap();
        let bin = with_git.path().join("Git").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("bash.exe"), b"").unwrap();
        let roots = [empty.path().to_path_buf(), with_git.path().to_path_buf()];
        assert_eq!(git_bash_path(&roots), Some(bin.join("bash.exe")));
        assert_eq!(git_bash_path(&roots[..1]), None);
    }

    #[test]
    fn powershell_agent_argv_quotes_prompt() {
        let ps = profile("pwsh", &["pwsh.exe", "-NoLogo"], ShellFamily::PowerShell);
        let argv = launch_argv(&ps, Some("claude"), Some("it's")).unwrap();
        assert_eq!(&argv[..3], ["pwsh.exe", "-NoLogo", "-NoExit"]);
        assert_eq!(argv[3], "-Command");
        assert_eq!(argv.last().unwrap(), "& claude 'it''s'");
    }

    #[test]
    fn agy_prompt_uses_dash_i() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv(&ps, Some("antigravity"), Some("hi")).unwrap();
        assert_eq!(argv.last().unwrap(), "& agy -i 'hi'");
        let sh = profile("login-shell", &["/bin/zsh", "-l"], ShellFamily::Posix);
        let argv = launch_argv(&sh, Some("antigravity"), Some("hi")).unwrap();
        assert!(argv.last().unwrap().starts_with("agy -i 'hi';"));
    }

    #[test]
    fn cmd_without_prompt_builds_k_line() {
        let cmd = profile("cmd", &["cmd.exe"], ShellFamily::Cmd);
        assert_eq!(
            launch_argv(&cmd, Some("codex"), None).unwrap(),
            ["cmd.exe", "/k", "codex"]
        );
    }

    #[test]
    fn cmd_rejects_any_prompt() {
        let cmd = profile("cmd", &["cmd.exe"], ShellFamily::Cmd);
        for prompt in ["fix the bug", "a\"b", "100%"] {
            let err = launch_argv(&cmd, Some("claude"), Some(prompt)).unwrap_err();
            assert_eq!(
                err,
                "Command Prompt can't pass a prompt safely. Use PowerShell or start without a prompt."
            );
        }
    }

    #[test]
    fn powershell_doubles_curly_quotes() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv(&ps, Some("claude"), Some("x\u{2019}; Write-Output INJECTED; \u{2019}")).unwrap();
        assert_eq!(
            argv.last().unwrap(),
            "& claude 'x\u{2019}\u{2019}; Write-Output INJECTED; \u{2019}\u{2019}'"
        );
        for q in ['\u{2018}', '\u{201A}', '\u{201B}'] {
            let argv = launch_argv(&ps, Some("claude"), Some(&format!("a{q}b"))).unwrap();
            assert_eq!(argv.last().unwrap(), &format!("& claude 'a{q}{q}b'"));
        }
    }

    #[test]
    fn windows_powershell_5_rejects_double_quotes() {
        let ps5 = profile("powershell", &["powershell.exe", "-NoLogo"], ShellFamily::PowerShell);
        let err = launch_argv(&ps5, Some("claude"), Some("say \"hi\"")).unwrap_err();
        assert_eq!(
            err,
            "Windows PowerShell 5 can't pass double quotes to the agent. Use PowerShell 7 or remove the quotes."
        );
        // The file name is matched without regard to case; a prompt without quotes is fine.
        let shouting = profile("powershell", &["C:\\Windows\\POWERSHELL.EXE"], ShellFamily::PowerShell);
        assert!(launch_argv(&shouting, Some("claude"), Some("a\"b")).is_err());
        assert!(launch_argv(&ps5, Some("claude"), Some("hi")).is_ok());
        // PowerShell 7 is unaffected.
        let pwsh = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        assert!(launch_argv(&pwsh, Some("claude"), Some("say \"hi\"")).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn docker_desktop_distros_are_not_profiles() {
        fn distros() -> Vec<u8> {
            utf16le("\u{feff}Ubuntu\r\ndocker-desktop\r\ndocker-desktop-data\r\nDebian\r\n")
        }
        let probe = Probe {
            on_path: |_| false,
            wsl_list: distros,
            program_roots: Vec::new(),
            login_shell: None,
        };
        let ids: Vec<String> = detect_profiles(&probe).into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["wsl-ubuntu", "wsl-debian"]);
    }

    #[test]
    fn posix_agent_argv_returns_to_shell() {
        let sh = profile("login-shell", &["/bin/zsh", "-l"], ShellFamily::Posix);
        let argv = launch_argv(&sh, Some("claude"), Some("it's")).unwrap();
        assert_eq!(argv, ["/bin/zsh", "-l", "-c", "claude 'it'\\''s'; exec \"$SHELL\" -l"]);
    }

    #[test]
    fn git_bash_returns_to_its_own_path() {
        let gb = profile("git-bash", &["C:\\Git\\bin\\bash.exe", "-l"], ShellFamily::Posix);
        let argv = launch_argv(&gb, Some("claude"), None).unwrap();
        assert_eq!(argv.last().unwrap(), "claude; exec 'C:\\Git\\bin\\bash.exe' -l");
    }

    #[test]
    fn wsl_agent_argv_runs_bash_login() {
        let wsl = profile("wsl-ubuntu", &["wsl.exe", "-d", "Ubuntu"], ShellFamily::Wsl);
        let argv = launch_argv(&wsl, Some("codex"), Some("hi")).unwrap();
        assert_eq!(
            argv,
            [
                "wsl.exe",
                "-d",
                "Ubuntu",
                "--exec",
                "bash",
                "-lc",
                "codex 'hi'; exec bash -l"
            ]
        );
    }

    #[test]
    fn no_agent_returns_profile_command() {
        let ps = profile("pwsh", &["pwsh.exe", "-NoLogo"], ShellFamily::PowerShell);
        assert_eq!(launch_argv(&ps, None, Some("ignored")).unwrap(), ps.command);
    }

    #[test]
    fn resume_args_per_agent() {
        assert_eq!(resume_args("claude"), ["--continue"]);
        assert_eq!(resume_args("codex"), ["resume", "--last"]);
        assert!(resume_args("antigravity").is_empty());
        assert!(resume_args("aider").is_empty());
    }

    #[test]
    fn resume_argv_adds_the_continue_flag_and_no_prompt() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv_resume(&ps, Some("claude")).unwrap();
        assert_eq!(argv.last().unwrap(), "& claude --continue");
        let sh = profile("bash", &["bash"], ShellFamily::Posix);
        let argv = launch_argv_resume(&sh, Some("codex")).unwrap();
        assert!(argv.last().unwrap().starts_with("codex resume --last; exec"));
        let cmd = profile("cmd", &["cmd.exe"], ShellFamily::Cmd);
        assert_eq!(
            launch_argv_resume(&cmd, Some("claude")).unwrap(),
            ["cmd.exe", "/k", "claude --continue"]
        );
        // Antigravity starts fresh; a plain shell is just its own command.
        let argv = launch_argv_resume(&ps, Some("antigravity")).unwrap();
        assert_eq!(argv.last().unwrap(), "& agy");
        assert_eq!(launch_argv_resume(&ps, None).unwrap(), ps.command);
    }

    #[test]
    fn unknown_agent_is_an_error() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        assert!(launch_argv(&ps, Some("aider"), None).is_err());
        assert_eq!(agent_command("antigravity"), Some("agy"));
        assert_eq!(agent_command("aider"), None);
    }
}
