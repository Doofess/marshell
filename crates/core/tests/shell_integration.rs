//! Real shells started through Marshell's integration scripts. Each test is skipped (with a
//! printed note) when its shell is not on this machine.

mod common;

use marshell_core::profiles::detect::{command_on_path, git_bash_path};
use marshell_core::pty::{Mark, Pull, Session, SpawnSpec};
use marshell_core::shell;
use marshell_protocol::api::{Profile, ShellFamily};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(20);

fn profile(command: &str, family: ShellFamily) -> Profile {
    Profile {
        id: "test".into(),
        name: "Test".into(),
        command: vec![command.into()],
        family,
        cwd: None,
        env: vec![],
        shell_integration: true,
        terminal_theme: None,
        detected: true,
    }
}

/// A shell running through its integration script, with the marks it has printed so far.
struct Shell {
    session: Arc<Session>,
    marks: Arc<Mutex<Vec<Mark>>>,
    output: Vec<u8>,
    cursor: u64,
    /// How many marks the test has already matched.
    seen: usize,
    _home: tempfile::TempDir,
}

impl Shell {
    /// `setup` writes rc files into the temp home before the shell starts.
    fn start(profile: Profile, setup: impl FnOnce(&Path)) -> Shell {
        Shell::start_with(profile, setup, |_, _| {})
    }

    /// Like `start`, and `tweak` may change the integrated argv (given the temp home) before the shell starts.
    fn start_with(profile: Profile, setup: impl FnOnce(&Path), tweak: impl FnOnce(&mut Vec<String>, &Path)) -> Shell {
        common::tie_children_to_test();
        let home = tempfile::tempdir().unwrap();
        setup(home.path());
        let dir = shell::install(&home.path().join(".marshell")).unwrap();
        let mut applied = shell::integrate(&profile, &dir, &|k| std::env::var(k).ok()).unwrap();
        tweak(&mut applied.argv, home.path());
        let home_str = home.path().to_string_lossy().into_owned();
        // The temp home first, so the runner's own rc files don't load; integration env after.
        let mut env = vec![
            ("HOME".to_string(), home_str.clone()),
            ("USERPROFILE".to_string(), home_str.clone()),
            ("ZDOTDIR".to_string(), home_str),
        ];
        env.extend(applied.env);
        let marks = Arc::new(Mutex::new(Vec::new()));
        let sink = marks.clone();
        let spec = SpawnSpec {
            argv: applied.argv,
            cwd: Some(home.path().to_path_buf()),
            env,
            env_remove: vec![],
            env_strip: None,
            cols: 100,
            rows: 30,
            on_exit: None,
            on_mark: Some(Box::new(move |m| sink.lock().unwrap().push(m))),
            on_tool: None,
            record: None,
            classify: None,
            initial_command: None,
        };
        let session = Session::spawn("t".into(), spec).unwrap();
        session.attach();
        session.resume(0);
        Shell {
            session,
            marks,
            output: Vec::new(),
            cursor: 0,
            seen: 0,
            _home: home,
        }
    }

    fn read_some(&mut self) {
        match self.session.pull(self.cursor, 64 * 1024) {
            Pull::Data { seq, bytes } => {
                self.cursor = seq + bytes.len() as u64;
                self.answer_cursor_queries(&bytes);
                self.output.extend(bytes);
                self.session.ack(self.cursor);
            }
            Pull::Reset { seq, screen } => {
                self.cursor = seq;
                self.output.extend(screen);
                self.session.ack(self.cursor);
            }
            Pull::Exit(code) => panic!("the shell exited ({code}): {}", self.text()),
            Pull::Idle => std::thread::sleep(Duration::from_millis(10)),
        }
    }

    /// PSReadLine asks where the cursor is (`ESC[6n`) and waits for the answer. A terminal (xterm.js in the app,
    /// ConPTY on Windows) replies; off Windows nothing else does here, so the test replies. A query split across two
    /// reads goes unanswered, and PSReadLine asks again.
    fn answer_cursor_queries(&self, bytes: &[u8]) {
        if cfg!(windows) {
            return;
        }
        for _ in bytes.windows(4).filter(|w| *w == b"\x1b[6n") {
            self.session.write_input(b"\x1b[1;1R".to_vec());
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.output).into_owned()
    }

