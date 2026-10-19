//! The command line, parsed by hand: `marshell uninstall [--yes]`, `--help`, `--version`.

pub const USAGE: &str = "Usage: marshell <command>

Commands:
  uninstall [--yes]   Take Marshell's hooks out of Claude Code's settings and remove the program
                      folder <home>/bin. Shows what changes first and asks; --yes skips the question.

Options:
  --help, -h          Show this text.
  --version, -V       Show the version.";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
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
