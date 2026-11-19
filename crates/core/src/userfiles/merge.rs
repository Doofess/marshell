//! The 3-way merge of a person's file that changed on disk under their unsaved edits, through
//! `diffy`. `base` is the text the edits started from, `mine` the person's text and `disk` what
//! the file holds now. Lines changed on both sides come back between `<<<<<<< mine`, `=======`
//! and `>>>>>>> disk`.
//!
//! `diffy` slows with the square of the lines that changed (a 20,000-line file rewritten whole
//! takes over a second in a release build), so a side that differs from the base by more than
//! `MAX_CHANGED_LINES` is refused before any work is done.

use std::collections::HashMap;

use diffy::{ConflictStyle, IncompleteHunkStyle, MergeOptions};

/// The most lines a side may differ from the base by: lines gone plus lines new.
pub const MAX_CHANGED_LINES: usize = 20_000;

const START: &str = "<<<<<<< mine";
const MIDDLE: &str = "=======";
const END: &str = ">>>>>>> disk";

/// A merged text, and how many conflicts the merge wrote into it.
#[derive(Debug, PartialEq, Eq)]
pub struct Merged {
    pub text: String,
    pub conflicts: u32,
}

/// A side differs from the base by more than `MAX_CHANGED_LINES`.
#[derive(Debug, PartialEq, Eq)]
pub struct TooDifferent;

/// How many lines the two texts don't share: the lines of `base` with no equal line left in
/// `other`, plus the lines of `other` with none left in `base`. Never more than the diff's size,
/// and less when lines only moved.
pub fn changed_lines(base: &str, other: &str) -> usize {
    let mut counts: HashMap<&str, i64> = HashMap::new();
    for line in lines(base) {
        *counts.entry(line).or_default() += 1;
    }
    for line in lines(other) {
        *counts.entry(line).or_default() -= 1;
    }
    counts.values().map(|n| n.unsigned_abs() as usize).sum()
}

/// Folds the disk's changes into the person's text.
pub fn merge(base: &str, mine: &str, disk: &str) -> Result<Merged, TooDifferent> {
    merge_with(base, mine, disk, MAX_CHANGED_LINES)
}

fn merge_with(base: &str, mine: &str, disk: &str, limit: usize) -> Result<Merged, TooDifferent> {
    if changed_lines(base, mine) > limit || changed_lines(base, disk) > limit {
        return Err(TooDifferent);
    }
    // diffy's markers are made longer than any such line the texts hold, so a marker line in the
    // result is diffy's own and can be given its label without touching the person's lines.
    let (len, own) = markers_unlike(&[base, mine, disk]);
    let mut options = MergeOptions::new();
    options
        .set_conflict_style(ConflictStyle::Merge)
        .set_incomplete_hunk_style(IncompleteHunkStyle::Git)
        .set_conflict_marker_length(len);
    let marked = match options.merge(base, mine, disk) {
        Ok(text) => return Ok(Merged { text, conflicts: 0 }),
        Err(marked) => marked,
    };
    let mut text = String::with_capacity(marked.len());
    let mut conflicts = 0;
    for line in marked.split_inclusive('\n') {
        let label = match line.strip_suffix('\n').unwrap_or(line) {
            l if l == own.start => Some(START),
            l if l == own.middle => Some(MIDDLE),
            l if l == own.end => Some(END),
            _ => None,
        };
        match label {
            Some(label) => {
                if label == START {
                    conflicts += 1;
                }
                text.push_str(label);
                text.push('\n');
            }
            None => text.push_str(line),
        }
    }
    Ok(Merged { text, conflicts })
}

/// The three marker lines diffy writes at one marker length.
struct Markers {
    start: String,
    middle: String,
    end: String,
}

/// The shortest marker length, from git's 7 up, at which none of the texts holds a line that
/// reads as one of diffy's markers.
fn markers_unlike(texts: &[&str]) -> (usize, Markers) {
    let mut len = 7;
    loop {
        let own = Markers {
            start: format!("{} ours", "<".repeat(len)),
            middle: "=".repeat(len),
            end: format!("{} theirs", ">".repeat(len)),
        };
        let held = texts
            .iter()
            .flat_map(|text| lines(text))
            .any(|l| l == own.start || l == own.middle || l == own.end);
        if !held {
            return (len, own);
        }
        len += 1;
    }
}

