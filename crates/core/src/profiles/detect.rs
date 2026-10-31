use crate::agents::manifest::{Agents, PromptStyle};
use marshell_protocol::api::{Profile, SessionMode, ShellFamily};
use std::path::{Path, PathBuf};

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
    command_path(name).is_some()
}

/// The file `name` runs from Marshell's own PATH, as `command_on_path` finds it.
pub fn command_path(name: &str) -> Option<PathBuf> {
    command_path_in(&std::env::var_os("PATH")?, name)
}

/// The PATH a session's program is looked up on. It is not Marshell's own: portable-pty builds a child's environment from
/// the process plus the registry's `Environment` values (PATH merged), so a CLI installed while Marshell runs is on it, and
/// one that only Marshell's PATH holds is not. It is read from portable-pty itself, so the two cannot drift. Reads the
/// registry on Windows: take it once per request.
pub fn tab_path() -> Option<std::ffi::OsString> {
    portable_pty::CommandBuilder::new("x")
        .get_env("PATH")
        .map(std::ffi::OsStr::to_os_string)
}

/// The environment a session's process starts from, before Marshell's own changes: the process's, plus the registry's
/// system and user variables on Windows (PATH merged). It is read from portable-pty itself, as `tab_path` is, so a
/// console opened outside gets what a session gets. A name or value that isn't valid Unicode is left out. Reads the
/// registry on Windows: take it once per request, off the async workers.
pub fn session_base_env() -> Vec<(String, String)> {
    portable_pty::CommandBuilder::new("x")
        .iter_full_env_as_str()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// The file `name` runs from the PATH `path`, with the PATHEXT endings on Windows.
pub fn command_path_in(path: &std::ffi::OsStr, name: &str) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = std::env::split_paths(path).collect();
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    let exts: Vec<&str> = if cfg!(windows) {
        pathext.split(';').filter(|e| !e.is_empty()).collect()
    } else {
        Vec::new()
    };
    found_in(&dirs, name, &exts)
}

/// The first directory's `name` plus one of the endings, or else `name` itself.
fn found_in(dirs: &[PathBuf], name: &str, exts: &[&str]) -> Option<PathBuf> {
    dirs.iter().find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .chain(std::iter::once(dir.join(name)))
            .find(|p| p.is_file())
    })
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
    run_wsl(&["-l", "-q"], std::time::Duration::from_secs(2))
}

/// Runs `wsl.exe` with the arguments on a thread, with no console window, and gives up after `timeout`.
/// Its raw stdout, or nothing when WSL is missing, fails or is too slow.
pub(crate) fn run_wsl(args: &[&str], timeout: std::time::Duration) -> Vec<u8> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut cmd = std::process::Command::new("wsl.exe");
        cmd.args(args);
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
    rx.recv_timeout(timeout).unwrap_or_default()
}

/// The text of `wsl.exe` output. WSL prints UTF-16LE, usually with a BOM.
pub(crate) fn decode_wsl_output(stdout: &[u8]) -> String {
    let has_bom = stdout.starts_with(&[0xFF, 0xFE]);
    // ASCII text in UTF-16LE has a zero in every odd byte.
    let looks_utf16 =
        stdout.len() >= 2 && stdout.len().is_multiple_of(2) && stdout.iter().skip(1).step_by(2).all(|b| *b == 0);
    if has_bom || looks_utf16 {
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
    }
}

/// Distro names from `wsl.exe -l -q` output.
pub fn parse_wsl_list(stdout: &[u8]) -> Vec<String> {
    decode_wsl_output(stdout)
        .lines()
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

/// The program an adapter agent id runs. Manifest agents are looked up through `Agents`.
pub fn agent_command(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        "antigravity" => Some("agy"),
        _ => None,
    }
}

/// A program's name as the agent table knows it: the basename, lowercase, with one `.exe`, `.cmd`,
/// `.bat`, `.ps1` or `.com` removed.
pub fn program_name(exe: &str) -> String {
    let base = exe.rsplit(['/', '\\']).next().unwrap_or(exe).to_lowercase();
    for ending in [".exe", ".cmd", ".bat", ".ps1", ".com"] {
        if let Some(stem) = base.strip_suffix(ending) {
            return stem.to_string();
        }
    }
    base
}

/// The words of a command line, with single and double quotes removed.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut word, mut quote, mut open) = (String::new(), None, false);
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                open = true;
            }
            (None, c) if c.is_whitespace() => {
                if open || !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
                open = false;
            }
            (None, c) => word.push(c),
        }
    }
    if open || !word.is_empty() {
        out.push(word);
    }
    out
}

/// The program a command line runs, skipping launchers such as `npx`, `pnpm dlx` and `&`.
/// An npm package (`@scope/name`) is returned as written, without a version.
pub fn program_of(command_line: &str) -> Option<String> {
    let words = words(command_line);
    let mut rest = words.iter().map(String::as_str).peekable();
    while let Some(&word) = rest.peek() {
        let lower = word.to_lowercase();
        let skip = match lower.as_str() {
            "&" | "npx" | "bunx" | "winpty" => 1,
            "pnpm" | "yarn" => {
                let next = rest.clone().nth(1).map(str::to_lowercase);
                match (lower.as_str(), next.as_deref()) {
                    ("pnpm", Some("dlx" | "exec")) | ("yarn", Some("dlx")) => 2,
                    _ => 0,
                }
            }
            _ => 0,
        };
        if skip == 0 {
            break;
        }
        rest.nth(skip - 1);
    }
    let first = rest.next()?;
    if let Some(scoped) = first.strip_prefix('@') {
        let name = scoped.split('@').next().unwrap_or(scoped);
        return Some(format!("@{}", name.to_lowercase()));
    }
    Some(program_name(first))
}

/// The adapter agent an executable name belongs to.
pub(crate) fn adapter_of_exe(name: &str) -> Option<&'static str> {
    match name {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        "agy" => Some("antigravity"),
        _ if name.starts_with("codex-") => Some("codex"),
        _ => None,
    }
}

/// The program of the first profile that runs the adapter `agent`: its command's first word.
pub fn agent_exe(profiles: &[Profile], agent: &str) -> Option<PathBuf> {
    profiles.iter().find_map(|p| {
        let exe = p.command.first()?;
        (adapter_of_exe(&program_name(exe)) == Some(agent)).then(|| PathBuf::from(exe))
    })
}

/// The adapter agent an npm package belongs to.
pub(crate) fn adapter_of_package(name: &str) -> Option<&'static str> {
    match name {
        "@anthropic-ai/claude-code" => Some("claude"),
        "@openai/codex" => Some("codex"),
        _ => None,
    }
}

/// The agent a foreground process chain (outermost first) or a typed command line runs, if any.
/// The chain is checked first, deepest process first, then the command line.
pub fn agent_in(agents: &Agents, chain: &[String], command_line: Option<&str>) -> Option<String> {
    if let Some(agent) = chain.iter().rev().find_map(|name| agents.agent_of_exe(name)) {
        return Some(agent);
    }
    let program = program_of(command_line?)?;
    agents
        .agent_of_exe(&program)
        .or_else(|| agents.agent_of_package(&program))
}

