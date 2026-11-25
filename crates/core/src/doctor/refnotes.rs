//! The Doctor's items about the reference notes Marshell adds to prompts: how many a project got in the last 30
//! days and what they cost, and two kinds of heavy use. Pure: the totals come from `references::note_totals`, and
//! nothing here reads a file or logs.

use std::path::PathBuf;

use marshell_protocol::doctor::{DoctorGroup, DoctorItem};

use super::ext::tokens;
use super::instructions::{folder_name, tokens_text};
use crate::references::{NOTE_DAYS, NoteTotals, project_id};

/// A note averaging more than this many tokens is a long one.
const LONG_TOKENS: usize = 150;
/// The fewest notes before a long average or a common alias is worth a line.
const LONG_MIN_NOTES: u32 = 5;
const COMMON_MIN_NOTES: u32 = 10;

fn item(id: String, group: DoctorGroup, title: String, detail: String, cannot_fix: Option<&str>) -> DoctorItem {
    DoctorItem {
        id,
        agent: None,
        distro: None,
        group,
        title,
        detail,
        fixable: false,
        cannot_fix: cannot_fix.map(str::to_string),
        path: None,
        open_agents: false,
        unchecked: false,
        action: None,
        loads: Vec::new(),
        opens: Vec::new(),
    }
}

/// The items for each project that has notes, in the order given: its line, then the long-notes item and the
/// common-alias item when they apply. A project with no note has no entry in `projects`.
pub fn items(projects: &[(PathBuf, NoteTotals)]) -> Vec<DoctorItem> {
    let mut out = Vec::new();
    for (root, totals) in projects {
        if totals.notes == 0 {
            continue;
        }
        let (id, name) = (project_id(root), folder_name(root));
        let notes = totals.notes;
        let average = tokens((totals.chars as usize).div_ceil(notes as usize));
        let plural = if notes == 1 { "note" } else { "notes" };
        out.push(item(
            format!("refnotes.{id}"),
            DoctorGroup::Info,
            format!(
                "{name}: {notes} reference {plural} in the last {NOTE_DAYS} days, about {} tokens",
                tokens_text(tokens(totals.chars as usize))
            ),
            format!(
                "About {} tokens a note. Marshell adds a note to a prompt that names an alias.",
                tokens_text(average)
            ),
            None,
        ));
        if notes >= LONG_MIN_NOTES && average > LONG_TOKENS {
            out.push(item(
                format!("refnotes.long.{id}"),
                DoctorGroup::Waste,
                format!(
                    "Reference notes in {name} average about {} tokens",
                    tokens_text(average)
                ),
                "A note this long names several aliases at once.".into(),
                Some("Shorten or remove aliases in Settings, Projects."),
            ));
        }
        if let Some((alias, count)) = &totals.top_alias
            && notes >= COMMON_MIN_NOTES
            && u64::from(*count) * 2 > u64::from(notes)
        {
            out.push(item(
                format!("refnotes.alias.{id}"),
                DoctorGroup::Waste,
                format!("The alias \u{201c}{alias}\u{201d} is in {count} of {notes} reference notes in {name}"),
                "A name this common adds a note to most prompts.".into(),
                Some("Rename it to something you only type when you mean it, or remove it, in Settings, Projects."),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::doctor::DoctorGroup;
    use std::path::PathBuf;

    fn proj() -> PathBuf {
        std::env::temp_dir().join("refnotes-test").join("proj")
    }

    fn totals(notes: u32, chars: u64, top: Option<(&str, u32)>) -> NoteTotals {
        NoteTotals {
            notes,
            chars,
            top_alias: top.map(|(n, c)| (n.to_string(), c)),
        }
    }

    fn id() -> String {
        crate::references::project_id(&proj())
    }

    #[test]
    fn a_project_with_notes_gets_its_line() {
        let got = items(&[(proj(), totals(14, 4_480, Some(("app", 3))))]);
        assert_eq!(got.len(), 1, "{got:?}");
        let item = &got[0];
        assert_eq!(item.id, format!("refnotes.{}", id()));
        assert_eq!(item.group, DoctorGroup::Info);
        assert_eq!(
            item.title,
            "proj: 14 reference notes in the last 30 days, about 1.1k tokens"
        );
        assert_eq!(
            item.detail,
            "About 80 tokens a note. Marshell adds a note to a prompt that names an alias."
        );
        assert!(!item.fixable);
        assert_eq!(item.cannot_fix, None);
        assert_eq!(item.path, None);
        assert!(item.opens.is_empty() && item.loads.is_empty());
    }

    #[test]
    fn one_note_is_singular() {
        let got = items(&[(proj(), totals(1, 320, Some(("app", 1))))]);
        assert_eq!(
            got[0].title,
            "proj: 1 reference note in the last 30 days, about 80 tokens"
        );
    }

    #[test]
    fn long_notes_are_waste() {
        let got = items(&[(proj(), totals(5, 4_200, None))]);
        assert_eq!(got.len(), 2, "{got:?}");
        let item = &got[1];
        assert_eq!(item.id, format!("refnotes.long.{}", id()));
        assert_eq!(item.group, DoctorGroup::Waste);
        assert_eq!(item.title, "Reference notes in proj average about 210 tokens");
        assert_eq!(item.detail, "A note this long names several aliases at once.");
        assert!(!item.fixable);
        assert_eq!(
            item.cannot_fix.as_deref(),
            Some("Shorten or remove aliases in Settings, Projects.")
        );
        // Four such notes are too few, and 150 tokens a note is not over the line.
        assert_eq!(items(&[(proj(), totals(4, 3_360, None))]).len(), 1);
        assert_eq!(items(&[(proj(), totals(5, 3_000, None))]).len(), 1);
    }

    #[test]
    fn an_alias_in_most_notes_is_waste() {
        let got = items(&[(proj(), totals(14, 1_400, Some(("app", 12))))]);
        assert_eq!(got.len(), 2, "{got:?}");
        let item = &got[1];
        assert_eq!(item.id, format!("refnotes.alias.{}", id()));
        assert_eq!(item.group, DoctorGroup::Waste);
        assert_eq!(
            item.title,
            "The alias \u{201c}app\u{201d} is in 12 of 14 reference notes in proj"
        );
        assert_eq!(item.detail, "A name this common adds a note to most prompts.");
        assert!(!item.fixable);
        assert_eq!(
            item.cannot_fix.as_deref(),
            Some("Rename it to something you only type when you mean it, or remove it, in Settings, Projects.")
        );
        // Too few notes, and exactly half, give none.
        assert_eq!(items(&[(proj(), totals(9, 900, Some(("app", 5))))]).len(), 1);
        assert_eq!(items(&[(proj(), totals(14, 1_400, Some(("app", 7))))]).len(), 1);
    }
}
