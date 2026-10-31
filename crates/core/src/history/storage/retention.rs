//! Retention: removes the oldest recordings and images once they pass a size or age limit.
//!
//! Only `<home>/history/recordings/*` and `<home>/attachments/<tab>/*` are ever removed. Transcript copies,
//! sidecars and marks stay. A recording or image of a pinned or bookmarked conversation is never chosen, though
//! it still counts toward the size. Nothing follows a symbolic link or junction.

use super::Open;
use crate::history::cast::valid_recording_name;
use crate::history::{archive, marks};
use crate::paths;
use marshell_protocol::history::{CleanupRun, RetentionLimits};
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const GB: u64 = 1024 * 1024 * 1024;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// One file retention may remove.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub path: PathBuf,
    pub bytes: u64,
    /// The file's modified time, unix ms.
    pub at_ms: u64,
    /// The file belongs to a pinned or bookmarked conversation: it counts toward the size and never goes.
    pub protected: bool,
}

/// A file seen in a listing, before the open set and the marks are known.
struct Seen {
    path: PathBuf,
    bytes: u64,
    modified: SystemTime,
    kind: Kind,
}

enum Kind {
    /// A recording, by its `.cast` link name.
    Recording(String),
    /// An image in this tab's folder.
    Attachment(String),
}

pub fn unix_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Every recording (both forms) and every image that retention may remove, with the files of open recordings and
/// open tabs and the ones modified within `settle` left out. `open` is asked after the folders are listed, so a
/// recording that finishes in between reads as open and one that starts in between isn't listed.
pub fn collect_with(home: &Path, open: &dyn Fn() -> Open, now: SystemTime, settle: Duration) -> Vec<Item> {
    let mut seen = list_recordings(home);
    seen.extend(list_attachments(home));
    let open = open();
    let open_names: HashSet<OsString> = open
        .recordings
        .iter()
        .filter_map(|p| p.file_name().map(OsString::from))
        .collect();
    let guard = Protection::read(home);
    seen.into_iter()
        .filter_map(|file| {
            // A modified time in the future counts as recent.
            if !now.duration_since(file.modified).is_ok_and(|age| age >= settle) {
                return None;
            }
            let protected = match &file.kind {
                Kind::Recording(link) => {
                    if open_names.contains(OsString::from(link).as_os_str()) {
                        return None;
                    }
                    guard.recording(link)
                }
                Kind::Attachment(tab) => {
                    if open.tabs.contains(tab) {
                        return None;
                    }
                    guard.tabs.contains(tab)
                }
            };
            Some(Item {
                path: file.path,
                bytes: file.bytes,
                at_ms: unix_ms(file.modified),
                protected,
            })
        })
        .collect()
}

/// `collect_with` for sets already in hand.
pub fn collect(
    home: &Path,
    open: &HashSet<PathBuf>,
    open_tabs: &HashSet<String>,
    now: SystemTime,
    settle: Duration,
) -> Vec<Item> {
    let open = Open {
        recordings: open.clone(),
        tabs: open_tabs.clone(),
    };
    collect_with(home, &move || open.clone(), now, settle)
}

/// A regular file's size and modified time. None for a link, a folder or anything unreadable.
fn regular(path: &Path) -> Option<(u64, SystemTime)> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    // A time that can't be read counts as just now, so the file waits.
    Some((meta.len(), meta.modified().unwrap_or_else(|_| SystemTime::now())))
}

fn list_recordings(home: &Path) -> Vec<Seen> {
    let Ok(entries) = std::fs::read_dir(paths::recordings_dir(home)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_owned();
            // `t1-1000.cast` and `t1-1000.cast.zst`; anything else (a `.zst.tmp` being written) is left alone.
            let link = name.strip_suffix(".zst").unwrap_or(&name);
            if !valid_recording_name(link) {
                return None;
            }
            let path = entry.path();
            let (bytes, modified) = regular(&path)?;
            Some(Seen {
                path,
                bytes,
                modified,
                kind: Kind::Recording(link.to_owned()),
            })
        })
        .collect()
}

fn list_attachments(home: &Path) -> Vec<Seen> {
    let Ok(tabs) = std::fs::read_dir(paths::attachments_dir(home)) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for tab in tabs.flatten() {
        let Some(tab_name) = tab.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let dir = tab.path();
        if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&dir) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if let Some((bytes, modified)) = regular(&path) {
                found.push(Seen {
                    path,
                    bytes,
                    modified,
                    kind: Kind::Attachment(tab_name.clone()),
                });
            }
        }
    }
    found
}

