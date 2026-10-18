//! Turns a ChangeSet into the bytes each file would have and a per-file unified diff, without writing
//! anything. A file that is refused (managed, read-only, unparseable, changed since the user looked) is
//! never described as about to change: its preview has an empty diff.

use crate::config::guard::{Guard, Refusal};
use crate::config::hash::hash_bytes;
use crate::config::jsonc::{EditError, JsonOp, apply_ops};
use crate::config::snapshots::Snapshots;
use crate::config::write::resolve_link;
use marshell_protocol::config::{
    Applies, ChangeSet, ConfigPreview, ConfigScope, EditOp, FileEdit, FilePreview, FileStatus, RefuseReason,
};
use serde_json::Value;
use std::path::PathBuf;

/// A config file bigger than this is refused rather than read.
pub const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

/// One file of a ChangeSet, with the bytes it has now and the bytes it would have.
#[derive(Debug, Clone)]
pub struct PlannedFile {
    /// The path as the ChangeSet named it.
    pub path: PathBuf,
    /// Where a write lands: `path` with links and `..` resolved. Equal to `path` for a refused file.
    pub target: PathBuf,
    pub scope: ConfigScope,
    pub applies: Applies,
    /// `None` when the file doesn't exist, and for a refused file.
    pub before: Option<Vec<u8>>,
    /// `None` when the file would not exist (a restore of a deleted file), and for a refused file.
    pub after: Option<Vec<u8>>,
    pub status: FileStatus,
    /// This file's ops, in order, for `managed.json`.
    pub ops: Vec<EditOp>,
}

/// Groups the edits by path (first-seen order) and works out each file's new bytes. Never writes.
pub fn plan(cs: &ChangeSet, guard: &Guard, snapshots: &Snapshots) -> Vec<PlannedFile> {
    let mut groups: Vec<(&str, Vec<&FileEdit>)> = Vec::new();
    for edit in &cs.edits {
        match groups.iter_mut().find(|(path, _)| *path == edit.path) {
            Some((_, edits)) => edits.push(edit),
            None => groups.push((&edit.path, vec![edit])),
        }
    }
    groups
        .into_iter()
        .map(|(path, edits)| plan_file(path, &edits, guard, snapshots))
        .collect()
}

pub fn to_preview(changeset_id: &str, cs: &ChangeSet, files: &[PlannedFile]) -> ConfigPreview {
    let previews: Vec<FilePreview> = files.iter().map(file_preview).collect();
    let any_refused = files.iter().any(|f| matches!(f.status, FileStatus::Refused { .. }));
    let any_changes = files.iter().any(|f| f.status == FileStatus::Changes);
    ConfigPreview {
        changeset_id: changeset_id.to_string(),
        title: cs.title.clone(),
        reason: cs.reason.clone(),
        files: previews,
        stale: false,
        can_apply: !any_refused && any_changes,
    }
}

fn file_preview(file: &PlannedFile) -> FilePreview {
    let diff = if file.status == FileStatus::Changes {
        let text = |bytes: &Option<Vec<u8>>| bytes.as_deref().map(|b| String::from_utf8_lossy(b).into_owned());
        let before = text(&file.before).unwrap_or_default();
        let after = text(&file.after).unwrap_or_default();
        diffy::create_patch(&before, &after).to_string()
    } else {
        String::new()
    };
    FilePreview {
        path: file.path.to_string_lossy().into_owned(),
        scope: file.scope,
        applies: file.applies,
        before_hash: file.before.as_deref().map(hash_bytes),
        after_hash: file.after.as_deref().map(hash_bytes),
        diff,
        status: file.status.clone(),
    }
}

