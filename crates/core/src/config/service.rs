//! `ConfigService`: the one place a ChangeSet is previewed and applied. It keeps the previews it has
//! shown, a lock per file, and the hash of every write it made itself (so the watcher can tell its own
//! writes from outside edits). Everything is keyed by the guard-resolved, link-resolved target path.
//!
//! Lock order: a per-path lock may be held while taking one of the short `Mutex`es below, never the
//! other way round, and no short `Mutex` is held across file I/O (beyond a hash).
//!
//! `managed` is held across file I/O on purpose, by `apply` alone, from reading `managed.json` to
//! saving it. It is taken after the path locks, and `apply` takes only the short `Mutex`es under it
//! (`own_writes`), never `watcher` and never a wait on the watcher. Nothing takes a path lock while
//! holding it.
//!
//! `watcher` is a leaf lock only `preview` takes, around registering a folder with
//! the OS. Nothing is taken or waited on under it, and the watch callback never touches it.
//!
//! The watcher's thread holds a `Weak<Shared>`, upgraded only for the length of one callback, so
//! dropping the last `ConfigService` drops the watcher, which ends its threads.

use crate::auth::new_id;
use crate::config::guard::Guard;
use crate::config::hash::{hash_bytes, hash_file};
use crate::config::managed::{self, Change};
use crate::config::preview::{PlannedFile, path_refusal, plan, to_preview};
use crate::config::snapshots::{SnapshotInput, Snapshots};
use crate::config::watch::Watcher;
use crate::config::write;
use anyhow::anyhow;
use marshell_protocol::config::{
    AppliedFile, ApplyRequest, ApplyResult, ChangeSet, ConfigPreview, EditOp, FileEdit, FileStatus,
};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Previews kept; when there are more, the oldest go first.
pub const MAX_PREVIEWS: usize = 32;
/// A preview older than this is dropped.
pub const PREVIEW_TTL: Duration = Duration::from_secs(600);

#[derive(Debug)]
pub enum ApplyError {
    UnknownChangeSet,
    /// Nothing was written.
    Conflict {
        reason: String,
        paths: Vec<String>,
    },
    Failed(anyhow::Error),
}

fn conflict(reason: &str, paths: Vec<String>) -> ApplyError {
    ApplyError::Conflict {
        reason: reason.to_string(),
        paths,
    }
}

/// A poisoned lock must not take the API down with it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// What `apply_with` calls to write one file; `None` bytes removes it.
type Writer<'a> = &'a dyn Fn(&Path, Option<&[u8]>) -> io::Result<()>;

fn real_write(path: &Path, bytes: Option<&[u8]>) -> io::Result<()> {
    match bytes {
        Some(bytes) => write::replace_file(path, bytes),
        None => write::remove_file(path),
    }
}

struct Stored {
    seq: u64,
    cs: ChangeSet,
    preview: ConfigPreview,
    /// The planned files' targets, in the preview's order.
    targets: Vec<PathBuf>,
    created: Instant,
    /// The person's own edit (Settings' form or raw text): `managed.json` records none of its keys.
    own: bool,
}

#[derive(Default)]
struct Previews {
    next_seq: u64,
    map: HashMap<String, Stored>,
}

impl Previews {
    /// Drops what is past its TTL, then the oldest until `MAX_PREVIEWS` are left.
    fn prune(&mut self) {
        let now = Instant::now();
        self.map.retain(|_, s| now.duration_since(s.created) < PREVIEW_TTL);
        while self.map.len() > MAX_PREVIEWS {
            let Some(oldest) = self.map.iter().min_by_key(|(_, s)| s.seq).map(|(id, _)| id.clone()) else {
                break;
            };
            self.map.remove(&oldest);
        }
    }
}

struct Shared {
    home: PathBuf,
    /// `None`: build `Guard::for_home` on every call, so a new project folder is seen.
    guard: Option<Guard>,
    snapshots: Snapshots,
    previews: Mutex<Previews>,
    locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
    /// Target path to the hash of the bytes we last wrote there, or `None` when we last removed it.
    /// A file we never wrote has no entry.
    own_writes: Mutex<HashMap<PathBuf, Option<String>>>,
    /// Started by the first `preview`; `None` before that, and when it could not start.
    watcher: Mutex<Option<Watcher>>,
    /// Held by an apply from reading `managed.json` to saving it, writes and rollback included, so
    /// two applies on different files can't each save a list the other's entries are missing from.
    managed: Mutex<()>,
}

impl Shared {
    /// The watcher saw `path` change (once it had been quiet for the debounce time). A change that
    /// leaves the file as we last wrote it is our own write; anything else marks the previews stale.
    fn on_change(&self, path: PathBuf) {
        // Only the files of stored previews matter. Events for anything else in the folder, such as
        // our own `.<name>.marshell-<pid>-<n>.tmp`, match no target.
        let wanted = lock(&self.previews).map.values().any(|s| s.targets.contains(&path));
        if !wanted {
            return;
        }
        let now = hash_file(&path).unwrap_or_else(|e| {
            tracing::warn!("hashing {}: {e}", path.display());
            None
        });
        // The file as we last left it (bytes or gone) is our own write; a file we never wrote that
        // changes or disappears is an outside edit.
        if lock(&self.own_writes).get(&path) == Some(&now) {
            return;
        }
        self.mark_stale(&[path]);
    }

