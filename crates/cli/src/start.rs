//! Starting Marshell for a command that needs it: `new` and `focus` ask first (or take `--start`), then run the
//! app the one Marshell wrote down in `<home>/bin/app.json` and wait until it answers.

use crate::client::{Client, NOT_RUNNING};
use marshell_protocol::brand;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Asked when the app is closed and a person is at the terminal. No newline: the answer follows on the same line.
pub const QUESTION: &str = "Marshell isn't running. Start it? [y/N] ";
const NO_RECORD: &str = "Couldn't start Marshell: open it once from the Start menu first.";
const TIMED_OUT: &str = "Marshell didn't start within 30 s.";
/// How long the app has to answer after it is started, and how often it is asked.
const LIMIT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(250);

/// A client of the running app. When the app is closed: with `start` it is started, else when `stdin_is_tty` the
/// person is asked, else the "isn't running" error. The `Err` is one line to print.
pub fn ensure_running(
    env: &dyn Fn(&str) -> Option<String>,
    stdin_is_tty: bool,
    ask: &mut dyn BufRead,
    out: &mut dyn Write,
    start: bool,
) -> Result<Client, String> {
    ensure(env, stdin_is_tty, ask, out, start, &spawn_app, LIMIT)
}

fn ensure(
    env: &dyn Fn(&str) -> Option<String>,
    stdin_is_tty: bool,
    ask: &mut dyn BufRead,
    out: &mut dyn Write,
    start: bool,
    spawn: &dyn Fn(&Path) -> io::Result<()>,
    limit: Duration,
) -> Result<Client, String> {
    if let Some(client) = answering(env) {
        return Ok(client);
    }
    if !start {
        if !stdin_is_tty {
            return Err(NOT_RUNNING.to_string());
        }
        let _ = write!(out, "{QUESTION}");
        let _ = out.flush();
        let mut answer = String::new();
        let _ = ask.read_line(&mut answer);
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            return Err(NOT_RUNNING.to_string());
        }
    }
    let exe = app_exe(env)?;
    spawn(&exe).map_err(|e| format!("Couldn't start Marshell: {e}."))?;
    let deadline = Instant::now() + limit;
    loop {
        if let Some(client) = answering(env) {
            return Ok(client);
        }
        if Instant::now() >= deadline {
            return Err(TIMED_OUT.to_string());
        }
        std::thread::sleep(POLL);
    }
}

/// A client of an app that answers `GET /v1/health`. A leftover `endpoint.json` of a crash is not one.
fn answering(env: &dyn Fn(&str) -> Option<String>) -> Option<Client> {
    Client::connect(env).ok().filter(Client::health)
}

/// The app's exe from `<home>/bin/app.json`, which the app writes each time it starts.
fn app_exe(env: &dyn Fn(&str) -> Option<String>) -> Result<PathBuf, String> {
    let home = marshell_hook::endpoint::home(env).ok_or(NO_RECORD)?;
    let text = std::fs::read_to_string(marshell_core::paths::app_record(&home)).map_err(|_| NO_RECORD)?;
    let exe = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("exe")?.as_str().map(PathBuf::from))
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or(NO_RECORD)?;
    if !exe.is_file() {
        return Err(format!("Couldn't start Marshell: {} isn't there.", exe.display()));
    }
    Ok(exe)
}

/// The app's command: no arguments, run in its own folder, and without the variables that tie a session to the
/// terminal this CLI may be running in.
fn app_command(exe: &Path) -> Command {
    let mut command = Command::new(exe);
    if let Some(dir) = exe.parent().filter(|d| !d.as_os_str().is_empty()) {
        command.current_dir(dir);
    }
    for name in ["TAB_ID", "SESSION_NUMBER", "AGENT"] {
        command.env_remove(brand::env_var(name));
    }
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    command
}

/// `DETACHED_PROCESS`, and out of the job of the terminal the CLI runs in, so closing that session doesn't close
/// the app.
#[cfg(windows)]
fn app_flags(breakaway: bool) -> u32 {
    use windows_sys::Win32::System::Threading::{CREATE_BREAKAWAY_FROM_JOB, DETACHED_PROCESS};
    if breakaway {
        DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB
    } else {
        DETACHED_PROCESS
    }
}