    /// Waits for the next mark after the ones already seen that equals `want`.
    fn expect(&mut self, want: Mark) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            {
                let marks = self.marks.lock().unwrap();
                if let Some(i) = marks[self.seen..].iter().position(|m| *m == want) {
                    self.seen += i + 1;
                    return;
                }
            }
            assert!(
                Instant::now() < deadline,
                "no {want:?} after {:?}\noutput: {}",
                &self.marks.lock().unwrap()[..self.seen],
                self.text()
            );
            self.read_some();
        }
    }

    fn expect_text(&mut self, want: &str) {
        let deadline = Instant::now() + TIMEOUT;
        while !self.text().contains(want) {
            assert!(Instant::now() < deadline, "no {want:?} in output: {}", self.text());
            self.read_some();
        }
    }

    fn type_line(&mut self, line: &str) {
        if cfg!(windows) {
            self.session.write_input(line.as_bytes().to_vec());
            return;
        }
        // Off Windows, PSReadLine reads keys while it waits for a cursor reply, and can drop the Enter. So type once
        // the shell is quiet, and press Enter on its own.
        let (text, enter) = match line.strip_suffix('\r') {
            Some(text) => (text, true),
            None => (line, false),
        };
        self.settle();
        self.session.write_input(text.as_bytes().to_vec());
        if enter {
            self.settle();
            self.session.write_input(b"\r".to_vec());
        }
    }

    /// Reads until the shell has printed nothing for a moment.
    fn settle(&mut self) {
        let quiet = Duration::from_millis(300);
        let deadline = Instant::now() + TIMEOUT;
        let mut len = self.output.len();
        let mut since = Instant::now();
        while since.elapsed() < quiet && Instant::now() < deadline {
            self.read_some();
            if self.output.len() != len {
                len = self.output.len();
                since = Instant::now();
            }
        }
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        self.session.force_kill();
    }
}

fn skipped(name: &str) {
    println!("skipped: {name} not found");
}

/// Git Bash on Windows, `bash` elsewhere.
fn bash() -> Option<String> {
    if cfg!(windows) {
        let roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)"]
            .iter()
            .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
            .collect();
        git_bash_path(&roots).map(|p| p.to_string_lossy().into_owned())
    } else {
        command_on_path("bash").then(|| "bash".to_string())
    }
}

fn exit_code_cycle(mut sh: Shell, command: &str, code: i32) {
    sh.expect(Mark::PromptStart);
    sh.type_line(command);
    sh.expect(Mark::Executed);
    sh.expect(Mark::Finished(Some(code)));
    sh.expect(Mark::PromptStart);
}

#[test]
fn pwsh_reports_prompt_running_and_exit() {
    let mut names = vec!["pwsh"];
    if cfg!(windows) {
        names.push("powershell");
    }
    for name in names {
        if !command_on_path(name) {
            skipped(name);
            continue;
        }
        let sh = Shell::start(profile(name, ShellFamily::PowerShell), |_| {});
        let command = if cfg!(windows) {
            "cmd /c exit 3\r"
        } else {
            "sh -c 'exit 3'\r"
        };
        exit_code_cycle(sh, command, 3);
    }
}

/// The PowerShells on this machine.
fn powershells() -> Vec<&'static str> {
    let mut names = vec!["pwsh"];
    if cfg!(windows) {
        names.push("powershell");
    }
    names
        .into_iter()
        .filter(|n| command_on_path(n) || (skipped(n), false).1)
        .collect()
}

/// PowerShell's profile lives in the Documents folder, which a temp HOME does not move. So the test skips the real
/// profile and loads `profile.ps1` from the temp home the way PowerShell would: before Marshell's script.
fn pwsh_with_profile(name: &str, body: &str) -> Shell {
    let body = body.to_string();
    Shell::start_with(
        profile(name, ShellFamily::PowerShell),
        |home| std::fs::write(home.join("profile.ps1"), body).unwrap(),
        |argv, home| {
            argv.insert(1, "-NoProfile".into());
            let load = argv.pop().unwrap();
            let path = home.join("profile.ps1").to_string_lossy().replace('\'', "''");
            // As a scriptblock, like Marshell's own script: a Restricted ExecutionPolicy blocks only script files.
            argv.push(format!(
                ". ([scriptblock]::Create([IO.File]::ReadAllText('{path}'))); {load}"
            ));
        },
    )
}

fn exit_3() -> &'static str {
    if cfg!(windows) {
        "cmd /c exit 3\r"
    } else {
        "sh -c 'exit 3'\r"
    }
}

/// The body of the first `OSC 633;E` in the terminal output: the command line as the shell escaped it.
fn command_line_body(text: &str) -> Option<&str> {
    const START: &str = "\x1b]633;E;";
    let rest = &text[text.find(START)? + START.len()..];
    Some(&rest[..rest.find("\x1b\\")?])
}

