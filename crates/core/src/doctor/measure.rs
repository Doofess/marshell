//! The Doctor's MCP cost line and failure items, from what "Measure MCP servers" listed. Pure: no I/O.

use marshell_protocol::doctor::{DoctorGroup, DoctorItem};

use super::checks::display_name;
use super::ext::{
    CommandState, CostRule, Inventory, Launch, Listing, NotMeasured, Origin, Server, Switch, Transport, fixability,
    item_key, tokens,
};
use super::probe::{Failure, Listed, Tool};
use crate::config::hash::hash_bytes;

/// One server's measurement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measured {
    pub agent: &'static str,
    /// 7c1's `item_key` of the server.
    pub key: String,
    pub fingerprint: String,
    pub outcome: Result<Listed, Failure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurements {
    pub at_ms: u64,
    pub servers: Vec<Measured>,
}

/// The first 16 hex digits of `hash_bytes` over the launch's `Debug` text. A launch that changed makes the old
/// measurement stale.
pub fn fingerprint(launch: &Launch) -> String {
    hash_bytes(format!("{launch:?}").as_bytes())[..16].to_string()
}

/// `s` cut to `cap` characters, in characters.
fn cut_len(s: &str, cap: Option<usize>) -> usize {
    let n = s.chars().count();
    cap.map_or(n, |c| n.min(c))
}

fn first_line(s: &str) -> &str {
    s.split('\n').next().unwrap_or("")
}

/// A tool's full definition: prefix, name, description cut at `desc_cap`, input schema.
fn tool_full(prefix: &str, desc_cap: Option<usize>, t: &Tool) -> usize {
    prefix.chars().count() + t.name.chars().count() + cut_len(&t.description, desc_cap) + t.schema_chars
}

/// The characters this rule loads up front for these tools. `FullUnder` is decided by the caller, which passes `full`.
pub fn server_chars(rule: &CostRule, listed: &Listed, full: bool) -> usize {
    let counted = listed
        .tools
        .iter()
        .filter(|t| rule.only.as_ref().is_none_or(|only| only.contains(&t.name)) && !rule.except.contains(&t.name));
    let always = |t: &Tool| {
        rule.always_load_meta
            .is_some_and(|k| t.meta_true.iter().any(|m| m == k))
    };
    // By name, or in full when the tool asks to always load.
    let by_name = |prefix: &str, t: &Tool| {
        if always(t) {
            tool_full(prefix, None, t)
        } else {
            prefix.chars().count() + t.name.chars().count()
        }
    };
    let instructions = |cap: Option<usize>| cut_len(&listed.instructions, cap);
    match &rule.listing {
        Listing::Full { prefix, desc_cap } => counted.map(|t| tool_full(prefix, *desc_cap, t)).sum(),
        Listing::Names {
            prefix,
            instructions_cap,
        } => instructions(*instructions_cap) + counted.map(|t| by_name(prefix, t)).sum::<usize>(),
        Listing::Namespace { namespace, cap } => {
            namespace.chars().count()
                + cut_len(first_line(&listed.instructions), Some(*cap))
                + counted
                    .filter(|t| always(t))
                    .map(|t| tool_full("", None, t))
                    .sum::<usize>()
        }
        Listing::FirstLines { heading, cap } => {
            heading.chars().count()
                + counted
                    .map(|t| {
                        if always(t) {
                            tool_full("", None, t)
                        } else {
                            t.name.chars().count() + cut_len(first_line(&t.description), Some(*cap))
                        }
                    })
                    .sum::<usize>()
        }
        Listing::NamesOnly => counted.map(|t| by_name("", t)).sum(),
        Listing::FullUnder {
            prefix,
            desc_cap,
            instructions_cap,
            ..
        } => {
            instructions(*instructions_cap)
                + if full {
                    counted.map(|t| tool_full(prefix, *desc_cap, t)).sum::<usize>()
                } else {
                    counted.map(|t| by_name(prefix, t)).sum::<usize>()
                }
        }
    }
}

fn why(f: &Failure) -> String {
    match f {
        Failure::CantStart => "its command couldn't start".to_string(),
        Failure::Exited(Some(n)) => format!("it exited with code {n} before answering"),
        Failure::Exited(None) => "it ended before answering".to_string(),
        Failure::NoAnswer(n) => format!("it didn't answer within {n} s"),
        Failure::NotMcp => "what it printed wasn't MCP".to_string(),
        Failure::Refused => "it refused to start a session".to_string(),
    }
}

