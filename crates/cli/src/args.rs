//! The command line, parsed by hand: `marshell list [--json]`, `send <session> <text>`, `uninstall [--yes]`,
//! `--help`, `--version`.

pub const USAGE: &str = "Usage: marshell <command>

Commands:
  list [--json]       List the open sessions of the running app: number, name, agent, state, mode and folder.
                      --json prints them as JSON, with each session's tab id.
  send <session> <text>
                      Put <text> on the session's prompt line. Marshell never presses Enter. <session> is
                      a session number or a tab id.
  uninstall [--yes]   Take Marshell's hooks out of Claude Code's settings and remove the program
                      folder <home>/bin. Shows what changes first and asks; --yes skips the question.

Options:
  --help, -h          Show this text.
  --version, -V       Show the version.";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    List { json: bool },
    Send { session: String, text: String },
    Uninstall { yes: bool },
    Help,
    Version,
}

/// `argv` is the arguments after the program name. An `Err` is a usage error: the message, then the usage.
pub fn parse(argv: &[String]) -> Result<Command, String> {
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
