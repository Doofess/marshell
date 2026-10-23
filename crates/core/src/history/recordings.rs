//! The recording parts of a conversation: which `.cast` files cover it, and the stretch of each it spans.
//! Files are only read, and only by names that pass `cast::valid_recording_name`, joined to the recordings
//! folder.

use super::SessionMeta;
use super::cast::{self, CastSummary};
use crate::paths;
use marshell_protocol::history::RecordingPart;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// How many summaries the cache holds; the oldest goes first.
const CACHE_SIZE: usize = 16;

type Key = (PathBuf, u64, Option<SystemTime>);

/// Summaries of recordings already scanned, keyed by path, length and modified time, so a file that grows
/// or is replaced is scanned again.
#[derive(Default)]
pub struct CastCache {
    entries: Mutex<Vec<(Key, CastSummary)>>,
}

impl CastCache {
    /// The file's summary, from the cache when the file is unchanged. Errors when the file can't be read.
    pub fn summary(&self, path: &Path, started_ms: u64) -> anyhow::Result<CastSummary> {
        let meta = std::fs::metadata(path)?;
        let key: Key = (path.to_path_buf(), meta.len(), meta.modified().ok());
        if let Some((_, found)) = self.entries.lock().unwrap().iter().find(|(k, _)| *k == key) {
            return Ok(found.clone());
        }
        let summary = cast::scan(path, started_ms)?;
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|(k, _)| k.0 != key.0);
        if entries.len() >= CACHE_SIZE {
            entries.remove(0);
        }
        entries.push((key, summary.clone()));
        Ok(summary)
    }
}