/// A text's lines without their `\n`. A last line with no `\n` is a line, and an empty text has none.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split_inclusive('\n')
        .map(|line| line.strip_suffix('\n').unwrap_or(line))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merged(text: &str, conflicts: u32) -> Result<Merged, TooDifferent> {
        Ok(Merged {
            text: text.to_string(),
            conflicts,
        })
    }

    #[test]
    fn edits_apart_merge_clean() {
        assert_eq!(merge("a\nb\nc\n", "A\nb\nc\n", "a\nb\nC\n"), merged("A\nb\nC\n", 0));
    }

    #[test]
    fn edits_to_one_line_conflict_as_mine_and_disk() {
        assert_eq!(
            merge("a\nb\nc\n", "a\nX\nc\n", "a\nY\nc\n"),
            merged("a\n<<<<<<< mine\nX\n=======\nY\n>>>>>>> disk\nc\n", 1)
        );
    }

    #[test]
    fn the_same_edit_on_both_sides_is_no_conflict() {
        assert_eq!(merge("a\nb\nc\n", "a\nZ\nc\n", "a\nZ\nc\n"), merged("a\nZ\nc\n", 0));
    }

    #[test]
    fn two_conflicts_are_counted() {
        let got = merge("1\n2\n3\n4\n5\n", "one\n2\n3\n4\nfive\n", "uno\n2\n3\n4\ncinco\n").unwrap();
        assert_eq!(got.conflicts, 2);
        assert_eq!(got.text.matches("<<<<<<< mine\n").count(), 2, "{}", got.text);
        assert_eq!(got.text.matches(">>>>>>> disk\n").count(), 2, "{}", got.text);
    }

    #[test]
    fn a_last_line_without_a_newline_keeps_markers_on_their_own_lines() {
        let got = merge("a", "b", "c").unwrap();
        assert_eq!(
            got.text.lines().collect::<Vec<_>>(),
            ["<<<<<<< mine", "b", "=======", "c", ">>>>>>> disk"]
        );
        assert_eq!(got.conflicts, 1);
    }

    #[test]
    fn an_empty_base_conflicts_two_new_texts() {
        let got = merge("", "a\n", "b\n").unwrap();
        assert_eq!(got.conflicts, 1);
        assert_eq!(got.text, "<<<<<<< mine\na\n=======\nb\n>>>>>>> disk\n");
        // The file was emptied on disk under an added line.
        let emptied = merge("x\n", "x\ny\n", "").unwrap();
        assert!(emptied.text.contains("y\n"), "{}", emptied.text);
    }

    #[test]
    fn a_text_that_holds_marker_lines_keeps_them() {
        let held = "<<<<<<< ours\n=======\n>>>>>>> theirs\n<<<<<<< mine\n";
        let base = format!("{held}b\n");
        let mine = format!("{held}X\n");
        let disk = format!("{held}Y\n");
        let got = merge(&base, &mine, &disk).unwrap();
        assert_eq!(got.conflicts, 1);
        assert_eq!(got.text, format!("{held}<<<<<<< mine\nX\n=======\nY\n>>>>>>> disk\n"));
    }

    #[test]
    fn a_line_as_long_as_the_longer_markers_is_kept_too() {
        // The merge's own markers are made longer than any line the texts hold, so these stay.
        let held = "<<<<<<<< ours\n========\n>>>>>>>> theirs\n<<<<<<< ours\n";
        let got = merge(&format!("{held}b\n"), &format!("{held}X\n"), &format!("{held}Y\n")).unwrap();
        assert_eq!(got.text, format!("{held}<<<<<<< mine\nX\n=======\nY\n>>>>>>> disk\n"));
        assert_eq!(got.conflicts, 1);
    }

    #[test]
    fn changed_lines_counts_both_ways() {
        assert_eq!(changed_lines("a\nb\nc\n", "a\nx\ny\nc\n"), 3);
        assert_eq!(changed_lines("a\nb\n", "a\nb\n"), 0);
        // Lines that only moved are not seen.
        assert_eq!(changed_lines("a\nb\n", "b\na\n"), 0);
        assert_eq!(changed_lines("a\na\n", "a\n"), 1);
        assert_eq!(changed_lines("", "a\n"), 1);
    }

    #[test]
    fn a_side_past_the_limit_is_too_different() {
        let base = "1\n2\n3\n4\n5\n6\n";
        // Three lines replaced is six lines changed: three gone, three new.
        assert_eq!(merge_with(base, "a\nb\nc\n4\n5\n6\n", base, 5), Err(TooDifferent));
        assert_eq!(merge_with(base, base, "a\nb\nc\n4\n5\n6\n", 5), Err(TooDifferent));
        assert!(merge_with(base, "a\nb\nc\n4\n5\n6\n", "1\n2\n3\nd\ne\nf\n", 6).is_ok());
    }
}
