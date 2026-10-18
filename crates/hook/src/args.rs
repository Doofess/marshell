use marshell_protocol::hook::HookRole;

/// The command line of one bridge call: `<role> --agent <name> [--tag <marker>]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub role: HookRole,
    pub agent: String,
    /// The marker the installer puts on the command, so it can find its own entries. Unused here.
    pub tag: Option<String>,
}

/// `argv` is the arguments after the program name.
pub fn parse(argv: &[String]) -> Result<Args, String> {
    let mut it = argv.iter();
    let role = it.next().ok_or("no role given")?;
    let role = HookRole::parse(role).ok_or_else(|| format!("unknown role {role:?}"))?;
    let mut agent = None;
    let mut tag = None;
    while let Some(flag) = it.next() {
        let slot = match flag.as_str() {
            "--agent" => &mut agent,
            "--tag" => &mut tag,
            other => return Err(format!("unknown argument {other:?}")),
        };
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        *slot = Some(value.clone());
    }
    let agent = agent.filter(|a| !a.is_empty()).ok_or("missing --agent")?;
    Ok(Args { role, agent, tag })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_role_agent_and_tag() {
        let a = parse(&v(&["permission", "--agent", "claude", "--tag", "mshl1"])).unwrap();
        assert_eq!(a.role, HookRole::Permission);
        assert_eq!(a.agent, "claude");
        assert_eq!(a.tag.as_deref(), Some("mshl1"));
        let a = parse(&v(&["event", "--agent", "codex"])).unwrap();
        assert_eq!(a.tag, None);
    }

    #[test]
    fn unknown_role_is_an_error() {
        assert!(parse(&v(&["bogus", "--agent", "claude"])).is_err());
        assert!(parse(&v(&[])).is_err());
    }

    #[test]
    fn missing_agent_is_an_error() {
        assert!(parse(&v(&["event"])).is_err());
        assert!(parse(&v(&["event", "--agent"])).is_err());
        assert!(parse(&v(&["event", "--agent", ""])).is_err());
        assert!(parse(&v(&["event", "--agent", "claude", "--what"])).is_err());
    }
}
