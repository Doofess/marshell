//! The person's annotations, kept beside each transcript so rebuilding the index never loses them.

use crate::{paths, store};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Marks {
    pub name: Option<String>,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub archived: bool,
    /// Sorted, unique times in milliseconds since the Unix epoch, oldest first.
    pub bookmarks: Vec<u64>,
}

impl store::Versioned for Marks {
    const VERSION: u32 = 1;
}

pub fn marks_path(home: &Path, agent: &str, session_id: &str) -> PathBuf {
    paths::archive_dir(home, agent).join(format!("{session_id}.marks.json"))
}

/// Uses the store's backup recovery. Missing marks are the default; marks that can't be read are an error.
pub fn try_read(home: &Path, agent: &str, session_id: &str) -> anyhow::Result<Marks> {
    Ok(store::read::<Marks>(&marks_path(home, agent, session_id))?.unwrap_or_default())
}

/// Uses the store's backup recovery; missing or unreadable annotations leave the conversation usable.
pub fn read(home: &Path, agent: &str, session_id: &str) -> Marks {
    try_read(home, agent, session_id).unwrap_or_else(|err| {
        tracing::warn!(
            "could not read history marks {}: {err:#}",
            marks_path(home, agent, session_id).display()
        );
        Marks::default()
    })
}

pub fn write(home: &Path, agent: &str, session_id: &str, marks: &Marks) -> anyhow::Result<()> {
    store::write(&marks_path(home, agent, session_id), marks)
}

/// Controls, including Unicode bidi controls, cannot change the direction or layout of a label.
fn label_text(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_control()
                || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                ' '
            } else {
                c
            }
        })
        .collect()
}

pub fn normalize_name(input: &str) -> Option<String> {
    let name: String = label_text(input).trim().chars().take(120).collect();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

pub fn normalize_tags(input: &[String]) -> Result<Vec<String>, &'static str> {
    let mut tags = Vec::new();
    let mut keys = std::collections::HashSet::new();
    for tag in input {
        let text = label_text(tag);
        let tag = text.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.contains(',') {
            return Err("A tag can't contain a comma.");
        }
        if tag.chars().count() > 40 {
            return Err("A tag can be up to 40 characters.");
        }
        if keys.insert(tag.to_lowercase()) {
            tags.push(tag.to_string());
            if tags.len() > 20 {
                return Err("A conversation can have up to 20 tags.");
            }
        }
    }
    Ok(tags)
}

pub fn add_bookmark(marks: &mut Marks, at_ms: u64) {
    marks.bookmarks.push(at_ms);
    marks.bookmarks.sort_unstable();
    marks.bookmarks.dedup();
    let excess = marks.bookmarks.len().saturating_sub(200);
    marks.bookmarks.drain(..excess);
}

/// The last turn that has started by this time; before the first prompt (or without turns), turn 0.
pub fn turn_for(at_ms: u64, turns: &[(u32, u64)]) -> u32 {
    turns
        .iter()
        .rev()
        .find(|(_, at)| *at <= at_ms)
        .map(|(turn, _)| *turn)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_table() {
        for (input, want) in [
            ("  x  ", Some("x")),
            ("", None),
            ("   ", None),
            ("a\nb\u{0}c", Some("a b c")),
            ("\u{202e}x", Some("x")),
            ("<img src=x onerror=alert(1)>", Some("<img src=x onerror=alert(1)>")),
        ] {
            assert_eq!(normalize_name(input).as_deref(), want, "{input:?}");
        }
        assert_eq!(normalize_name(&"界".repeat(10_000)), Some("界".repeat(120)));
    }

    #[test]
    fn normalize_tags_table() {
        let tags = |items: &[&str]| items.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        assert_eq!(normalize_tags(&tags(&[" Bug ", "bug", "ui"])).unwrap(), ["Bug", "ui"]);
        assert_eq!(normalize_tags(&tags(&["", "  ", "ui", ""])).unwrap(), ["ui"]);
        assert_eq!(
            normalize_tags(&tags(&["a\nb\u{0}c", "\u{202e}x"])).unwrap(),
            ["a b c", "x"]
        );
        assert_eq!(normalize_tags(&tags(&["a,b"])), Err("A tag can't contain a comma."));
        assert_eq!(
            normalize_tags(&["界".repeat(41)]),
            Err("A tag can be up to 40 characters.")
        );
        let many = (0..21).map(|i| format!("tag{i}")).collect::<Vec<_>>();
        assert_eq!(normalize_tags(&many), Err("A conversation can have up to 20 tags."));
    }

    #[test]
    fn bookmarks_stay_sorted_unique_and_capped_at_200() {
        let mut marks = Marks::default();
        for at in (0..201).rev() {
            add_bookmark(&mut marks, at);
            add_bookmark(&mut marks, at);
        }
        assert_eq!(marks.bookmarks, (1..=200).collect::<Vec<_>>());
        add_bookmark(&mut marks, 300);
        assert_eq!(marks.bookmarks.len(), 200);
        assert_eq!(marks.bookmarks.first(), Some(&2));
        assert_eq!(marks.bookmarks.last(), Some(&300));
    }

    #[test]
    fn turn_for_table() {
        let turns = [(0, 100), (1, 200), (2, 300)];
        for (at, want) in [(50, 0), (100, 0), (250, 1), (999, 2)] {
            assert_eq!(turn_for(at, &turns), want);
        }
        assert_eq!(turn_for(999, &[]), 0);
    }

    #[test]
    fn missing_or_broken_marks_read_as_default() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(read(home.path(), "claude", "a"), Marks::default());
        let path = marks_path(home.path(), "claude", "a");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"broken json").unwrap();
        assert_eq!(read(home.path(), "claude", "a"), Marks::default());
        std::fs::write(&path, br#"{"version":1}"#).unwrap();
        assert_eq!(read(home.path(), "claude", "a"), Marks::default());
    }

    #[test]
    fn marks_round_trip_with_version_and_backup() {
        let home = tempfile::tempdir().unwrap();
        let marks = Marks {
            name: Some("My name".into()),
            tags: vec!["Bug".into(), "ui".into()],
            pinned: true,
            archived: true,
            bookmarks: vec![10, 20],
        };
        write(home.path(), "claude", "a", &marks).unwrap();
        let path = marks_path(home.path(), "claude", "a");
        assert_eq!(
            path,
            crate::paths::archive_dir(home.path(), "claude").join("a.marks.json")
        );
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(read(home.path(), "claude", "a"), marks);
        write(home.path(), "claude", "a", &Marks::default()).unwrap();
        assert_eq!(
            crate::store::read::<Marks>(&crate::store::bak_path(&path)).unwrap(),
            Some(marks.clone())
        );
        std::fs::write(&path, b"broken json").unwrap();
        assert_eq!(read(home.path(), "claude", "a"), marks);
    }
}