/// The conversation's recordings overlapping `span` (from, to in unix ms), in time order and numbered from 0.
pub fn parts(home: &Path, meta: &SessionMeta, span: (u64, u64), cache: &CastCache) -> Vec<RecordingPart> {
    let dir = paths::recordings_dir(home);
    let mut names: Vec<String> = Vec::new();
    for name in meta.recordings.iter().filter(|n| cast::valid_recording_name(n)) {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    let mut found = summaries(&dir, &names, cache);
    if found.is_empty() {
        found = summaries(&dir, &tab_files(&dir, &meta.tab_id), cache);
    }
    // A part with no length plays nothing; one outside the span isn't this conversation's.
    found.retain(|(_, s)| s.ended_ms > s.started_ms && s.started_ms <= span.1 && s.ended_ms >= span.0);
    found.sort_by(|a, b| (a.1.started_ms, &a.0).cmp(&(b.1.started_ms, &b.0)));
    found
        .into_iter()
        .enumerate()
        .map(|(index, (file, s))| RecordingPart {
            index: index as u32,
            file,
            started_ms: s.started_ms,
            ended_ms: s.ended_ms,
            cols: s.cols,
            rows: s.rows,
            span_from_ms: span.0.max(s.started_ms),
            span_to_ms: span.1.min(s.ended_ms),
            marks_ms: s.marks_ms,
            stopped: s.stopped,
        })
        .collect()
}

/// The summary of each named file that exists and reads, in the order given. The names have passed the check.
fn summaries(dir: &Path, names: &[String], cache: &CastCache) -> Vec<(String, CastSummary)> {
    names
        .iter()
        .filter_map(|name| {
            let started = cast::started_ms(name)?;
            match cache.summary(&dir.join(name), started) {
                Ok(summary) => Some((name.clone(), summary)),
                Err(err) => {
                    tracing::debug!("skipping recording {name}: {err:#}");
                    None
                }
            }
        })
        .collect()
}

/// The `<tab_id>-<ms>.cast` names in the recordings folder, for a conversation whose sidecar lists none.
fn tab_files(dir: &Path, tab_id: &str) -> Vec<String> {
    let prefix = format!("{tab_id}-");
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(&prefix) && cast::valid_recording_name(name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::record::{CastEncoder, CastHeader, header_line};

    const T0: u64 = 1_761_300_000_000;

    fn meta(tab: &str, recordings: &[&str]) -> SessionMeta {
        SessionMeta {
            agent: "claude".into(),
            session_id: "s1".into(),
            tab_id: tab.into(),
            cwd: String::new(),
            transcript_path: String::new(),
            started_ms: T0,
            updated_ms: T0 + 100_000,
            previous_in_tab: None,
            recordings: recordings.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Writes `<tab>-<start>.cast` with one output event at each offset (seconds) and returns its name.
    fn cast_file(home: &Path, tab: &str, start: u64, offsets: &[f64]) -> String {
        let dir = paths::recordings_dir(home);
        std::fs::create_dir_all(&dir).unwrap();
        let name = format!("{tab}-{start}.cast");
        let mut text = header_line(&CastHeader {
            cols: 80,
            rows: 24,
            started_unix: start / 1000,
            shell: None,
        });
        for t in offsets {
            text += &CastEncoder::default().output(*t, b"x").unwrap();
        }
        std::fs::write(dir.join(&name), text).unwrap();
        name
    }

    fn files(parts: &[RecordingPart]) -> Vec<&str> {
        parts.iter().map(|p| p.file.as_str()).collect()
    }

    #[test]
    fn parts_follow_the_sidecar_in_time_order() {
        let home = tempfile::tempdir().unwrap();
        let later = cast_file(home.path(), "tb", T0 + 50_000, &[1.0, 5.0]);
        let earlier = cast_file(home.path(), "ta", T0, &[1.0, 5.0]);
        let parts = parts(
            home.path(),
            &meta("tb", &[&later, &earlier]),
            (T0, T0 + 100_000),
            &CastCache::default(),
        );
        assert_eq!(files(&parts), [earlier.as_str(), later.as_str()]);
        assert_eq!(parts.iter().map(|p| p.index).collect::<Vec<_>>(), [0, 1]);
        assert_eq!((parts[0].started_ms, parts[0].ended_ms), (T0, T0 + 5000));
        assert_eq!((parts[0].cols, parts[0].rows), (80, 24));
    }

    #[test]
    fn parts_fall_back_to_the_tab_files_that_overlap() {
        let home = tempfile::tempdir().unwrap();
        let overlapping = cast_file(home.path(), "t1", T0 + 10_000, &[1.0, 20.0]);
        cast_file(home.path(), "t1", T0 + 500_000, &[1.0, 2.0]);
        cast_file(home.path(), "t2", T0 + 10_000, &[1.0, 20.0]);
        let parts = parts(home.path(), &meta("t1", &[]), (T0, T0 + 100_000), &CastCache::default());
        assert_eq!(files(&parts), [overlapping.as_str()]);
    }

    #[test]
    fn parts_skip_hostile_and_missing_names() {
        let home = tempfile::tempdir().unwrap();
        let good = cast_file(home.path(), "t1", T0, &[1.0, 2.0]);
        let long = format!("{}-12.cast", "a".repeat(200));
        let listed = [
            r"..\x.cast",
            r"C:\x.cast",
            "a/b-1.cast",
            "t1-12.cast.exe",
            long.as_str(),
            "t1-999.cast",
            good.as_str(),
        ];
        let parts = parts(
            home.path(),
            &meta("t1", &listed),
            (T0, T0 + 100_000),
            &CastCache::default(),
        );
        assert_eq!(files(&parts), [good.as_str()]);
        // With nothing usable listed, nothing in the folder under the tab's name matches either.
        let none = parts_of_hostile_only(home.path());
        assert!(none.is_empty());
    }

    fn parts_of_hostile_only(home: &Path) -> Vec<RecordingPart> {
        let hostile = [r"..\x.cast", r"C:\x.cast", "a/b-1.cast", "t9-12.cast.exe"];
        parts(home, &meta("t9", &hostile), (T0, T0 + 100_000), &CastCache::default())
    }

    #[test]
    fn a_header_only_part_is_left_out() {
        let home = tempfile::tempdir().unwrap();
        let empty = cast_file(home.path(), "t1", T0, &[]);
        let parts = parts(
            home.path(),
            &meta("t1", &[&empty]),
            (T0, T0 + 100_000),
            &CastCache::default(),
        );
        assert!(parts.is_empty());
    }

    #[test]
    fn span_is_clipped_to_each_part() {
        let home = tempfile::tempdir().unwrap();
        // 10 s to 60 s on the clock.
        let name = cast_file(home.path(), "t1", T0 + 10_000, &[1.0, 50.0]);
        let m = meta("t1", &[&name]);
        let cache = CastCache::default();
        let inside = parts(home.path(), &m, (T0 + 20_000, T0 + 30_000), &cache);
        assert_eq!(
            (inside[0].span_from_ms, inside[0].span_to_ms),
            (T0 + 20_000, T0 + 30_000)
        );
        let around = parts(home.path(), &m, (T0, T0 + 100_000), &cache);
        assert_eq!(
            (around[0].span_from_ms, around[0].span_to_ms),
            (T0 + 10_000, T0 + 60_000)
        );
        // A span that ends before the part starts leaves it out.
        assert!(parts(home.path(), &m, (T0, T0 + 5_000), &cache).is_empty());
    }

    #[test]
    fn the_cache_rescans_a_file_that_grew() {
        let home = tempfile::tempdir().unwrap();
        let name = cast_file(home.path(), "t1", T0, &[1.0]);
        let path = paths::recordings_dir(home.path()).join(&name);
        let cache = CastCache::default();
        assert_eq!(cache.summary(&path, T0).unwrap().ended_ms, T0 + 1000);
        let mut text = std::fs::read_to_string(&path).unwrap();
        text += &CastEncoder::default().output(9.0, b"y").unwrap();
        std::fs::write(&path, text).unwrap();
        assert_eq!(cache.summary(&path, T0).unwrap().ended_ms, T0 + 9000);
    }
}