/// What the pinned and bookmarked conversations hold on to.
struct Protection {
    /// Recording link names (`.cast`) that a protected sidecar lists.
    links: HashSet<String>,
    /// Tabs of protected conversations: their images, and the recordings of one whose sidecar lists none.
    tabs: HashSet<String>,
    /// Tabs of protected sidecars with no usable recording link. The player finds those recordings by tab.
    unlinked_tabs: HashSet<String>,
}

impl Protection {
    fn read(home: &Path) -> Protection {
        let mut guard = Protection {
            links: HashSet::new(),
            tabs: HashSet::new(),
            unlinked_tabs: HashSet::new(),
        };
        for meta in archive::read_all_meta(home) {
            // Marks that can't be read may say pinned, so the conversation is kept until they can.
            match marks::try_read(home, &meta.agent, &meta.session_id) {
                Ok(marks) if !marks.pinned && marks.bookmarks.is_empty() => continue,
                Ok(_) => {}
                Err(err) => tracing::warn!(
                    "keeping the files of {} {}: its marks can't be read: {err:#}",
                    meta.agent,
                    meta.session_id
                ),
            }
            let mut linked = false;
            for link in meta.recordings.iter().filter(|l| valid_recording_name(&l.file)) {
                guard.links.insert(link.file.clone());
                linked = true;
            }
            if !meta.tab_id.is_empty() {
                guard.tabs.insert(meta.tab_id.clone());
                if !linked {
                    guard.unlinked_tabs.insert(meta.tab_id);
                }
            }
        }
        guard
    }

    fn recording(&self, link: &str) -> bool {
        if self.links.contains(link) {
            return true;
        }
        // `<tab>-<ms>.cast`: a tab id has no `-`, so everything before the first one is the tab.
        link.split_once('-')
            .is_some_and(|(tab, _)| self.unlinked_tabs.contains(tab))
    }
}

/// The items to remove, as indexes into `items`: first every unprotected item older than the age limit, then,
/// while what is left (protected items included) is over the size limit, the oldest unprotected item left.
pub fn plan(items: &[Item], limits: &RetentionLimits, now_ms: u64) -> Vec<usize> {
    let mut chosen: Vec<usize> = Vec::new();
    let mut taken = vec![false; items.len()];
    if let Some(days) = limits.limit_days {
        let cutoff = now_ms.saturating_sub(u64::from(days) * DAY_MS);
        for (i, item) in items.iter().enumerate() {
            if !item.protected && item.at_ms < cutoff {
                taken[i] = true;
                chosen.push(i);
            }
        }
    }
    if let Some(gb) = limits.limit_gb {
        let limit = u64::from(gb) * GB;
        let mut total: u64 = items
            .iter()
            .enumerate()
            .filter(|(i, _)| !taken[*i])
            .fold(0u64, |sum, (_, item)| sum.saturating_add(item.bytes));
        let mut oldest: Vec<usize> = (0..items.len()).filter(|&i| !taken[i] && !items[i].protected).collect();
        oldest.sort_by_key(|&i| (items[i].at_ms, i));
        for i in oldest {
            if total <= limit {
                break;
            }
            total -= items[i].bytes;
            chosen.push(i);
        }
    }
    chosen
}

/// Removes the chosen files. One that fails is logged and skipped; one already gone is skipped quietly. An
/// image folder left empty goes too. Returns what was removed.
pub fn apply(items: &[Item], chosen: &[usize]) -> CleanupRun {
    let (mut files, mut bytes) = (0u32, 0u64);
    for item in chosen.iter().filter_map(|&i| items.get(i)) {
        match std::fs::remove_file(&item.path) {
            Ok(()) => {
                files += 1;
                bytes += item.bytes;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                tracing::warn!("retention could not remove {}: {err}", item.path.display());
                continue;
            }
        }
        remove_empty_tab_folder(&item.path);
    }
    tracing::info!("retention removed {files} files ({bytes} bytes)");
    CleanupRun {
        at_ms: unix_ms(SystemTime::now()),
        files,
        bytes,
    }
}

