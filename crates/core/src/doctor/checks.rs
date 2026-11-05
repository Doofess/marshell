//! The Doctor's checks: facts about each CLI in, report items out. No I/O.

use std::path::PathBuf;

use marshell_protocol::agents::{HookInstall, HookTrust};
use marshell_protocol::doctor::{DoctorGroup, DoctorItem};

/// What the retention item says about the other CLIs.
pub const RETENTION_OTHERS: &str =
    "Codex and Qwen Code have no setting that deletes chats by age, and Antigravity documents none.";

const READ_ONLY: &str = "The file is read-only.";
const CANNOT_TRUST: &str = "Marshell can't trust hooks for you. Open /hooks in Codex and trust them.";
const ORDER: [&str; 4] = ["claude", "codex", "antigravity", "qwen"];

/// The state of the file a CLI's hooks live in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    Ok,
    Missing,
    ReadOnly,
    Unparsable { line: u32, message: String },
}

/// What a CLI notifies by itself, besides Marshell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotifyFacts {
    /// Antigravity: its settings.json. Codex: its config.toml. Claude Code and Qwen Code: the hooks file (`path`).
    pub file: PathBuf,
    /// Antigravity's `notifications` is `true`.
    pub desktop: bool,
    /// Codex's top-level `notify` array, its strings joined with spaces.
    pub command: Option<String>,
    /// The `command` of every hook under `hooks.Notification` that `agents::common::is_ours` doesn't claim
    /// (Claude Code, Codex and Qwen Code), in file order.
    pub hook_commands: Vec<String>,
}

/// Everything the checks need to know about one CLI, on Windows or in one WSL distro.
#[derive(Debug, Clone)]
pub struct AgentFacts {
    /// "claude" | "codex" | "antigravity" | "qwen"
    pub agent: &'static str,
    pub distro: Option<String>,
    /// The CLI is on this machine (always true for a distro's facts).
    pub found: bool,
    /// The file its hooks live in.
    pub path: PathBuf,
    pub file: FileState,
    pub hooks: HookInstall,
    pub approve_installed: bool,
    pub has_own_statusline: bool,
    pub chained: bool,
    /// Codex only.
    pub trust: Option<HookTrust>,
    /// managed.json names hooks Marshell wrote in `path`.
    pub set_up: bool,
    /// The bridge program the hooks run.
    pub bridge: PathBuf,
    /// The file at `bridge` is there. Orphaned hooks with it in place run another program (Codex and Antigravity
    /// read an entry naming another home's bridge as Orphaned).
    pub bridge_exists: bool,
    /// Gate::Unknown's note; None when known, not found, or a distro.
    pub version_note: Option<String>,
    /// Some(true): the key is unset; Some(false): the person set it; None: no such setting or not read.
    pub retention_unset: Option<bool>,
    /// An organisation file applies on top (Claude, Qwen).
    pub managed_settings: Option<PathBuf>,
    /// What the CLI notifies by itself.
    pub notify: NotifyFacts,
    /// The reader failed; every other field is a default.
    pub error: Option<String>,
}

/// The CLI's name as titles say it.
pub fn display_name(agent: &str) -> &'static str {
    match agent {
        "claude" => "Claude Code",
        "codex" => "Codex",
        "antigravity" => "Antigravity",
        "qwen" => "Qwen Code",
        _ => "This CLI",
    }
}

/// Broken and Waste items.
pub fn issues(items: &[DoctorItem]) -> u32 {
    items.iter().filter(|i| i.group != DoctorGroup::Info).count() as u32
}

fn rank(agent: &str) -> usize {
    ORDER.iter().position(|a| *a == agent).unwrap_or(ORDER.len())
}

