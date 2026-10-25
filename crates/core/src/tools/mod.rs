//! Tool profiles: how Marshell recognises a developer tool (`vercel`, `terraform`, `gcloud`, ...) in a shell
//! and what it reads from the tool's output.

pub mod listen;
pub mod matcher;
pub mod own;
pub mod profile;
pub mod secrets;

pub use marshell_protocol::tools::PortChip;
pub use marshell_protocol::tools::{
    LaunchPreset, NeedsYouPattern, OwnToolProblem, PortPattern, PresetField, PromptKind, ToolProfile,
};
pub use matcher::{Matcher, ToolEvent, normalize_port};
pub use profile::{Classified, CompiledNeedsYou, Profile, Tools, validate_own};

use std::sync::Arc;

/// What a session knows about the command in front of its shell: the profiles whose patterns apply, whether the
/// command prints secrets, and whether its port lines are transient. It never holds the command line itself.
#[derive(Clone, Default)]
pub struct Watch {
    /// The launcher profiles that apply, then the tool's own.
    pub profiles: Vec<Arc<Profile>>,
    /// The id of the tool's profile, if one matched.
    pub tool: Option<String>,
    /// The command prints secrets: its output is neither matched nor recorded.
    pub secret: bool,
    /// `secrets::shown` of the command, set only when `secret` is. Names the command in the recording's marker.
    pub label: String,
    /// A profile's `quiet_ports` prefix matches the command, so its port lines make no chip.
    pub quiet_ports: bool,
}

impl Watch {
    /// Whether `other` differs from this watch in anything a reader of it would act on.
    pub fn differs(&self, other: &Watch) -> bool {
        self.tool != other.tool
            || self.secret != other.secret
            || self.label != other.label
            || self.quiet_ports != other.quiet_ports
            || self.profiles.len() != other.profiles.len()
            || self
                .profiles
                .iter()
                .zip(&other.profiles)
                .any(|(a, b)| !Arc::ptr_eq(a, b))
    }
}

/// The watch for a command line as typed in a shell.
pub fn watch_for(tools: &Tools, line: &str) -> Watch {
    let secret = secrets::prints_secrets(line);
    let label = if secret { secrets::shown(line) } else { String::new() };
    watch_of(tools.classify_line(line), secret, label)
}

/// The watch for a process, from its argument vector.
pub fn watch_for_argv(tools: &Tools, argv: &[String]) -> Watch {
    let classified = tools.classify_argv(argv);
    // A node or python child is named by its script; `rest` then holds the tool's own command word.
    let (secret, label) = if secrets::prints_secrets_words(argv) {
        (true, secrets::shown_words(argv))
    } else if classified.tool.is_some() && secrets::prints_secrets(&classified.rest) {
        (true, secrets::shown(&classified.rest))
    } else {
        (false, String::new())
    };
    watch_of(classified, secret, label)
}

fn watch_of(classified: Classified, secret: bool, label: String) -> Watch {
    let after_program = classified
        .rest
        .split_once(' ')
        .map_or("", |(_, after)| after)
        .to_lowercase();
    let quiet_ports = classified.profiles.iter().any(|p| {
        p.quiet_ports.iter().any(|prefix| {
            let prefix = prefix.trim().to_lowercase();
            !prefix.is_empty()
                && after_program
                    .strip_prefix(&prefix)
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
        })
    });
    Watch {
        profiles: classified.profiles,
        tool: classified.tool,
        secret,
        label,
        quiet_ports,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_watch_names_the_tool_and_marks_secret_commands() {
        let tools = Tools::shipped();
        let w = watch_for(&tools, "npx supabase status -o env");
        assert_eq!(w.tool.as_deref(), Some("supabase"));
        assert_eq!(w.profiles.len(), 2);
        assert!(w.secret);
        assert_eq!(w.label, "supabase status …");
        let w = watch_for(&tools, "vercel dev --port 3000");
        assert_eq!(w.tool.as_deref(), Some("vercel"));
        assert!(!w.secret);
        assert_eq!(w.label, "");
        assert!(!w.quiet_ports);
        let w = watch_for(&tools, "git status");
        assert!(w.profiles.is_empty() && w.tool.is_none() && !w.secret);
    }

    #[test]
    fn a_quiet_ports_prefix_matches_after_the_program_word() {
        let tools = Tools::shipped();
        assert!(watch_for(&tools, "vercel login").quiet_ports);
        assert!(watch_for(&tools, "npx vercel login --github").quiet_ports);
        assert!(!watch_for(&tools, "vercel loginx").quiet_ports);
        assert!(!watch_for(&tools, "vercel dev").quiet_ports);
    }

    #[test]
    fn a_watch_from_an_argument_vector_sees_secret_commands() {
        let tools = Tools::shipped();
        let w = watch_for_argv(
            &tools,
            &argv(&["C:\\Program Files\\GitHub CLI\\gh.exe", "auth", "token"]),
        );
        assert_eq!(w.tool.as_deref(), Some("gh"));
        assert!(w.secret);
        assert_eq!(w.label, "gh auth token …");
        let w = watch_for_argv(
            &tools,
            &argv(&["node", "C:\\x\\node_modules\\supabase\\bin\\main.js", "status"]),
        );
        assert_eq!(w.tool.as_deref(), Some("supabase"));
        assert!(w.secret);
        let w = watch_for_argv(&tools, &argv(&["vercel", "dev"]));
        assert!(!w.secret);
        assert!(!w.differs(&w.clone()));
        assert!(w.differs(&Watch::default()));
    }
}
