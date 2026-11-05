//! The CLI doctors' output (`claude doctor`, `codex doctor --json`) as report-only Doctor items. No I/O.

use marshell_protocol::doctor::{DoctorGroup, DoctorItem};
use serde_json::Value;

use super::checks::display_name;
use crate::config::hash::hash_bytes;

/// What running one CLI doctor gave: `None` when it didn't finish in time or couldn't start.
pub type Ran = Option<(i32, String)>;

pub const CLI_DOCTOR_TIMEOUT_SECS: u64 = 60;

const CLAUDE_OK_LINE: &str = "No installation issues found.";

fn place(distro: Option<&str>) -> String {
    distro.map(|d| format!(" in {d}")).unwrap_or_default()
}

/// An id is `cli.<agent>` or `cli.<agent>.<key>`, with `@<distro>` appended in a distro.
fn id(agent: &str, key: Option<&str>, distro: Option<&str>) -> String {
    let mut id = format!("cli.{agent}");
    if let Some(k) = key {
        id.push('.');
        id.push_str(k);
    }
    if let Some(d) = distro {
        id.push('@');
        id.push_str(d);
    }
    id
}

fn item(
    agent: &str,
    distro: Option<&str>,
    key: Option<&str>,
    group: DoctorGroup,
    title: String,
    detail: String,
    cannot_fix: Option<String>,
) -> DoctorItem {
    DoctorItem {
        id: id(agent, key, distro),
        agent: Some(agent.to_string()),
        distro: distro.map(str::to_string),
        group,
        title,
        detail,
        fixable: false,
        cannot_fix,
        path: None,
        open_agents: false,
    }
}

/// The one item for a doctor that gave nothing usable: `detail` says why.
fn no_result(agent: &str, distro: Option<&str>, detail: String) -> DoctorItem {
    item(
        agent,
        distro,
        None,
        DoctorGroup::Info,
        format!("{}'s doctor{} didn't give a result", display_name(agent), place(distro)),
        detail,
        Some(format!("Run {agent} doctor in a terminal to see what it says.")),
    )
}

fn cannot_read(agent: &str, distro: Option<&str>, code: i32) -> DoctorItem {
    no_result(
        agent,
        distro,
        format!("Marshell can't read what it printed (exit code {code})."),
    )
}

fn timed_out(agent: &str, distro: Option<&str>) -> DoctorItem {
    no_result(
        agent,
        distro,
        format!("It didn't finish in {CLI_DOCTOR_TIMEOUT_SECS} seconds."),
    )
}

/// Drops `ESC [ … <letter>` sequences.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for n in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&n) {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `^\d+ warnings? found$`
fn is_warning_header(line: &str) -> bool {
    let line = line.trim();
    let Some((count, rest)) = line.split_once(' ') else {
        return false;
    };
    !count.is_empty()
        && count.bytes().all(|b| b.is_ascii_digit())
        && (rest == "warning found" || rest == "warnings found")
}

/// Claude Code's `claude doctor`: what it printed becomes one item per warning.
pub fn claude_items(ran: &Ran, distro: Option<&str>) -> Vec<DoctorItem> {
    let Some((code, output)) = ran else {
        return vec![timed_out("claude", distro)];
    };
    let text = strip_ansi(output);
    let lines: Vec<&str> = text.lines().collect();
    if *code == 0 && lines.iter().any(|l| l.trim() == CLAUDE_OK_LINE) {
        return Vec::new();
    }
    let Some(header) = lines.iter().position(|l| is_warning_header(l)) else {
        return vec![cannot_read("claude", distro, *code)];
    };
    let mut out: Vec<DoctorItem> = Vec::new();
    let rest = &lines[header + 1..];
    for (n, line) in rest.iter().enumerate() {
        let Some(issue) = line.trim_end().strip_prefix("- ") else {
            continue;
        };
        let issue = issue.trim();
        if issue.is_empty() {
            continue;
        }
        let fix = rest
            .get(n + 1)
            .and_then(|next| next.trim().strip_prefix("Fix: "))
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .map(|f| format!("Fix: {f}"));
        let key = hash_bytes(issue.as_bytes());
        let it = item(
            "claude",
            distro,
            Some(&key[..8]),
            DoctorGroup::Info,
            format!("Claude Code's doctor{}: {issue}", place(distro)),
            "Found by claude doctor. Marshell reports it and doesn't change it.".into(),
            fix,
        );
        // The same issue twice is one finding, so ids stay unique.
        if !out.iter().any(|o| o.id == it.id) {
            out.push(it);
        }
    }
    if out.is_empty() {
        return vec![cannot_read("claude", distro, *code)];
    }
    out
}