/// The report's items: Windows CLIs in order, then each distro by name, then the stopped distros.
pub fn items(facts: &[AgentFacts], stopped: &[String], banners_on: bool) -> Vec<DoctorItem> {
    let mut sorted: Vec<&AgentFacts> = facts.iter().collect();
    sorted.sort_by(|a, b| (&a.distro, rank(a.agent)).cmp(&(&b.distro, rank(b.agent))));
    let mut out = Vec::new();
    for f in sorted {
        cli_items(f, banners_on, &mut out);
    }
    for d in stopped {
        out.push(DoctorItem {
            id: format!("wsl.{d}"),
            agent: None,
            distro: Some(d.clone()),
            group: DoctorGroup::Info,
            title: format!("Not checked: {d} isn't running."),
            detail: "Start it, then run the check again.".into(),
            fixable: false,
            cannot_fix: None,
            path: None,
            open_agents: false,
        });
    }
    out
}

fn cli_items(f: &AgentFacts, banners_on: bool, out: &mut Vec<DoctorItem>) {
    let cli = display_name(f.agent);
    let path = f.path.display().to_string();
    // A distro's ids add `@<distro>` and its titles say " in <distro>".
    let suffix = f.distro.as_ref().map(|d| format!("@{d}")).unwrap_or_default();
    let place = f.distro.as_ref().map(|d| format!(" in {d}")).unwrap_or_default();
    let mk = |kind: &str, group: DoctorGroup, fixable: bool, title: String, detail: String| DoctorItem {
        id: format!("{kind}.{}{suffix}", f.agent),
        agent: Some(f.agent.to_string()),
        distro: f.distro.clone(),
        group,
        title,
        detail,
        fixable,
        cannot_fix: None,
        path: None,
        open_agents: false,
    };

    if let Some(err) = &f.error {
        out.push(mk(
            "error",
            DoctorGroup::Info,
            false,
            format!("{cli}{place} couldn't be checked"),
            err.clone(),
        ));
        return;
    }

    let unparsable = matches!(f.file, FileState::Unparsable { .. });
    let mut found = Vec::new();

    if let FileState::Unparsable { line, message } = &f.file {
        let mut i = mk(
            "parse",
            DoctorGroup::Broken,
            false,
            format!("{path} doesn't parse"),
            format!("Line {line}: {message}."),
        );
        i.cannot_fix =
            Some("Marshell doesn't change a file it can't read. Fix it by hand, then run the check again.".into());
        i.path = Some(path.clone());
        found.push(i);
    } else {
        let hooks = match f.hooks {
            HookInstall::None if f.set_up => Some((
                format!("{cli}'s hooks{place} are gone"),
                format!("Marshell set them up in {path}, and they aren't there now. The fix sets them up again."),
            )),
            HookInstall::Partial => {
                let mut d = format!("Some of Marshell's entries in {path} are missing or changed.");
                if f.has_own_statusline && !f.chained {
                    d.push_str(" Your status line isn't chained through Marshell any more.");
                }
                d.push_str(" The fix sets them up again.");
                Some((format!("{cli}'s hooks{place} are incomplete"), d))
            }
            HookInstall::Orphaned if f.bridge_exists => Some((
                format!("{cli}'s hooks{place} point at a moved program"),
                format!(
                    "Some of Marshell's entries in {path} run a program other than {}. The fix points them at it again.",
                    f.bridge.display()
                ),
            )),
            HookInstall::Orphaned => Some((
                format!("{cli}'s hooks{place} run a program that's missing"),
                format!(
                    "The hooks in {path} run {}, which isn't there. The fix puts it back and sets the hooks up again.",
                    f.bridge.display()
                ),
            )),
            _ => None,
        };
        if let Some((title, detail)) = hooks {
            let mut i = mk("hooks", DoctorGroup::Broken, true, title, detail);
            i.path = Some(path.clone());
            found.push(i);
        }
        if f.agent == "codex" && f.hooks != HookInstall::None {
            let row = match f.trust {
                Some(HookTrust::Untrusted) => Some((
                    format!("Codex{place} doesn't trust Marshell's hooks yet"),
                    "Codex runs a hook only after you trust it.",
                )),
                Some(HookTrust::Changed) => Some((
                    format!("Codex's trust in Marshell's hooks{place} is out of date"),
                    "The hooks changed since you trusted them, so Codex doesn't run them.",
                )),
                _ => None,
            };
            if let Some((title, detail)) = row {
                let mut i = mk("trust", DoctorGroup::Broken, false, title, detail.into());
                i.cannot_fix = Some(CANNOT_TRUST.into());
                i.path = Some(path.clone());
                found.push(i);
            }
        }
    }

    if let Some(note) = &f.version_note {
        found.push(mk(
            "version",
            DoctorGroup::Info,
            false,
            note.clone(),
            format!("Status limited: rows show less for {cli} until Marshell has recorded this version."),
        ));
    }
    if let Some(m) = &f.managed_settings {
        let mut i = mk(
            "managed",
            DoctorGroup::Info,
            false,
            format!("Managed settings apply on top of yours{place}"),
            format!(
                "{} is set by your organisation. Marshell reports it and never changes it.",
                m.display()
            ),
        );
        i.path = Some(m.display().to_string());
        found.push(i);
    }
    if !unparsable {
        if f.agent == "claude" && f.retention_unset == Some(true) {
            let mut i = mk(
                "retention",
                DoctorGroup::Info,
                true,
                format!("{cli}{place} deletes transcripts after 30 days"),
                format!(
                    "History and resume lose sessions older than that. The fix sets cleanupPeriodDays to 365. {RETENTION_OTHERS}"
                ),
            );
            i.path = Some(path.clone());
            found.push(i);
        }
        if f.distro.is_none() && f.found && f.hooks == HookInstall::None && !f.set_up {
            let mut i = mk(
                "setup",
                DoctorGroup::Info,
                false,
                format!("Hooks aren't set up for {cli}."),
                "Agents sets them up, with a preview first.".into(),
            );
            i.open_agents = true;
            found.push(i);
        }
    }

    if banners_on && !unparsable {
        let n = &f.notify;
        let with_file = |mut i: DoctorItem| {
            i.path = Some(n.file.display().to_string());
            i
        };
        if f.agent == "antigravity" && n.desktop {
            found.push(with_file(mk(
                "notify",
                DoctorGroup::Broken,
                true,
                format!("{cli}{place} sends its own desktop notifications"),
                "With Marshell's banners on, you get each one twice. The fix sets notifications to false.".into(),
            )));
        }
        let twice = "If that shows a desktop notification, you get two.";
        if f.agent == "codex"
            && let Some(command) = &n.command
        {
            let mut i = with_file(mk(
                "notify",
                DoctorGroup::Info,
                false,
                format!("{cli}{place} runs its own notify command"),
                format!("After each turn it runs {command}. {twice}"),
            ));
            i.cannot_fix = Some("Marshell doesn't change your notify command.".into());
            found.push(i);
        }
        if !n.hook_commands.is_empty() {
            let mut i = with_file(mk(
                "notify",
                DoctorGroup::Info,
                false,
                format!("{cli}{place} runs your own Notification hook"),
                format!("It runs {}. {twice}", n.hook_commands.join("; ")),
            ));
            if f.agent == "codex" {
                i.id = format!("notify.codex.hooks{suffix}");
            }
            i.cannot_fix = Some("Marshell doesn't change your hooks.".into());
            found.push(i);
        }
    }

    if f.file == FileState::ReadOnly {
        for i in found.iter_mut().filter(|i| i.fixable) {
            i.fixable = false;
            i.cannot_fix = Some(READ_ONLY.into());
        }
    }
    out.extend(found);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(agent: &'static str) -> AgentFacts {
        AgentFacts {
            agent,
            distro: None,
            found: true,
            path: PathBuf::from("C:/h/settings.json"),
            file: FileState::Ok,
            hooks: HookInstall::Installed,
            approve_installed: false,
            has_own_statusline: false,
            chained: false,
            trust: None,
            set_up: true,
            bridge: PathBuf::from("C:/m/bridge.exe"),
            bridge_exists: true,
            version_note: None,
            retention_unset: None,
            managed_settings: None,
            notify: NotifyFacts::default(),
            error: None,
        }
    }

    fn one(f: AgentFacts) -> DoctorItem {
        let mut v = items(&[f], &[], true);
        assert_eq!(v.len(), 1, "{v:?}");
        v.remove(0)
    }

    fn check(
        i: &DoctorItem,
        id: &str,
        group: DoctorGroup,
        fixable: bool,
        title: &str,
        detail: &str,
        cannot: Option<&str>,
    ) {
        assert_eq!(i.id, id);
        assert_eq!(i.group, group);
        assert_eq!(i.fixable, fixable);
        assert_eq!(i.title, title);
        assert_eq!(i.detail, detail);
        assert_eq!(i.cannot_fix.as_deref(), cannot);
    }

    #[test]
    fn a_healthy_cli_has_no_items() {
        assert!(items(&[facts("claude")], &[], true).is_empty());
    }

    #[test]
    fn error_row() {
        let mut f = facts("claude");
        f.error = Some("access denied".into());
        let i = one(f);
        check(
            &i,
            "error.claude",
            DoctorGroup::Info,
            false,
            "Claude Code couldn't be checked",
            "access denied",
            None,
        );
    }

    #[test]
    fn parse_row() {
        let mut f = facts("claude");
        f.file = FileState::Unparsable {
            line: 4,
            message: "expected a comma".into(),
        };
        let i = one(f);
        check(
            &i,
            "parse.claude",
            DoctorGroup::Broken,
            false,
            "C:/h/settings.json doesn't parse",
            "Line 4: expected a comma.",
            Some("Marshell doesn't change a file it can't read. Fix it by hand, then run the check again."),
        );
    }

    #[test]
    fn hooks_gone_row() {
        let mut f = facts("claude");
        f.hooks = HookInstall::None;
        let i = one(f);
        check(
            &i,
            "hooks.claude",
            DoctorGroup::Broken,
            true,
            "Claude Code's hooks are gone",
            "Marshell set them up in C:/h/settings.json, and they aren't there now. The fix sets them up again.",
            None,
        );
    }

    #[test]
    fn hooks_partial_row_and_status_line_sentence() {
        let mut f = facts("claude");
        f.hooks = HookInstall::Partial;
        let i = one(f.clone());
        check(
            &i,
            "hooks.claude",
            DoctorGroup::Broken,
            true,
            "Claude Code's hooks are incomplete",
            "Some of Marshell's entries in C:/h/settings.json are missing or changed. The fix sets them up again.",
            None,
        );
        f.has_own_statusline = true;
        assert_eq!(
            one(f.clone()).detail,
            "Some of Marshell's entries in C:/h/settings.json are missing or changed. Your status line isn't chained through Marshell any more. The fix sets them up again."
        );
        f.chained = true;
        assert!(!one(f).detail.contains("status line"));
    }

    #[test]
    fn hooks_orphaned_row() {
        let mut f = facts("qwen");
        f.hooks = HookInstall::Orphaned;
        f.bridge_exists = false;
        let i = one(f);
        check(
            &i,
            "hooks.qwen",
            DoctorGroup::Broken,
            true,
            "Qwen Code's hooks run a program that's missing",
            "The hooks in C:/h/settings.json run C:/m/bridge.exe, which isn't there. The fix puts it back and sets the hooks up again.",
            None,
        );
    }

    /// Found in the 7a live check: Codex reads an entry naming another bridge as Orphaned while this home's bridge is
    /// there, and the row said that bridge "isn't there".
    #[test]
    fn hooks_pointing_at_a_moved_bridge_row() {
        let mut f = facts("codex");
        f.path = PathBuf::from("C:/h/hooks.json");
        f.hooks = HookInstall::Orphaned;
        f.trust = Some(HookTrust::Trusted);
        let v = items(&[f], &[], true);
        let i = v.iter().find(|i| i.id == "hooks.codex").expect("a hooks row");
        check(
            i,
            "hooks.codex",
            DoctorGroup::Broken,
            true,
            "Codex's hooks point at a moved program",
            "Some of Marshell's entries in C:/h/hooks.json run a program other than C:/m/bridge.exe. The fix points them at it again.",
            None,
        );
    }

    #[test]
    fn trust_untrusted_and_changed_rows() {
        let mut f = facts("codex");
        f.trust = Some(HookTrust::Untrusted);
        let i = one(f.clone());
        check(
            &i,
            "trust.codex",
            DoctorGroup::Broken,
            false,
            "Codex doesn't trust Marshell's hooks yet",
            "Codex runs a hook only after you trust it.",
            Some("Marshell can't trust hooks for you. Open /hooks in Codex and trust them."),
        );
        f.trust = Some(HookTrust::Changed);
        let i = one(f.clone());
        check(
            &i,
            "trust.codex",
            DoctorGroup::Broken,
            false,
            "Codex's trust in Marshell's hooks is out of date",
            "The hooks changed since you trusted them, so Codex doesn't run them.",
            Some("Marshell can't trust hooks for you. Open /hooks in Codex and trust them."),
        );
        f.trust = Some(HookTrust::Trusted);
        assert!(items(&[f.clone()], &[], true).is_empty());
        // No hooks, no trust item.
        f.trust = Some(HookTrust::Untrusted);
        f.hooks = HookInstall::None;
        f.set_up = false;
        f.found = false;
        assert!(items(&[f], &[], true).is_empty());
    }

    #[test]
    fn version_row() {
        let mut f = facts("claude");
        f.version_note = Some("Claude Code 9.9 isn't recorded yet".into());
        let i = one(f);
        check(
            &i,
            "version.claude",
            DoctorGroup::Info,
            false,
            "Claude Code 9.9 isn't recorded yet",
            "Status limited: rows show less for Claude Code until Marshell has recorded this version.",
            None,
        );
    }

    #[test]
    fn managed_row() {
        let mut f = facts("qwen");
        f.managed_settings = Some(PathBuf::from("C:/org/settings.json"));
        let i = one(f);
        check(
            &i,
            "managed.qwen",
            DoctorGroup::Info,
            false,
            "Managed settings apply on top of yours",
            "C:/org/settings.json is set by your organisation. Marshell reports it and never changes it.",
            None,
        );
    }

    #[test]
    fn retention_row() {
        let mut f = facts("claude");
        f.retention_unset = Some(true);
        let i = one(f.clone());
        check(
            &i,
            "retention.claude",
            DoctorGroup::Info,
            true,
            "Claude Code deletes transcripts after 30 days",
            "History and resume lose sessions older than that. The fix sets cleanupPeriodDays to 365. Codex and Qwen Code have no setting that deletes chats by age, and Antigravity documents none.",
            None,
        );
        f.retention_unset = Some(false);
        assert!(items(&[f], &[], true).is_empty());
    }

    #[test]
    fn retention_is_claude_codes_only() {
        for agent in ["codex", "antigravity", "qwen"] {
            let mut f = facts(agent);
            f.retention_unset = Some(true);
            assert!(items(&[f], &[], true).is_empty(), "{agent}");
        }
    }

    #[test]
    fn setup_row() {
        let mut f = facts("antigravity");
        f.hooks = HookInstall::None;
        f.set_up = false;
        let i = one(f.clone());
        check(
            &i,
            "setup.antigravity",
            DoctorGroup::Info,
            false,
            "Hooks aren't set up for Antigravity.",
            "Agents sets them up, with a preview first.",
            None,
        );
        assert!(i.open_agents);
        f.found = false;
        assert!(items(&[f], &[], true).is_empty());
    }

    #[test]
    fn stopped_row() {
        let v = items(&[], &["Ubuntu".to_string()], true);
        assert_eq!(v.len(), 1);
        check(
            &v[0],
            "wsl.Ubuntu",
            DoctorGroup::Info,
            false,
            "Not checked: Ubuntu isn't running.",
            "Start it, then run the check again.",
            None,
        );
        assert_eq!(v[0].distro.as_deref(), Some("Ubuntu"));
    }

    #[test]
    fn read_only_turns_off_the_fix_and_says_why() {
        let mut f = facts("claude");
        f.file = FileState::ReadOnly;
        f.hooks = HookInstall::Partial;
        f.retention_unset = Some(true);
        let v = items(&[f], &[], true);
        assert_eq!(v.len(), 2);
        for i in &v {
            assert!(!i.fixable, "{}", i.id);
            assert_eq!(i.cannot_fix.as_deref(), Some("The file is read-only."));
        }
    }

    #[test]
    fn an_unparsable_file_has_only_its_parse_item_and_the_notes() {
        let mut f = facts("claude");
        f.file = FileState::Unparsable {
            line: 1,
            message: "bad".into(),
        };
        f.hooks = HookInstall::Partial;
        f.retention_unset = Some(true);
        f.version_note = Some("note".into());
        f.managed_settings = Some(PathBuf::from("C:/org/s.json"));
        let ids: Vec<_> = items(&[f], &[], true).into_iter().map(|i| i.id).collect();
        assert_eq!(ids, ["parse.claude", "version.claude", "managed.claude"]);
    }

    #[test]
    fn a_reader_error_is_one_info_item_and_the_other_clis_are_untouched() {
        let mut bad = facts("codex");
        bad.error = Some("boom".into());
        bad.hooks = HookInstall::Partial;
        let mut other = facts("claude");
        other.hooks = HookInstall::Partial;
        let v = items(&[other, bad], &[], true);
        let ids: Vec<_> = v.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["hooks.claude", "error.codex"]);
        assert_eq!(v[1].group, DoctorGroup::Info);
    }

    #[test]
    fn order_is_claude_codex_antigravity_qwen_then_distros_then_stopped() {
        let mk = |agent, distro: Option<&str>| {
            let mut f = facts(agent);
            f.hooks = HookInstall::Partial;
            f.distro = distro.map(String::from);
            f
        };
        let all = [
            mk("qwen", None),
            mk("claude", Some("Ubuntu")),
            mk("antigravity", None),
            mk("codex", Some("Debian")),
            mk("codex", None),
            mk("claude", None),
        ];
        let ids: Vec<_> = items(&all, &["Zed".to_string()], true)
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(
            ids,
            [
                "hooks.claude",
                "hooks.codex",
                "hooks.antigravity",
                "hooks.qwen",
                "hooks.codex@Debian",
                "hooks.claude@Ubuntu",
                "wsl.Zed"
            ]
        );
    }

    #[test]
    fn hooks_never_set_up_is_info_and_not_counted() {
        let mut never = facts("claude");
        never.hooks = HookInstall::None;
        never.set_up = false;
        let v = items(&[never.clone()], &["Ubuntu".to_string()], true);
        assert_eq!(v.len(), 2);
        assert_eq!(issues(&v), 0);
        let mut gone = never;
        gone.set_up = true;
        let mut v = items(&[gone], &[], true);
        assert_eq!(issues(&v), 1);
        v.push(DoctorItem {
            group: DoctorGroup::Waste,
            ..v[0].clone()
        });
        assert_eq!(issues(&v), 2);
    }

    #[test]
    fn a_distro_title_names_the_distro() {
        let mut f = facts("codex");
        f.distro = Some("Ubuntu".into());
        f.hooks = HookInstall::None;
        let i = one(f);
        assert_eq!(i.id, "hooks.codex@Ubuntu");
        assert_eq!(i.title, "Codex's hooks in Ubuntu are gone");
        assert_eq!(i.distro.as_deref(), Some("Ubuntu"));
        assert_eq!(i.agent.as_deref(), Some("codex"));
    }

    fn notify_file() -> PathBuf {
        PathBuf::from("C:/h/settings.json")
    }

    #[test]
    fn antigravity_desktop_notifications_are_broken_and_fixable() {
        let mut f = facts("antigravity");
        f.notify = NotifyFacts {
            file: notify_file(),
            desktop: true,
            ..Default::default()
        };
        let i = one(f);
        check(
            &i,
            "notify.antigravity",
            DoctorGroup::Broken,
            true,
            "Antigravity sends its own desktop notifications",
            "With Marshell's banners on, you get each one twice. The fix sets notifications to false.",
            None,
        );
        assert_eq!(i.path.as_deref(), Some("C:/h/settings.json"));
        assert_eq!(i.agent.as_deref(), Some("antigravity"));
    }

    #[test]
    fn codex_notify_and_hooks_are_info() {
        let mut f = facts("codex");
        f.notify = NotifyFacts {
            file: PathBuf::from("C:/h/config.toml"),
            desktop: false,
            command: Some("a.exe x".into()),
            hook_commands: vec!["toast.exe".into()],
        };
        let v = items(&[f], &[], true);
        assert_eq!(v.len(), 2, "{v:?}");
        check(
            &v[0],
            "notify.codex",
            DoctorGroup::Info,
            false,
            "Codex runs its own notify command",
            "After each turn it runs a.exe x. If that shows a desktop notification, you get two.",
            Some("Marshell doesn't change your notify command."),
        );
        check(
            &v[1],
            "notify.codex.hooks",
            DoctorGroup::Info,
            false,
            "Codex runs your own Notification hook",
            "It runs toast.exe. If that shows a desktop notification, you get two.",
            Some("Marshell doesn't change your hooks."),
        );
        assert_eq!(v[0].path.as_deref(), Some("C:/h/config.toml"));
        assert_eq!(v[1].path.as_deref(), Some("C:/h/config.toml"));
    }

    #[test]
    fn claude_notification_hook_is_info() {
        let mut f = facts("claude");
        f.notify.file = notify_file();
        f.notify.hook_commands = vec!["toast.exe a".into(), "b.sh".into()];
        let i = one(f);
        check(
            &i,
            "notify.claude",
            DoctorGroup::Info,
            false,
            "Claude Code runs your own Notification hook",
            "It runs toast.exe a; b.sh. If that shows a desktop notification, you get two.",
            Some("Marshell doesn't change your hooks."),
        );
    }

    #[test]
    fn banners_off_gives_no_notify_item() {
        let mut f = facts("antigravity");
        f.notify.desktop = true;
        f.notify.command = Some("x".into());
        f.notify.hook_commands = vec!["y".into()];
        assert!(items(&[f], &[], false).is_empty());
    }

    #[test]
    fn read_only_antigravity_cannot_be_fixed() {
        let mut f = facts("antigravity");
        f.file = FileState::ReadOnly;
        f.notify.desktop = true;
        let i = one(f);
        assert!(!i.fixable);
        assert_eq!(i.cannot_fix.as_deref(), Some("The file is read-only."));
    }

    #[test]
    fn a_distro_notify_item_has_its_suffix() {
        let mut f = facts("antigravity");
        f.distro = Some("Ubuntu".into());
        f.notify.desktop = true;
        let i = one(f);
        assert_eq!(i.id, "notify.antigravity@Ubuntu");
        assert_eq!(i.title, "Antigravity in Ubuntu sends its own desktop notifications");
        let mut c = facts("codex");
        c.distro = Some("Ubuntu".into());
        c.notify.command = Some("a".into());
        c.notify.hook_commands = vec!["b".into()];
        let ids: Vec<_> = items(&[c], &[], true).into_iter().map(|i| i.id).collect();
        assert_eq!(ids, ["notify.codex@Ubuntu", "notify.codex.hooks@Ubuntu"]);
    }
}
