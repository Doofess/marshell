//! The recording parts of a conversation: which `.cast` files cover it, and the stretch of each it spans.
//! Files are only read, and only by names that pass `cast::valid_recording_name`, joined to the recordings
//! folder.

use super::SessionMeta;
use super::cast::{self, CastSummary};
use crate::paths;
use marshell_protocol::history::RecordingPart;
use std::collections::HashMap;
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
/// A recording the sidecar links is also clipped to the conversation's first and last event in it, so the
/// output of another conversation in the same file (after a /clear) stays out of the span. The newest
/// recording keeps the span's end, as the conversation may still be running in it.
pub fn parts(home: &Path, meta: &SessionMeta, span: (u64, u64), cache: &CastCache) -> Vec<RecordingPart> {
    let dir = paths::recordings_dir(home);
    let mut names: Vec<String> = Vec::new();
    // The events of the conversation in each recording: (first, last) in unix ms.
    let mut events: HashMap<&str, (u64, u64)> = HashMap::new();
    for link in meta.recordings.iter().filter(|l| cast::valid_recording_name(&l.file)) {
        if !names.contains(&link.file) {
            names.push(link.file.clone());
        }
        let seen = events
            .entry(link.file.as_str())
            .or_insert((link.first_ms, link.last_ms));
        seen.0 = seen.0.min(link.first_ms);
        seen.1 = seen.1.max(link.last_ms);
    }
    let mut found = summaries(&dir, &names, cache);
    if found.is_empty() {
        found = summaries(&dir, &tab_files(&dir, &meta.tab_id), cache);
    }
    // The newest recording is the one the conversation runs in now.
    let newest = found
        .iter()
        .max_by(|a, b| (a.1.started_ms, &a.0).cmp(&(b.1.started_ms, &b.0)))
        .map(|(file, _)| file.clone());
    // A part with no length plays nothing; one outside the span isn't this conversation's.
    found.retain(|(_, s)| s.ended_ms > s.started_ms && s.started_ms <= span.1 && s.ended_ms >= span.0);
    found.sort_by(|a, b| (a.1.started_ms, &a.0).cmp(&(b.1.started_ms, &b.0)));
    found
        .into_iter()
        .enumerate()
        .map(|(index, (file, s))| {
            let whole = (span.0.max(s.started_ms), span.1.min(s.ended_ms));
            let mut clipped = whole;
            if let Some(&(first_ms, last_ms)) = events.get(file.as_str()) {
                clipped.0 = clipped.0.max(first_ms);
                if newest.as_deref() != Some(file.as_str()) {
                    clipped.1 = clipped.1.min(last_ms);
                }
            }
            // A link that leaves nothing of the part doesn't get to empty it.
            let (span_from_ms, span_to_ms) = if clipped.0 < clipped.1 { clipped } else { whole };
            RecordingPart {
                index: index as u32,
                file,
                started_ms: s.started_ms,
                ended_ms: s.ended_ms,
                cols: s.cols,
                rows: s.rows,
                span_from_ms,
                span_to_ms,
                marks_ms: s.marks_ms,
                stopped: s.stopped,
            }
        })
        .collect()
}

/// The summary of each named file that exists and reads, in the order given. The names have passed the check.
fn summaries(dir: &Path, names: &[String], cache: &CastCache) -> Vec<(String, CastSummary)> {
    names
        .iter()
        .filter_map(|name| {
            let started = cast::started_ms(name)?;
            let path = cast::resolve(dir, name)?;
            match cache.summary(&path, started) {
                Ok(summary) => Some((name.clone(), summary)),
                Err(err) => {
                    tracing::debug!("skipping recording {name}: {err:#}");
                    None
                }
            }
        })
        .collect()
}

