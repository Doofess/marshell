//! The Doctor's items for the instruction library's copies (slice 8a): a copy edited outside Marshell, and a place
//! whose copies are out of date. The judging is `instructions::library::level_state`'s; this only words it.

use super::instructions::{display, folder_name, hash_part, id_tail};
use crate::agents::skill::cli_name;
use crate::instructions::library::{CopyAt, Level, LevelState, Place};
use marshell_protocol::doctor::{DoctorAction, DoctorGroup, DoctorItem};
use marshell_protocol::instructions::CopyState;
use std::path::Path;

/// A level's project and distro.
fn where_of(place: &Place) -> (Option<&Path>, Option<&str>) {
    match &place.level {
        Level::Project { root, .. } => (Some(root.as_path()), None),
        Level::User { distro, .. } => (None, distro.as_deref()),
    }
}

/// The id tail of one changed-outside copy: `<hash of its path>.<doc name lower case>`.
pub(crate) fn drift_tail(copy: &CopyAt) -> String {
    format!("{}.{}", hash_part(&copy.path), copy.doc.to_lowercase())
}

/// The id of the fix that puts a changed-outside copy's text back.
pub(crate) fn drift_id(copy: &CopyAt) -> String {
    format!("libdrift.{}", drift_tail(copy))
}

/// The id of a place's out-of-date item: `libstale.<place><suffix>`.
pub(crate) fn stale_id(place: &Place) -> String {
    let (project, distro) = where_of(place);
    let (at, suffix) = id_tail(project, distro);
    format!("libstale.{at}{suffix}")
}

fn shown(place: &Place, path: &Path) -> String {
    match &place.level {
        Level::Project { root, .. } => display(path, Some(root), root),
        Level::User { roots, .. } => display(path, None, &roots.home),
    }
}

fn item(id: String, group: DoctorGroup, title: String, detail: String) -> DoctorItem {
    DoctorItem {
        id,
        agent: None,
        distro: None,
        group,
        title,
        detail,
        fixable: true,
        cannot_fix: None,
        path: None,
        open_agents: false,
        unchecked: true,
        action: None,
        loads: Vec::new(),
        opens: Vec::new(),
    }
}

fn drift_row(place: &Place, copy: &CopyAt) -> DoctorItem {
    let (_, distro) = where_of(place);
    let cli = cli_name(copy.cli).unwrap_or(copy.cli);
    let mut it = item(
        drift_id(copy),
        DoctorGroup::Broken,
        format!("{} changed outside Marshell", shown(place, &copy.path)),
        format!(
            "{cli} loads text the doc {} doesn't hold. The fix puts the doc's text back. Use this copy's text takes the \
             copy's text into the doc instead: the text, not the frontmatter.",
            copy.doc
        ),
    );
    it.agent = Some(copy.cli.to_string());
    it.distro = distro.map(str::to_string);
    it.path = Some(copy.path.to_string_lossy().into_owned());
    it.action = Some(DoctorAction::UseCopy);
    it
}

fn stale_title(place: &Place, copies: &[&CopyAt]) -> String {
    let mut docs: Vec<&str> = Vec::new();
    for c in copies {
        if !docs.iter().any(|d| d.eq_ignore_ascii_case(&c.doc)) {
            docs.push(&c.doc);
        }
    }
    let n = copies.len();
    let of = if docs.len() == 1 {
        format!("{} {} of {}", n, if n == 1 { "copy" } else { "copies" }, docs[0])
    } else {
        format!("{n} copies of {} docs", docs.len())
    };
    let verb = if n == 1 { "is" } else { "are" };
    let (project, distro) = where_of(place);
    let at = match (project, distro) {
        (Some(root), _) => format!("in {}", folder_name(root)),
        (None, None) => "at user level".to_string(),
        (None, Some(d)) => format!("at user level in {d}"),
    };
    format!("{of} {verb} out of date {at}")
}