    fn guard(&self) -> Guard {
        self.guard.clone().unwrap_or_else(|| Guard::for_home(&self.home))
    }

    /// The per-path locks for `targets`, in sorted path order, without any of them taken.
    fn path_locks(&self, targets: &[PathBuf]) -> Vec<Arc<Mutex<()>>> {
        let mut sorted: Vec<&PathBuf> = targets.iter().collect();
        sorted.sort();
        sorted.dedup();
        let mut map = lock(&self.locks);
        // A lock nobody else holds a handle to can be forgotten.
        map.retain(|_, l| Arc::strong_count(l) > 1);
        sorted
            .into_iter()
            .map(|t| map.entry(t.clone()).or_default().clone())
            .collect()
    }

    fn set_own_write(&self, target: &Path, bytes: Option<&[u8]>) {
        lock(&self.own_writes).insert(target.to_path_buf(), bytes.map(hash_bytes));
    }

    /// Marks every stored preview that touches one of `targets` as stale.
    fn mark_stale(&self, targets: &[PathBuf]) {
        let mut previews = lock(&self.previews);
        for stored in previews.map.values_mut() {
            if stored.targets.iter().any(|t| targets.contains(t)) {
                stored.preview.stale = true;
            }
        }
    }
}

#[derive(Clone)]
pub struct ConfigService {
    shared: Arc<Shared>,
}

impl ConfigService {
    pub fn new(home: PathBuf) -> Self {
        Self::build(home, None)
    }

    /// For tests: a fixed guard instead of `Guard::for_home`.
    pub fn with_guard(home: PathBuf, guard: Guard) -> Self {
        Self::build(home, Some(guard))
    }

    fn build(home: PathBuf, guard: Option<Guard>) -> Self {
        Self {
            shared: Arc::new(Shared {
                snapshots: Snapshots::new(&home),
                home,
                guard,
                previews: Mutex::new(Previews::default()),
                locks: Mutex::new(HashMap::new()),
                own_writes: Mutex::new(HashMap::new()),
                watcher: Mutex::new(None),
                managed: Mutex::new(()),
            }),
        }
    }

    pub fn snapshots(&self) -> &Snapshots {
        &self.shared.snapshots
    }

    /// The hash of the bytes this service last wrote to `target`, if it wrote it and didn't remove it.
    pub fn own_write_hash(&self, target: &Path) -> Option<String> {
        lock(&self.shared.own_writes).get(target).cloned().flatten()
    }

    /// Plans the ChangeSet from what is on disk now, stores it, and returns what the user would see.
    pub fn preview(&self, cs: ChangeSet) -> ConfigPreview {
        self.preview_shown(cs, &|_| None)
    }

    /// `preview`, except that when `shown` gives `Some` for a file, that text is the file's diff in the returned and
    /// in the stored preview. Apply still writes the planned bytes; only what is shown differs.
    pub fn preview_shown(&self, cs: ChangeSet, shown: &dyn Fn(&PlannedFile) -> Option<String>) -> ConfigPreview {
        self.store_preview(cs, shown, false)
    }

    /// `preview_shown` for the person's own edit of a file: the keys it sets and removes are theirs, not Marshell's
    /// entries, so applying it records nothing in `managed.json`.
    pub fn preview_own(&self, cs: ChangeSet, shown: &dyn Fn(&PlannedFile) -> Option<String>) -> ConfigPreview {
        self.store_preview(cs, shown, true)
    }

    fn store_preview(&self, cs: ChangeSet, shown: &dyn Fn(&PlannedFile) -> Option<String>, own: bool) -> ConfigPreview {
        let files = plan(&cs, &self.shared.guard(), &self.shared.snapshots);
        let id = new_id("c");
        let mut preview = to_preview(&id, &cs, &files);
        for (file, shown_file) in files.iter().zip(preview.files.iter_mut()) {
            if let Some(diff) = shown(file) {
                shown_file.diff = diff;
            }
        }
        let targets = files.iter().map(|f| f.target.clone()).collect();
        let mut previews = lock(&self.shared.previews);
        let seq = previews.next_seq;
        previews.next_seq += 1;
        previews.map.insert(
            id,
            Stored {
                seq,
                cs,
                preview: preview.clone(),
                targets,
                created: Instant::now(),
                own,
            },
        );
        previews.prune();
        drop(previews);
        self.watch(&files);
        preview
    }

    /// Why a preview would refuse `path` on its location alone (the detail the person is shown), or None.
    pub fn path_refusal(&self, path: &Path) -> Option<String> {
        path_refusal(&self.shared.guard(), path)
    }

