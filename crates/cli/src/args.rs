//! The command line, parsed by hand: `marshell list [--json]`, `send <session> <text>`, `new [options]`,
//! `focus <session> [--start]`, `uninstall [--yes]`, `--help`, `--version`.

use marshell_protocol::api::SessionMode;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "Usage: marshell <command>

Commands:
  list [--json]       List the open sessions of the running app: number, name, agent, state, mode and folder.
                      --json prints them as JSON, with each session's tab id.
  send <session> <text>
                      Put <text> on the session's prompt line. Marshell never presses Enter. <session> is
                      a session number or a tab id.
  new [--agent <agent>] [--cwd <folder>] [--prompt <text>] [--mode <mode>] [--name <name>] [--start]
                      Start a session. --cwd defaults to the current folder. --prompt puts <text> on the
                      agent's prompt line once it is ready; Marshell never presses Enter. <mode> is one of
                      manual, plan, auto-edit, full-auto, bypass, dont-ask. --start starts Marshell first
                      when it is closed, without asking.
  focus <session> [--start]
                      Bring Marshell's window to the session. --start starts Marshell first when it is
                      closed, without asking.
  uninstall [--yes]   Take Marshell's hooks out of Claude Code's settings and remove the program
                      folder <home>/bin. Shows what changes first and asks; --yes skips the question.

Options:
  --help, -h          Show this text.
  --version, -V       Show the version.";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    List {
        json: bool,
    },
    Send {
        session: String,
        text: String,
    },
    New {
        agent: Option<String>,
        /// Absolute. `None` when `--cwd` was not given: the command then uses the CLI's own folder.
        cwd: Option<String>,
        prompt: Option<String>,
        mode: Option<SessionMode>,
        name: Option<String>,
        start: bool,
    },
    Focus {
        session: String,
        start: bool,
    },
    Uninstall {
        yes: bool,
    },
    Help,
    Version,
}

/// `argv` is the arguments after the program name. An `Err` is a usage error: the message, then the usage.
/// A relative `--cwd` is made absolute from the CLI's own folder.
pub fn parse(argv: &[String]) -> Result<Command, String> {
    parse_in(argv, &std::env::current_dir)
}

/// `parse`, with the way to read the CLI's own folder given. It is read only for a relative `--cwd`, so a command
/// that has no use for it works in a folder that can't be read.
pub fn parse_in(argv: &[String], current_dir: &dyn Fn() -> std::io::Result<PathBuf>) -> Result<Command, String> {
    let Some((first, rest)) = argv.split_first() else {
        return Err(USAGE.to_string());
    };
    match first.as_str() {
        "--help" | "-h" => Ok(Command::Help),
        "--version" | "-V" => Ok(Command::Version),
        "list" => {
            let mut json = false;
            for arg in rest {
                match arg.as_str() {
                    "--json" => json = true,
                    other => {
                        return Err(format!(
                            "Unknown argument `{other}` for list.

{USAGE}"
                        ));
                    }
                }
            }
            Ok(Command::List { json })
        }
        "send" => match rest {
            [session, text] => Ok(Command::Send {
                session: session.clone(),
                text: text.clone(),
            }),
            _ => Err(format!(
                "send takes a session and the text to put on its prompt line.

{USAGE}"
            )),
        },
        "new" => parse_new(rest, current_dir),
        "focus" => {
            let mut session = None;
            let mut start = false;
            for arg in rest {
                match arg.as_str() {
                    "--start" => start = true,
                    flag if flag.starts_with("--") => {
                        return Err(format!(
                            "Unknown argument `{flag}` for focus.

{USAGE}"
                        ));
                    }
                    other if session.is_none() => session = Some(other.to_string()),
                    _ => {
                        return Err(format!(
                            "focus takes one session.

{USAGE}"
                        ));
                    }
                }
            }
            match session {
                Some(session) => Ok(Command::Focus { session, start }),
                None => Err(format!(
                    "focus takes a session number or a tab id.

{USAGE}"
                )),
            }
        }
        "uninstall" => {
            let mut yes = false;
            for arg in rest {
                match arg.as_str() {
                    "--yes" | "-y" => yes = true,
                    other => return Err(format!("Unknown argument `{other}` for uninstall.\n\n{USAGE}")),
                }
            }
            Ok(Command::Uninstall { yes })
        }
        other => Err(format!("Unknown command `{other}`.\n\n{USAGE}")),
    }
}

