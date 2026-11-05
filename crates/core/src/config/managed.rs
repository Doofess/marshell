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

/// Records that `changeset_id` set the pointer. An entry that is there already stays as it is, so the change that
/// first set it is the one an undo of a later change that sets it again does not forget. An entry this same change
/// removed earlier comes back as it was, for the same reason: a repair removes Marshell's entry and appends it again.
fn upsert(
    entries: &mut Vec<ManagedEntry>,
    dropped: &mut Vec<ManagedEntry>,
    path: &str,
    pointer: Option<&str>,
    changeset_id: &str,
    now_ms: u64,
) {
    let same = |e: &ManagedEntry| same_path(&e.path, path) && e.pointer.as_deref() == pointer;
    if entries.iter().any(same) {
        return;
    }
    if let Some(i) = dropped.iter().position(same) {
        entries.push(dropped.remove(i));
        return;
    }
    entries.push(ManagedEntry {
        path: path.to_string(),
        pointer: pointer.map(str::to_string),
        changeset_id: changeset_id.to_string(),
        applied_ms: now_ms,
    });
}

/// The `<array>/-` pointers of every array `pointer` goes into: each parent of an all-digit segment.
fn appended_records(pointer: &str) -> Vec<String> {
    let segments: Vec<&str> = pointer.split('/').collect();
    (1..segments.len())
        .filter(|&i| !segments[i].is_empty() && segments[i].bytes().all(|b| b.is_ascii_digit()))
        .map(|i| format!("{}/-", segments[..i].join("/")))
        .collect()
}

/// Computes the new list without writing it, so apply can write files first and roll back if this fails.
pub fn updated(home: &Path, changeset_id: &str, changes: &[Change], now_ms: u64) -> anyhow::Result<Vec<ManagedEntry>> {
    let mut entries = list(home)?;
    // What this change's removes took out, in case a later step of the same change sets it again.
    let mut dropped = Vec::new();
    for change in changes {
        match change {
            Change::Set { path, pointer } => upsert(
                &mut entries,
                &mut dropped,
                &path.to_string_lossy(),
                Some(pointer),
                changeset_id,
                now_ms,
            ),
            Change::Remove { path, pointer } => {
                let path = path.to_string_lossy();
                let child = format!("{pointer}/");
                // Removing an element of an array leaves nothing appended to that array that this record still
                // describes, so the array's `<array>/-` record goes too.
                let appended = appended_records(pointer);
                let under = |p: &str, root: &str| p == root || p.starts_with(&format!("{root}/"));
                let (gone, kept): (Vec<_>, Vec<_>) = entries.into_iter().partition(|e| {
                    same_path(&e.path, &path)
                        && e.pointer.as_deref().is_some_and(|p| {
                            p == *pointer || p.starts_with(&child) || appended.iter().any(|a| under(p, a))
                        })
                });
                entries = kept;
                dropped.extend(gone);
            }
            Change::Created { path } => upsert(
                &mut entries,
                &mut dropped,
                &path.to_string_lossy(),
                None,
                changeset_id,
                now_ms,
            ),
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

    fn pointers(home: &Path) -> Vec<String> {
        list(home).unwrap().into_iter().filter_map(|e| e.pointer).collect()
    }

    fn set(pointer: &'static str) -> Change<'static> {
        Change::Set { path: file(), pointer }
    }

    fn remove(pointer: &'static str) -> Change<'static> {
        Change::Remove { path: file(), pointer }
    }

    #[test]
    fn removing_an_element_of_an_array_drops_that_arrays_appended_record() {
        let home = tempfile::tempdir().unwrap();
        apply(
            home.path(),
            "c1",
            &[
                set("/hooks/Stop/-"),
                set("/hooks/Stop/-/hooks/0"),
                set("/hooks/Start/-"),
                set("/statusLine"),
            ],
            1,
        );
        apply(home.path(), "c2", &[remove("/hooks/Stop/1")], 2);
        // Another array's record and an unrelated key stay.
        assert_eq!(pointers(home.path()), ["/hooks/Start/-", "/statusLine"]);
    }

    #[test]
    fn removing_deeper_in_an_array_element_drops_each_arrays_record() {
        let home = tempfile::tempdir().unwrap();
        apply(
            home.path(),
            "c1",
            &[
                set("/hooks/Stop/-"),
                set("/hooks/Stop/1/hooks/-"),
                set("/hooks/Start/-"),
            ],
            1,
        );
        apply(home.path(), "c2", &[remove("/hooks/Stop/1/hooks/2")], 2);
        assert_eq!(pointers(home.path()), ["/hooks/Start/-"]);
    }

    #[test]
    fn removing_a_key_keeps_the_appended_records_of_other_keys() {
        let home = tempfile::tempdir().unwrap();
        apply(home.path(), "c1", &[set("/hooks/Stop/-"), set("/hooks/Start/-")], 1);
        apply(home.path(), "c2", &[remove("/hooks/Start")], 2);
        assert_eq!(pointers(home.path()), ["/hooks/Stop/-"]);
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
        // The first writer stays.
        let entries = list(home.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!((entries[0].changeset_id.as_str(), entries[0].applied_ms), ("c1", 1));
        assert_eq!(entries[0].pointer.as_deref(), Some("/a"));
    }

    #[test]
    fn undoing_a_change_that_set_a_pointer_again_keeps_the_first_entry() {
        let home = tempfile::tempdir().unwrap();
        apply(home.path(), "c1", &[set("/hooks/Stop"), set("/statusLine")], 1);
        // c2 sets one of them again and adds one more.
        apply(home.path(), "c2", &[set("/hooks/Stop"), set("/hooks/Start")], 2);
        apply(
            home.path(),
            "c3",
            &[Change::Restored {
                path: file(),
                changeset_id: "c2",
            }],
            3,
        );
        assert_eq!(pointers(home.path()), ["/hooks/Stop", "/statusLine"]);
    }

    // Found in the 7a final review: a repair removes Marshell's entry and appends it again in one change. The remove
    // dropped the setup's record and the append made a new one under the repair, so undoing the repair forgot the
    // entry the file still held.
    #[test]
    fn a_change_that_removes_and_sets_a_pointer_again_keeps_the_first_entry() {
        let home = tempfile::tempdir().unwrap();
        apply(home.path(), "c1", &[set("/hooks/Stop/-"), set("/statusLine")], 1);
        apply(home.path(), "c2", &[remove("/hooks/Stop/1"), set("/hooks/Stop/-")], 2);
        let stop: Vec<_> = list(home.path())
            .unwrap()
            .into_iter()
            .filter(|e| e.pointer.as_deref() == Some("/hooks/Stop/-"))
            .map(|e| (e.changeset_id, e.applied_ms))
            .collect();
        assert_eq!(stop, [("c1".to_string(), 1)]);
        apply(
            home.path(),
            "c3",
            &[Change::Restored {
                path: file(),
                changeset_id: "c2",
            }],
            3,
        );
        let mut left = pointers(home.path());
        left.sort();
        assert_eq!(left, ["/hooks/Stop/-", "/statusLine"]);
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