/// Codex's `codex doctor --json`: one item per check that isn't ok. A check's `details` is never used.
pub fn codex_items(ran: &Ran, distro: Option<&str>) -> Vec<DoctorItem> {
    let Some((code, output)) = ran else {
        return vec![timed_out("codex", distro)];
    };
    let unreadable = || vec![cannot_read("codex", distro, *code)];
    if *code != 0 && *code != 1 {
        return unreadable();
    }
    let text = output.trim_start_matches('\u{feff}');
    let Some(Ok(Value::Object(root))) = serde_json::Deserializer::from_str(text).into_iter::<Value>().next() else {
        return unreadable();
    };
    let Some(Value::Object(checks)) = root.get("checks") else {
        return unreadable();
    };
    let mut ids: Vec<&String> = checks.keys().collect();
    ids.sort();
    let mut out = Vec::new();
    for check_id in ids {
        let check = &checks[check_id];
        let field = |name: &str| {
            check
                .get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        };
        let group = match field("status") {
            Some("ok") => continue,
            Some("fail") => DoctorGroup::Broken,
            _ => DoctorGroup::Info,
        };
        out.push(item(
            "codex",
            distro,
            Some(check_id),
            group,
            format!(
                "Codex's doctor{}: {}",
                place(distro),
                field("summary").unwrap_or(check_id)
            ),
            format!("Check {check_id}, found by codex doctor. Marshell reports it and doesn't change it."),
            field("remediation").map(str::to_string),
        ));
    }
    out
}

