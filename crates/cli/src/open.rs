//! `marshell open [--session <session>] <address or file>...`: asks the app to open each one as a tab in a
//! session's side pane, in the background.
//!
//! Exit codes: 0 every target was opened or is waiting for the window, 1 any was refused or the app is closed.

use crate::client::{Client, NOT_RUNNING, resolve};
use marshell_protocol::api::{OpenRequest, SessionsSnapshot};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

/// The answer to one open request.
#[derive(Debug, PartialEq, Eq)]
pub enum Opened {
    /// The app opened the tab.
    Done,
    /// The app hasn't answered; the line says the tab opens when its window does.
    Late(String),
    /// A refusal or a failure: the line to print.
    Refused(String),
}

/// What `run` needs from the app, so tests need no server.
pub trait Api {
    fn sessions(&self) -> io::Result<SessionsSnapshot>;
    fn open(&self, tab: &str, req: &OpenRequest) -> Opened;
}

impl Api for Client {
    fn sessions(&self) -> io::Result<SessionsSnapshot> {
        Client::sessions(self)
    }
    fn open(&self, tab: &str, req: &OpenRequest) -> Opened {
        Client::open(self, tab, req)
    }
}

/// What to send for the text a person typed: an http or https address as it is, anything else as an absolute
/// path, joined to the current folder when relative, with `.` and `..` folded without touching the disk.
pub fn target_for(text: &str, cwd: &dyn Fn() -> io::Result<PathBuf>) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("open takes at least one address or file.".to_string());
    }
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(text.to_string());
    }
    let path = Path::new(text);
    // A drive with no root (`D:x.md`) is a file in the folder that drive was last in, which nothing here knows.
    // It goes as typed, and the app refuses it, rather than opening another file of that name.
    if !path.is_absolute() && matches!(path.components().next(), Some(Component::Prefix(_))) {
        return Ok(text.to_string());
    }
    // The root (a drive and its separator) and the folder names after it. Names are joined as text, as
    // `PathBuf::push` would read a name such as `C:` as a new drive.
    let mut root = PathBuf::new();
    let mut names: Vec<String> = Vec::new();
    let base = if path.is_absolute() {
        PathBuf::new()
    } else {
        cwd().map_err(|e| format!("Could not read the current folder: {e}"))?
    };
    for (part, from_text) in base
        .components()
        .map(|p| (p, false))
        .chain(path.components().map(|p| (p, true)))
    {
        match part {
            // A leading separator in relative text is the root of the current drive.
            Component::RootDir if from_text && !path.is_absolute() => names.clear(),
            Component::Prefix(_) | Component::RootDir => root.push(part.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                names.pop();
            }
            Component::Normal(name) => names.push(name.to_string_lossy().into_owned()),
        }
    }
    let mut folded = root.to_string_lossy().into_owned();
    for name in names {
        if !folded.is_empty() && !folded.ends_with(std::path::MAIN_SEPARATOR) {
            folded.push(std::path::MAIN_SEPARATOR);
        }
        folded.push_str(&name);
    }
    Ok(folded)
}

/// The core's line for a path it won't look at. On Windows `localhost:3000` gets it rather than "No file": a colon in
/// a file name names a stream.
const BAD_PATH: &str = "That path can't be opened in the side pane.";

/// Typed text that looks like a web address without its scheme: no separator or drive letter, a `.` or a `:`, and a
/// colon only before a port.
fn looks_like_bare_address(text: &str) -> bool {
    let b = text.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    !drive && !text.contains(['\\', '/']) && text.contains(['.', ':']) && (!text.contains(':') || has_port(text))
}

/// `localhost:3000`: a name, one colon, and a port of up to five digits.
fn has_port(text: &str) -> bool {
    text.split_once(':').is_some_and(|(host, port)| {
        !host.is_empty() && (1..=5).contains(&port.len()) && port.bytes().all(|b| b.is_ascii_digit())
    })
}