fn parse_new(rest: &[String], current_dir: &dyn Fn() -> std::io::Result<PathBuf>) -> Result<Command, String> {
    let (mut agent, mut cwd, mut prompt, mut mode, mut name, mut start) = (None, None, None, None, None, false);
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next().cloned().ok_or_else(|| {
                format!(
                    "{flag} takes a value.

{USAGE}"
                )
            })
        };
        match arg.as_str() {
            "--agent" => agent = Some(value("--agent")?),
            "--cwd" => {
                let given = value("--cwd")?;
                let joined = if Path::new(&given).is_absolute() {
                    PathBuf::from(&given)
                } else {
                    let here = current_dir().map_err(|e| format!("Could not read the current folder: {e}"))?;
                    here.join(&given)
                };
                let absolute = std::path::absolute(&joined).unwrap_or(joined);
                cwd = Some(absolute.to_string_lossy().into_owned());
            }
            "--prompt" => prompt = Some(value("--prompt")?),
            "--name" => name = Some(value("--name")?),
            "--mode" => {
                let word = value("--mode")?;
                mode = Some(
                    serde_json::from_value::<SessionMode>(serde_json::Value::String(word.clone())).map_err(|_| {
                        format!(
                            "Unknown mode `{word}`: use manual, plan, auto-edit, full-auto, bypass or dont-ask.

{USAGE}"
                        )
                    })?,
                );
            }
            "--start" => start = true,
            other => {
                return Err(format!(
                    "Unknown argument `{other}` for new.

{USAGE}"
                ));
            }
        }
    }
    Ok(Command::New {
        agent,
        cwd,
        prompt,
        mode,
        name,
        start,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_arguments_is_a_usage_error() {
        assert_eq!(parse(&args(&[])), Err(USAGE.to_string()));
    }

    #[test]
    fn uninstall_asks_unless_told_not_to() {
        assert_eq!(parse(&args(&["uninstall"])), Ok(Command::Uninstall { yes: false }));
        assert_eq!(
            parse(&args(&["uninstall", "--yes"])),
            Ok(Command::Uninstall { yes: true })
        );
    }

    #[test]
    fn args_parse_list_and_send() {
        assert_eq!(parse(&args(&["list"])), Ok(Command::List { json: false }));
        assert_eq!(parse(&args(&["list", "--json"])), Ok(Command::List { json: true }));
        assert!(parse(&args(&["list", "--bogus"])).is_err());
        assert_eq!(
            parse(&args(&["send", "6", "hi"])),
            Ok(Command::Send {
                session: "6".into(),
                text: "hi".into()
            })
        );
        // The text is whatever follows the session, even when it looks like a flag.
        assert_eq!(
            parse(&args(&["send", "6", "--json"])),
            Ok(Command::Send {
                session: "6".into(),
                text: "--json".into()
            })
        );
        let e = parse(&args(&["send", "6"])).unwrap_err();
        assert!(e.contains("Usage:"), "{e}");
        assert!(parse(&args(&["send"])).is_err());
        assert!(parse(&args(&["send", "6", "a", "b"])).is_err());
    }

    #[test]
    fn args_parse_new_and_focus() {
        let here = std::env::temp_dir().join("app");
        let p = |list: &[&str]| parse_in(&args(list), &|| Ok(here.clone()));
        let abs = std::path::absolute(here.join("."))
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            p(&[
                "new", "--agent", "claude", "--cwd", ".", "--prompt", "x", "--mode", "plan", "--name", "n", "--start"
            ]),
            Ok(Command::New {
                agent: Some("claude".into()),
                cwd: Some(abs.clone()),
                prompt: Some("x".into()),
                mode: Some(SessionMode::Plan),
                name: Some("n".into()),
                start: true,
            })
        );
        assert_eq!(
            p(&["new"]),
            Ok(Command::New {
                agent: None,
                cwd: None,
                prompt: None,
                mode: None,
                name: None,
                start: false
            })
        );
        // An absolute folder is kept as it is, and a relative one is joined to the CLI's own folder.
        let other = std::env::temp_dir().join("other").to_string_lossy().into_owned();
        let Ok(Command::New { cwd, .. }) = p(&["new", "--cwd", &other]) else {
            panic!()
        };
        assert_eq!(cwd, Some(other));
        let Ok(Command::New { cwd, .. }) = p(&["new", "--cwd", "sub"]) else {
            panic!()
        };
        assert_eq!(
            cwd.map(std::path::PathBuf::from),
            Some(here.join("sub")).map(|p| std::path::absolute(&p).unwrap())
        );
        // Every mode by its protocol name.
        for (word, mode) in [
            ("manual", SessionMode::Manual),
            ("auto-edit", SessionMode::AutoEdit),
            ("full-auto", SessionMode::FullAuto),
            ("bypass", SessionMode::Bypass),
            ("dont-ask", SessionMode::DontAsk),
        ] {
            let Ok(Command::New { mode: got, .. }) = p(&["new", "--mode", word]) else {
                panic!("{word}")
            };
            assert_eq!(got, Some(mode));
        }
        assert_eq!(
            p(&["focus", "3", "--start"]),
            Ok(Command::Focus {
                session: "3".into(),
                start: true
            })
        );
        assert_eq!(
            p(&["focus", "tab-a"]),
            Ok(Command::Focus {
                session: "tab-a".into(),
                start: false
            })
        );
    }

    #[test]
    fn new_and_focus_usage_errors() {
        let here = std::env::temp_dir().join("app");
        for list in [
            &["new", "--mode", "sideways"][..],
            &["new", "--agent"],
            &["new", "--bogus"],
            &["new", "stray"],
            &["focus"],
            &["focus", "1", "2"],
            &["focus", "1", "--bogus"],
        ] {
            let e = parse_in(&args(list), &|| Ok(here.clone())).unwrap_err();
            assert!(e.contains("Usage:"), "{list:?}: {e}");
        }
        let e = parse_in(&args(&["new", "--mode", "sideways"]), &|| Ok(here.clone())).unwrap_err();
        assert!(e.contains("sideways") && e.contains("plan"), "{e}");
    }

    #[test]
    fn the_current_folder_is_read_only_for_a_relative_cwd() {
        let unreadable = || Err(std::io::Error::other("gone"));
        for list in [
            &["list"][..],
            &["--version"],
            &["--help"],
            &["send", "1", "hi"],
            &["new", "--agent", "codex"],
        ] {
            assert!(parse_in(&args(list), &unreadable).is_ok(), "{list:?}");
        }
        let absolute = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(parse_in(&args(&["new", "--cwd", &absolute]), &unreadable).is_ok());
        let e = parse_in(&args(&["new", "--cwd", "sub"]), &unreadable).unwrap_err();
        assert_eq!(e, "Could not read the current folder: gone");
    }

    #[test]
    fn help_and_version() {
        assert_eq!(parse(&args(&["--help"])), Ok(Command::Help));
        assert_eq!(parse(&args(&["--version"])), Ok(Command::Version));
    }

    #[test]
    fn unknown_arguments_are_usage_errors() {
        let e = parse(&args(&["uninstall", "--bogus"])).unwrap_err();
        assert!(e.contains("--bogus") && e.contains("Usage:"), "{e}");
        assert!(parse(&args(&["frobnicate"])).is_err());
    }
}