/// The `<tab_id>-<ms>.cast` link names for either form, for a conversation whose sidecar lists none.
fn tab_files(dir: &Path, tab_id: &str) -> Vec<String> {
    let prefix = format!("{tab_id}-");
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .map(|name| name.strip_suffix(".zst").unwrap_or(&name).to_string())
        .filter(|name| name.starts_with(&prefix) && cast::resolve(dir, name).is_some())
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::RecordingLink;
    use crate::pty::record::{CastEncoder, CastHeader, header_line};

    const T0: u64 = 1_761_300_000_000;

    /// A sidecar whose links cover the whole of `T0..T0 + 100 s`, so they clip nothing.
    fn meta(tab: &str, recordings: &[&str]) -> SessionMeta {
        let links: Vec<(&str, u64, u64)> = recordings.iter().map(|n| (*n, T0, T0 + 100_000)).collect();
        meta_links(tab, &links)
    }

    /// A sidecar with a link of (file, first_ms, last_ms) per recording.
    fn meta_links(tab: &str, links: &[(&str, u64, u64)]) -> SessionMeta {
        SessionMeta {
            agent: "claude".into(),
            session_id: "s1".into(),
            tab_id: tab.into(),
            cwd: String::new(),
            transcript_path: String::new(),
            started_ms: T0,
            updated_ms: T0 + 100_000,
            previous_in_tab: None,
            recordings: links
                .iter()
                .map(|(file, first_ms, last_ms)| RecordingLink {
                    file: file.to_string(),
                    first_ms: *first_ms,
                    last_ms: *last_ms,
                })
                .collect(),
            client: Default::default(),
            preset: None,
            distro: None,
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
    fn parts_find_a_compressed_recording_by_its_cast_link() {
        let home = tempfile::tempdir().unwrap();
        let name = cast_file(home.path(), "t1", 1000, &[1.0, 2.0]);
        assert_eq!(name, "t1-1000.cast");
        let dir = paths::recordings_dir(home.path());
        let path = dir.join(&name);
        let bytes = std::fs::read(&path).unwrap();
        let cache = CastCache::default();
        let linked = meta_links("other", &[(&name, 1000, 3000)]);
        let plain = parts(home.path(), &linked, (1000, 3000), &cache);
        assert_eq!(files(&plain), [name.as_str()]);
        std::fs::write(
            path.with_extension("cast.zst"),
            zstd::encode_all(bytes.as_slice(), 3).unwrap(),
        )
        .unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(parts(home.path(), &linked, (1000, 3000), &cache), plain);
        let fallback = meta("t1", &[]);
        assert_eq!(parts(home.path(), &fallback, (1000, 3000), &cache), plain);
        assert_eq!(tab_files(&dir, "t1"), std::slice::from_ref(&name));
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(tab_files(&dir, "t1"), [name]);
        assert_eq!(parts(home.path(), &fallback, (1000, 3000), &cache), plain);
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
    fn a_conversation_with_no_tab_has_no_parts() {
        let home = tempfile::tempdir().unwrap();
        cast_file(home.path(), "t1", T0, &[1.0, 20.0]);
        cast_file(home.path(), "t2", T0, &[1.0, 20.0]);
        cast_file(home.path(), "", T0, &[1.0, 20.0]);
        let parts = parts(home.path(), &meta("", &[]), (T0, T0 + 100_000), &CastCache::default());
        assert!(parts.is_empty(), "{:?}", files(&parts));
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
    fn parts_clip_an_earlier_recording_at_the_conversations_last_event_in_it() {
        let home = tempfile::tempdir().unwrap();
        // Tab A runs 0 to 20 s; the conversation's last event there is at 5 s (a /clear started another
        // conversation after it). Tab B starts at 15 s and runs to 21 s.
        let a = cast_file(home.path(), "ta", T0, &[1.0, 20.0]);
        let b = cast_file(home.path(), "tb", T0 + 15_000, &[1.0, 6.0]);
        let m = meta_links("tb", &[(&a, T0 + 1000, T0 + 5000), (&b, T0 + 15_000, T0 + 16_000)]);
        let got = parts(home.path(), &m, (T0 + 1000, T0 + 18_000), &CastCache::default());
        assert_eq!(files(&got), [a.as_str(), b.as_str()]);
        assert_eq!(got[0].span_to_ms, T0 + 5000);
        // The part the conversation runs in now is not cut at its last turn end: the turn may still run.
        assert_eq!(got[1].span_to_ms, T0 + 18_000);
    }

    #[test]
    fn a_later_part_starts_at_the_conversations_first_event_in_it() {
        let home = tempfile::tempdir().unwrap();
        let a = cast_file(home.path(), "ta", T0, &[1.0, 20.0]);
        let b = cast_file(home.path(), "tb", T0 + 15_000, &[1.0, 6.0]);
        let m = meta_links("tb", &[(&a, T0 + 1000, T0 + 5000), (&b, T0 + 17_000, T0 + 17_500)]);
        let got = parts(home.path(), &m, (T0 + 1500, T0 + 100_000), &CastCache::default());
        // The first part still starts at the span (the first prompt, after the first event), the second where the conversation came back.
        assert_eq!(got[0].span_from_ms, T0 + 1500);
        assert_eq!(got[1].span_from_ms, T0 + 17_000);
        assert_eq!(got[1].span_to_ms, T0 + 21_000);
    }

    #[test]
    fn a_link_that_leaves_nothing_of_a_part_falls_back_to_the_file_clip() {
        let home = tempfile::tempdir().unwrap();
        let a = cast_file(home.path(), "ta", T0, &[1.0, 20.0]);
        let b = cast_file(home.path(), "tb", T0 + 15_000, &[1.0, 6.0]);
        // A's last event is before the span starts, so clipping there would end the part before it begins.
        let m = meta_links("tb", &[(&a, T0, T0 + 2000), (&b, T0 + 15_000, T0 + 16_000)]);
        let got = parts(home.path(), &m, (T0 + 3000, T0 + 18_000), &CastCache::default());
        assert_eq!((got[0].span_from_ms, got[0].span_to_ms), (T0 + 3000, T0 + 18_000));
        assert!(got.iter().all(|p| p.span_from_ms <= p.span_to_ms));
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