fn plan_file(path: &str, edits: &[&FileEdit], guard: &Guard, snapshots: &Snapshots) -> PlannedFile {
    let path = PathBuf::from(path);
    let mut file = PlannedFile {
        target: path.clone(),
        path,
        scope: edits[0].scope,
        applies: edits[0].applies,
        before: None,
        after: None,
        status: FileStatus::NoChange,
        ops: edits.iter().map(|e| e.op.clone()).collect(),
    };
    match compute(&mut file, edits, guard, snapshots) {
        Ok((before, after)) => {
            file.status = if before == after {
                FileStatus::NoChange
            } else {
                FileStatus::Changes
            };
            file.before = before;
            file.after = after;
        }
        Err((reason, detail)) => file.status = FileStatus::Refused { reason, detail },
    }
    file
}

type Refused = (RefuseReason, String);
type Bytes = Option<Vec<u8>>;

fn refuse<T>(reason: RefuseReason, detail: impl Into<String>) -> Result<T, Refused> {
    Err((reason, detail.into()))
}

/// The file's bytes before and after, or the first reason it can't be changed. Sets `file.target`.
fn compute(
    file: &mut PlannedFile,
    edits: &[&FileEdit],
    guard: &Guard,
    snapshots: &Snapshots,
) -> Result<(Bytes, Bytes), Refused> {
    let resolved = guard.check(&file.path).map_err(|r| match r {
        Refusal::NotAbsolute => (RefuseReason::NotAbsolute, "The path is not absolute.".to_string()),
        Refusal::Managed => (
            RefuseReason::Managed,
            "This is a managed or organization file.".to_string(),
        ),
        Refusal::OutsideAllowed => (
            RefuseReason::OutsideAllowed,
            "The path is outside the home folder and the project folders.".to_string(),
        ),
        Refusal::MarshellOwn => (
            RefuseReason::MarshellOwn,
            "This is Marshell's own data folder.".to_string(),
        ),
    })?;
    let target = match resolve_link(&resolved) {
        Ok(target) => target,
        Err(e) => return refuse(RefuseReason::Unreadable, format!("Could not follow the link: {e}")),
    };
    file.target = target.clone();

    let before = match std::fs::metadata(&target) {
        Ok(meta) => {
            if meta.len() > MAX_CONFIG_BYTES {
                return refuse(RefuseReason::TooLarge, "The file is larger than 4 MiB.");
            }
            if meta.permissions().readonly() {
                return refuse(RefuseReason::ReadOnly, "The file is read-only.");
            }
            match std::fs::read(&target) {
                Ok(bytes) => Some(bytes),
                Err(e) => return refuse(RefuseReason::Unreadable, format!("Could not read the file: {e}")),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return refuse(RefuseReason::Unreadable, format!("Could not read the file: {e}")),
    };

    let before_hash = before.as_deref().map(hash_bytes);
    if edits
        .iter()
        .any(|e| e.expected_hash.is_some() && e.expected_hash != before_hash)
    {
        return refuse(RefuseReason::Conflict, "The file changed since the edit was made.");
    }

    let mut working = before.clone();
    for edit in edits {
        working = apply_op(&edit.op, working, &target, snapshots)?;
    }
    Ok((before, working))
}

fn apply_op(op: &EditOp, working: Bytes, target: &std::path::Path, snapshots: &Snapshots) -> Result<Bytes, Refused> {
    match op {
        EditOp::JsonSet { pointer, value } => {
            let text = json_text(working, "{}\n")?;
            let value: Value = match serde_json::from_str(value) {
                Ok(v) => v,
                Err(e) => return refuse(RefuseReason::BadValue, format!("The value is not JSON: {e}")),
            };
            edit_json(&text, JsonOp::Set { pointer, value: &value })
        }
        EditOp::JsonRemove { pointer } => {
            if working.is_none() {
                return Ok(None);
            }
            let text = json_text(working, "{}\n")?;
            edit_json(&text, JsonOp::Remove { pointer })
        }
        EditOp::CreateFile { content } => {
            if working.is_some() {
                return refuse(RefuseReason::Exists, "The file already exists.");
            }
            Ok(Some(content.clone().into_bytes()))
        }
        EditOp::Restore { snapshot_id } => match snapshots.blob(snapshot_id, target) {
            Ok(Some(bytes)) => Ok(bytes),
            Ok(None) => refuse(RefuseReason::SnapshotMissing, "The snapshot has no copy of this file."),
            Err(e) => refuse(RefuseReason::Unreadable, format!("Could not read the snapshot: {e:#}")),
        },
    }
}

/// The file as text for a JSON edit; a missing file starts from `empty`.
fn json_text(working: Bytes, empty: &str) -> Result<String, Refused> {
    match working {
        None => Ok(empty.to_string()),
        Some(bytes) => match String::from_utf8(bytes) {
            Ok(text) => Ok(text),
            Err(_) => refuse(RefuseReason::NotUtf8, "The file is not UTF-8 text."),
        },
    }
}

fn edit_json(text: &str, op: JsonOp) -> Result<Bytes, Refused> {
    match apply_ops(text, &[op]) {
        Ok(new_text) => Ok(Some(new_text.into_bytes())),
        Err(EditError::DoesNotParse(detail)) | Err(EditError::Internal(detail)) => {
            refuse(RefuseReason::DoesNotParse, detail)
        }
        Err(EditError::BadPointer { pointer, detail }) => {
            refuse(RefuseReason::BadPointer, format!("{pointer}: {detail}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::canonical;
    use std::path::Path;

    fn setup() -> (tempfile::TempDir, PathBuf, Guard, Snapshots) {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical(dir.path());
        let guard = Guard::new(vec![root.clone()], vec![], root.join("home"));
        let snapshots = Snapshots::new(&root.join("home"));
        (dir, root, guard, snapshots)
    }

    fn edit(path: &Path, op: EditOp, expected_hash: Option<String>) -> FileEdit {
        FileEdit {
            path: path.to_string_lossy().into_owned(),
            scope: ConfigScope::User,
            op,
            expected_hash,
            applies: Applies::NewSessions,
        }
    }

    fn set(pointer: &str, value: &str) -> EditOp {
        EditOp::JsonSet {
            pointer: pointer.into(),
            value: value.into(),
        }
    }

    fn changeset(edits: Vec<FileEdit>) -> ChangeSet {
        ChangeSet {
            title: "t".into(),
            reason: "r".into(),
            edits,
        }
    }

    fn refused(file: &PlannedFile) -> Option<RefuseReason> {
        match &file.status {
            FileStatus::Refused { reason, .. } => Some(*reason),
            _ => None,
        }
    }

    #[test]
    fn json_set_on_a_missing_file_creates_it() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("settings.json");
        let cs = changeset(vec![edit(&path, set("/a", "1"), None)]);
        let files = plan(&cs, &guard, &snapshots);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, FileStatus::Changes);
        assert_eq!(files[0].before, None);
        let after = String::from_utf8(files[0].after.clone().unwrap()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(value, serde_json::json!({"a": 1}));
        let preview = to_preview("id", &cs, &files);
        assert_eq!(preview.files[0].before_hash, None);
        assert_eq!(preview.files[0].after_hash, Some(hash_bytes(after.as_bytes())));
        assert!(preview.can_apply);
        assert!(!preview.stale);
    }

    #[test]
    fn a_managed_path_is_refused_and_shows_no_diff() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("managed-settings.json");
        let cs = changeset(vec![edit(&path, set("/a", "1"), None)]);
        let files = plan(&cs, &guard, &snapshots);
        assert_eq!(refused(&files[0]), Some(RefuseReason::Managed));
        assert_eq!(files[0].before, None);
        assert_eq!(files[0].after, None);
        let preview = to_preview("id", &cs, &files);
        assert_eq!(preview.files[0].diff, "");
        assert!(!preview.can_apply);
    }

    #[test]
    fn expected_hash_mismatch_is_a_conflict() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("settings.json");
        std::fs::write(&path, b"{}\n").unwrap();
        let cs = changeset(vec![edit(&path, set("/a", "1"), Some(hash_bytes(b"something else")))]);
        let files = plan(&cs, &guard, &snapshots);
        assert_eq!(refused(&files[0]), Some(RefuseReason::Conflict));
        assert!(!to_preview("id", &cs, &files).can_apply);
        // The matching hash is accepted.
        let cs = changeset(vec![edit(&path, set("/a", "1"), Some(hash_bytes(b"{}\n")))]);
        assert_eq!(plan(&cs, &guard, &snapshots)[0].status, FileStatus::Changes);
    }

    #[test]
    fn two_edits_to_one_file_make_one_preview() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("settings.json");
        std::fs::write(&path, b"{}\n").unwrap();
        let cs = changeset(vec![
            edit(&path, set("/a", "1"), None),
            edit(&path, set("/b", "2"), None),
        ]);
        let files = plan(&cs, &guard, &snapshots);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].ops.len(), 2);
        let after = String::from_utf8(files[0].after.clone().unwrap()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(value, serde_json::json!({"a": 1, "b": 2}));
        assert_eq!(to_preview("id", &cs, &files).files.len(), 1);
    }

    #[test]
    fn create_file_over_an_existing_file_is_refused() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("notes.md");
        std::fs::write(&path, b"old").unwrap();
        let op = EditOp::CreateFile { content: "new".into() };
        let files = plan(&changeset(vec![edit(&path, op, None)]), &guard, &snapshots);
        assert_eq!(refused(&files[0]), Some(RefuseReason::Exists));
    }

    #[test]
    fn unparseable_file_is_refused_not_described_as_changing() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("settings.json");
        std::fs::write(&path, b"{ \"a\": ").unwrap();
        let cs = changeset(vec![edit(&path, set("/b", "1"), None)]);
        let files = plan(&cs, &guard, &snapshots);
        assert_eq!(refused(&files[0]), Some(RefuseReason::DoesNotParse));
        assert_eq!(to_preview("id", &cs, &files).files[0].diff, "");
    }

    #[test]
    fn read_only_file_is_refused() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("settings.json");
        std::fs::write(&path, b"{}\n").unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions).unwrap();
        let files = plan(&changeset(vec![edit(&path, set("/a", "1"), None)]), &guard, &snapshots);
        // Clear it again so the temp folder can be removed on Windows.
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
        assert_eq!(refused(&files[0]), Some(RefuseReason::ReadOnly));
    }

    #[test]
    fn remove_of_missing_pointer_is_no_change() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("settings.json");
        std::fs::write(&path, b"{\"a\": 1}\n").unwrap();
        let op = EditOp::JsonRemove { pointer: "/zzz".into() };
        let cs = changeset(vec![edit(&path, op, None)]);
        let files = plan(&cs, &guard, &snapshots);
        assert_eq!(files[0].status, FileStatus::NoChange);
        let preview = to_preview("id", &cs, &files);
        assert_eq!(preview.files[0].diff, "");
        assert!(!preview.can_apply);
    }

    #[test]
    fn diff_is_unified() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("settings.json");
        std::fs::write(&path, b"{}\n").unwrap();
        let cs = changeset(vec![edit(&path, set("/a", "1"), None)]);
        let files = plan(&cs, &guard, &snapshots);
        let diff = to_preview("id", &cs, &files).files.remove(0).diff;
        assert!(diff.contains("@@"), "{diff}");
        assert!(diff.lines().any(|l| l.starts_with('+') && l.contains('a')), "{diff}");
    }

    #[test]
    fn bad_value_text_is_refused() {
        let (_dir, root, guard, snapshots) = setup();
        let path = root.join("settings.json");
        let files = plan(
            &changeset(vec![edit(&path, set("/a", "{nope"), None)]),
            &guard,
            &snapshots,
        );
        assert_eq!(refused(&files[0]), Some(RefuseReason::BadValue));
    }
}