    /// Where a write to `path` would land: the guard's resolved path with links followed. None when the guard refuses
    /// the path.
    pub fn resolve_target(&self, path: &Path) -> Option<PathBuf> {
        let resolved = self.shared.guard().check(path).ok()?;
        write::resolve_link(&resolved).ok()
    }

    /// Starts the watcher on the first call and watches the folder of each file that isn't refused.
    /// A failure is logged and never fails the preview. A folder that doesn't exist yet (a JSON set
    /// that creates `.claude/settings.json`) can't be watched, and the file is not watched either.
    fn watch(&self, files: &[PlannedFile]) {
        let mut slot = lock(&self.shared.watcher);
        if slot.is_none() {
            let weak = Arc::downgrade(&self.shared);
            let started = Watcher::start(move |path| {
                if let Some(shared) = weak.upgrade() {
                    shared.on_change(path);
                }
            });
            match started {
                Ok(watcher) => *slot = Some(watcher),
                Err(e) => {
                    tracing::warn!("could not start the config watcher: {e}");
                    return;
                }
            }
        }
        let Some(watcher) = slot.as_mut() else { return };
        for file in files {
            if matches!(file.status, FileStatus::Refused { .. }) {
                continue;
            }
            if let Err(e) = watcher.watch_file(&file.target) {
                tracing::warn!("could not watch {}: {e}", file.target.display());
            }
        }
    }

    pub fn get_preview(&self, id: &str) -> Option<ConfigPreview> {
        let mut previews = lock(&self.shared.previews);
        previews.prune();
        previews.map.get(id).map(|s| s.preview.clone())
    }

    pub fn apply(&self, req: &ApplyRequest, now_ms: u64) -> Result<ApplyResult, ApplyError> {
        self.apply_with(req, now_ms, &real_write)
    }

    /// A restore is an ordinary ChangeSet with one `Restore` edit per file of the snapshot, so it is
    /// previewed, applied, snapshotted and undoable like any other change.
    pub fn restore_preview(&self, snapshot_id: &str) -> Option<ConfigPreview> {
        let info = self.shared.snapshots.get(snapshot_id).ok().flatten()?;
        let edits = info
            .files
            .iter()
            .map(|file| FileEdit {
                path: file.path.clone(),
                scope: file.scope,
                op: EditOp::Restore {
                    snapshot_id: snapshot_id.to_string(),
                },
                // A file changed since still shows its diff, and the user decides.
                expected_hash: None,
                applies: file.applies,
            })
            .collect();
        Some(self.preview(ChangeSet {
            title: format!("Restore: {}", info.title),
            reason: "Puts the files back as they were before this change.".to_string(),
            edits,
        }))
    }

    /// The stored preview, or why it can't be applied now.
    fn stored(&self, id: &str) -> Result<(ChangeSet, ConfigPreview, Vec<PathBuf>, bool), ApplyError> {
        let mut previews = lock(&self.shared.previews);
        previews.prune();
        let stored = previews.map.get(id).ok_or(ApplyError::UnknownChangeSet)?;
        let preview = &stored.preview;
        let paths_where = |keep: &dyn Fn(&FileStatus) -> bool| -> Vec<String> {
            preview
                .files
                .iter()
                .filter(|f| keep(&f.status))
                .map(|f| f.path.clone())
                .collect()
        };
        if preview.stale {
            return Err(conflict(
                "changed outside Marshell",
                paths_where(&|s| *s == FileStatus::Changes),
            ));
        }
        if !preview.can_apply {
            return Err(conflict(
                "refused",
                paths_where(&|s| matches!(s, FileStatus::Refused { .. })),
            ));
        }
        Ok((stored.cs.clone(), preview.clone(), stored.targets.clone(), stored.own))
    }