/// One Broken item for each changed-outside copy a doc wants, and one Info item for each place whose copies are out of
/// date. Orphans and edited copies no doc wants are not in `copies`, so they give none.
pub fn items(levels: &[(&Place, &LevelState)]) -> Vec<DoctorItem> {
    let mut out = Vec::new();
    for (place, state) in levels {
        for copy in state.copies.iter().filter(|c| c.state == CopyState::ChangedOutside) {
            out.push(drift_row(place, copy));
        }
        if state.out_of_date > 0 {
            let stale: Vec<&CopyAt> = state
                .copies
                .iter()
                .filter(|c| matches!(c.state, CopyState::Missing | CopyState::OutOfDate | CopyState::Extra))
                .collect();
            let (_, distro) = where_of(place);
            let mut it = item(
                stale_id(place),
                DoctorGroup::Info,
                stale_title(place, &stale),
                "The CLIs load what the copies held before the docs changed. The fix writes them as Review on the \
                 Instructions page does."
                    .to_string(),
            );
            it.distro = distro.map(str::to_string);
            out.push(it);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instructions::library::Notes;
    use std::path::PathBuf;

    fn place(root: &str, distro: Option<&str>) -> Place {
        let level = match distro {
            None => Level::Project {
                root: PathBuf::from(root),
                notes: Notes::default(),
            },
            Some(d) => Level::User {
                roots: crate::instructions::roots::Roots::under(Path::new(root)),
                distro: Some(d.to_string()),
            },
        };
        Place {
            level,
            docs: Vec::new(),
            found: Vec::new(),
        }
    }

    fn copy(doc: &str, cli: &'static str, path: &str, state: CopyState) -> CopyAt {
        CopyAt {
            doc: doc.to_string(),
            cli,
            path: PathBuf::from(path),
            state,
        }
    }

    fn state_of(copies: Vec<CopyAt>) -> LevelState {
        let count = |wanted: &[CopyState]| copies.iter().filter(|c| wanted.contains(&c.state)).count() as u32;
        LevelState {
            rows: Vec::new(),
            orphans: Vec::new(),
            out_of_date: count(&[CopyState::Missing, CopyState::OutOfDate, CopyState::Extra]),
            changed_outside: count(&[CopyState::ChangedOutside]),
            copies,
        }
    }

    #[test]
    fn drift_item() {
        let p = place(r"C:\p\Airport", None);
        let st = state_of(vec![copy(
            "security",
            "qwen",
            r"C:\p\Airport\.qwen\rules\marshell-security.md",
            CopyState::ChangedOutside,
        )]);
        let out = items(&[(&p, &st)]);
        assert_eq!(out.len(), 1, "{out:?}");
        let it = &out[0];
        let (head, rest) = it.id.split_at("libdrift.".len());
        assert_eq!(head, "libdrift.");
        let (hash, name) = rest.split_once('.').unwrap();
        assert_eq!(hash.len(), 8);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(name, "security");
        assert_eq!(it.group, DoctorGroup::Broken);
        assert_eq!(it.title, ".qwen/rules/marshell-security.md changed outside Marshell");
        assert!(it.fixable && it.unchecked);
        assert_eq!(it.action, Some(DoctorAction::UseCopy));
        assert_eq!(it.agent.as_deref(), Some("qwen"));
        assert_eq!(
            it.detail,
            "Qwen Code loads text the doc security doesn't hold. The fix puts the doc's text back. Use this copy's text \
             takes the copy's text into the doc instead: the text, not the frontmatter."
        );
    }

    #[test]
    fn two_codex_drifts_have_two_ids() {
        let p = place(r"C:\p\Airport", None);
        let file = r"C:\p\Airport\.codex\config.toml";
        let st = state_of(vec![
            copy("a", "codex", file, CopyState::ChangedOutside),
            copy("b", "codex", file, CopyState::ChangedOutside),
        ]);
        let out = items(&[(&p, &st)]);
        assert_eq!(out.len(), 2);
        assert_ne!(out[0].id, out[1].id);
        assert!(out[0].id.ends_with(".a") && out[1].id.ends_with(".b"));
    }

    #[test]
    fn stale_titles() {
        let sec = |cli, state| {
            copy(
                "SECURITY",
                cli,
                &format!(r"C:\p\Airport\{cli}\marshell-security.md"),
                state,
            )
        };
        let four = || {
            ["claude", "codex", "qwen", "copilot"]
                .into_iter()
                .map(|c| sec(c, CopyState::OutOfDate))
                .collect::<Vec<_>>()
        };
        let title = |p: &Place, copies: Vec<CopyAt>| {
            let st = state_of(copies);
            let out = items(&[(p, &st)]);
            assert_eq!(out.len(), 1, "{out:?}");
            assert_eq!(out[0].group, DoctorGroup::Info);
            assert!(out[0].fixable && out[0].unchecked);
            out[0].title.clone()
        };
        let project = place(r"C:\p\Airport", None);
        assert_eq!(
            title(&project, four()),
            "4 copies of SECURITY are out of date in Airport"
        );
        assert_eq!(
            title(&project, vec![sec("claude", CopyState::Missing)]),
            "1 copy of SECURITY is out of date in Airport"
        );
        let mut many = four();
        many.push(copy("API", "claude", r"C:\p\Airport\a.md", CopyState::Extra));
        many.push(copy("API", "qwen", r"C:\p\Airport\b.md", CopyState::OutOfDate));
        assert_eq!(title(&project, many), "6 copies of 2 docs are out of date in Airport");
        let user = place(r"C:\Users\me", Some("MarshellTestDistro"));
        assert_eq!(
            title(&user, four()),
            "4 copies of SECURITY are out of date at user level in MarshellTestDistro"
        );
        let windows = Place {
            level: Level::User {
                roots: crate::instructions::roots::Roots::under(Path::new(r"C:\Users\me")),
                distro: None,
            },
            docs: Vec::new(),
            found: Vec::new(),
        };
        assert_eq!(
            title(&windows, four()),
            "4 copies of SECURITY are out of date at user level"
        );
    }

    #[test]
    fn in_sync_and_unchanged_places_give_nothing() {
        let p = place(r"C:\p\Airport", None);
        let st = state_of(vec![copy("a", "claude", r"C:\p\Airport\x.md", CopyState::InSync)]);
        assert!(items(&[(&p, &st)]).is_empty());
    }
}