/// Antigravity and Qwen Code.
pub fn no_doctor_item(agent: &str, distro: Option<&str>) -> DoctorItem {
    item(
        agent,
        distro,
        None,
        DoctorGroup::Info,
        format!("{}{} has no doctor command.", display_name(agent), place(distro)),
        "Marshell's own checks still cover its hooks and settings.".into(),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE_OK: &str = "Diagnostics\n └ Currently running: native (2.1.318)\n\nNo installation issues found.\n";

    fn titles(items: &[DoctorItem]) -> Vec<&str> {
        items.iter().map(|i| i.title.as_str()).collect()
    }

    fn assert_report_only(item: &DoctorItem, agent: &str, distro: Option<&str>) {
        assert_eq!(item.agent.as_deref(), Some(agent));
        assert_eq!(item.distro.as_deref(), distro);
        assert!(!item.fixable);
        assert_eq!(item.path, None);
        assert!(!item.open_agents);
    }

    #[test]
    fn claude_ok_gives_nothing() {
        assert!(claude_items(&Some((0, CLAUDE_OK.into())), None).is_empty());
    }

    #[test]
    fn claude_warnings_give_one_item_each() {
        let out = "2 warnings found\n- Multiple installs found\n  Fix: Remove the npm one\n- Ripgrep missing\n";
        let items = claude_items(&Some((0, out.into())), None);
        assert_eq!(items.len(), 2);
        for i in &items {
            assert_report_only(i, "claude", None);
            assert_eq!(i.group, DoctorGroup::Info);
            assert_eq!(
                i.detail,
                "Found by claude doctor. Marshell reports it and doesn't change it."
            );
        }
        assert_eq!(items[0].title, "Claude Code's doctor: Multiple installs found");
        assert_eq!(items[0].cannot_fix.as_deref(), Some("Fix: Remove the npm one"));
        assert_eq!(items[1].title, "Claude Code's doctor: Ripgrep missing");
        assert_eq!(items[1].cannot_fix, None);
        let key = &crate::config::hash::hash_bytes(b"Multiple installs found")[..8];
        assert_eq!(items[0].id, format!("cli.claude.{key}"));
        assert_ne!(items[0].id, items[1].id);
    }

    #[test]
    fn claude_ansi_header_is_read() {
        let out = "\x1b[33m1 warning found\x1b[39m\n- X\n  Fix: Y\n";
        let items = claude_items(&Some((0, out.into())), None);
        assert_eq!(titles(&items), ["Claude Code's doctor: X"]);
        assert_eq!(items[0].cannot_fix.as_deref(), Some("Fix: Y"));
    }

    #[test]
    fn claude_unreadable_and_header_without_lines_are_one_item() {
        for (code, out) in [
            (0, "Aucun problème d'installation trouvé.\n"),
            (3, "1 warning found\nnothing else\n"),
        ] {
            let items = claude_items(&Some((code, out.into())), None);
            assert_eq!(items.len(), 1);
            let i = &items[0];
            assert_report_only(i, "claude", None);
            assert_eq!(i.id, "cli.claude");
            assert_eq!(i.group, DoctorGroup::Info);
            assert_eq!(i.title, "Claude Code's doctor didn't give a result");
            assert_eq!(
                i.detail,
                format!("Marshell can't read what it printed (exit code {code}).")
            );
            assert_eq!(
                i.cannot_fix.as_deref(),
                Some("Run claude doctor in a terminal to see what it says.")
            );
            assert!(!format!("{i:?}").contains("nothing else"));
        }
    }

    #[test]
    fn claude_timeout() {
        let items = claude_items(&None, None);
        assert_eq!(items.len(), 1);
        let i = &items[0];
        assert_report_only(i, "claude", None);
        assert_eq!(i.id, "cli.claude");
        assert_eq!(i.group, DoctorGroup::Info);
        assert_eq!(i.title, "Claude Code's doctor didn't give a result");
        assert_eq!(i.detail, "It didn't finish in 60 seconds.");
        assert_eq!(
            i.cannot_fix.as_deref(),
            Some("Run claude doctor in a terminal to see what it says.")
        );
        assert_eq!(CLI_DOCTOR_TIMEOUT_SECS, 60);
        let c = codex_items(&None, None);
        assert_eq!(c[0].title, "Codex's doctor didn't give a result");
        assert_eq!(
            c[0].cannot_fix.as_deref(),
            Some("Run codex doctor in a terminal to see what it says.")
        );
    }

    #[test]
    fn codex_maps_statuses() {
        let json = r#"{"schemaVersion":1,"overallStatus":"fail","checks":{
            "zeta":{"id":"zeta","status":"skipped","summary":"Zeta skipped","details":"SECRET-DETAILS","remediation":""},
            "alpha":{"id":"alpha","status":"ok","summary":"Fine","details":"SECRET-DETAILS"},
            "mid":{"id":"mid","status":"warning","summary":"Mid is odd","details":"SECRET-DETAILS","remediation":"Do the thing"},
            "beta":{"id":"beta","status":"fail","summary":"","details":"SECRET-DETAILS"}
        }}"#;
        let items = codex_items(&Some((1, json.into())), None);
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["cli.codex.beta", "cli.codex.mid", "cli.codex.zeta"]);
        for i in &items {
            assert_report_only(i, "codex", None);
            assert!(!format!("{i:?}").contains("SECRET-DETAILS"));
        }
        assert_eq!(items[0].group, DoctorGroup::Broken);
        assert_eq!(items[0].title, "Codex's doctor: beta");
        assert_eq!(
            items[0].detail,
            "Check beta, found by codex doctor. Marshell reports it and doesn't change it."
        );
        assert_eq!(items[0].cannot_fix, None);
        assert_eq!(items[1].group, DoctorGroup::Info);
        assert_eq!(items[1].title, "Codex's doctor: Mid is odd");
        assert_eq!(items[1].cannot_fix.as_deref(), Some("Do the thing"));
        assert_eq!(items[2].group, DoctorGroup::Info);
        assert_eq!(items[2].title, "Codex's doctor: Zeta skipped");
        assert_eq!(items[2].cannot_fix, None);
        // Exit 0 is read too, and an all-ok report gives nothing.
        let ok = r#"{"checks":{"a":{"status":"ok"}}}"#;
        assert!(codex_items(&Some((0, ok.into())), None).is_empty());
    }

    #[test]
    fn codex_json_then_noise_is_read() {
        let out = "\n  {\"checks\":{\"a\":{\"status\":\"warning\",\"summary\":\"S\"}}}\nWARN something";
        let items = codex_items(&Some((0, out.into())), None);
        assert_eq!(titles(&items), ["Codex's doctor: S"]);
    }

    #[test]
    fn codex_bad_shapes_are_one_item() {
        let cases = [
            (0, r#"{"checks":[1,2]}"#),
            (0, r#"{"nochecks":{}}"#),
            (0, "[1]"),
            (0, "{not json"),
            (0, "WARN first\n{\"checks\":{}}"),
            (0, ""),
            (2, r#"{"checks":{"a":{"status":"fail"}}}"#),
        ];
        for (code, out) in cases {
            let items = codex_items(&Some((code, out.into())), None);
            assert_eq!(items.len(), 1, "{out}");
            let i = &items[0];
            assert_report_only(i, "codex", None);
            assert_eq!(i.id, "cli.codex");
            assert_eq!(i.group, DoctorGroup::Info);
            assert_eq!(i.title, "Codex's doctor didn't give a result");
            assert_eq!(
                i.detail,
                format!("Marshell can't read what it printed (exit code {code}).")
            );
        }
    }

    #[test]
    fn distro_ids_and_titles() {
        let d = Some("Ubuntu");
        let json = r#"{"checks":{"a":{"status":"warning","summary":"S"}}}"#;
        let items = codex_items(&Some((0, json.into())), d);
        assert_eq!(items[0].id, "cli.codex.a@Ubuntu");
        assert_eq!(items[0].title, "Codex's doctor in Ubuntu: S");
        assert_report_only(&items[0], "codex", d);

        let c = claude_items(&Some((0, "1 warning found\n- X\n".into())), d);
        assert!(c[0].id.starts_with("cli.claude.") && c[0].id.ends_with("@Ubuntu"));
        assert_eq!(c[0].title, "Claude Code's doctor in Ubuntu: X");

        let unread = claude_items(&None, d);
        assert_eq!(unread[0].id, "cli.claude@Ubuntu");
        assert_eq!(unread[0].title, "Claude Code's doctor in Ubuntu didn't give a result");

        let q = no_doctor_item("qwen", d);
        assert_eq!(q.id, "cli.qwen@Ubuntu");
        assert_eq!(q.title, "Qwen Code in Ubuntu has no doctor command.");
        assert_eq!(q.detail, "Marshell's own checks still cover its hooks and settings.");
        assert_eq!(q.group, DoctorGroup::Info);
        assert_eq!(q.cannot_fix, None);
        assert_report_only(&q, "qwen", d);

        let a = no_doctor_item("antigravity", None);
        assert_eq!(a.id, "cli.antigravity");
        assert_eq!(a.title, "Antigravity has no doctor command.");
        assert_report_only(&a, "antigravity", None);
    }
}