    fn apply_with(&self, req: &ApplyRequest, now_ms: u64, write: Writer) -> Result<ApplyResult, ApplyError> {
        let shared = &*self.shared;
        // 1. The stored preview.
        let (cs, preview, targets, own) = self.stored(&req.changeset_id)?;

        // 2. Exactly the hashes that were shown, one per file.
        let mut mismatched: Vec<String> = Vec::new();
        for file in &preview.files {
            let mut entries = req.expected_hashes.iter().filter(|e| e.path == file.path);
            let ok = match (entries.next(), entries.next()) {
                (Some(entry), None) => entry.hash == file.before_hash,
                _ => false,
            };
            if !ok {
                mismatched.push(file.path.clone());
            }
        }
        if req.expected_hashes.len() != preview.files.len() || !mismatched.is_empty() {
            if mismatched.is_empty() {
                mismatched = preview.files.iter().map(|f| f.path.clone()).collect();
            }
            return Err(conflict("not what was shown", mismatched));
        }

        // 3. Every target's lock, in sorted order, held to the end.
        let arcs = shared.path_locks(&targets);
        let _held: Vec<MutexGuard<'_, ()>> = arcs.iter().map(|l| lock(l)).collect();
        // A racing apply may have used or staled this preview while we waited.
        let (_, _, _, _) = self.stored(&req.changeset_id)?;

        // 4. Plan again from disk: nothing may have moved since the user looked.
        let files = plan(&cs, &shared.guard(), &shared.snapshots);
        let all_paths = || preview.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>();
        if files.len() != preview.files.len() || files.iter().map(|f| &f.target).ne(targets.iter()) {
            return Err(conflict("changed", all_paths()));
        }
        let changed: Vec<String> = files
            .iter()
            .zip(&preview.files)
            .filter(|(now, shown)| {
                matches!(now.status, FileStatus::Refused { .. })
                    || now.before.as_deref().map(hash_bytes) != shown.before_hash
                    || now.after.as_deref().map(hash_bytes) != shown.after_hash
            })
            .map(|(_, shown)| shown.path.clone())
            .collect();
        if !changed.is_empty() {
            return Err(conflict("changed", changed));
        }
        let changing: Vec<&PlannedFile> = files.iter().filter(|f| f.status == FileStatus::Changes).collect();

        // 5. The snapshot.
        let inputs: Vec<SnapshotInput> = changing
            .iter()
            .map(|f| SnapshotInput {
                path: f.target.clone(),
                before: f.before.clone(),
                after_hash: f.after.as_deref().map(hash_bytes),
                scope: f.scope,
                applies: f.applies,
            })
            .collect();
        let snapshot_id = shared
            .snapshots
            .take(&preview.changeset_id, &cs.title, &inputs, now_ms)
            .map_err(ApplyError::Failed)?;
        // Nothing was written, or every file was put back: the snapshot is no use, so drop it.
        let abandon = |error: anyhow::Error| {
            let _ = shared.snapshots.discard(&snapshot_id);
            ApplyError::Failed(error)
        };
        // Some files could not be put back, so the snapshot holds their only originals: keep it.
        let leftover = |problems: &[String]| {
            format!(
                "could not put back {}; restore snapshot {snapshot_id}",
                problems.join(", ")
            )
        };

        // 6. The new `managed.json`, computed before anything is written.
        let mut restored: HashMap<&str, String> = HashMap::new();
        for op in changing.iter().flat_map(|f| &f.ops) {
            if let EditOp::Restore { snapshot_id } = op {
                match shared.snapshots.get(snapshot_id) {
                    Ok(Some(info)) => {
                        restored.insert(snapshot_id, info.changeset_id);
                    }
                    Ok(None) => return Err(abandon(anyhow!("snapshot {snapshot_id} is gone"))),
                    Err(e) => return Err(abandon(e)),
                }
            }
        }
        let mut changes: Vec<Change> = Vec::new();
        for file in &changing {
            for op in &file.ops {
                changes.push(match op {
                    // The person's own TOML keys and whole-text edits aren't Marshell's entries, so
                    // `managed.json` records nothing for them.
                    EditOp::TomlSet { .. } | EditOp::TomlRemove { .. } | EditOp::ReplaceText { .. } => continue,
                    EditOp::JsonSet { .. } | EditOp::JsonRemove { .. } if own => continue,
                    EditOp::JsonSet { pointer, .. } => Change::Set {
                        path: &file.target,
                        pointer,
                    },
                    EditOp::JsonRemove { pointer } => Change::Remove {
                        path: &file.target,
                        pointer,
                    },
                    EditOp::CreateFile { .. } => Change::Created { path: &file.target },
                    EditOp::DeleteFile => Change::Deleted { path: &file.target },
                    EditOp::Restore { snapshot_id } => Change::Restored {
                        path: &file.target,
                        changeset_id: &restored[snapshot_id.as_str()],
                    },
                });
            }
        }
        let managed_lock = lock(&shared.managed);
        let entries = match managed::updated(&shared.home, &preview.changeset_id, &changes, now_ms) {
            Ok(entries) => entries,
            Err(e) => return Err(abandon(e)),
        };

        // 7. Write each file; if one fails, put the ones already written back.
        for (k, file) in changing.iter().enumerate() {
            shared.set_own_write(&file.target, file.after.as_deref());
            if let Err(e) = write(&file.target, file.after.as_deref()) {
                // This file's own write is atomic, so it still holds its old bytes.
                shared.set_own_write(&file.target, file.before.as_deref());
                let problems = self.roll_back(&changing[..k], write);
                if problems.is_empty() {
                    return Err(abandon(failure(&file.target, &e, None)));
                }
                return Err(ApplyError::Failed(failure(&file.target, &e, Some(leftover(&problems)))));
            }
        }

        // 8. Record what we wrote.
        if let Err(e) = managed::save(&shared.home, &entries) {
            let problems = self.roll_back(&changing, write);
            if problems.is_empty() {
                return Err(abandon(e.context("recording the change in managed.json")));
            }
            return Err(ApplyError::Failed(e.context(format!(
                "recording the change in managed.json; {}",
                leftover(&problems)
            ))));
        }

        drop(managed_lock);

        // A removed skill leaves its `marshell` folder empty: take that folder too. `remove_dir` refuses a
        // folder that holds anything else, which is then left as it is.
        for file in &changing {
            let deletes = file.ops.iter().any(|op| matches!(op, EditOp::DeleteFile));
            if deletes
                && file.target.file_name().is_some_and(|n| n == "SKILL.md")
                && let Some(folder) = file.target.parent()
            {
                let _ = std::fs::remove_dir(folder);
            }
        }

        // 9. This preview is spent, and the others on these files no longer describe the disk.
        let written: Vec<PathBuf> = changing.iter().map(|f| f.target.clone()).collect();
        // Old snapshots go only now: a failed apply must not cost one, and a restore of the oldest
        // snapshot looked it up in step 6, before this.
        if let Err(e) = shared.snapshots.trim(&written) {
            tracing::warn!("trimming old snapshots: {e}");
        }
        lock(&shared.previews).map.remove(&req.changeset_id);
        shared.mark_stale(&written);
        Ok(ApplyResult {
            snapshot_id,
            files: changing
                .iter()
                .map(|f| AppliedFile {
                    path: f.path.to_string_lossy().into_owned(),
                    after_hash: f.after.as_deref().map(hash_bytes),
                })
                .collect(),
        })
    }