pub fn run(
    env: &dyn Fn(&str) -> Option<String>,
    targets: &[String],
    session: Option<&str>,
    cwd: &dyn Fn() -> io::Result<PathBuf>,
    client: Option<&dyn Api>,
    err: &mut dyn Write,
) -> i32 {
    let own = env("MARSHELL_TAB_ID").filter(|v| !v.is_empty());
    let Some(session) = session.map(str::to_string).or_else(|| own.clone()) else {
        let _ = writeln!(err, "Outside a Marshell session, open needs --session <session>.");
        return 1;
    };
    let Some(client) = client else {
        let _ = writeln!(err, "{NOT_RUNNING}");
        return 1;
    };
    let snapshot = match client.sessions() {
        Ok(snapshot) => snapshot,
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            let _ = writeln!(err, "{NOT_RUNNING}");
            return 1;
        }
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return 1;
        }
    };
    let Some(row) = resolve(&snapshot, &session) else {
        let _ = writeln!(err, "No session {session}.");
        return 1;
    };
    let mut code = 0;
    for text in targets {
        let target = match target_for(text, cwd) {
            Ok(target) => target,
            Err(line) => {
                let _ = writeln!(err, "{line}");
                code = 1;
                continue;
            }
        };
        let req = OpenRequest {
            target,
            from: own.clone(),
        };
        match client.open(&row.tab_id, &req) {
            Opened::Done => {}
            Opened::Late(line) => {
                let _ = writeln!(err, "{line}");
            }
            Opened::Refused(line) => {
                let line = if line == "No such session." {
                    format!("No session {session}.")
                } else if looks_like_bare_address(text)
                    && (line.starts_with("No file ") || (line == BAD_PATH && has_port(text)))
                {
                    format!("No file {text}. For a web page, type the whole address: http://{text}")
                } else {
                    line
                };
                let _ = writeln!(err, "{line}");
                code = 1;
            }
        }
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::sample;
    use marshell_protocol::api::SessionState;
    use std::cell::RefCell;

    struct Fake {
        answers: RefCell<Vec<Opened>>,
        asked: RefCell<Vec<(String, OpenRequest)>>,
    }

    impl Fake {
        fn new(answers: Vec<Opened>) -> Fake {
            Fake {
                answers: RefCell::new(answers),
                asked: RefCell::new(vec![]),
            }
        }
    }

    impl Api for Fake {
        fn sessions(&self) -> io::Result<SessionsSnapshot> {
            Ok(SessionsSnapshot {
                sessions: vec![sample(2, "tab-b", SessionState::Running)],
                closed: vec![],
            })
        }
        fn open(&self, tab: &str, req: &OpenRequest) -> Opened {
            self.asked.borrow_mut().push((tab.to_string(), req.clone()));
            self.answers.borrow_mut().remove(0)
        }
    }

    struct Closed;
    impl Api for Closed {
        fn sessions(&self) -> io::Result<SessionsSnapshot> {
            Err(io::Error::from(io::ErrorKind::ConnectionRefused))
        }
        fn open(&self, _: &str, _: &OpenRequest) -> Opened {
            unreachable!()
        }
    }

    fn cwd() -> io::Result<PathBuf> {
        Ok(PathBuf::from(if cfg!(windows) { r"C:\w" } else { "/w" }))
    }

    /// Runs `open`; returns the exit code and what went to `err`.
    fn go(env: &[(&str, &str)], targets: &[&str], session: Option<&str>, client: Option<&dyn Api>) -> (i32, String) {
        let env = |name: &str| env.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string());
        let targets: Vec<String> = targets.iter().map(|s| s.to_string()).collect();
        let mut err = Vec::new();
        let code = run(&env, &targets, session, &cwd, client, &mut err);
        (code, String::from_utf8(err).unwrap())
    }

    #[cfg(windows)]
    #[test]
    fn target_for_makes_a_path_absolute_or_keeps_an_address() {
        let t = |text: &str, here: &str| target_for(text, &|| Ok(PathBuf::from(here)));
        assert_eq!(t("https://example.com/a", r"C:\w").unwrap(), "https://example.com/a");
        assert_eq!(t("HTTP://EXAMPLE.COM/", r"C:\w").unwrap(), "HTTP://EXAMPLE.COM/");
        assert_eq!(t("a.md", r"C:\w").unwrap(), r"C:\w\a.md");
        assert_eq!(t(r"..\x.md", r"C:\w\p").unwrap(), r"C:\w\x.md");
        assert_eq!(t(r".\x.md", r"C:\w").unwrap(), r"C:\w\x.md");
        assert_eq!(t(r"C:\x\a b.md", r"C:\w").unwrap(), r"C:\x\a b.md");
        assert_eq!(t("localhost:3000", r"C:\w").unwrap(), r"C:\w\localhost:3000");
        assert!(t("file:///C:/x", r"C:\w").unwrap().starts_with(r"C:\w\"));
        assert!(t("", r"C:\w").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn target_for_reads_a_rooted_or_drive_relative_path_as_windows_does() {
        let t = |text: &str, here: &str| target_for(text, &|| Ok(PathBuf::from(here))).unwrap();
        // A leading separator is the root of the current drive, not the current folder.
        assert_eq!(t(r"\x.md", r"C:\w\p"), r"C:\x.md");
        assert_eq!(t("/a/../x.md", r"D:\w"), r"D:\x.md");
        // `D:x.md` is a file in whatever folder drive D was last in, which nothing here knows: it goes as typed,
        // and the app refuses it, rather than opening another file of that name.
        assert_eq!(t("D:x.md", r"C:\w"), "D:x.md");
        // `..` stops at the root.
        assert_eq!(t(r"..\..\..\x.md", r"C:\w"), r"C:\x.md");
    }

    #[cfg(not(windows))]
    #[test]
    fn target_for_makes_a_path_absolute_or_keeps_an_address() {
        let t = |text: &str, here: &str| target_for(text, &|| Ok(PathBuf::from(here)));
        assert_eq!(t("HTTP://EXAMPLE.COM/", "/w").unwrap(), "HTTP://EXAMPLE.COM/");
        assert_eq!(t("../x.md", "/w/p").unwrap(), "/w/x.md");
        assert_eq!(t("localhost:3000", "/w").unwrap(), "/w/localhost:3000");
        assert!(t("", "/w").is_err());
    }

    #[test]
    fn open_run_outside_marshell_needs_a_session() {
        let fake = Fake::new(vec![]);
        let (code, err) = go(&[], &["a.md"], None, Some(&fake));
        assert_eq!(code, 1);
        assert_eq!(err, "Outside a Marshell session, open needs --session <session>.\n");
        assert!(fake.asked.borrow().is_empty());
    }

    #[test]
    fn open_run_uses_the_callers_session() {
        let fake = Fake::new(vec![Opened::Done, Opened::Done]);
        let (code, err) = go(
            &[("MARSHELL_TAB_ID", "tab-b")],
            &["https://x.dev/", "a.md"],
            None,
            Some(&fake),
        );
        assert_eq!((code, err.as_str()), (0, ""));
        let asked = fake.asked.borrow();
        assert_eq!(asked.len(), 2);
        assert_eq!(asked[0].0, "tab-b");
        assert_eq!(asked[0].1.target, "https://x.dev/");
        assert_eq!(asked[0].1.from.as_deref(), Some("tab-b"));
        // --session wins and takes a number.
        let fake = Fake::new(vec![Opened::Done]);
        let (code, _) = go(
            &[("MARSHELL_TAB_ID", "tab-z")],
            &["https://x.dev/"],
            Some("2"),
            Some(&fake),
        );
        assert_eq!(code, 0);
        assert_eq!(fake.asked.borrow()[0].0, "tab-b");
        assert_eq!(fake.asked.borrow()[0].1.from.as_deref(), Some("tab-z"));
    }

    #[test]
    fn open_run_reports_each_refusal_and_exits_1() {
        let fake = Fake::new(vec![
            Opened::Refused("That address can't be opened in the side pane.".into()),
            Opened::Done,
            Opened::Refused("Session 2 has closed.".into()),
        ]);
        let (code, err) = go(&[], &["http://a", "http://b", "http://c"], Some("tab-b"), Some(&fake));
        assert_eq!(code, 1);
        assert_eq!(
            err,
            "That address can't be opened in the side pane.\nSession 2 has closed.\n"
        );
        // An unknown session is named as the person gave it.
        let (code, err) = go(&[], &["http://a"], Some("nope"), Some(&Fake::new(vec![])));
        assert_eq!((code, err.as_str()), (1, "No session nope.\n"));
        let fake = Fake::new(vec![Opened::Refused("No such session.".into())]);
        let (_, err) = go(&[], &["http://a"], Some("2"), Some(&fake));
        assert_eq!(err, "No session 2.\n");
    }

    #[test]
    fn open_run_a_late_answer_exits_0() {
        let late = "Marshell hasn't answered yet. The tab opens when its window does.";
        let fake = Fake::new(vec![Opened::Late(late.into())]);
        let (code, err) = go(&[], &["http://a"], Some("2"), Some(&fake));
        assert_eq!((code, err), (0, format!("{late}\n")));
    }

    #[test]
    fn open_run_hints_at_the_whole_address() {
        let fake = Fake::new(vec![Opened::Refused("No file C:\\w\\localhost:3000.".into())]);
        let (code, err) = go(&[], &["localhost:3000"], Some("2"), Some(&fake));
        assert_eq!(code, 1);
        assert_eq!(
            err,
            "No file localhost:3000. For a web page, type the whole address: http://localhost:3000\n"
        );
        // On Windows the core refuses `C:\w\localhost:3000` before it looks for the file: a colon in a file name is
        // a stream, not a file. The person still gets the hint.
        let fake = Fake::new(vec![Opened::Refused(
            "That path can't be opened in the side pane.".into(),
        )]);
        let (code, err) = go(&[], &["localhost:3000"], Some("2"), Some(&fake));
        assert_eq!(code, 1);
        assert_eq!(
            err,
            "No file localhost:3000. For a web page, type the whole address: http://localhost:3000\n"
        );
        // A path with a separator or a drive letter, or a name with no dot or colon, gets the core's line.
        for text in ["docs/a.md", r"C:\x\a.md", "C:a.md", "README"] {
            let fake = Fake::new(vec![Opened::Refused("No file P.".into())]);
            let (_, err) = go(&[], &[text], Some("2"), Some(&fake));
            assert_eq!(err, "No file P.\n", "{text}");
        }
        // A colon that is no port isn't a host: the core's line.
        let fake = Fake::new(vec![Opened::Refused("No file javascript:alert(1).".into())]);
        let (_, err) = go(&[], &["javascript:alert(1)"], Some("2"), Some(&fake));
        assert_eq!(err, "No file javascript:alert(1).\n");
        // A path refused for another reason (a file in a network folder, `README.md` in `\\wsl.localhost\...`) is
        // no web page: only `host:port` gets the hint after that line.
        for text in ["README.md", "javascript:alert(1)", "a:b:3000"] {
            let fake = Fake::new(vec![Opened::Refused(BAD_PATH.into())]);
            let (_, err) = go(&[], &[text], Some("2"), Some(&fake));
            assert_eq!(err, format!("{BAD_PATH}\n"), "{text}");
        }
    }

    #[test]
    fn open_run_says_when_marshell_is_closed() {
        let (code, err) = go(&[], &["http://a"], Some("2"), None);
        assert_eq!((code, err.as_str()), (1, "Marshell isn't running.\n"));
        let (code, err) = go(&[], &["http://a"], Some("2"), Some(&Closed));
        assert_eq!((code, err.as_str()), (1, "Marshell isn't running.\n"));
    }
}