/// Removes `<home>/attachments/<tab>` when `file` was in it and it is now empty. `remove_dir` fails on a
/// folder with anything in it, so nothing else goes.
fn remove_empty_tab_folder(file: &Path) {
    let Some(folder) = file.parent() else { return };
    let in_attachments = folder
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "attachments");
    if in_attachments {
        let _ = std::fs::remove_dir(folder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{RecordingLink, SessionMeta};
    use marshell_protocol::history::Client;
    use std::fs::File;

    const MIB: u64 = 1024 * 1024;

    fn item(name: &str, bytes: u64, at_ms: u64, protected: bool) -> Item {
        Item {
            path: PathBuf::from(name),
            bytes,
            at_ms,
            protected,
        }
    }

    fn limits(gb: Option<u32>, days: Option<u32>) -> RetentionLimits {
        RetentionLimits {
            limit_gb: gb,
            limit_days: days,
        }
    }

    #[test]
    fn age_limit_removes_only_old_unprotected_items() {
        let now = 1_000 * DAY_MS;
        let items = [
            item("old", 1, now - 31 * DAY_MS, false),
            item("old-protected", 1, now - 400 * DAY_MS, true),
            item("recent", 1, now - 29 * DAY_MS, false),
            item("older", 1, now - 90 * DAY_MS, false),
        ];
        assert_eq!(plan(&items, &limits(None, Some(30)), now), vec![0, 3]);
        assert_eq!(plan(&items, &limits(None, Some(365)), now), Vec::<usize>::new());
        assert_eq!(plan(&items, &limits(None, None), now), Vec::<usize>::new());
    }

    #[test]
    fn size_limit_removes_oldest_first_until_under() {
        // Newest listed first, so the order of the list is not the order of removal.
        let items = [
            item("c", 400 * MIB, 3, false),
            item("a", 400 * MIB, 1, false),
            item("b", 400 * MIB, 2, false),
        ];
        assert_eq!(plan(&items, &limits(Some(1), None), 10), vec![1]);
        // 1200 MiB against a 1 GB limit: one file is enough, and nothing is chosen when already under.
        assert!(plan(&items[..2], &limits(Some(1), None), 10).is_empty());
    }

    #[test]
    fn protected_items_count_toward_size_but_stay() {
        let items = [
            item("oldest-protected", 600 * MIB, 1, true),
            item("next", 600 * MIB, 2, false),
            item("newest", 600 * MIB, 3, false),
        ];
        // 1800 MiB: the protected oldest is skipped, "next" goes (1200 MiB left), then "newest" (600 MiB left).
        assert_eq!(plan(&items, &limits(Some(1), None), 10), vec![1, 2]);
        // Items the age pass took no longer count toward the size.
        let now = 100 * DAY_MS;
        let aged = [
            item("aged", 900 * MIB, now - 40 * DAY_MS, false),
            item("kept", 900 * MIB, now - DAY_MS, false),
        ];
        assert_eq!(plan(&aged, &limits(Some(1), Some(30)), now), vec![0]);
    }

    /// Writes `path` with `body` and sets its modified time `age` ago.
    fn write_aged(path: &Path, body: &[u8], age: Duration) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - age).unwrap();
    }

    const HOUR: Duration = Duration::from_secs(3600);

    fn sidecar(home: &Path, id: &str, tab: &str, links: &[&str]) {
        let meta = SessionMeta {
            agent: "claude".into(),
            session_id: id.into(),
            tab_id: tab.into(),
            cwd: String::new(),
            // `read_all_meta` drops a sidecar whose transcript path isn't `<absolute>/<id>.jsonl`.
            transcript_path: home.join(format!("{id}.jsonl")).to_string_lossy().into_owned(),
            started_ms: 1,
            updated_ms: 2,
            previous_in_tab: None,
            recordings: links
                .iter()
                .map(|file| RecordingLink {
                    file: (*file).into(),
                    first_ms: 1,
                    last_ms: 2,
                })
                .collect(),
            client: Client::default(),
            preset: None,
            distro: None,
        };
        archive::write_meta(home, &meta).unwrap();
    }

    fn mark(home: &Path, id: &str, pinned: bool, bookmarks: Vec<u64>) {
        let marks = marks::Marks {
            pinned,
            bookmarks,
            ..marks::Marks::default()
        };
        marks::write(home, "claude", id, &marks).unwrap();
    }

    fn by_name(items: &[Item], name: &str) -> Item {
        items
            .iter()
            .find(|i| i.path.file_name().unwrap() == name)
            .unwrap_or_else(|| panic!("{name} was not collected"))
            .clone()
    }

    #[test]
    fn collect_protects_pinned_and_bookmarked_conversations() {
        let home = tempfile::tempdir().unwrap();
        let rec = paths::recordings_dir(home.path());
        let att = paths::attachments_dir(home.path());
        write_aged(&rec.join("tp-1000.cast"), b"pinned", HOUR);
        write_aged(&rec.join("tb-2000.cast.zst"), b"bookmarked", HOUR);
        write_aged(&rec.join("tu-3000.cast"), b"unmarked", HOUR);
        write_aged(&rec.join("tn-4000.cast"), b"no links", HOUR);
        write_aged(&att.join("tb").join("a.png"), b"png", HOUR);
        write_aged(&att.join("tu").join("a.png"), b"png", HOUR);
        sidecar(home.path(), "s-pinned", "tp", &["tp-1000.cast"]);
        mark(home.path(), "s-pinned", true, Vec::new());
        sidecar(home.path(), "s-book", "tb", &["tb-2000.cast"]);
        mark(home.path(), "s-book", false, vec![5]);
        sidecar(home.path(), "s-plain", "tu", &["tu-3000.cast"]);
        mark(home.path(), "s-plain", false, Vec::new());
        // A pinned conversation whose sidecar lists no recording: the player falls back on the tab's files.
        sidecar(home.path(), "s-none", "tn", &[]);
        mark(home.path(), "s-none", true, Vec::new());

        let items = collect(
            home.path(),
            &HashSet::new(),
            &HashSet::new(),
            SystemTime::now(),
            Duration::ZERO,
        );
        assert_eq!(items.len(), 6);
        assert!(by_name(&items, "tp-1000.cast").protected);
        assert!(by_name(&items, "tb-2000.cast.zst").protected);
        assert!(!by_name(&items, "tu-3000.cast").protected);
        assert!(by_name(&items, "tn-4000.cast").protected);
        let png = |tab: &str| items.iter().find(|i| i.path.parent().unwrap().ends_with(tab)).unwrap();
        assert!(png("tb").protected);
        assert!(!png("tu").protected);
        assert_eq!(by_name(&items, "tp-1000.cast").bytes, 6);
    }

    #[test]
    fn collect_protects_a_conversation_whose_marks_cannot_be_read() {
        let home = tempfile::tempdir().unwrap();
        let rec = paths::recordings_dir(home.path());
        let att = paths::attachments_dir(home.path());
        write_aged(&rec.join("tx-1000.cast"), b"unreadable marks", HOUR);
        write_aged(&rec.join("tm-2000.cast"), b"no marks", HOUR);
        write_aged(&att.join("tx").join("a.png"), b"png", HOUR);
        sidecar(home.path(), "s-broken", "tx", &["tx-1000.cast"]);
        // Marks that are not valid JSON, with no backup to fall back on: they may well say pinned.
        let path = marks::marks_path(home.path(), "claude", "s-broken");
        std::fs::write(&path, b"{ not json").unwrap();
        // A conversation with no marks file at all has nothing to protect it.
        sidecar(home.path(), "s-missing", "tm", &["tm-2000.cast"]);

        let items = collect(
            home.path(),
            &HashSet::new(),
            &HashSet::new(),
            SystemTime::now(),
            Duration::ZERO,
        );
        assert!(by_name(&items, "tx-1000.cast").protected);
        assert!(by_name(&items, "a.png").protected);
        assert!(!by_name(&items, "tm-2000.cast").protected);
        let now = unix_ms(SystemTime::now()) + 400 * DAY_MS;
        let chosen = plan(&items, &limits(None, Some(30)), now);
        assert_eq!(chosen.len(), 1, "only the unmarked conversation's recording goes");
        assert!(items[chosen[0]].path.ends_with("tm-2000.cast"));
    }

    #[test]
    fn collect_leaves_out_open_and_recent_files() {
        let home = tempfile::tempdir().unwrap();
        let rec = paths::recordings_dir(home.path());
        let att = paths::attachments_dir(home.path());
        write_aged(&rec.join("t1-1000.cast"), b"open", HOUR);
        write_aged(&rec.join("t2-2000.cast"), b"old", HOUR);
        write_aged(&rec.join("t3-3000.cast"), b"recent", Duration::ZERO);
        write_aged(&rec.join("t4-4000.cast.zst.tmp"), b"half written", HOUR);
        write_aged(&rec.join("notes.txt"), b"not a recording", HOUR);
        write_aged(&att.join("open-tab").join("a.png"), b"png", HOUR);
        write_aged(&att.join("gone-tab").join("a.png"), b"png", HOUR);
        write_aged(&att.join("gone-tab").join("b.png"), b"png", Duration::ZERO);
        let open: HashSet<PathBuf> = [rec.join("t1-1000.cast")].into();
        let tabs: HashSet<String> = ["open-tab".to_owned()].into();
        let items = collect(home.path(), &open, &tabs, SystemTime::now(), Duration::from_secs(10));
        let mut names: Vec<_> = items
            .iter()
            .map(|i| i.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["a.png", "t2-2000.cast"]);
        assert!(items.iter().all(|i| !i.path.starts_with(att.join("open-tab"))));
    }

    #[test]
    fn collect_asks_for_the_open_set_after_listing_the_folders() {
        let home = tempfile::tempdir().unwrap();
        let rec = paths::recordings_dir(home.path());
        write_aged(&rec.join("t1-1000.cast"), b"listed", HOUR);
        let asked = std::cell::Cell::new(false);
        let late = rec.join("t2-2000.cast");
        let items = collect_with(
            home.path(),
            &|| {
                // A recording that appears while the set is being taken wasn't listed, so it isn't a candidate.
                write_aged(&late, b"late", HOUR);
                asked.set(true);
                Open::default()
            },
            SystemTime::now(),
            Duration::ZERO,
        );
        assert!(asked.get());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path.file_name().unwrap(), "t1-1000.cast");
    }

    #[cfg(unix)]
    #[test]
    fn collect_skips_links() {
        let home = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let rec = paths::recordings_dir(home.path());
        write_aged(&outside.path().join("x.cast"), b"x", HOUR);
        std::fs::create_dir_all(&rec).unwrap();
        std::os::unix::fs::symlink(outside.path().join("x.cast"), rec.join("t1-1000.cast")).unwrap();
        std::os::unix::fs::symlink(outside.path(), paths::attachments_dir(home.path()).join("tab")).ok();
        let items = collect(
            home.path(),
            &HashSet::new(),
            &HashSet::new(),
            SystemTime::now(),
            Duration::ZERO,
        );
        assert!(items.is_empty());
    }

    #[test]
    fn apply_skips_a_file_already_gone_and_removes_an_empty_tab_folder() {
        let home = tempfile::tempdir().unwrap();
        let att = paths::attachments_dir(home.path());
        let rec = paths::recordings_dir(home.path());
        write_aged(&att.join("t1").join("a.png"), b"12345", HOUR);
        write_aged(&att.join("t1").join("b.png"), b"123", HOUR);
        write_aged(&att.join("t2").join("a.png"), b"1", HOUR);
        write_aged(&rec.join("t3-1000.cast"), b"1234567", HOUR);
        write_aged(&rec.join("t4-2000.cast"), b"kept", HOUR);
        let items = collect(
            home.path(),
            &HashSet::new(),
            &HashSet::new(),
            SystemTime::now(),
            Duration::ZERO,
        );
        let index = |tab: &str, name: &str| {
            items
                .iter()
                .position(|i| i.path.ends_with(Path::new(tab).join(name)))
                .unwrap()
        };
        let chosen = [
            index("t1", "a.png"),
            index("t1", "b.png"),
            index("t2", "a.png"),
            index("", "t3-1000.cast"),
        ];
        // One of them goes before apply gets to it.
        std::fs::remove_file(&items[chosen[2]].path).unwrap();
        let run = apply(&items, &chosen);
        assert_eq!(run.files, 3);
        assert_eq!(
            run.bytes,
            items[chosen[0]].bytes + items[chosen[1]].bytes + items[chosen[3]].bytes
        );
        assert!(!att.join("t1").exists(), "the emptied tab folder goes");
        assert!(!att.join("t2").exists(), "a folder emptied by someone else goes too");
        assert!(att.exists());
        assert!(rec.exists(), "the recordings folder stays");
        assert!(rec.join("t4-2000.cast").exists());
        assert!(!rec.join("t3-1000.cast").exists());
    }

    #[test]
    fn apply_keeps_a_tab_folder_that_still_holds_a_file() {
        let home = tempfile::tempdir().unwrap();
        let att = paths::attachments_dir(home.path());
        write_aged(&att.join("t1").join("a.png"), b"1", HOUR);
        write_aged(&att.join("t1").join("b.png"), b"1", HOUR);
        let items = collect(
            home.path(),
            &HashSet::new(),
            &HashSet::new(),
            SystemTime::now(),
            Duration::ZERO,
        );
        let a = items.iter().position(|i| i.path.ends_with("a.png")).unwrap();
        let run = apply(&items, &[a]);
        assert_eq!(run.files, 1);
        assert!(att.join("t1").join("b.png").exists());
    }
}