/// Which conversation a resumed agent should pick up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumeTarget<'a> {
    /// The CLI's own "last conversation in this folder".
    Last,
    /// One conversation by the CLI's session id. Claude Code, Codex and Antigravity take one.
    Session(&'a str),
    /// A new conversation that starts from one by the CLI's session id, leaving the original as it is.
    /// Only Claude Code can fork; every other agent continues its last conversation as `Last` does.
    Fork(&'a str),
}

/// The arguments that make an agent continue a conversation: the one named by `target` when the CLI
/// can name it, otherwise its last conversation in the current folder.
/// A manifest agent continues its last conversation for every target; one without `continue` starts fresh.
pub fn resume_args(agents: &Agents, agent: &str, target: ResumeTarget) -> Vec<String> {
    if let Some(m) = agents.manifest(agent) {
        return m.continue_args.clone().unwrap_or_default();
    }
    let args: Vec<&str> = match (agent, target) {
        ("claude", ResumeTarget::Session(id)) => vec!["--resume", id],
        ("claude", ResumeTarget::Fork(id)) => vec!["--resume", id, "--fork-session"],
        ("claude", ResumeTarget::Last) => vec!["--continue"],
        ("codex", ResumeTarget::Session(id)) => vec!["resume", id],
        ("codex", _) => vec!["resume", "--last"],
        ("antigravity", ResumeTarget::Session(id)) => vec!["--conversation", id],
        ("antigravity", _) => vec!["--continue"],
        _ => Vec::new(),
    };
    args.into_iter().map(String::from).collect()
}

/// What to type into `agent`'s prompt so it reads the image at `path`: the absolute path in quotes.
pub fn attachment_ref(agent: Option<&str>, path: &Path) -> String {
    match agent {
        Some("claude") => format!("\"{}\"", path.display()),
        // Codex attaches an image from a path pasted alone, so the reference is the path and nothing else.
        Some("codex") => format!("\"{}\"", path.display()),
        Some("antigravity") => format!("\"{}\"", path.display()),
        _ => format!("\"{}\"", path.display()),
    }
}

/// What to type for a path dropped on a session whose shell is `family`: the agent's reference, unless
/// the name holds a character the shell would act on inside double quotes. Then it is the shell's own
/// single quoting, since the agent may have exited and left the shell's prompt behind.
pub fn dropped_path_ref(agent: Option<&str>, family: ShellFamily, path: &Path) -> String {
    let text = path.to_string_lossy();
    match family {
        ShellFamily::PowerShell if text.contains(['$', '`', '\u{201C}', '\u{201D}', '\u{201E}']) => {
            powershell_quote(&text)
        }
        ShellFamily::Posix | ShellFamily::Wsl if text.contains(['$', '`', '!']) => posix_quote(&text),
        // cmd runs nothing from inside "…".
        _ => attachment_ref(agent, path),
    }
}

/// The CLI's own flags for starting `agent` in `mode`. Hooks are off, so these are the native column of
/// PLAN "Session modes". `sessionModes.ts` (`launchArgs` with hooks off) must show the same flags.
/// Codex has no plan flag, so Plan starts like Manual.
/// An adapter always has an answer, possibly no flags. A manifest agent has one only for a mode it maps,
/// and an unknown agent has none.
pub fn mode_args(agents: &Agents, agent: &str, mode: SessionMode) -> Option<Vec<String>> {
    use SessionMode::*;
    let adapter: Option<&[&str]> = match (agent, mode) {
        ("claude", Manual) => Some(&["--permission-mode", "manual"]),
        ("claude", Plan) => Some(&["--permission-mode", "plan"]),
        ("claude", AutoEdit) => Some(&["--permission-mode", "acceptEdits"]),
        ("claude", FullAuto) => Some(&["--permission-mode", "auto"]),
        ("claude", Bypass) => Some(&["--permission-mode", "bypassPermissions"]),
        ("claude", DontAsk) => Some(&["--permission-mode", "dontAsk"]),
        ("codex", Manual | Plan) => Some(&["--sandbox", "read-only", "--ask-for-approval", "on-request"]),
        ("codex", AutoEdit) => Some(&["--sandbox", "workspace-write", "--ask-for-approval", "on-request"]),
        ("codex", FullAuto) => Some(&["--approve-for-me"]),
        ("codex", Bypass) => Some(&["--dangerously-bypass-approvals-and-sandbox"]),
        ("antigravity", Plan) => Some(&["--mode", "plan"]),
        ("antigravity", AutoEdit) => Some(&["--mode", "accept-edits"]),
        ("antigravity", FullAuto) => Some(&["--mode", "auto"]),
        ("antigravity", Bypass) => Some(&["--dangerously-skip-permissions"]),
        ("codex" | "antigravity", _) => Some(&[]),
        _ => None,
    };
    if let Some(flags) = adapter {
        return Some(flags.iter().map(|f| f.to_string()).collect());
    }
    agents.manifest(agent)?.modes.get(&mode).cloned()
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

/// What `build_argv` puts first: a name the shell looks up, or a path to one file.
#[derive(Clone, Copy, Debug)]
pub enum Program<'a> {
    Name(&'a str),
    Path(&'a Path),
}

impl<'a> Program<'a> {
    /// A command holding a path separator is a path; any other is a name.
    pub fn of_command(command: &'a str) -> Self {
        if command.contains(['/', '\\']) {
            Program::Path(Path::new(command))
        } else {
            Program::Name(command)
        }
    }
}

/// True when `word` can go into a command line as it is: `[A-Za-z0-9._:/\\=-]` and not empty.
fn is_plain_word(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '\\' | '=' | '-'))
}

/// The argv that starts `agent` (with an optional first prompt and mode) inside `profile`'s shell.
/// With no agent it is the profile's own command. `no_daemon` adds Codex's `--no-daemon`, and only for
/// Codex: the caller says so when the detected version accepts it (slice 6a decision 14).
pub fn launch_argv(
    profile: &Profile,
    agents: &Agents,
    agent: Option<(&str, Program)>,
    prompt: Option<&str>,
    mode: Option<SessionMode>,
    no_daemon: bool,
) -> Result<Vec<String>, String> {
    build_argv(profile, agents, agent, prompt, &[], mode, no_daemon)
}

/// Like `launch_argv` with no prompt, but the agent is told to continue the conversation `target` names.
/// With no agent it is the profile's own command. A session id is only ever letters, digits, `_` and `-`,
/// so it is safe as its own word in a shell command line.
pub fn launch_argv_resume(
    profile: &Profile,
    agents: &Agents,
    agent: Option<(&str, Program)>,
    mode: Option<SessionMode>,
    target: ResumeTarget,
    no_daemon: bool,
) -> Result<Vec<String>, String> {
    let extra = agent.map_or_else(Vec::new, |(id, _)| resume_args(agents, id, target));
    build_argv(profile, agents, agent, None, &extra, mode, no_daemon)
}

/// `extra` goes right after the agent's program, then the mode flags, then any prompt.
/// A mode the manifest does not map adds no flags here; the server refuses it before it gets this far.
fn build_argv(
    profile: &Profile,
    agents: &Agents,
    agent: Option<(&str, Program)>,
    prompt: Option<&str>,
    extra: &[String],
    mode: Option<SessionMode>,
    no_daemon: bool,
) -> Result<Vec<String>, String> {
    let Some((agent, program)) = agent else {
        return Ok(profile.command.clone());
    };
    Ok(line_argv(
        profile,
        build_line(profile, agents, agent, program, prompt, extra, mode, no_daemon)?,
    ))
}

/// The line that starts `agent` in a shell of `profile`'s family, before the shell's own wrapper (`line_argv`).
/// The one place an agent's command line is written: a session runs it, and "Copy resume command" copies it.
#[allow(clippy::too_many_arguments)]
fn build_line(
    profile: &Profile,
    agents: &Agents,
    agent: &str,
    program: Program,
    prompt: Option<&str>,
    extra: &[String],
    mode: Option<SessionMode>,
    no_daemon: bool,
) -> Result<String, String> {
    if !agents.is_known(agent) {
        return Err(format!("Unknown agent: {agent}."));
    }
    let manifest = agents.manifest(agent);
    let prompt = prompt.filter(|p| !p.is_empty());
    // The flag before the first prompt: agy's and a manifest's own, else none (a positional argument).
    let prompt_flag: Option<&str> = match manifest.map(|m| &m.prompt) {
        None if agent == "antigravity" => Some("-i"),
        None | Some(PromptStyle::Positional) => None,
        Some(PromptStyle::Flag { flag }) => Some(flag),
        Some(PromptStyle::None) => {
            if prompt.is_some() {
                let cli = manifest.map_or(agent, |m| m.cli.as_str());
                return Err(format!("{cli} can't take a first prompt. Start it, then type it."));
            }
            None
        }
    };
    let mode_flags = mode.and_then(|m| mode_args(agents, agent, m)).unwrap_or_default();
    // Codex's shared daemon runs every session's hooks with the first tab's id (slice 6a decision 12).
    // Only a version that accepts the flag gets it (decision 14).
    let own_server: &[&str] = if agent == "codex" && no_daemon {
        &["--no-daemon"]
    } else {
        &[]
    };
    let program_word = match program {
        Program::Name(name) => name.to_string(),
        Program::Path(path) => path.to_string_lossy().into_owned(),
    };
    let args: Vec<&str> = extra
        .iter()
        .map(String::as_str)
        .chain(own_server.iter().copied())
        .chain(mode_flags.iter().map(String::as_str))
        .collect();
    // Plain words go in as they are; anything else is quoted for the shell, or refused where it can't be.
    // A POSIX shell eats a backslash outside quotes, so there it is not plain.
    let posix = matches!(profile.family, ShellFamily::Posix | ShellFamily::Wsl);
    let word = |text: &str, quote: fn(&str) -> String| {
        if is_plain_word(text) && !(posix && text.contains('\\')) {
            text.to_string()
        } else {
            quote(text)
        }
    };
    let line = match profile.family {
        ShellFamily::PowerShell => {
            let mut line = format!("& {}", word(&program_word, powershell_quote));
            for arg in &args {
                line.push(' ');
                line.push_str(&word(arg, powershell_quote));
            }
            if let Some(p) = prompt {
                // Windows PowerShell 5 mangles double quotes when it calls a native command.
                if p.contains('"') && is_windows_powershell_5(profile) {
                    return Err(
                        "Windows PowerShell 5 can't pass double quotes to the agent. Use PowerShell 7 or remove the quotes."
                            .into(),
                    );
                }
                if let Some(flag) = prompt_flag {
                    line.push(' ');
                    line.push_str(&word(flag, powershell_quote));
                }
                line.push_str(&format!(" {}", powershell_quote(p)));
            }
            line
        }
        ShellFamily::Cmd => {
            // The `/k` string gets quoted on its way to cmd, which splits a prompt into
            // broken arguments. So cmd starts agents only without a prompt.
            if prompt.is_some() {
                return Err(
                    "Command Prompt can't pass a prompt safely. Use PowerShell or start without a prompt.".into(),
                );
            }
            if !is_plain_word(&program_word) {
                return Err("Command Prompt can't start a program whose path has a space. Use PowerShell.".into());
            }
            if args.iter().any(|a| !is_plain_word(a)) {
                return Err("Command Prompt can't pass an argument that needs quoting. Use PowerShell.".into());
            }
            let mut line = program_word;
            for arg in &args {
                line.push(' ');
                line.push_str(arg);
            }
            line
        }
        ShellFamily::Posix | ShellFamily::Wsl => {
            let mut line = word(&program_word, posix_quote);
            for arg in &args {
                line.push(' ');
                line.push_str(&word(arg, posix_quote));
            }
            if let Some(p) = prompt {
                if let Some(flag) = prompt_flag {
                    line.push(' ');
                    line.push_str(&word(flag, posix_quote));
                }
                line.push_str(&format!(" {}", posix_quote(p)));
            }
            line
        }
    };
    Ok(line)
}

/// The line that starts `agent` again for a person to paste into a terminal: its own command with the resume
/// arguments for `target` (None starts a new conversation) and the flags of `mode`. Never `--no-daemon`: that is for
/// the sessions Marshell starts. Refused where the shell can't quote a word (Command Prompt).
pub fn resume_line(
    profile: &Profile,
    agents: &Agents,
    agent: &str,
    program: Program,
    mode: Option<SessionMode>,
    target: Option<ResumeTarget>,
) -> Result<String, String> {
    let extra = target.map_or_else(Vec::new, |t| resume_args(agents, agent, t));
    build_line(profile, agents, agent, program, None, &extra, mode, false)
}

/// The argv that runs `line` in `profile`'s shell, the way an agent session does: PowerShell stays open after it,
/// `cmd /k` stays open, and a POSIX shell runs it and then replaces itself with a login shell. No integration.
pub(crate) fn line_argv(profile: &Profile, mut line: String) -> Vec<String> {
    let mut argv = profile.command.clone();
    match profile.family {
        ShellFamily::PowerShell => argv.extend(["-NoExit".into(), "-Command".into(), line]),
        ShellFamily::Cmd => argv.extend(["/k".into(), line]),
        ShellFamily::Wsl => {
            line.push_str("; exec bash -l");
            // --exec starts bash directly; plain `--` would run the line through the
            // distro's default shell first, which expands $(...) inside the prompt.
            argv.extend(["--exec".into(), "bash".into(), "-lc".into(), line]);
        }
        ShellFamily::Posix => {
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
    argv
}

/// One argument of a launch preset, as it goes into the command line.
#[derive(Clone, PartialEq, Eq)]
pub enum PresetArg {
    /// Written in the preset itself.
    Literal(String),
    /// A value the person typed: always one argument, and never read as a flag where the shell can't quote it.
    Field(String),
}

/// What cmd takes as it is inside a `/k` string: no character it treats as an operator, quote, expansion or space.
fn is_cmd_safe(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | '=' | '@' | ',' | '+' | '-'))
}

/// True when the profile runs `powershell.exe` (Windows PowerShell 5): it mangles double quotes in a native call.
pub(crate) fn mangles_double_quotes(profile: &Profile) -> bool {
    is_windows_powershell_5(profile)
}

/// The line that runs `program` with `args` in a shell of `family`. The program comes from the preset only. Each argument
/// is exactly one argument of the program: quoted per shell, and refused where the shell can't quote it (cmd).
pub fn preset_line(family: ShellFamily, program: &str, args: &[PresetArg]) -> Result<String, String> {
    const CMD_REFUSED: &str = "Command Prompt can't pass an argument that needs quoting. Use PowerShell.";
    let posix = matches!(family, ShellFamily::Posix | ShellFamily::Wsl);
    let word = |text: &str, quote: fn(&str) -> String| {
        if is_plain_word(text) && !(posix && text.contains('\\')) {
            text.to_string()
        } else {
            quote(text)
        }
    };
    let mut line = String::new();
    match family {
        ShellFamily::PowerShell => {
            line.push_str(&format!("& {}", word(program, powershell_quote)));
            for arg in args {
                let (PresetArg::Literal(text) | PresetArg::Field(text)) = arg;
                line.push(' ');
                line.push_str(&word(text, powershell_quote));
            }
        }
        ShellFamily::Cmd => {
            if !is_plain_word(program) {
                return Err("Command Prompt can't start a program whose path has a space. Use PowerShell.".into());
            }
            line.push_str(program);
            for arg in args {
                let ok = match arg {
                    PresetArg::Literal(text) => is_plain_word(text) || is_cmd_safe(text),
                    PresetArg::Field(text) => is_cmd_safe(text) && !text.starts_with('-'),
                };
                if !ok {
                    return Err(CMD_REFUSED.into());
                }
                let (PresetArg::Literal(text) | PresetArg::Field(text)) = arg;
                line.push(' ');
                line.push_str(text);
            }
        }
        ShellFamily::Posix | ShellFamily::Wsl => {
            line.push_str(&word(program, posix_quote));
            for arg in args {
                let (PresetArg::Literal(text) | PresetArg::Field(text)) = arg;
                line.push(' ');
                line.push_str(&word(text, posix_quote));
            }
        }
    }
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::manifest::{AgentManifest, PromptStyle, shipped};

    /// The program a test launches for `agent`: an adapter's command, else the manifest's first command.
    fn program_of_agent(agent: &str) -> Program<'static> {
        if let Some(command) = agent_command(agent) {
            return Program::Name(command);
        }
        match shipped().iter().find(|m| m.id == agent) {
            Some(m) => Program::Name(&m.commands[0]),
            None => Program::Name("nope"),
        }
    }

    /// `launch_argv` without Codex's `--no-daemon`; the tests that want the flag call `launch_argv_nd` with `true`.
    fn launch_argv(
        profile: &Profile,
        agent: Option<&str>,
        prompt: Option<&str>,
        mode: Option<SessionMode>,
    ) -> Result<Vec<String>, String> {
        launch_argv_nd(profile, agent, prompt, mode, false)
    }

    fn launch_argv_nd(
        profile: &Profile,
        agent: Option<&str>,
        prompt: Option<&str>,
        mode: Option<SessionMode>,
        no_daemon: bool,
    ) -> Result<Vec<String>, String> {
        super::launch_argv(
            profile,
            &Agents::shipped(),
            agent.map(|a| (a, program_of_agent(a))),
            prompt,
            mode,
            no_daemon,
        )
    }

    /// `launch_argv_resume` without Codex's `--no-daemon`.
    fn launch_argv_resume(
        profile: &Profile,
        agent: Option<&str>,
        mode: Option<SessionMode>,
        target: ResumeTarget,
    ) -> Result<Vec<String>, String> {
        launch_argv_resume_nd(profile, agent, mode, target, false)
    }

    fn launch_argv_resume_nd(
        profile: &Profile,
        agent: Option<&str>,
        mode: Option<SessionMode>,
        target: ResumeTarget,
        no_daemon: bool,
    ) -> Result<Vec<String>, String> {
        super::launch_argv_resume(
            profile,
            &Agents::shipped(),
            agent.map(|a| (a, program_of_agent(a))),
            mode,
            target,
            no_daemon,
        )
    }

    fn resume_args(agent: &str, target: ResumeTarget) -> Vec<String> {
        super::resume_args(&Agents::shipped(), agent, target)
    }

    fn mode_args(agent: &str, mode: SessionMode) -> Option<Vec<String>> {
        super::mode_args(&Agents::shipped(), agent, mode)
    }

    fn agent_in(chain: &[String], command_line: Option<&str>) -> Option<String> {
        super::agent_in(&Agents::shipped(), chain, command_line)
    }

    #[test]
    fn found_in_tries_the_endings() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("claude.cmd"), "").unwrap();
        std::fs::write(dir.path().join("plain"), "").unwrap();
        let dirs = vec![dir.path().to_path_buf()];
        assert!(found_in(&dirs, "claude", &[".EXE", ".cmd"]).is_some());
        assert!(found_in(&dirs, "plain", &[]).is_some());
        assert!(found_in(&dirs, "claude", &[".EXE"]).is_none());
        assert!(found_in(&dirs, "agy", &[".EXE", ".cmd"]).is_none());
    }

    #[test]
    fn found_in_prefers_an_ending_to_the_bare_name() {
        // npm puts a sh script named `claude` next to `claude.cmd`; Windows can only run the second.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("claude"), "").unwrap();
        std::fs::write(dir.path().join("claude.cmd"), "").unwrap();
        let dirs = vec![dir.path().to_path_buf()];
        assert_eq!(
            found_in(&dirs, "claude", &[".EXE", ".cmd"]),
            Some(dir.path().join("claude.cmd"))
        );
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
    fn agent_exe_finds_the_claude_profile() {
        let profiles = vec![
            profile("pwsh", &["pwsh.exe", "-NoLogo"], ShellFamily::PowerShell),
            profile(
                "claude",
                &["C:\\Tools\\claude.exe", "--verbose"],
                ShellFamily::PowerShell,
            ),
        ];
        assert_eq!(
            agent_exe(&profiles, "claude"),
            Some(PathBuf::from("C:\\Tools\\claude.exe"))
        );
        assert_eq!(agent_exe(&profiles, "codex"), None);
        assert_eq!(agent_exe(&profiles[..1], "claude"), None);
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
        let argv = launch_argv(&ps, Some("claude"), Some("it's"), None).unwrap();
        assert_eq!(&argv[..3], ["pwsh.exe", "-NoLogo", "-NoExit"]);
        assert_eq!(argv[3], "-Command");
        assert_eq!(argv.last().unwrap(), "& claude 'it''s'");
    }

    #[test]
    fn agy_prompt_uses_dash_i() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv(&ps, Some("antigravity"), Some("hi"), None).unwrap();
        assert_eq!(argv.last().unwrap(), "& agy -i 'hi'");
        let sh = profile("login-shell", &["/bin/zsh", "-l"], ShellFamily::Posix);
        let argv = launch_argv(&sh, Some("antigravity"), Some("hi"), None).unwrap();
        assert!(argv.last().unwrap().starts_with("agy -i 'hi';"));
    }

    #[test]
    fn cmd_without_prompt_builds_k_line() {
        let cmd = profile("cmd", &["cmd.exe"], ShellFamily::Cmd);
        assert_eq!(
            launch_argv_nd(&cmd, Some("codex"), None, None, true).unwrap(),
            ["cmd.exe", "/k", "codex --no-daemon"]
        );
    }

    #[test]
    fn cmd_rejects_any_prompt() {
        let cmd = profile("cmd", &["cmd.exe"], ShellFamily::Cmd);
        for prompt in ["fix the bug", "a\"b", "100%"] {
            let err = launch_argv(&cmd, Some("claude"), Some(prompt), None).unwrap_err();
            assert_eq!(
                err,
                "Command Prompt can't pass a prompt safely. Use PowerShell or start without a prompt."
            );
        }
    }

    #[test]
    fn powershell_doubles_curly_quotes() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv(
            &ps,
            Some("claude"),
            Some("x\u{2019}; Write-Output INJECTED; \u{2019}"),
            None,
        )
        .unwrap();
        assert_eq!(
            argv.last().unwrap(),
            "& claude 'x\u{2019}\u{2019}; Write-Output INJECTED; \u{2019}\u{2019}'"
        );
        for q in ['\u{2018}', '\u{201A}', '\u{201B}'] {
            let argv = launch_argv(&ps, Some("claude"), Some(&format!("a{q}b")), None).unwrap();
            assert_eq!(argv.last().unwrap(), &format!("& claude 'a{q}{q}b'"));
        }
    }

    #[test]
    fn windows_powershell_5_rejects_double_quotes() {
        let ps5 = profile("powershell", &["powershell.exe", "-NoLogo"], ShellFamily::PowerShell);
        let err = launch_argv(&ps5, Some("claude"), Some("say \"hi\""), None).unwrap_err();
        assert_eq!(
            err,
            "Windows PowerShell 5 can't pass double quotes to the agent. Use PowerShell 7 or remove the quotes."
        );
        // The file name is matched without regard to case; a prompt without quotes is fine.
        let shouting = profile("powershell", &["C:\\Windows\\POWERSHELL.EXE"], ShellFamily::PowerShell);
        assert!(launch_argv(&shouting, Some("claude"), Some("a\"b"), None).is_err());
        assert!(launch_argv(&ps5, Some("claude"), Some("hi"), None).is_ok());
        // PowerShell 7 is unaffected.
        let pwsh = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        assert!(launch_argv(&pwsh, Some("claude"), Some("say \"hi\""), None).is_ok());
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
        let argv = launch_argv(&sh, Some("claude"), Some("it's"), None).unwrap();
        assert_eq!(argv, ["/bin/zsh", "-l", "-c", "claude 'it'\\''s'; exec \"$SHELL\" -l"]);
    }

    #[test]
    fn git_bash_returns_to_its_own_path() {
        let gb = profile("git-bash", &["C:\\Git\\bin\\bash.exe", "-l"], ShellFamily::Posix);
        let argv = launch_argv(&gb, Some("claude"), None, None).unwrap();
        assert_eq!(argv.last().unwrap(), "claude; exec 'C:\\Git\\bin\\bash.exe' -l");
    }

    #[test]
    fn wsl_agent_argv_runs_bash_login() {
        let wsl = profile("wsl-ubuntu", &["wsl.exe", "-d", "Ubuntu"], ShellFamily::Wsl);
        let argv = launch_argv_nd(&wsl, Some("codex"), Some("hi"), None, true).unwrap();
        assert_eq!(
            argv,
            [
                "wsl.exe",
                "-d",
                "Ubuntu",
                "--exec",
                "bash",
                "-lc",
                "codex --no-daemon 'hi'; exec bash -l"
            ]
        );
    }

    #[test]
    fn no_agent_returns_profile_command() {
        let ps = profile("pwsh", &["pwsh.exe", "-NoLogo"], ShellFamily::PowerShell);
        assert_eq!(launch_argv(&ps, None, Some("ignored"), None).unwrap(), ps.command);
    }

    #[test]
    fn attachment_ref_quotes_the_absolute_path() {
        let p = Path::new(r"C:\Users\Home Office\.marshell\attachments\t1\1.png");
        let quoted = format!("\"{}\"", p.display());
        for agent in [Some("claude"), Some("codex"), Some("antigravity"), Some("other"), None] {
            assert_eq!(attachment_ref(agent, p), quoted);
        }
    }

    #[test]
    fn dropped_path_ref_keeps_double_quotes_for_a_plain_name() {
        let p = Path::new(r"C:\Users\Home Office\my notes.txt");
        let quoted = format!("\"{}\"", p.display());
        for family in [
            ShellFamily::PowerShell,
            ShellFamily::Cmd,
            ShellFamily::Posix,
            ShellFamily::Wsl,
        ] {
            for agent in [Some("claude"), Some("codex"), None] {
                assert_eq!(dropped_path_ref(agent, family, p), quoted, "{family:?} {agent:?}");
            }
        }
    }

    #[test]
    fn dropped_path_ref_single_quotes_what_powershell_expands_in_double_quotes() {
        // `$`, the backtick and the typographic double quotes all act inside "…" in PowerShell.
        for name in [
            "a$(calc).txt",
            "a$env:x.txt",
            "a`b.txt",
            "a\u{201C};calc;\u{201D}.txt",
            "a\u{201E}b.txt",
        ] {
            let path = format!(r"C:\d\{name}");
            let p = Path::new(&path);
            for agent in [Some("claude"), None] {
                assert_eq!(
                    dropped_path_ref(agent, ShellFamily::PowerShell, p),
                    powershell_quote(&path),
                    "{name:?} {agent:?}"
                );
            }
        }
        assert_eq!(
            dropped_path_ref(None, ShellFamily::PowerShell, Path::new("C:\\d\\it's $x\u{2019}.txt")),
            "'C:\\d\\it''s $x\u{2019}\u{2019}.txt'"
        );
    }

    #[test]
    fn dropped_path_ref_single_quotes_what_a_posix_shell_expands_in_double_quotes() {
        for family in [ShellFamily::Posix, ShellFamily::Wsl] {
            for name in ["a$(id).txt", "a`id`.txt", "a!!.txt"] {
                let path = format!("/home/u/{name}");
                assert_eq!(
                    dropped_path_ref(None, family, Path::new(&path)),
                    posix_quote(&path),
                    "{family:?} {name:?}"
                );
            }
        }
        assert_eq!(
            dropped_path_ref(Some("codex"), ShellFamily::Posix, Path::new("/u/it's $x")),
            r"'/u/it'\''s $x'"
        );
    }

    #[test]
    fn dropped_path_ref_keeps_double_quotes_in_cmd() {
        // cmd runs nothing from inside "…": `%x%` reads a variable, and `&` or `|` is plain text.
        let p = Path::new(r"C:\d\a$(calc) & b %x%.txt");
        assert_eq!(
            dropped_path_ref(None, ShellFamily::Cmd, p),
            format!("\"{}\"", p.display())
        );
    }

    #[test]
    fn codex_resumes_a_session_by_id() {
        assert_eq!(
            resume_args("codex", ResumeTarget::Session("abc-1")),
            ["resume", "abc-1"]
        );
    }

    #[test]
    fn antigravity_resumes_a_conversation_by_id() {
        let id = "00000000-0000-4000-8000-000000000001";
        assert_eq!(
            resume_args("antigravity", ResumeTarget::Session(id)),
            ["--conversation", id]
        );
    }

    #[test]
    fn resume_args_per_agent() {
        assert_eq!(resume_args("claude", ResumeTarget::Last), ["--continue"]);
        assert_eq!(resume_args("codex", ResumeTarget::Last), ["resume", "--last"]);
        assert_eq!(resume_args("antigravity", ResumeTarget::Last), ["--continue"]);
        assert_eq!(resume_args("antigravity", ResumeTarget::Fork("abc-1")), ["--continue"]);
        assert_eq!(resume_args("aider", ResumeTarget::Last), ["--restore-chat-history"]);
        assert!(resume_args("nope", ResumeTarget::Last).is_empty());
    }

    #[test]
    fn resume_argv_adds_the_continue_flag_and_no_prompt() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv_resume(&ps, Some("claude"), None, ResumeTarget::Last).unwrap();
        assert_eq!(argv.last().unwrap(), "& claude --continue");
        let sh = profile("bash", &["bash"], ShellFamily::Posix);
        let argv = launch_argv_resume_nd(&sh, Some("codex"), None, ResumeTarget::Last, true).unwrap();
        assert!(
            argv.last()
                .unwrap()
                .starts_with("codex resume --last --no-daemon; exec")
        );
        let cmd = profile("cmd", &["cmd.exe"], ShellFamily::Cmd);
        assert_eq!(
            launch_argv_resume(&cmd, Some("claude"), None, ResumeTarget::Last).unwrap(),
            ["cmd.exe", "/k", "claude --continue"]
        );
        let argv = launch_argv_resume(&ps, Some("antigravity"), None, ResumeTarget::Last).unwrap();
        assert_eq!(argv.last().unwrap(), "& agy --continue");
        // A plain shell is just its own command.
        assert_eq!(
            launch_argv_resume(&ps, None, None, ResumeTarget::Last).unwrap(),
            ps.command
        );
    }

    #[test]
    fn resume_by_session_id() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let by_id = ResumeTarget::Session("abc-1");
        let argv = launch_argv_resume(&ps, Some("claude"), None, by_id).unwrap();
        assert_eq!(argv.last().unwrap(), "& claude --resume abc-1");
        let argv = launch_argv_resume(&ps, Some("claude"), Some(SessionMode::Bypass), by_id).unwrap();
        assert_eq!(
            argv.last().unwrap(),
            "& claude --resume abc-1 --permission-mode bypassPermissions"
        );
        let argv = launch_argv_resume_nd(&ps, Some("codex"), None, by_id, true).unwrap();
        assert_eq!(argv.last().unwrap(), "& codex resume abc-1 --no-daemon");
    }

    #[test]
    fn resume_command_line_is_the_one_the_session_runs() {
        let agents = Agents::shipped();
        let target = Some(ResumeTarget::Session("abc-1"));
        for (family, command) in [
            (ShellFamily::PowerShell, "pwsh.exe"),
            (ShellFamily::Cmd, "cmd.exe"),
            (ShellFamily::Posix, "bash"),
        ] {
            let p = profile("p", &[command], family);
            let line = resume_line(
                &p,
                &agents,
                "claude",
                Program::Name("claude"),
                Some(SessionMode::Plan),
                target,
            )
            .unwrap();
            let argv = launch_argv_resume(
                &p,
                Some("claude"),
                Some(SessionMode::Plan),
                ResumeTarget::Session("abc-1"),
            )
            .unwrap();
            // The shell's own wrapper is all the argv adds to the line.
            assert!(argv.iter().any(|a| a.starts_with(&line)), "{line:?} not in {argv:?}");
            assert!(line.contains("--resume abc-1 --permission-mode plan"), "{line}");
        }
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        // No target: a new conversation, with the mode's flags.
        let fresh = resume_line(
            &ps,
            &agents,
            "claude",
            Program::Name("claude"),
            Some(SessionMode::Plan),
            None,
        )
        .unwrap();
        assert_eq!(fresh, "& claude --permission-mode plan");
        // Codex never gets `--no-daemon` in a line meant for a person's own terminal.
        let codex = resume_line(
            &ps,
            &agents,
            "codex",
            Program::Name("codex"),
            None,
            Some(ResumeTarget::Session("x1")),
        )
        .unwrap();
        assert_eq!(codex, "& codex resume x1");
        // Command Prompt refuses what it can't quote.
        let cmd = profile("cmd", &["cmd.exe"], ShellFamily::Cmd);
        assert!(
            resume_line(
                &cmd,
                &agents,
                "claude",
                Program::Path(Path::new(r"C:\Program Files\claude.exe")),
                None,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn fork_args_add_fork_session() {
        assert_eq!(
            resume_args("claude", ResumeTarget::Fork("abc-1")),
            ["--resume", "abc-1", "--fork-session"]
        );
        // Codex and Antigravity can't fork by id, so they take the same arguments as `Last`.
        assert_eq!(resume_args("codex", ResumeTarget::Fork("abc-1")), ["resume", "--last"]);
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv_resume(&ps, Some("claude"), None, ResumeTarget::Fork("abc-1")).unwrap();
        assert_eq!(argv.last().unwrap(), "& claude --resume abc-1 --fork-session");
        let argv = launch_argv_resume(
            &ps,
            Some("claude"),
            Some(SessionMode::Bypass),
            ResumeTarget::Fork("abc-1"),
        )
        .unwrap();
        assert_eq!(
            argv.last().unwrap(),
            "& claude --resume abc-1 --fork-session --permission-mode bypassPermissions"
        );
    }

    #[test]
    fn mode_args_match_the_plan_table() {
        use SessionMode::*;
        let mode_args = |agent: &str, mode| mode_args(agent, mode).unwrap();
        assert_eq!(mode_args("claude", Manual), ["--permission-mode", "manual"]);
        assert_eq!(mode_args("claude", Plan), ["--permission-mode", "plan"]);
        assert_eq!(mode_args("claude", AutoEdit), ["--permission-mode", "acceptEdits"]);
        assert_eq!(mode_args("claude", FullAuto), ["--permission-mode", "auto"]);
        assert_eq!(mode_args("claude", Bypass), ["--permission-mode", "bypassPermissions"]);
        assert_eq!(mode_args("claude", DontAsk), ["--permission-mode", "dontAsk"]);
        assert!(mode_args("codex", DontAsk).is_empty());
        assert!(mode_args("antigravity", DontAsk).is_empty());
        let manual = ["--sandbox", "read-only", "--ask-for-approval", "on-request"];
        assert_eq!(mode_args("codex", Manual), manual);
        assert_eq!(mode_args("codex", Plan), mode_args("codex", Manual));
        assert_eq!(
            mode_args("codex", AutoEdit),
            ["--sandbox", "workspace-write", "--ask-for-approval", "on-request"]
        );
        assert_eq!(mode_args("codex", FullAuto), ["--approve-for-me"]);
        assert_eq!(
            mode_args("codex", Bypass),
            ["--dangerously-bypass-approvals-and-sandbox"]
        );
        assert!(mode_args("antigravity", Manual).is_empty());
        assert_eq!(mode_args("antigravity", Plan), ["--mode", "plan"]);
        assert_eq!(mode_args("antigravity", AutoEdit), ["--mode", "accept-edits"]);
        assert_eq!(mode_args("antigravity", FullAuto), ["--mode", "auto"]);
        assert_eq!(mode_args("antigravity", Bypass), ["--dangerously-skip-permissions"]);
        assert_eq!(mode_args("aider", Bypass), ["--yes-always"]);
    }

    #[test]
    fn codex_starts_with_its_own_app_server() {
        // In Codex's shared daemon every session's hooks carry the first tab's id (slice 6a decision 12).
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv_nd(&ps, Some("codex"), Some("hi"), Some(SessionMode::Manual), true).unwrap();
        assert!(argv.last().unwrap().starts_with("& codex --no-daemon "), "{argv:?}");
        // The flag is Codex's own: no other agent gets it, whatever the caller says.
        let argv = launch_argv_nd(&ps, Some("claude"), None, None, true).unwrap();
        assert!(!argv.last().unwrap().contains("--no-daemon"));
    }

    #[test]
    fn codex_starts_without_no_daemon_when_the_version_does_not_take_it() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv_nd(&ps, Some("codex"), Some("hi"), None, false).unwrap();
        assert_eq!(argv.last().unwrap(), "& codex 'hi'");
        let argv = launch_argv_resume_nd(&ps, Some("codex"), None, ResumeTarget::Last, false).unwrap();
        assert_eq!(argv.last().unwrap(), "& codex resume --last");
    }

    #[test]
    fn resume_puts_the_mode_flags_after_the_resume_args() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv_resume(&ps, Some("claude"), Some(SessionMode::Bypass), ResumeTarget::Last).unwrap();
        assert_eq!(
            argv.last().unwrap(),
            "& claude --continue --permission-mode bypassPermissions"
        );
        let argv = launch_argv_resume_nd(
            &ps,
            Some("codex"),
            Some(SessionMode::FullAuto),
            ResumeTarget::Last,
            true,
        )
        .unwrap();
        assert_eq!(
            argv.last().unwrap(),
            "& codex resume --last --no-daemon --approve-for-me"
        );
    }

    #[test]
    fn a_new_agent_session_starts_with_its_mode_flags_before_the_prompt() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        let argv = launch_argv(&ps, Some("claude"), Some("hi"), Some(SessionMode::Plan)).unwrap();
        assert_eq!(argv.last().unwrap(), "& claude --permission-mode plan 'hi'");
    }

    #[test]
    fn antigravity_launches_keep_their_native_mode_flags() {
        // With Marshell's hooks installed or not, agy starts with its own flags: a call the core passes goes
        // back to agy's mode, and in Bypass only the flag makes agy run what a card allowed.
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        for (mode, flags) in [
            (SessionMode::Manual, ""),
            (SessionMode::AutoEdit, " --mode accept-edits"),
            (SessionMode::Bypass, " --dangerously-skip-permissions"),
        ] {
            let argv = launch_argv(&ps, Some("antigravity"), None, Some(mode)).unwrap();
            assert_eq!(argv.last().unwrap(), &format!("& agy{flags}"), "{mode:?}");
        }
        assert!(mode_args("antigravity", SessionMode::Manual).unwrap().is_empty());
        assert_eq!(
            mode_args("antigravity", SessionMode::AutoEdit).unwrap(),
            ["--mode", "accept-edits"]
        );
        assert_eq!(
            mode_args("antigravity", SessionMode::Bypass).unwrap(),
            ["--dangerously-skip-permissions"]
        );
    }

    #[test]
    fn a_plain_shell_ignores_the_mode() {
        let ps = profile("pwsh", &["pwsh.exe", "-NoLogo"], ShellFamily::PowerShell);
        assert_eq!(
            launch_argv(&ps, None, None, Some(SessionMode::Plan)).unwrap(),
            ps.command
        );
    }

    #[test]
    fn unknown_agent_is_an_error() {
        let ps = profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell);
        assert_eq!(
            launch_argv(&ps, Some("nope"), None, None).unwrap_err(),
            "Unknown agent: nope."
        );
        assert_eq!(agent_command("antigravity"), Some("agy"));
        assert_eq!(agent_command("aider"), None);
    }

    #[test]
    fn program_name_is_the_lowercase_basename_without_one_ending() {
        assert_eq!(program_name("C:\\x\\Codex.EXE"), "codex");
        assert_eq!(program_name("/usr/bin/claude"), "claude");
        assert_eq!(program_name("run.cmd"), "run");
        assert_eq!(program_name("a.exe.exe"), "a.exe");
    }

    #[test]
    fn program_of_skips_wrappers_and_quotes() {
        assert_eq!(program_of("docker compose up").as_deref(), Some("docker"));
        assert_eq!(program_of("npx @openai/codex").as_deref(), Some("@openai/codex"));
        assert_eq!(program_of("pnpm dlx @openai/codex").as_deref(), Some("@openai/codex"));
        assert_eq!(program_of("pnpm exec codex").as_deref(), Some("codex"));
        assert_eq!(program_of("yarn dlx codex").as_deref(), Some("codex"));
        assert_eq!(program_of("bunx codex").as_deref(), Some("codex"));
        assert_eq!(program_of("winpty claude").as_deref(), Some("claude"));
        assert_eq!(program_of(".\\codex.exe").as_deref(), Some("codex"));
        assert_eq!(program_of("./codex").as_deref(), Some("codex"));
        assert_eq!(program_of("& 'C:\\a b\\claude.cmd' -c").as_deref(), Some("claude"));
        assert_eq!(program_of("pnpm test").as_deref(), Some("pnpm"));
        assert_eq!(program_of("   "), None);
    }

    #[test]
    fn agent_in_reads_the_chain_then_the_command_line() {
        let chain = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let agent = |names: &[&str], line: Option<&str>| agent_in(&chain(names), line);
        assert_eq!(
            agent(&["cmd", "node", "codex-x86_64-pc-windows-msvc"], None).as_deref(),
            Some("codex")
        );
        assert_eq!(agent(&["claude"], None).as_deref(), Some("claude"));
        assert_eq!(agent(&["agy"], None).as_deref(), Some("antigravity"));
        assert_eq!(
            agent(&[], Some("npx @anthropic-ai/claude-code")).as_deref(),
            Some("claude")
        );
        assert_eq!(agent(&["node"], Some("git status")), None);
        assert_eq!(agent(&["claude-helper"], None), None);
        assert_eq!(agent(&["codex"], None).as_deref(), Some("codex"));
    }

    fn pwsh() -> Profile {
        profile("pwsh", &["pwsh.exe"], ShellFamily::PowerShell)
    }

    #[test]
    fn grok_launches_with_a_positional_prompt_and_its_mode() {
        let argv = launch_argv(&pwsh(), Some("grok"), Some("fix it"), Some(SessionMode::Manual)).unwrap();
        assert_eq!(argv.last().unwrap(), "& grok --permission-mode default 'fix it'");
    }

    #[test]
    fn qwen_takes_its_prompt_after_dash_i() {
        let argv = launch_argv(&pwsh(), Some("qwen"), Some("x"), Some(SessionMode::Plan)).unwrap();
        assert_eq!(argv.last().unwrap(), "& qwen --approval-mode plan -i 'x'");
    }

    #[test]
    fn opencode_takes_its_prompt_after_dash_dash_prompt() {
        let argv = launch_argv(&pwsh(), Some("opencode"), Some("x"), None).unwrap();
        assert_eq!(argv.last().unwrap(), "& opencode --prompt 'x'");
        let sh = profile("login-shell", &["/bin/zsh", "-l"], ShellFamily::Posix);
        let argv = launch_argv(&sh, Some("opencode"), Some("it's"), None).unwrap();
        assert!(argv.last().unwrap().starts_with("opencode --prompt 'it'\\''s';"));
    }

    #[test]
    fn a_prompt_for_a_cli_without_one_is_refused() {
        for (agent, cli) in [
            ("kimi", "Kimi Code CLI"),
            ("crush", "Crush"),
            ("amp", "Amp"),
            ("aider", "Aider"),
        ] {
            let err = launch_argv(&pwsh(), Some(agent), Some("hi"), None).unwrap_err();
            assert_eq!(err, format!("{cli} can't take a first prompt. Start it, then type it."));
            // No prompt, or an empty one, starts it.
            assert!(launch_argv(&pwsh(), Some(agent), None, None).is_ok());
            assert!(launch_argv(&pwsh(), Some(agent), Some(""), None).is_ok());
        }
    }

    #[test]
    fn manifest_resume_uses_continue_for_every_target() {
        for target in [
            ResumeTarget::Last,
            ResumeTarget::Session("abc-1"),
            ResumeTarget::Fork("abc-1"),
        ] {
            let line = |agent| {
                launch_argv_resume(&pwsh(), Some(agent), None, target)
                    .unwrap()
                    .pop()
                    .unwrap()
            };
            assert_eq!(line("grok"), "& grok --continue");
            assert_eq!(line("amp"), "& amp threads continue");
            assert_eq!(line("aider"), "& aider --restore-chat-history");
        }
    }

    #[test]
    fn an_unmapped_mode_adds_no_flags_and_is_refused_by_the_server() {
        assert_eq!(mode_args("cursor", SessionMode::AutoEdit), None);
        assert_eq!(mode_args("opencode", SessionMode::Manual), None);
        assert_eq!(mode_args("cursor", SessionMode::Manual), Some(vec![]));
        assert_eq!(mode_args("cursor", SessionMode::Plan), Some(vec!["--plan".to_string()]));
        // The adapters keep `Some(&[])` where they had no flags.
        assert_eq!(mode_args("antigravity", SessionMode::Manual), Some(vec![]));
        assert_eq!(mode_args("codex", SessionMode::DontAsk), Some(vec![]));
        // build_argv itself adds nothing for it; the server's check is what refuses.
        let argv = launch_argv(&pwsh(), Some("cursor"), None, Some(SessionMode::AutoEdit)).unwrap();
        assert_eq!(argv.last().unwrap(), "& cursor-agent");
    }

    fn own_agent(command: &str) -> AgentManifest {
        AgentManifest {
            id: "mine".into(),
            name: "Mine".into(),
            cli: "Mine".into(),
            mark: "generic".into(),
            commands: vec![command.into()],
            reject_paths: vec![],
            version_shape: None,
            exe_names: vec!["my agent".into()],
            packages: vec![],
            prompt: PromptStyle::Positional,
            continue_args: None,
            modes: Default::default(),
        }
    }

    #[test]
    fn an_own_command_with_a_space_is_quoted_per_shell() {
        let path = Path::new("C:\\Users\\Home Office\\tools\\my agent.exe");
        let agents = Agents::with_own(vec![own_agent("x")]);
        let launch = |profile: &Profile, flags: &[&str]| {
            let mut agent = own_agent("x");
            agent
                .modes
                .insert(SessionMode::Bypass, flags.iter().map(|f| f.to_string()).collect());
            let agents = Agents::with_own(vec![agent]);
            super::launch_argv(
                profile,
                &agents,
                Some(("mine", Program::Path(path))),
                None,
                Some(SessionMode::Bypass),
                false,
            )
        };
        let argv = launch(&pwsh(), &[]).unwrap();
        assert_eq!(argv.last().unwrap(), "& 'C:\\Users\\Home Office\\tools\\my agent.exe'");
        // Own flags with a space or a quote are quoted the same way.
        let argv = launch(&pwsh(), &["--name", "it's mine", "--safe=1"]).unwrap();
        assert_eq!(
            argv.last().unwrap(),
            "& 'C:\\Users\\Home Office\\tools\\my agent.exe' --name 'it''s mine' --safe=1"
        );
        let sh = profile("login-shell", &["/bin/zsh", "-l"], ShellFamily::Posix);
        let argv = launch(&sh, &["--name", "it's mine"]).unwrap();
        assert!(
            argv.last()
                .unwrap()
                .starts_with("'C:\\Users\\Home Office\\tools\\my agent.exe' --name 'it'\\''s mine'; exec"),
            "{argv:?}"
        );
        let cmd = profile("cmd", &["cmd.exe"], ShellFamily::Cmd);
        assert_eq!(
            launch(&cmd, &[]).unwrap_err(),
            "Command Prompt can't start a program whose path has a space. Use PowerShell."
        );
        // A safe program with an argument that needs quoting is refused in cmd too.
        let err = super::launch_argv(
            &cmd,
            &{
                let mut agent = own_agent("x");
                agent.modes.insert(SessionMode::Bypass, vec!["a b".into()]);
                Agents::with_own(vec![agent])
            },
            Some(("mine", Program::Name("mine"))),
            None,
            Some(SessionMode::Bypass),
            false,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "Command Prompt can't pass an argument that needs quoting. Use PowerShell."
        );
        // The ids and the program names of the agents are known to the value.
        assert!(agents.is_known("mine"));
    }

    #[test]
    fn foreground_names_come_from_manifests() {
        let chain = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let agents = Agents::with_own(vec![own_agent("my-agent-marshell-test")]);
        let agent = |names: &[&str], line: Option<&str>| super::agent_in(&agents, &chain(names), line);
        assert_eq!(agent(&["pwsh", "grok"], None).as_deref(), Some("grok"));
        // Grok installs an `agent.exe` too: a bare `agent` is nobody's.
        assert_eq!(agent(&["pwsh", "agent"], None), None);
        assert_eq!(agent(&["pwsh", "grok", "agent"], None).as_deref(), Some("grok"));
        assert_eq!(agent(&["pwsh", "cursor-agent"], None).as_deref(), Some("cursor"));
        assert_eq!(agent(&[], Some("npx @qwen-code/qwen-code")).as_deref(), Some("qwen"));
        assert_eq!(agent(&[], Some("npx opencode-ai run")).as_deref(), Some("opencode"));
        assert_eq!(agent(&["my agent"], None).as_deref(), Some("mine"));
        // The deepest process wins, adapter or not.
        assert_eq!(agent(&["claude", "grok"], None).as_deref(), Some("grok"));
        assert_eq!(agent(&["grok", "claude"], None).as_deref(), Some("claude"));
        // Shipped-only agents don't know the own one.
        assert_eq!(super::agent_in(&Agents::shipped(), &chain(&["my agent"]), None), None);
    }

    #[test]
    fn adapters_launch_exactly_as_before() {
        let sh = profile("login-shell", &["/bin/zsh", "-l"], ShellFamily::Posix);
        let line = |profile: &Profile, agent: &str, mode| launch_argv(profile, Some(agent), Some("hi"), mode).unwrap();
        assert_eq!(
            line(&pwsh(), "claude", Some(SessionMode::Plan)).last().unwrap(),
            "& claude --permission-mode plan 'hi'"
        );
        assert_eq!(
            line(&pwsh(), "codex", Some(SessionMode::AutoEdit)).last().unwrap(),
            "& codex --sandbox workspace-write --ask-for-approval on-request 'hi'"
        );
        assert_eq!(line(&pwsh(), "antigravity", None).last().unwrap(), "& agy -i 'hi'");
        assert_eq!(
            line(&sh, "claude", None).last().unwrap(),
            "claude 'hi'; exec \"$SHELL\" -l"
        );
    }

    fn field(text: &str) -> PresetArg {
        PresetArg::Field(text.into())
    }

    fn literal(text: &str) -> PresetArg {
        PresetArg::Literal(text.into())
    }

    /// The text a PowerShell single-quoted string holds, or None if `quoted` is not exactly one such string.
    fn powershell_unquote(quoted: &str) -> Option<String> {
        let inner = quoted.strip_prefix('\'')?.strip_suffix('\'')?;
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
                // A quote inside must be doubled.
                if chars.next() != Some(c) {
                    return None;
                }
            }
            out.push(c);
        }
        Some(out)
    }

    /// The text a POSIX single-quoted word holds, or None if `quoted` is not exactly one such word.
    fn posix_unquote(quoted: &str) -> Option<String> {
        let mut rest = quoted;
        let mut out = String::new();
        loop {
            let body = rest.strip_prefix('\'')?;
            let end = body.find('\'')?;
            out.push_str(&body[..end]);
            rest = &body[end + 1..];
            if rest.is_empty() {
                return Some(out);
            }
            // The only thing between two quoted runs is an escaped quote.
            rest = rest.strip_prefix("\\'")?;
            out.push('\'');
        }
    }

    #[test]
    fn a_preset_line_quotes_per_shell() {
        let args = [literal("run"), field("my dir")];
        assert_eq!(
            preset_line(ShellFamily::PowerShell, "cargo", &args).unwrap(),
            "& cargo run 'my dir'"
        );
        assert_eq!(
            preset_line(ShellFamily::Posix, "cargo", &args).unwrap(),
            "cargo run 'my dir'"
        );
        assert_eq!(
            preset_line(ShellFamily::Wsl, "cargo", &args).unwrap(),
            "cargo run 'my dir'"
        );
        assert_eq!(
            preset_line(ShellFamily::Cmd, "npx", &[literal("vite"), field("web:3000")]).unwrap(),
            "npx vite web:3000"
        );
        assert_eq!(
            preset_line(ShellFamily::PowerShell, "npx", &[literal("vite")]).unwrap(),
            "& npx vite"
        );
    }

    #[test]
    fn cmd_refuses_what_needs_quoting() {
        let refused = "Command Prompt can't pass an argument that needs quoting. Use PowerShell.";
        assert_eq!(
            preset_line(ShellFamily::Cmd, "kubectl", &[field("my dir")]).unwrap_err(),
            refused
        );
        // A leading dash from a field would be read as a flag of the tool.
        assert_eq!(
            preset_line(ShellFamily::Cmd, "kubectl", &[field("-v")]).unwrap_err(),
            refused
        );
        // The preset's own flags are fine.
        assert_eq!(
            preset_line(ShellFamily::Cmd, "kubectl", &[literal("-v"), field("pod/x")]).unwrap(),
            "kubectl -v pod/x"
        );
        assert!(preset_line(ShellFamily::Cmd, "npx", &[field("")]).is_err());
    }

    #[test]
    fn a_field_value_cannot_break_out_of_its_argument() {
        let values = [
            "x'; calc; '",
            "$(calc)",
            "`calc`",
            "a b",
            "a\"b",
            "x & calc",
            "x | calc",
            "%PATH%",
            "x^y",
            "(a)",
            "<a>",
            "!a!",
            "a\\b",
            "\u{2019}; calc; \u{2018}",
            "a;b",
            "x\ty",
        ];
        for value in values {
            let ps = preset_line(ShellFamily::PowerShell, "tool", &[literal("go"), field(value)]).unwrap();
            let arg = ps.strip_prefix("& tool go ").unwrap();
            // A plain word (letters, digits and `._:/\=-`) is one argument as it is; anything else is one quoted string.
            let bare = is_plain_word(value) && arg == value;
            assert!(
                bare || powershell_unquote(arg).as_deref() == Some(value),
                "powershell {value:?}"
            );
            for family in [ShellFamily::Posix, ShellFamily::Wsl] {
                let sh = preset_line(family, "tool", &[literal("go"), field(value)]).unwrap();
                let arg = sh.strip_prefix("tool go ").unwrap();
                assert_eq!(posix_unquote(arg).as_deref(), Some(value), "posix {value:?}");
            }
            assert_eq!(
                preset_line(ShellFamily::Cmd, "tool", &[literal("go"), field(value)]).unwrap_err(),
                "Command Prompt can't pass an argument that needs quoting. Use PowerShell.",
                "cmd {value:?}"
            );
        }
        // A plain value goes in as it is, and the program is never quoted from a field.
        assert_eq!(
            preset_line(ShellFamily::Posix, "tool", &[field("web-1")]).unwrap(),
            "tool web-1"
        );
    }

    #[test]
    fn a_run_line_goes_after_the_shell_the_way_an_agent_does() {
        let line = |p: &Profile| line_argv(p, "tool go".into());
        let pwsh = profile("pwsh", &["pwsh.exe", "-NoLogo"], ShellFamily::PowerShell);
        assert_eq!(line(&pwsh), ["pwsh.exe", "-NoLogo", "-NoExit", "-Command", "tool go"]);
        let cmd = profile("cmd", &["cmd.exe"], ShellFamily::Cmd);
        assert_eq!(line(&cmd), ["cmd.exe", "/k", "tool go"]);
        let sh = profile("bash", &["bash", "-l"], ShellFamily::Posix);
        assert_eq!(line(&sh), ["bash", "-l", "-c", "tool go; exec \"$SHELL\" -l"]);
        let wsl = profile("wsl", &["wsl.exe"], ShellFamily::Wsl);
        assert_eq!(
            line(&wsl),
            ["wsl.exe", "--exec", "bash", "-lc", "tool go; exec bash -l"]
        );
    }
}