pub(crate) fn items(inv: &Inventory, measured: Option<&Measurements>, out: &mut Vec<(DoctorItem, Option<Switch>)>) {
    let Some(measured) = measured else { return };
    let cli = display_name(inv.agent);
    let mut active: Vec<&Server> = inv
        .servers
        .iter()
        .filter(|s| !s.off && s.origin != Origin::Bundled)
        .collect();
    active.sort_by(|a, b| (&a.name, &a.file).cmp(&(&b.name, &b.file)));
    let mk = |id: String, group: DoctorGroup, title: String, detail: String, path: Option<String>| DoctorItem {
        id,
        agent: Some(inv.agent.to_string()),
        distro: inv.distro.clone(),
        group,
        title,
        detail,
        fixable: false,
        cannot_fix: None,
        path,
        open_agents: false,
        unchecked: false,
    };

    if let Some(distro) = &inv.distro {
        if !active.is_empty() {
            out.push((
                mk(
                    format!("mcpcost.{}@{distro}", inv.agent),
                    DoctorGroup::Info,
                    format!("{cli}'s MCP servers in {distro} weren't measured"),
                    format!("Not measured: in {distro}. Measuring runs on Windows only."),
                    None,
                ),
                None,
            ));
        }
        return;
    }

    let (mut remote, mut not_approved, mut added) = (0usize, 0usize, 0usize);
    // Each server that was measured and is up to date, with its listing.
    let mut ok: Vec<(&Server, &Listed)> = Vec::new();
    for s in active {
        // 7c1 already reports these.
        if matches!(
            &s.transport,
            Transport::Nothing
                | Transport::Stdio {
                    state: CommandState::Missing | CommandState::NotOnPath,
                    ..
                }
        ) {
            continue;
        }
        let launch = match (&s.transport, &s.launch) {
            (Transport::Remote, _) | (_, Err(NotMeasured::Remote)) => {
                remote += 1;
                continue;
            }
            (_, Err(NotMeasured::NotApproved)) => {
                not_approved += 1;
                continue;
            }
            (_, Ok(l)) => l,
        };
        let key = item_key(&s.file, &s.origin, &s.name);
        let fp = fingerprint(launch);
        let Some(m) = measured
            .servers
            .iter()
            .find(|m| m.agent == inv.agent && m.key == key && m.fingerprint == fp)
        else {
            added += 1;
            continue;
        };
        match &m.outcome {
            Ok(listed) => ok.push((s, listed)),
            Err(failure) => {
                let mut item = mk(
                    format!("mcpfail.{}.{key}", inv.agent),
                    DoctorGroup::Broken,
                    format!("{cli}'s MCP server {} didn't start", s.name),
                    format!(
                        "When Marshell started it to measure it, {}. A CLI started from your shell can have an environment Marshell's doesn't.",
                        why(failure)
                    ),
                    Some(s.file.display().to_string()),
                );
                item.unchecked = true;
                let switch = fixability(&mut item, &s.switch, cli, "server", &s.name);
                out.push((item, switch));
            }
        }
    }

    // FullUnder: every such server counts in full while their full cost stays under `percent`% of 200,000 tokens.
    let full_sum: usize = ok
        .iter()
        .filter(|(s, _)| matches!(s.cost.listing, Listing::FullUnder { .. }))
        .map(|(s, l)| server_chars(&s.cost, l, true))
        .sum();
    let in_full = ok.iter().find_map(|(s, _)| match s.cost.listing {
        Listing::FullUnder { percent, .. } => Some(tokens(full_sum) < percent as usize * 2000),
        _ => None,
    });
    let in_full = in_full.unwrap_or(false);

    let mut costs: Vec<(usize, &str)> = ok
        .iter()
        .map(|(s, l)| (server_chars(&s.cost, l, in_full), s.name.as_str()))
        .collect();
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut parts = Vec::new();
    if remote > 0 {
        parts.push(plural(remote, "remote server", "remote servers"));
    }
    if not_approved > 0 {
        parts.push(format!("{not_approved} that {cli} hasn't approved"));
    }
    if added > 0 {
        parts.push(format!("{added} added since the measurement"));
    }
    if costs.is_empty() && parts.is_empty() {
        return;
    }
    let mut sentences = Vec::new();
    let title = if costs.is_empty() {
        format!("{cli}'s MCP servers weren't measured")
    } else {
        let total: usize = costs.iter().map(|c| c.0).sum();
        costs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        let top: Vec<String> = costs
            .iter()
            .take(3)
            .map(|(c, n)| format!("{n} ({})", tokens(*c)))
            .collect();
        sentences.push(format!("{} measured. The largest: {}.", costs.len(), top.join(", ")));
        format!("{cli}'s MCP servers add about {} tokens to each session", tokens(total))
    };
    if !parts.is_empty() {
        sentences.push(format!("Not measured: {}.", parts.join(", ")));
    }
    if let Some(note) = inv.cost_note {
        sentences.push(note.to_string());
    }
    out.push((
        mk(
            format!("mcpcost.{}", inv.agent),
            DoctorGroup::Info,
            title,
            sentences.join(" "),
            None,
        ),
        None,
    ));
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::doctor::ext::{Caps, Expand, Hints, Inherit, NoSwitch, UsedAs};

    fn tool(name: &str, desc: &str, schema: usize) -> Tool {
        Tool {
            name: name.into(),
            description: desc.into(),
            schema_chars: schema,
            meta_true: vec![],
        }
    }

    fn listed() -> Listed {
        Listed {
            instructions: "Line one\nLine two".into(),
            tools: vec![tool("alpha", "First.\nSecond.", 10), tool("beta", "", 2)],
        }
    }

    fn rule(listing: Listing) -> CostRule {
        CostRule {
            listing,
            always_load_meta: None,
            only: None,
            except: vec![],
        }
    }

    #[test]
    #[allow(clippy::identity_op)] // The sums are written as the brief states them.
    fn server_chars_by_listing() {
        let l = listed();
        let names = rule(Listing::Names {
            prefix: "mcp__s__".into(),
            instructions_cap: None,
        });
        assert_eq!(
            server_chars(&names, &l, false),
            "mcp__s__alpha".len() + "mcp__s__beta".len() + 17
        );
        let ns = rule(Listing::Namespace {
            namespace: "mcp__s".into(),
            cap: 4,
        });
        assert_eq!(server_chars(&ns, &l, false), 6 + 4);
        let fl = rule(Listing::FirstLines {
            heading: "s".into(),
            cap: 3,
        });
        assert_eq!(server_chars(&fl, &l, false), 1 + (5 + 3) + (4 + 0));
        let full = rule(Listing::Full {
            prefix: "".into(),
            desc_cap: Some(5),
        });
        assert_eq!(server_chars(&full, &l, false), (5 + 5 + 10) + (4 + 0 + 2));
        assert_eq!(server_chars(&rule(Listing::NamesOnly), &l, false), 9);
    }

    #[test]
    fn only_except_and_always_load() {
        let l = listed();
        let mut r = rule(Listing::NamesOnly);
        r.only = Some(vec!["alpha".into()]);
        assert_eq!(server_chars(&r, &l, false), 5);
        r.only = None;
        r.except = vec!["alpha".into()];
        assert_eq!(server_chars(&r, &l, false), 4);
        let mut l2 = listed();
        l2.tools[0].meta_true = vec!["anthropic/alwaysLoad".into()];
        let mut r = rule(Listing::Names {
            prefix: "p_".into(),
            instructions_cap: Some(0),
        });
        r.always_load_meta = Some("anthropic/alwaysLoad");
        // alpha in full: prefix + name + description + schema; beta by name.
        assert_eq!(server_chars(&r, &l2, false), (2 + 5 + 14 + 10) + (2 + 4));
    }

    fn launch(cmd: &str) -> Launch {
        Launch {
            command: cmd.into(),
            args: vec![],
            env: vec![],
            inherit: Inherit::All,
            set: vec![],
            cwd: None,
            expand: Expand::None,
            timeout_secs: 20,
        }
    }

    fn server(name: &str, transport: Transport, launch: Result<Launch, NotMeasured>, cost: CostRule) -> Server {
        Server {
            name: name.into(),
            file: PathBuf::from("C:/h/.claude.json"),
            origin: Origin::User,
            transport,
            off: false,
            switch: Err(NoSwitch::ClaudeUser),
            launch,
            cost,
            used_as: UsedAs::Unknown,
        }
    }

    fn found() -> Transport {
        Transport::Stdio {
            command: "x".into(),
            state: CommandState::Found,
        }
    }

    fn inv(agent: &'static str, servers: Vec<Server>) -> Inventory {
        Inventory {
            agent,
            distro: None,
            found: true,
            caps: Caps::default(),
            servers,
            skills: vec![],
            unreadable: vec![],
            hints: Hints::default(),
            cost_note: None,
        }
    }

    /// A startable server, listed by name only.
    fn sized(name: &str) -> Server {
        server(name, found(), Ok(launch(name)), rule(Listing::NamesOnly))
    }

    fn remote(name: &str) -> Server {
        server(
            name,
            Transport::Remote,
            Err(NotMeasured::Remote),
            CostRule::names_only(),
        )
    }

    /// The server's measurement: one tool whose name has `n` characters.
    fn measured_of(s: &Server, n: usize) -> Measured {
        Measured {
            agent: "claude",
            key: item_key(&s.file, &s.origin, &s.name),
            fingerprint: fingerprint(s.launch.as_ref().unwrap()),
            outcome: Ok(Listed {
                instructions: String::new(),
                tools: vec![tool(&"t".repeat(n), "", 0)],
            }),
        }
    }

    fn measurements(servers: Vec<Measured>) -> Measurements {
        Measurements { at_ms: 1, servers }
    }

    fn run(i: &Inventory, m: Option<&Measurements>) -> Vec<(DoctorItem, Option<Switch>)> {
        let mut out = vec![];
        items(i, m, &mut out);
        out
    }

    #[test]
    fn cost_line_copy() {
        let (a, b, c) = (sized("a"), sized("b"), sized("c"));
        let unapproved = server("u", found(), Err(NotMeasured::NotApproved), CostRule::names_only());
        let m = measurements(vec![measured_of(&a, 400), measured_of(&b, 80), measured_of(&c, 40)]);
        let i = inv("claude", vec![a, b, c, remote("r"), unapproved]);
        let out = run(&i, Some(&m));
        assert_eq!(out.len(), 1);
        let it = &out[0].0;
        assert_eq!(it.id, "mcpcost.claude");
        assert_eq!(it.group, DoctorGroup::Info);
        assert_eq!(
            it.title,
            "Claude Code's MCP servers add about 130 tokens to each session"
        );
        assert_eq!(
            it.detail,
            "3 measured. The largest: a (100), b (20), c (10). Not measured: 1 remote server, 1 that Claude Code hasn't approved."
        );
        // Only remote servers, and a cost note.
        let mut i = inv("codex", vec![remote("r1"), remote("r2")]);
        i.cost_note = Some("Codex counts it its own way.");
        let out = run(&i, Some(&measurements(vec![])));
        assert_eq!(out[0].0.title, "Codex's MCP servers weren't measured");
        assert_eq!(
            out[0].0.detail,
            "Not measured: 2 remote servers. Codex counts it its own way."
        );
        // Singular, and the order of the parts.
        let i = inv("qwen", vec![remote("r"), sized("n")]);
        let out = run(&i, Some(&measurements(vec![])));
        assert_eq!(
            out[0].0.detail,
            "Not measured: 1 remote server, 1 added since the measurement."
        );
    }

    #[test]
    fn stale_measurement_counts_as_added_since() {
        let s = sized("a");
        let mut m = measured_of(&s, 10);
        m.fingerprint = "0000000000000000".into();
        let out = run(&inv("claude", vec![s]), Some(&measurements(vec![m])));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0.id, "mcpcost.claude");
        assert_eq!(out[0].0.detail, "Not measured: 1 added since the measurement.");
    }

    #[test]
    fn failure_items() {
        let cases = [
            (Failure::CantStart, "its command couldn't start"),
            (Failure::Exited(Some(3)), "it exited with code 3 before answering"),
            (Failure::Exited(None), "it ended before answering"),
            (Failure::NoAnswer(20), "it didn't answer within 20 s"),
            (Failure::NotMcp, "what it printed wasn't MCP"),
            (Failure::Refused, "it refused to start a session"),
        ];
        for fixable in [false, true] {
            for (failure, why) in cases.clone() {
                let mut s = sized("a");
                if fixable {
                    s.switch = Ok(Switch::CodexServer {
                        file: PathBuf::from("C:/h/c.toml"),
                        name: "a".into(),
                    });
                }
                let mut m = measured_of(&s, 1);
                m.outcome = Err(failure);
                let key = item_key(&s.file, &s.origin, &s.name);
                let i = inv("claude", vec![s]);
                let out = run(&i, Some(&measurements(vec![m])));
                let fail: Vec<_> = out.iter().filter(|(it, _)| it.id.starts_with("mcpfail.")).collect();
                assert_eq!(fail.len(), 1);
                let it = &fail[0].0;
                assert_eq!(it.id, format!("mcpfail.claude.{key}"));
                assert_eq!(it.group, DoctorGroup::Broken);
                assert_eq!(it.title, "Claude Code's MCP server a didn't start");
                let base = format!(
                    "When Marshell started it to measure it, {why}. A CLI started from your shell can have an environment Marshell's doesn't."
                );
                assert!(it.unchecked);
                assert_eq!(it.fixable, fixable);
                if fixable {
                    assert_eq!(it.detail, format!("{base} The fix turns it off and doesn't delete it."));
                    assert!(fail[0].1.is_some());
                } else {
                    assert_eq!(it.detail, base);
                    assert!(it.cannot_fix.is_some());
                }
            }
        }
    }

    #[test]
    fn no_measurement_no_items() {
        assert!(run(&inv("claude", vec![remote("r"), sized("a")]), None).is_empty());
    }

    #[test]
    fn broken_by_7c1_are_skipped() {
        let missing = server(
            "m",
            Transport::Stdio {
                command: "ghost".into(),
                state: CommandState::Missing,
            },
            Ok(launch("ghost")),
            CostRule::names_only(),
        );
        let nothing = server(
            "n",
            Transport::Nothing,
            Err(NotMeasured::NotApproved),
            CostRule::names_only(),
        );
        let out = run(&inv("claude", vec![missing, nothing]), Some(&measurements(vec![])));
        assert!(out.is_empty());
    }

    #[test]
    fn distro_line() {
        let mut i = inv("codex", vec![sized("a")]);
        i.distro = Some("Ubuntu".into());
        let out = run(&i, Some(&measurements(vec![])));
        assert_eq!(out.len(), 1);
        let it = &out[0].0;
        assert_eq!(it.id, "mcpcost.codex@Ubuntu");
        assert_eq!(it.group, DoctorGroup::Info);
        assert_eq!(it.title, "Codex's MCP servers in Ubuntu weren't measured");
        assert_eq!(it.detail, "Not measured: in Ubuntu. Measuring runs on Windows only.");
        assert_eq!(it.distro.as_deref(), Some("Ubuntu"));
        // No server, no line.
        let mut e = inv("codex", vec![]);
        e.distro = Some("Ubuntu".into());
        assert!(run(&e, Some(&measurements(vec![]))).is_empty());
    }

    fn full_under(name: &str, schema: usize) -> (Server, Measured) {
        let s = server(
            name,
            found(),
            Ok(launch(name)),
            rule(Listing::FullUnder {
                percent: 1,
                prefix: "".into(),
                desc_cap: None,
                instructions_cap: None,
            }),
        );
        let mut m = measured_of(&s, 2);
        m.outcome = Ok(Listed {
            instructions: String::new(),
            tools: vec![tool("ab", "", schema)],
        });
        (s, m)
    }

    #[test]
    fn full_under_threshold() {
        // percent 1 is 2,000 tokens. Each server costs 2 + schema characters in full and 2 by name.
        for (schema, expect) in [(3_000, 1_501), (4_100, 1)] {
            let (a, ma) = full_under("a", schema);
            let (b, mb) = full_under("b", schema);
            let out = run(&inv("claude", vec![a, b]), Some(&measurements(vec![ma, mb])));
            assert_eq!(
                out[0].0.title,
                format!("Claude Code's MCP servers add about {expect} tokens to each session")
            );
        }
    }
}