/// Starts the app and doesn't wait for it.
#[cfg(windows)]
fn spawn_app(exe: &Path) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    const ERROR_ACCESS_DENIED: i32 = 5;
    let start = |breakaway: bool| app_command(exe).creation_flags(app_flags(breakaway)).spawn().map(drop);
    match start(true) {
        // A job that doesn't allow breakaway refuses it. The app then stays in that job, which is better than no app.
        Err(err) if err.raw_os_error() == Some(ERROR_ACCESS_DENIED) => start(false),
        other => other,
    }
}

#[cfg(not(windows))]
fn spawn_app(exe: &Path) -> io::Result<()> {
    app_command(exe).spawn().map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_core::server::{CoreConfig, RunningCore, start};
    use std::cell::RefCell;
    use std::ffi::OsStr;
    use std::io::Cursor;

    fn env_for(home: &Path) -> impl Fn(&str) -> Option<String> + use<> {
        let home = home.to_string_lossy().into_owned();
        move |name: &str| (name == "MARSHELL_HOME").then(|| home.clone())
    }

    fn write_record(home: &Path, json: &str) {
        let path = marshell_core::paths::app_record(home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, json).unwrap();
    }

    /// Never spawns: a test that reaches it has gone wrong.
    fn no_spawn(_: &Path) -> io::Result<()> {
        panic!("the app must not be started here");
    }

    fn run(
        home: &Path,
        tty: bool,
        input: &str,
        start: bool,
        spawn: &dyn Fn(&Path) -> io::Result<()>,
        limit: Duration,
    ) -> (Result<Client, String>, String) {
        let mut out = Vec::new();
        let env = env_for(home);
        let result = ensure(
            &env,
            tty,
            &mut Cursor::new(input.to_string()),
            &mut out,
            start,
            spawn,
            limit,
        );
        (result, String::from_utf8(out).unwrap())
    }

    #[test]
    fn ensure_running_asks_once() {
        let home = tempfile::tempdir().unwrap();
        // A person at the terminal answers n: the question was printed once, and nothing starts.
        let (r, out) = run(home.path(), true, "n\n", false, &no_spawn, LIMIT);
        assert_eq!(r.err().as_deref(), Some("Marshell isn't running."));
        assert_eq!(out, "Marshell isn't running. Start it? [y/N] ");
        // An empty answer (just Enter, or the end of input) is a no.
        let (r, _) = run(home.path(), true, "\n", false, &no_spawn, LIMIT);
        assert_eq!(r.err().as_deref(), Some("Marshell isn't running."));
        let (r, _) = run(home.path(), true, "", false, &no_spawn, LIMIT);
        assert_eq!(r.err().as_deref(), Some("Marshell isn't running."));
        // No terminal and no --start: it doesn't ask.
        let (r, out) = run(home.path(), false, "y\n", false, &no_spawn, LIMIT);
        assert_eq!(r.err().as_deref(), Some("Marshell isn't running."));
        assert_eq!(out, "");
        // Yes: with no record of the app, it says to open it once.
        let (r, out) = run(home.path(), true, "y\n", false, &no_spawn, LIMIT);
        assert_eq!(
            r.err().as_deref(),
            Some("Couldn't start Marshell: open it once from the Start menu first.")
        );
        assert_eq!(out, QUESTION);
        // --start never asks.
        let (r, out) = run(home.path(), true, "", true, &no_spawn, LIMIT);
        assert_eq!(
            r.err().as_deref(),
            Some("Couldn't start Marshell: open it once from the Start menu first.")
        );
        assert_eq!(out, "");
    }

    #[test]
    fn a_bad_record_is_not_a_start() {
        let home = tempfile::tempdir().unwrap();
        let missing = home.path().join("nowhere").join("marshell-app.exe");
        write_record(home.path(), &serde_json::json!({ "exe": missing }).to_string());
        let (r, _) = run(home.path(), false, "", true, &no_spawn, LIMIT);
        assert_eq!(
            r.err().unwrap(),
            format!("Couldn't start Marshell: {} isn't there.", missing.display())
        );
        for junk in ["not json", "{}", r#"{"exe":""}"#, r#"{"exe":7}"#] {
            write_record(home.path(), junk);
            let (r, _) = run(home.path(), false, "", true, &no_spawn, LIMIT);
            assert_eq!(
                r.err().as_deref(),
                Some("Couldn't start Marshell: open it once from the Start menu first."),
                "{junk}"
            );
        }
    }

    #[test]
    fn an_app_that_never_answers_times_out() {
        let home = tempfile::tempdir().unwrap();
        let exe = home.path().join("app.exe");
        std::fs::write(&exe, b"").unwrap();
        write_record(home.path(), &serde_json::json!({ "exe": exe }).to_string());
        let spawned = RefCell::new(Vec::new());
        let spawn = |p: &Path| {
            spawned.borrow_mut().push(p.to_path_buf());
            Ok(())
        };
        let (r, _) = run(home.path(), false, "", true, &spawn, Duration::from_millis(600));
        assert_eq!(r.err().as_deref(), Some("Marshell didn't start within 30 s."));
        assert_eq!(*spawned.borrow(), vec![exe.clone()]);
        // A spawn that fails says why, in one line.
        let fail = |_: &Path| Err(io::Error::other("no luck"));
        let (r, _) = run(home.path(), false, "", true, &fail, Duration::from_millis(600));
        assert_eq!(r.err().as_deref(), Some("Couldn't start Marshell: no luck."));
    }

    #[test]
    fn a_leftover_endpoint_is_not_a_running_app() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("run")).unwrap();
        std::fs::write(
            home.path().join("run").join("endpoint.json"),
            r#"{"version":1,"port":1,"token":"t","platform":"windows"}"#,
        )
        .unwrap();
        let (r, _) = run(home.path(), false, "", false, &no_spawn, LIMIT);
        assert_eq!(r.err().as_deref(), Some("Marshell isn't running."));
    }

    #[test]
    fn a_started_app_is_waited_for() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let home = tempfile::tempdir().unwrap();
        let exe = home.path().join("app.exe");
        std::fs::write(&exe, b"").unwrap();
        write_record(home.path(), &serde_json::json!({ "exe": exe }).to_string());
        let core: RefCell<Option<RunningCore>> = RefCell::new(None);
        // The stand-in "app" is an in-process core that comes up when the CLI starts it.
        let spawn = |_: &Path| {
            let config = CoreConfig {
                home: home.path().to_path_buf(),
                allowed_origins: vec![],
                programs_dir: None,
                notify: None,
                focus: None,
                notify_paused: Default::default(),
                window_hidden: Default::default(),
                import: None,
            };
            *core.borrow_mut() = Some(rt.block_on(start(config)).unwrap());
            Ok(())
        };
        let (r, out) = run(home.path(), false, "", true, &spawn, LIMIT);
        assert!(r.is_ok(), "{:?}", r.err());
        assert_eq!(out, "");
        assert!(r.unwrap().health());
        core.take().unwrap().shutdown();
    }

    #[test]
    fn the_app_is_started_without_the_terminals_session() {
        let exe = Path::new("some").join("dir").join("app.exe");
        let command = app_command(&exe);
        assert_eq!(command.get_args().count(), 0);
        assert_eq!(command.get_current_dir(), Some(exe.parent().unwrap()));
        let removed: Vec<&OsStr> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name)
            .collect();
        for name in ["MARSHELL_TAB_ID", "MARSHELL_SESSION_NUMBER", "MARSHELL_AGENT"] {
            assert!(
                removed.contains(&OsStr::new(name)),
                "{name} stays in the app's environment"
            );
        }
        // Nothing else is touched: MARSHELL_HOME and the rest pass through.
        assert_eq!(removed.len(), 3);
    }

    #[cfg(windows)]
    #[test]
    fn the_app_is_detached_and_out_of_the_job() {
        const DETACHED: u32 = 0x8;
        const BREAKAWAY: u32 = 0x0100_0000;
        assert_eq!(app_flags(true), DETACHED | BREAKAWAY);
        assert_eq!(app_flags(false), DETACHED);
    }

    /// A batch file stands in for the app: it writes a marker beside itself, so the test sees it ran from its own
    /// folder. The real app is never started here.
    #[cfg(windows)]
    #[test]
    fn spawn_runs_the_exe_in_its_own_folder() {
        let dir = tempfile::tempdir().unwrap();
        let stand_in = dir.path().join("standin.bat");
        std::fs::write(&stand_in, "@echo off\r\necho started> standin-ran.txt\r\n").unwrap();
        spawn_app(&stand_in).unwrap();
        let marker = dir.path().join("standin-ran.txt");
        let deadline = Instant::now() + Duration::from_secs(15);
        while !marker.exists() {
            assert!(Instant::now() < deadline, "the stand-in never ran");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
