//! What a session knows about the command in front of its shell.

use crate::{listen, secrets};

/// Whether the command prints secrets and whether its listener is transient. It never holds the command line itself.
#[derive(Clone, Default)]
pub struct Watch {
    /// The command prints secrets: its output is not recorded.
    pub secret: bool,
    /// `secrets::shown` of the command, set only when `secret` is. Names the command in the recording's marker.
    pub label: String,
    /// The command is a login (`listen::quiet_line`), so its listener makes no chip.
    pub quiet_ports: bool,
}

impl Watch {
    /// Whether `other` differs from this watch in anything a reader of it would act on.
    pub fn differs(&self, other: &Watch) -> bool {
        self.secret != other.secret || self.label != other.label || self.quiet_ports != other.quiet_ports
    }
}

/// The watch for a command line as typed in a shell.
pub fn watch_for(line: &str) -> Watch {
    let secret = secrets::prints_secrets(line);
    Watch {
        secret,
        label: if secret { secrets::shown(line) } else { String::new() },
        quiet_ports: listen::quiet_line(line),
    }
}

/// The watch for a process, from its argument vector.
pub fn watch_for_argv(argv: &[String]) -> Watch {
    let secret = secrets::prints_secrets_words(argv);
    Watch {
        secret,
        label: if secret {
            secrets::shown_words(argv)
        } else {
            String::new()
        },
        quiet_ports: listen::quiet_argv(argv),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_command_is_hidden_with_its_label() {
        let w = watch_for("op read op://v/i/f");
        assert!(w.secret);
        // The label is `secrets::shown`: the program and its first two words, then an ellipsis. An `op://` reference
        // names a secret and is not one.
        assert_eq!(w.label, "op read op://v/i/f …");
        assert_eq!(w.label, secrets::shown("op read op://v/i/f"));
        // A secret flag's value never reaches it.
        let flagged = watch_for("op read op://v/i/f --token hunter2");
        assert!(
            flagged.secret && !flagged.label.contains("hunter2"),
            "{}",
            flagged.label
        );
        assert!(!w.quiet_ports);
    }

    #[test]
    fn a_plain_dev_server_is_neither_secret_nor_quiet() {
        let w = watch_for("npx vite");
        assert!(!w.secret && !w.quiet_ports);
        assert_eq!(w.label, "");
        assert!(!w.differs(&w.clone()));
        assert!(w.differs(&watch_for("vercel login")));
    }

    #[test]
    fn a_watch_from_an_argument_vector_sees_secret_and_login_commands() {
        let argv = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let w = watch_for_argv(&argv(&["C:\\Program Files\\GitHub CLI\\gh.exe", "auth", "token"]));
        assert!(w.secret);
        assert_eq!(w.label, "gh auth token …");
        assert!(watch_for_argv(&argv(&["gh", "auth", "login"])).quiet_ports);
        assert!(!watch_for_argv(&argv(&["vercel", "dev"])).secret);
    }
}