    /// Writes back the old bytes of `written` (`None` removes the file), newest first. Returns the
    /// paths that could not be put back.
    fn roll_back(&self, written: &[&PlannedFile], write: Writer) -> Vec<String> {
        let mut problems = Vec::new();
        for file in written.iter().rev() {
            self.shared.set_own_write(&file.target, file.before.as_deref());
            if write(&file.target, file.before.as_deref()).is_err() {
                problems.push(file.target.to_string_lossy().into_owned());
            }
        }
        problems
    }
}

/// `leftover` says what the roll back could not put back, when it left something.
fn failure(target: &Path, error: &io::Error, leftover: Option<String>) -> anyhow::Error {
    match leftover {
        None => anyhow!("writing {}: {error}", target.display()),
        Some(leftover) => anyhow!("writing {}: {error}; {leftover}", target.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::hash::hash_bytes;
    use crate::projects::canonical;
    use marshell_protocol::config::{Applies, ConfigScope, EditOp, ExpectedHash, FileEdit};
    use std::path::Path;
    use std::sync::{Arc, Barrier};

    struct Env {
        _dir: tempfile::TempDir,
        root: PathBuf,
        home: PathBuf,
        svc: ConfigService,
    }

    fn env() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical(dir.path());
        let home = root.join("home");
        let guard = Guard::new(vec![root.clone()], vec![], home.clone());
        let svc = ConfigService::with_guard(home.clone(), guard);
        Env {
            _dir: dir,
            root,
            home,
            svc,
        }
    }

    fn edit(path: &Path, op: EditOp) -> FileEdit {
        FileEdit {
            path: path.to_string_lossy().into_owned(),
            scope: ConfigScope::User,
            op,
            expected_hash: None,
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
            title: "Add hook".into(),
            reason: "r".into(),
            edits,
        }
    }

    /// The request a user sends after looking at `preview`: its own hashes.
    fn shown(preview: &ConfigPreview) -> ApplyRequest {
        ApplyRequest {
            changeset_id: preview.changeset_id.clone(),
            expected_hashes: preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        }
    }

    fn apply_shown(svc: &ConfigService, preview: &ConfigPreview) -> Result<ApplyResult, ApplyError> {
        svc.apply(&shown(preview), 100)
    }

    fn is_conflict<T>(result: &Result<T, ApplyError>) -> bool {
        matches!(result, Err(ApplyError::Conflict { .. }))
    }

    fn write(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn apply_writes_snapshots_and_records_managed() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        let preview = e.svc.preview(changeset(vec![edit(&path, set("/hooks/Stop", "[]"))]));
        assert!(preview.can_apply);
        let result = apply_shown(&e.svc, &preview).unwrap();
        let value: serde_json::Value = serde_json::from_str(&read(&path)).unwrap();
        assert_eq!(value, serde_json::json!({"hooks": {"Stop": []}}));
        assert_eq!(e.svc.snapshots().list(Some(&path)).unwrap().len(), 1);
        assert_eq!(result.files.len(), 1);
        assert_eq!(
            result.files[0].after_hash.as_deref(),
            Some(hash_bytes(read(&path).as_bytes()).as_str())
        );
        let managed = crate::config::managed::list(&e.home).unwrap();
        assert_eq!(managed.len(), 1);
        assert_eq!(managed[0].pointer.as_deref(), Some("/hooks/Stop"));
        assert_eq!(Path::new(&managed[0].path), path);
    }

    #[test]
    fn the_persons_own_edit_records_nothing_in_managed() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{\"model\": \"a\", \"x\": 1}\n");
        let preview = e.svc.preview_own(
            changeset(vec![
                edit(&path, set("/model", "\"b\"")),
                edit(&path, EditOp::JsonRemove { pointer: "/x".into() }),
            ]),
            &|_| None,
        );
        apply_shown(&e.svc, &preview).unwrap();
        let value: serde_json::Value = serde_json::from_str(&read(&path)).unwrap();
        assert_eq!(value, serde_json::json!({"model": "b"}));
        assert!(crate::config::managed::list(&e.home).unwrap().is_empty());
    }

    #[test]
    fn apply_needs_the_hashes_that_were_shown() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        let preview = e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        let mut wrong = shown(&preview);
        wrong.expected_hashes[0].hash = Some(hash_bytes(b"something else"));
        assert!(is_conflict(&e.svc.apply(&wrong, 1)));
        // A missing entry and an extra entry are refused too.
        let mut missing = shown(&preview);
        missing.expected_hashes.clear();
        assert!(is_conflict(&e.svc.apply(&missing, 1)));
        let mut extra = shown(&preview);
        extra.expected_hashes.push(ExpectedHash {
            path: "elsewhere".into(),
            hash: None,
        });
        assert!(is_conflict(&e.svc.apply(&extra, 1)));
        assert_eq!(read(&path), "{}\n");
        // The preview survives a refused apply, and the right hashes still work.
        assert!(apply_shown(&e.svc, &preview).is_ok());
    }

    #[test]
    fn a_file_changed_after_preview_is_a_conflict_and_untouched() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        let preview = e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        write(&path, "{\"mine\":1}");
        assert!(is_conflict(&apply_shown(&e.svc, &preview)));
        assert_eq!(read(&path), "{\"mine\":1}");
        assert!(e.svc.snapshots().list(None).unwrap().is_empty());
    }

    #[test]
    fn a_failed_second_write_rolls_back_the_first() {
        let e = env();
        let a = e.root.join("a.json");
        let b = e.root.join("b.json");
        write(&a, "{}\n");
        write(&b, "{}\n");
        let preview = e
            .svc
            .preview(changeset(vec![edit(&a, set("/x", "1")), edit(&b, set("/y", "2"))]));
        assert!(preview.can_apply);
        let fail_on_b = |path: &Path, bytes: Option<&[u8]>| -> std::io::Result<()> {
            if path == b {
                return Err(std::io::Error::other("disk full"));
            }
            real_write(path, bytes)
        };
        let result = e.svc.apply_with(&shown(&preview), 1, &fail_on_b);
        assert!(matches!(result, Err(ApplyError::Failed(_))), "{result:?}");
        assert_eq!(read(&a), "{}\n");
        assert_eq!(read(&b), "{}\n");
        assert!(e.svc.snapshots().list(None).unwrap().is_empty());
        assert!(!crate::paths::managed_file(&e.home).exists());
    }

    #[test]
    fn a_failed_roll_back_keeps_the_snapshot() {
        let e = env();
        let a = e.root.join("a.json");
        let b = e.root.join("b.json");
        write(&a, "{}\n");
        write(&b, "{}\n");
        let preview = e
            .svc
            .preview(changeset(vec![edit(&a, set("/x", "1")), edit(&b, set("/y", "2"))]));
        assert!(preview.can_apply);
        // A is written, B fails, and then A can't be put back either.
        let writes_to_a = std::cell::Cell::new(0);
        let writer = |path: &Path, bytes: Option<&[u8]>| -> std::io::Result<()> {
            if path == b {
                return Err(std::io::Error::other("disk full"));
            }
            writes_to_a.set(writes_to_a.get() + 1);
            if writes_to_a.get() > 1 {
                return Err(std::io::Error::other("locked"));
            }
            real_write(path, bytes)
        };
        let error = match e.svc.apply_with(&shown(&preview), 1, &writer) {
            Err(ApplyError::Failed(error)) => error.to_string(),
            other => panic!("{other:?}"),
        };
        let listed = e.svc.snapshots().list(None).unwrap();
        assert_eq!(listed.len(), 1, "the snapshot with the originals must stay");
        let id = &listed[0].id;
        assert!(error.contains(id.as_str()), "{error}");
        assert_eq!(e.svc.snapshots().blob(id, &a).unwrap(), Some(Some(b"{}\n".to_vec())));
    }

    /// `count` applies on `path`, each adding one key, so the file ends with `count` snapshots.
    fn apply_many(e: &Env, path: &Path, count: u64) {
        for n in 0..count {
            let preview = e.svc.preview(changeset(vec![edit(path, set(&format!("/k{n}"), "1"))]));
            e.svc.apply(&shown(&preview), n + 1).unwrap();
        }
    }

    #[test]
    fn restoring_the_oldest_of_twenty_snapshots_works() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        apply_many(&e, &path, 20);
        let listed = e.svc.snapshots().list(Some(&path)).unwrap();
        assert_eq!(listed.len(), 20);
        let oldest = listed.last().unwrap();
        let restore = e.svc.restore_preview(&oldest.id).unwrap();
        assert!(restore.can_apply);
        apply_shown(&e.svc, &restore).unwrap();
        assert_eq!(read(&path), "{}\n");
    }

    #[test]
    fn a_failed_apply_keeps_every_old_snapshot() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        apply_many(&e, &path, 20);
        let oldest = e.svc.snapshots().list(Some(&path)).unwrap().pop().unwrap().id;
        let preview = e.svc.preview(changeset(vec![edit(&path, set("/z", "1"))]));
        let fail = |_: &Path, _: Option<&[u8]>| -> std::io::Result<()> { Err(std::io::Error::other("disk full")) };
        let result = e.svc.apply_with(&shown(&preview), 99, &fail);
        assert!(matches!(result, Err(ApplyError::Failed(_))), "{result:?}");
        let listed = e.svc.snapshots().list(Some(&path)).unwrap();
        assert_eq!(listed.len(), 20);
        assert!(listed.iter().any(|s| s.id == oldest));
    }

    #[test]
    fn concurrent_applies_on_different_files_keep_both_managed_entries() {
        let e = env();
        let a = e.root.join("a.json");
        let b = e.root.join("b.json");
        write(&a, "{}\n");
        write(&b, "{}\n");
        let pa = e.svc.preview(changeset(vec![edit(&a, set("/x", "1"))]));
        let pb = e.svc.preview(changeset(vec![edit(&b, set("/y", "2"))]));
        // Each writer waits, up to a second, for the other apply to reach its own write. Both are
        // then past the read of managed.json (step 6) and before its save (step 8). With the
        // `managed` lock the second apply can't get that far, so the first gives up waiting.
        let arrived = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let handles: Vec<_> = [pa, pb]
            .into_iter()
            .map(|preview| {
                let svc = e.svc.clone();
                let arrived = arrived.clone();
                std::thread::spawn(move || {
                    let writer = |path: &Path, bytes: Option<&[u8]>| -> std::io::Result<()> {
                        use std::sync::atomic::Ordering;
                        arrived.fetch_add(1, Ordering::SeqCst);
                        let deadline = Instant::now() + Duration::from_secs(1);
                        while arrived.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        real_write(path, bytes)
                    };
                    svc.apply_with(&shown(&preview), 1, &writer).map(|_| ())
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        let managed = crate::config::managed::list(&e.home).unwrap();
        let mut paths: Vec<_> = managed.iter().map(|m| m.path.clone()).collect();
        paths.sort();
        let want: Vec<String> = [&a, &b].iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert_eq!(paths, want, "{managed:?}");
    }

    #[test]
    fn concurrent_applies_on_one_file_one_conflicts() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        let px = e.svc.preview(changeset(vec![edit(&path, set("/x", "1"))]));
        let py = e.svc.preview(changeset(vec![edit(&path, set("/y", "1"))]));
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = [px, py]
            .into_iter()
            .map(|preview| {
                let svc = e.svc.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    apply_shown(&svc, &preview)
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1, "{results:?}");
        assert_eq!(results.iter().filter(|r| is_conflict(r)).count(), 1, "{results:?}");
        let value: serde_json::Value = serde_json::from_str(&read(&path)).unwrap();
        let keys = value.as_object().unwrap();
        assert_eq!(keys.len(), 1, "{value}");
        assert!(keys.contains_key("x") != keys.contains_key("y"));
    }

    #[test]
    fn a_preview_applies_once() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        let preview = e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        apply_shown(&e.svc, &preview).unwrap();
        assert!(matches!(
            apply_shown(&e.svc, &preview),
            Err(ApplyError::UnknownChangeSet)
        ));
        assert!(e.svc.get_preview(&preview.changeset_id).is_none());
    }

    #[test]
    fn applying_one_preview_makes_others_on_that_file_stale() {
        let e = env();
        let path = e.root.join("settings.json");
        let other = e.root.join("other.json");
        write(&path, "{}\n");
        write(&other, "{}\n");
        let first = e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        let second = e.svc.preview(changeset(vec![edit(&path, set("/b", "1"))]));
        let unrelated = e.svc.preview(changeset(vec![edit(&other, set("/c", "1"))]));
        assert!(!e.svc.get_preview(&second.changeset_id).unwrap().stale);
        apply_shown(&e.svc, &first).unwrap();
        assert!(e.svc.get_preview(&second.changeset_id).unwrap().stale);
        assert!(!e.svc.get_preview(&unrelated.changeset_id).unwrap().stale);
        assert!(is_conflict(&apply_shown(&e.svc, &second)));
    }

    #[test]
    fn restore_puts_the_file_back_and_is_itself_undoable() {
        let e = env();
        let path = e.root.join("settings.json");
        let original = "{\"keep\": true}\n";
        write(&path, original);
        let preview = e.svc.preview(changeset(vec![edit(&path, set("/hooks/Stop", "[]"))]));
        let applied = apply_shown(&e.svc, &preview).unwrap();
        assert_ne!(read(&path), original);
        let restore = e.svc.restore_preview(&applied.snapshot_id).unwrap();
        assert_eq!(restore.title, "Restore: Add hook");
        assert!(restore.can_apply);
        apply_shown(&e.svc, &restore).unwrap();
        assert_eq!(read(&path), original);
        assert_eq!(e.svc.snapshots().list(Some(&path)).unwrap().len(), 2);
        // The entry the first change recorded is forgotten.
        assert!(crate::config::managed::list(&e.home).unwrap().is_empty());
    }

    #[test]
    fn restore_of_a_created_file_deletes_it() {
        let e = env();
        let path = e.root.join("notes.md");
        let op = EditOp::CreateFile { content: "hi".into() };
        let preview = e.svc.preview(changeset(vec![edit(&path, op)]));
        let applied = apply_shown(&e.svc, &preview).unwrap();
        assert_eq!(read(&path), "hi");
        let restore = e.svc.restore_preview(&applied.snapshot_id).unwrap();
        let result = apply_shown(&e.svc, &restore).unwrap();
        assert!(!path.exists());
        assert_eq!(result.files[0].after_hash, None);
    }

    #[test]
    fn refused_preview_cannot_apply() {
        let e = env();
        let path = e.root.join("managed-settings.json");
        let preview = e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        assert!(!preview.can_apply);
        match apply_shown(&e.svc, &preview) {
            Err(ApplyError::Conflict { reason, paths }) => {
                assert_eq!(reason, "refused");
                assert_eq!(paths, vec![path.to_string_lossy().into_owned()]);
            }
            other => panic!("{other:?}"),
        }
        assert!(!path.exists());
    }

    #[test]
    fn old_previews_are_dropped() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        let first = e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        for _ in 0..MAX_PREVIEWS {
            e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        }
        assert!(e.svc.get_preview(&first.changeset_id).is_none());
        assert!(matches!(apply_shown(&e.svc, &first), Err(ApplyError::UnknownChangeSet)));
    }

    /// Polls every 50 ms for up to 5 s.
    fn wait_until(mut done: impl FnMut() -> bool) -> bool {
        for _ in 0..100 {
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        done()
    }

    #[test]
    fn an_outside_edit_marks_the_preview_stale() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        let preview = e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        assert!(!e.svc.get_preview(&preview.changeset_id).unwrap().stale);
        write(&path, "{\"mine\": 1}\n");
        assert!(
            wait_until(|| e.svc.get_preview(&preview.changeset_id).unwrap().stale),
            "the preview never went stale"
        );
    }

    #[test]
    fn an_outside_delete_marks_the_preview_stale() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        let preview = e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        std::fs::remove_file(&path).unwrap();
        assert!(
            wait_until(|| e.svc.get_preview(&preview.changeset_id).unwrap().stale),
            "the preview never went stale"
        );
    }

    #[test]
    fn our_own_write_does_not_mark_a_new_preview_stale() {
        let e = env();
        let path = e.root.join("settings.json");
        write(&path, "{}\n");
        let p1 = e.svc.preview(changeset(vec![edit(&path, set("/a", "1"))]));
        apply_shown(&e.svc, &p1).unwrap();
        let p3 = e.svc.preview(changeset(vec![edit(&path, set("/b", "1"))]));
        std::thread::sleep(Duration::from_secs(1));
        assert!(!e.svc.get_preview(&p3.changeset_id).unwrap().stale);
    }

    /// The edit a caller sends to delete `path`, against the bytes it read.
    fn delete(path: &Path) -> FileEdit {
        let mut e = edit(path, EditOp::DeleteFile);
        e.expected_hash = Some(hash_bytes(&std::fs::read(path).unwrap()));
        e
    }

    #[test]
    fn delete_takes_a_snapshot_and_removes_the_file() {
        let e = env();
        let path = e.root.join("hooks.json");
        write(&path, "{}\n");
        let preview = e.svc.preview(changeset(vec![delete(&path)]));
        assert!(preview.can_apply);
        let result = apply_shown(&e.svc, &preview).unwrap();
        assert!(!path.exists());
        assert_eq!(result.files[0].after_hash, None);
        let snapshots = e.svc.snapshots().list(Some(&path)).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, result.snapshot_id);
        assert_eq!(snapshots[0].files[0].after_hash, None);
    }

    #[test]
    fn undo_of_a_delete_puts_the_bytes_back() {
        let e = env();
        let path = e.root.join("hooks.json");
        let original: &[u8] = b"\xEF\xBB\xBF{\r\n  \"a\": 1\r\n}\r\n";
        std::fs::write(&path, original).unwrap();
        let preview = e.svc.preview(changeset(vec![delete(&path)]));
        let applied = apply_shown(&e.svc, &preview).unwrap();
        assert!(!path.exists());
        let restore = e.svc.restore_preview(&applied.snapshot_id).unwrap();
        assert!(restore.can_apply);
        apply_shown(&e.svc, &restore).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}
