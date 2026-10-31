use marshell_protocol::hook::HookRole;

/// The command line of one bridge call: `<role> --agent <name> [--event <Name>] [--tag <marker>]`, and for the
/// `statusline` role `--no-chain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub role: HookRole,
    pub agent: String,
    /// The marker the installer puts on the command, so it can find its own entries. Unused here.
    pub tag: Option<String>,
    /// The hook's event name, for an agent whose payload doesn't carry one (Antigravity's).
    pub event: Option<String>,
    /// `statusline` only: print only Marshell's own line, never run or read the chain. A distro has no chain file.
    pub no_chain: bool,
}

/// `argv` is the arguments after the program name.
pub fn parse(argv: &[String]) -> Result<Args, String> {
    let mut it = argv.iter();
    let role = it.next().ok_or("no role given")?;
    let role = HookRole::parse(role).ok_or_else(|| format!("unknown role {role:?}"))?;
    let mut agent = None;
    let mut tag = None;
    let mut event = None;
    let mut no_chain = false;
    while let Some(flag) = it.next() {
        if flag == "--no-chain" && role == HookRole::Statusline {
            no_chain = true;
            continue;
        }
        let slot = match flag.as_str() {
            "--agent" => &mut agent,
            "--tag" => &mut tag,
            "--event" => &mut event,
            other => return Err(format!("unknown argument {other:?}")),
        };
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        if flag == "--event" {
            if slot.is_some() {
                return Err("--event given twice".into());
            }
            if value.is_empty() || !value.chars().all(|c| c.is_ascii_alphabetic()) {
                return Err(format!("--event {value:?} is not an identifier"));
            }
        }
        *slot = Some(value.clone());
    }
    let agent = agent.filter(|a| !a.is_empty()).ok_or("missing --agent")?;
    Ok(Args {
        role,
        agent,
        tag,
        event,
        no_chain,
    })
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

    #[test]
    fn parses_an_event() {
        let a = parse(&v(&[
            "event",
            "--agent",
            "antigravity",
            "--event",
            "PreInvocation",
            "--tag",
            "mshl1",
        ]))
        .unwrap();
        assert_eq!(a.event.as_deref(), Some("PreInvocation"));
        assert_eq!(a.tag.as_deref(), Some("mshl1"));
        assert_eq!(parse(&v(&["event", "--agent", "claude"])).unwrap().event, None);
    }

    #[test]
    fn no_chain_is_a_flag_of_the_status_line_only() {
        let line = parse(&v(&["statusline", "--agent", "claude", "--tag", "mshl1", "--no-chain"])).unwrap();
        assert!(line.no_chain);
        assert!(!parse(&v(&["statusline", "--agent", "claude"])).unwrap().no_chain);
        assert!(parse(&v(&["event", "--agent", "claude", "--no-chain"])).is_err());
    }

    #[test]
    fn an_event_needs_a_value() {
        assert!(parse(&v(&["event", "--agent", "antigravity", "--event"])).is_err());
        assert!(parse(&v(&["event", "--agent", "antigravity", "--event", ""])).is_err());
    }

    #[test]
    fn an_event_that_is_not_an_identifier_is_an_error() {
        for bad in [
            "Pre Tool", "Pre-Tool", "Pre_Tool", "Stop1", "Stöp", "Stop\n", "--tag", "\"Stop\"", "%Stop%",
        ] {
            assert!(
                parse(&v(&["event", "--agent", "antigravity", "--event", bad])).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_second_event_is_an_error() {
        let argv = v(&["event", "--agent", "antigravity", "--event", "Stop", "--event", "Stop"]);
        assert!(parse(&argv).is_err());
    }
}
