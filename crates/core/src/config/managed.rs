//! `<home>/managed.json`: the entries Marshell wrote into CLI config files, so the UI can show
//! what is ours and a restore can forget what it undid.

use crate::projects::same_path;
use crate::store::{self, Versioned};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ManagedEntry {
    pub path: String,
    /// The JSON pointer Marshell set; `None` for a whole file it created.
    pub pointer: Option<String>,
    pub changeset_id: String,
    pub applied_ms: u64,
}

/// What a ChangeSet did to one path.
pub enum Change<'a> {
    Set { path: &'a Path, pointer: &'a str },
    Remove { path: &'a Path, pointer: &'a str },
    Created { path: &'a Path },
    Deleted { path: &'a Path },
    Restored { path: &'a Path, changeset_id: &'a str },
}

#[derive(Serialize, Deserialize, Default)]
struct File {
    #[serde(default)]
    entries: Vec<ManagedEntry>,
}

impl Versioned for File {
    const VERSION: u32 = 1;
}

pub fn list(home: &Path) -> anyhow::Result<Vec<ManagedEntry>> {
    Ok(store::read::<File>(&crate::paths::managed_file(home))?
        .unwrap_or_default()
        .entries)
}

fn upsert(entries: &mut Vec<ManagedEntry>, path: &str, pointer: Option<&str>, changeset_id: &str, now_ms: u64) {
    let entry = ManagedEntry {
        path: path.to_string(),
        pointer: pointer.map(str::to_string),
        changeset_id: changeset_id.to_string(),
        applied_ms: now_ms,
    };
    match entries
        .iter_mut()
        .find(|e| same_path(&e.path, path) && e.pointer.as_deref() == pointer)
    {
        Some(existing) => *existing = entry,
        None => entries.push(entry),
    }
}

/// Computes the new list without writing it, so apply can write files first and roll back if this fails.
pub fn updated(home: &Path, changeset_id: &str, changes: &[Change], now_ms: u64) -> anyhow::Result<Vec<ManagedEntry>> {
    let mut entries = list(home)?;
    for change in changes {
        match change {
            Change::Set { path, pointer } => upsert(
                &mut entries,
                &path.to_string_lossy(),
                Some(pointer),
                changeset_id,
                now_ms,
            ),
            Change::Remove { path, pointer } => {
                let path = path.to_string_lossy();
                let child = format!("{pointer}/");
                entries.retain(|e| {
                    !(same_path(&e.path, &path)
                        && e.pointer
                            .as_deref()
                            .is_some_and(|p| p == *pointer || p.starts_with(&child)))
                });
            }
            Change::Created { path } => upsert(&mut entries, &path.to_string_lossy(), None, changeset_id, now_ms),
            // Nothing of Marshell's is left in a file that is gone, so its entries go.
            Change::Deleted { path } => {
                let path = path.to_string_lossy();
                entries.retain(|e| !same_path(&e.path, &path));
            }
            Change::Restored {
                path,
                changeset_id: undone,
            } => {
                let path = path.to_string_lossy();
                entries.retain(|e| !(same_path(&e.path, &path) && e.changeset_id == *undone));
            }
        }
    }
    Ok(entries)
}

pub fn save(home: &Path, entries: &[ManagedEntry]) -> anyhow::Result<()> {
    store::write(
        &crate::paths::managed_file(home),
        &File {
            entries: entries.to_vec(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn file() -> &'static Path {
        Path::new("/home/u/.claude/settings.json")
    }

    fn apply(home: &Path, id: &str, changes: &[Change], now: u64) {
        let next = updated(home, id, changes, now).unwrap();
        save(home, &next).unwrap();
    }

    #[test]
    fn set_then_remove_leaves_nothing() {
        let home = tempfile::tempdir().unwrap();
        apply(
            home.path(),
            "c1",
            &[Change::Set {
                path: file(),
                pointer: "/hooks/Stop",
            }],
            1,
        );
        assert_eq!(list(home.path()).unwrap().len(), 1);
        apply(
            home.path(),
            "c2",
            &[Change::Remove {
                path: file(),
                pointer: "/hooks/Stop",
            }],
            2,
        );
        assert!(list(home.path()).unwrap().is_empty());
    }

    #[test]
    fn set_twice_keeps_one_entry() {
        let home = tempfile::tempdir().unwrap();
        apply(
            home.path(),
            "c1",
            &[Change::Set {
                path: file(),
                pointer: "/a",
            }],
            1,
        );
        apply(
            home.path(),
            "c2",
            &[Change::Set {
                path: file(),
                pointer: "/a",
            }],
            2,
        );
        let entries = list(home.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!((entries[0].changeset_id.as_str(), entries[0].applied_ms), ("c2", 2));
        assert_eq!(entries[0].pointer.as_deref(), Some("/a"));
    }

    #[test]
    fn removing_a_parent_drops_its_children() {
        let home = tempfile::tempdir().unwrap();
        let changes = [
            Change::Set {
                path: file(),
                pointer: "/hooks/Stop",
            },
            Change::Set {
                path: file(),
                pointer: "/hooks/Notification",
            },
            Change::Set {
                path: file(),
                pointer: "/hooksX",
            },
        ];
        apply(home.path(), "c1", &changes, 1);
        apply(
            home.path(),
            "c2",
            &[Change::Remove {
                path: file(),
                pointer: "/hooks",
            }],
            2,
        );
        let entries = list(home.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].pointer.as_deref(), Some("/hooksX"));
    }

    #[test]
    fn restore_drops_only_that_changesets_entries() {
        let home = tempfile::tempdir().unwrap();
        apply(
            home.path(),
            "c1",
            &[Change::Set {
                path: file(),
                pointer: "/a",
            }],
            1,
        );
        apply(
            home.path(),
            "c2",
            &[
                Change::Set {
                    path: file(),
                    pointer: "/b",
                },
                Change::Created {
                    path: Path::new("/x/new.json"),
                },
            ],
            2,
        );
        apply(
            home.path(),
            "c3",
            &[Change::Restored {
                path: file(),
                changeset_id: "c2",
            }],
            3,
        );
        let mut left: Vec<_> = list(home.path())
            .unwrap()
            .into_iter()
            .map(|e| (e.path, e.pointer))
            .collect();
        left.sort();
        assert_eq!(left.len(), 2);
        assert!(left.iter().any(|(_, p)| p.as_deref() == Some("/a")));
        assert!(left.iter().any(|(path, p)| path.ends_with("new.json") && p.is_none()));
    }

    #[test]
    fn a_newer_managed_file_is_left_alone() {
        let home = tempfile::tempdir().unwrap();
        let path = crate::paths::managed_file(home.path());
        let newer = r#"{"version":9,"entries":[]}"#;
        std::fs::write(&path, newer).unwrap();
        assert!(updated(home.path(), "c1", &[Change::Created { path: file() }], 1).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
    }
}