#[test]
fn pwsh_reports_only_the_first_line_of_a_long_command_capped_at_200_chars() {
    for name in powershells() {
        let mut sh = pwsh_with_profile(name, "");
        sh.expect(Mark::PromptStart);
        // An open quote makes PSReadLine take Enter as a new line, so the command has two lines.
        sh.type_line(&format!("\"{}\r", "a".repeat(300)));
        sh.type_line("second line\"\r");
        sh.expect(Mark::Executed);
        sh.settle();
        let text = sh.text();
        let body = command_line_body(&text).unwrap_or_else(|| panic!("{name}: no 633;E in {text}"));
        assert!(body.starts_with("\"aaa"), "{name}: {body}");
        assert!(body.chars().count() <= 200, "{name}: {} chars", body.chars().count());
        assert!(!body.contains("second"), "{name}: the second line leaked: {body}");
    }
}

#[test]
fn pwsh_strict_mode_profile_still_marks() {
    for name in powershells() {
        let mut sh = pwsh_with_profile(name, "Set-StrictMode -Version Latest\n");
        sh.expect(Mark::PromptStart);
        sh.type_line(exit_3());
        sh.expect(Mark::Executed);
        sh.expect(Mark::Finished(Some(3)));
        sh.expect(Mark::PromptStart);
        assert!(!sh.text().contains("cannot be retrieved"), "{name}: {}", sh.text());
    }
}

#[test]
fn pwsh_user_prompt_still_sees_a_failure() {
    for name in powershells() {
        let mut sh = pwsh_with_profile(name, "function prompt { \"ok=$($global:?)> \" }\n");
        sh.expect(Mark::PromptStart);
        sh.type_line("Get-Item ./no-such-marshell-file\r");
        sh.expect(Mark::Finished(Some(1)));
        sh.expect_text("ok=False");
    }
}

#[test]
fn pwsh_failed_cmdlet_after_native_exit_reports_1() {
    for name in powershells() {
        let mut sh = pwsh_with_profile(name, "");
        sh.expect(Mark::PromptStart);
        sh.type_line(exit_3());
        sh.expect(Mark::Finished(Some(3)));
        sh.type_line("Get-Item ./no-such-marshell-file\r");
        sh.expect(Mark::Finished(Some(1)));
    }
}

#[test]
fn pwsh_reports_its_folder_after_cd() {
    for name in powershells() {
        let mut sh = pwsh_with_profile(name, "");
        std::fs::create_dir(sh._home.path().join("sub")).unwrap();
        sh.expect(Mark::PromptStart);
        sh.type_line("cd sub\r");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let cwd = sh.session.current_cwd();
            if cwd.as_deref().is_some_and(|p| p.ends_with("sub")) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{name}: folder is {cwd:?}\noutput: {}",
                sh.text()
            );
            sh.read_some();
        }
    }
}

#[test]
fn bash_reports_prompt_running_and_exit() {
    let Some(bash) = bash() else {
        return skipped("bash");
    };
    let sh = Shell::start(profile(&bash, ShellFamily::Posix), |_| {});
    exit_code_cycle(sh, "false\r", 1);
}

#[test]
fn zsh_reports_prompt_running_and_exit() {
    if !command_on_path("zsh") {
        return skipped("zsh");
    }
    let sh = Shell::start(profile("zsh", ShellFamily::Posix), |_| {});
    exit_code_cycle(sh, "false\r", 1);
}

#[test]
fn cmd_reports_prompts() {
    if !cfg!(windows) {
        return skipped("cmd");
    }
    let mut sh = Shell::start(profile("cmd.exe", ShellFamily::Cmd), |_| {});
    sh.expect(Mark::PromptStart);
    sh.type_line("echo hi\r");
    sh.expect(Mark::PromptStart);
}

#[test]
fn profile_error_still_marks() {
    let Some(bash) = bash() else {
        return skipped("bash");
    };
    let mut sh = Shell::start(profile(&bash, ShellFamily::Posix), |home| {
        std::fs::write(home.join(".bashrc"), "false; echo broken >&2\n").unwrap();
    });
    sh.expect_text("broken");
    sh.expect(Mark::PromptStart);
}

#[test]
fn bash_prompt_command_overwriting_ps1_keeps_marks() {
    let Some(bash) = bash() else {
        return skipped("bash");
    };
    let mut sh = Shell::start(profile(&bash, ShellFamily::Posix), |home| {
        std::fs::write(home.join(".bashrc"), "PROMPT_COMMAND='PS1=\"x$ \"'\n").unwrap();
    });
    sh.expect(Mark::PromptStart);
    sh.type_line("\r");
    sh.expect(Mark::PromptStart);
}
