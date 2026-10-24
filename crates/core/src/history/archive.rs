//! Compressed copies of agent transcripts, each with a small `.meta.json` sidecar.

use super::{SessionMeta, sidecar_allowed};
use crate::{paths, store};
use anyhow::Context;
use std::path::{Path, PathBuf};

/// The archive file and the sidecar for a session: `(<id>.jsonl.zst, <id>.meta.json)`.
pub fn archive_paths(home: &Path, agent: &str, session_id: &str) -> (PathBuf, PathBuf) {
    let dir = paths::archive_dir(home, agent);
    (
        dir.join(format!("{session_id}.jsonl.zst")),
        dir.join(format!("{session_id}.meta.json")),
    )
}

/// The most recording names a sidecar keeps; the oldest go first.
pub const MAX_RECORDINGS: usize = 100;

/// Writes the sidecar for a session. The recording links already in the file stay, in their order, and the
/// new ones follow, so no writer (a copy running on old data included) drops a link. A recording listed twice
/// is one link, from the earliest first event to the latest last one. A stored owner (a tab) and its client stay.
pub fn write_meta(home: &Path, meta: &SessionMeta) -> anyhow::Result<()> {
    let (_, sidecar) = archive_paths(home, &meta.agent, &meta.session_id);
    let stored = store::read::<SessionMeta>(&sidecar).ok().flatten();
    let mut merged = stored.as_ref().map(|old| old.recordings.clone()).unwrap_or_default();
    for link in &meta.recordings {
        match merged.iter_mut().find(|l| l.file == link.file) {
            Some(known) => {
                known.first_ms = known.first_ms.min(link.first_ms);
                known.last_ms = known.last_ms.max(link.last_ms);
            }
            None => merged.push(link.clone()),
        }
    }
    let excess = merged.len().saturating_sub(MAX_RECORDINGS);
    merged.drain(..excess);
    let mut out = SessionMeta {
        recordings: merged,
        ..meta.clone()
    };
    // A conversation that ran in a Marshell tab stays Marshell's when a writer with no tab (an import) comes
    // after. A writer with a tab, such as a resume in another tab, takes it.
    if let Some(old) = stored.filter(|old| !old.tab_id.is_empty() && meta.tab_id.is_empty()) {
        out.tab_id = old.tab_id;
        out.client = old.client;
        out.previous_in_tab = old.previous_in_tab;
        out.started_ms = old.started_ms.min(meta.started_ms);
        out.updated_ms = old.updated_ms.max(meta.updated_ms);
    }
    store::write(&sidecar, &out)
}

/// The session's sidecar, or None when it has none.
pub fn read_meta(home: &Path, agent: &str, session_id: &str) -> anyhow::Result<Option<SessionMeta>> {
    let (_, sidecar) = archive_paths(home, agent, session_id);
    store::read::<SessionMeta>(&sidecar)
}

/// Reads the whole transcript, compresses it, and replaces the archive copy and the sidecar.
/// If the transcript can't be read, returns an error and leaves the old copy as it was.
pub fn copy_transcript(home: &Path, meta: &SessionMeta) -> anyhow::Result<PathBuf> {
    copy_from(home, meta, Path::new(&meta.transcript_path))
}

/// Like `copy_transcript`, but from `source`, which may be a `.jsonl.zst` (decoded first, with the archive's
/// cap). The source is only opened for reading.
pub fn copy_from(home: &Path, meta: &SessionMeta, source: &Path) -> anyhow::Result<PathBuf> {
    let bytes = read_source(source)?;
    let packed = zstd::encode_all(bytes.as_slice(), 3).context("compressing the transcript")?;
    let (archive, _) = archive_paths(home, &meta.agent, &meta.session_id);
    store::write_bytes_atomic(&archive, &packed)?;
    write_meta(home, meta)?;
    // A copy made again is the user's to keep: the conversation is no longer a deleted one.
    clear_tombstone(home, &meta.agent, &meta.session_id);
    Ok(archive)
}

/// One conversation whose copy the user deleted.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Tombstone {
    agent: String,
    session_id: String,
    deleted_at_ms: u64,
}

/// `<home>/history/deleted.json`: the conversations whose copy was deleted, so the importer doesn't copy an
/// unchanged source file back. A conversation leaves the list when a copy of it is made again.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Tombstones {
    #[serde(default)]
    deleted: Vec<Tombstone>,
}

impl store::Versioned for Tombstones {
    const VERSION: u32 = 1;
}

/// One writer at a time: the file is read, changed and written back.
static TOMBSTONES: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn tombstones_path(home: &Path) -> PathBuf {
    paths::history_dir(home).join("deleted.json")
}

fn read_tombstones(home: &Path) -> anyhow::Result<Tombstones> {
    Ok(store::read::<Tombstones>(&tombstones_path(home))?.unwrap_or_default())
}

/// Notes that the session's copy was deleted at `deleted_at_ms`.
pub fn add_tombstone(home: &Path, agent: &str, session_id: &str, deleted_at_ms: u64) -> anyhow::Result<()> {
    let _guard = TOMBSTONES.lock().unwrap_or_else(|e| e.into_inner());
    let mut doc = read_tombstones(home)?;
    doc.deleted
        .retain(|t| !(t.agent == agent && t.session_id == session_id));
    doc.deleted.push(Tombstone {
        agent: agent.into(),
        session_id: session_id.into(),
        deleted_at_ms,
    });
    store::write(&tombstones_path(home), &doc)
}

/// When the session's copy was deleted, or None when it wasn't (or the list can't be read).
pub fn deleted_at(home: &Path, agent: &str, session_id: &str) -> Option<u64> {
    let _guard = TOMBSTONES.lock().unwrap_or_else(|e| e.into_inner());
    match read_tombstones(home) {
        Ok(doc) => doc
            .deleted
            .iter()
            .find(|t| t.agent == agent && t.session_id == session_id)
            .map(|t| t.deleted_at_ms),
        Err(err) => {
            tracing::warn!("the list of deleted conversations could not be read: {err:#}");
            None
        }
    }
}

/// Forgets that the session's copy was deleted. Does nothing when it wasn't.
pub fn clear_tombstone(home: &Path, agent: &str, session_id: &str) {
    let _guard = TOMBSTONES.lock().unwrap_or_else(|e| e.into_inner());
    let result = read_tombstones(home).and_then(|mut doc| {
        let before = doc.deleted.len();
        doc.deleted
            .retain(|t| !(t.agent == agent && t.session_id == session_id));
        if doc.deleted.len() == before {
            return Ok(());
        }
        store::write(&tombstones_path(home), &doc)
    });
    if let Err(err) = result {
        tracing::warn!("the list of deleted conversations could not be updated: {err:#}");
    }
}

/// The bytes of a transcript file; a `.zst` file is decoded, and fails once it decodes past `MAX_DECODED_BYTES`.
pub fn read_source(source: &Path) -> anyhow::Result<Vec<u8>> {
    use std::io::Read;
    let reading = || format!("reading {}", source.display());
    if source.extension().is_some_and(|ext| ext == "zst") {
        let file = std::fs::File::open(source).with_context(reading)?;
        let decoder = zstd::Decoder::new(file).context("starting to decompress the transcript")?;
        let mut bytes = Vec::new();
        decoder
            .take(MAX_DECODED_BYTES + 1)
            .read_to_end(&mut bytes)
            .with_context(reading)?;
        anyhow::ensure!(
            bytes.len() as u64 <= MAX_DECODED_BYTES,
            "{} is over {} MB decoded",
            source.display(),
            MAX_DECODED_BYTES / (1024 * 1024)
        );
        Ok(bytes)
    } else {
        std::fs::read(source).with_context(reading)
    }
}

/// The most a transcript may decode to before Marshell stops reading it.
pub const MAX_DECODED_BYTES: u64 = 200 * 1024 * 1024;

/// The archived transcript as text, or None when the session has no archive. Decodes in a stream and stops
/// with an error once more than `MAX_DECODED_BYTES` has come out. Invalid UTF-8 becomes U+FFFD.
pub fn read_archive(home: &Path, agent: &str, session_id: &str) -> anyhow::Result<Option<String>> {
    read_archive_capped(home, agent, session_id, MAX_DECODED_BYTES)
}

fn read_archive_capped(home: &Path, agent: &str, session_id: &str, cap: u64) -> anyhow::Result<Option<String>> {
    use std::io::Read;
    let (archive, _) = archive_paths(home, agent, session_id);
    let file = match std::fs::File::open(&archive) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("opening {}", archive.display())),
    };
    let decoder = zstd::Decoder::new(file).context("starting to decompress the archive")?;
    let mut bytes = Vec::new();
    decoder
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("decompressing {}", archive.display()))?;
    anyhow::ensure!(
        bytes.len() as u64 <= cap,
        "{} is over {} MB decoded",
        archive.display(),
        cap / (1024 * 1024)
    );
    Ok(Some(String::from_utf8(bytes).unwrap_or_else(|e| {
        String::from_utf8_lossy(e.as_bytes()).into_owned()
    })))
}

/// Every readable sidecar in the archive. One that can't be read, or whose id or transcript path fails
/// `sidecar_allowed`, is skipped with a warning, so a rebuild never indexes it.
pub fn read_all_meta(home: &Path) -> Vec<SessionMeta> {
    let mut found = Vec::new();
    let Ok(agents) = std::fs::read_dir(paths::history_dir(home).join("archive")) else {
        return found;
    };
    for agent in agents.flatten() {
        let Ok(files) = std::fs::read_dir(agent.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if !file.file_name().to_string_lossy().ends_with(".meta.json") {
                continue;
            }
            match store::read::<SessionMeta>(&path) {
                Ok(Some(meta)) if sidecar_allowed(&meta.agent, &meta.transcript_path, &meta.session_id) => {
                    found.push(meta)
                }
                Ok(Some(_)) => tracing::warn!(
                    "skipping history sidecar {}: unsafe session id or transcript path",
                    path.display()
                ),
                Ok(None) => {}
                Err(err) => tracing::warn!("skipping history sidecar {}: {err:#}", path.display()),
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::super::{RecordingLink, SessionMeta};
    use super::*;
    use crate::store;
    use marshell_protocol::history::Client;

    fn meta(id: &str, transcript: &std::path::Path) -> SessionMeta {
        SessionMeta {
            agent: "claude".into(),
            session_id: id.into(),
            tab_id: "t1".into(),
            cwd: "c".into(),
            transcript_path: transcript.to_string_lossy().into_owned(),
            started_ms: 1,
            updated_ms: 2,
            previous_in_tab: None,
            recordings: vec![],
            client: Default::default(),
        }
    }

    #[test]
    fn a_tombstone_is_kept_until_a_copy_is_made_again() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(deleted_at(home.path(), "claude", "s1"), None);
        add_tombstone(home.path(), "claude", "s1", 10).unwrap();
        add_tombstone(home.path(), "codex", "s1", 20).unwrap();
        add_tombstone(home.path(), "claude", "s1", 30).unwrap();
        assert_eq!(deleted_at(home.path(), "claude", "s1"), Some(30));
        assert_eq!(deleted_at(home.path(), "codex", "s1"), Some(20));
        let src = home.path().join("s1.jsonl");
        std::fs::write(&src, b"{\"a\":1}\n").unwrap();
        copy_transcript(home.path(), &meta("s1", &src)).unwrap();
        assert_eq!(deleted_at(home.path(), "claude", "s1"), None);
        assert_eq!(deleted_at(home.path(), "codex", "s1"), Some(20));
    }

    #[test]
    fn copy_round_trips() {
        let home = tempfile::tempdir().unwrap();
        let src = home.path().join("s1.jsonl");
        let bytes = b"{\"a\":1}\n{\"b\":2}\n".repeat(50);
        std::fs::write(&src, &bytes).unwrap();
        let m = meta("s1", &src);
        let archived = copy_transcript(home.path(), &m).unwrap();
        assert_eq!(
            zstd::decode_all(std::fs::File::open(&archived).unwrap()).unwrap(),
            bytes
        );
        let (_, sidecar) = archive_paths(home.path(), "claude", "s1");
        assert_eq!(store::read::<SessionMeta>(&sidecar).unwrap(), Some(m));
    }

    fn link(file: &str, first_ms: u64, last_ms: u64) -> RecordingLink {
        RecordingLink {
            file: file.into(),
            first_ms,
            last_ms,
        }
    }

    fn files(listed: &[RecordingLink]) -> Vec<&str> {
        listed.iter().map(|l| l.file.as_str()).collect()
    }

    #[test]
    fn write_meta_keeps_recordings_already_listed() {
        let home = tempfile::tempdir().unwrap();
        let mut m = meta("s1", &home.path().join("s1.jsonl"));
        assert_eq!(read_meta(home.path(), "claude", "s1").unwrap(), None);
        m.recordings = vec![link("a-1.cast", 1, 1), link("b-2.cast", 2, 2)];
        write_meta(home.path(), &m).unwrap();
        // A writer that knows only the newest one, or none, drops nothing.
        m.recordings = vec![link("c-3.cast", 3, 3), link("a-1.cast", 1, 1)];
        write_meta(home.path(), &m).unwrap();
        m.recordings = vec![];
        write_meta(home.path(), &m).unwrap();
        let listed = read_meta(home.path(), "claude", "s1").unwrap().unwrap().recordings;
        assert_eq!(files(&listed), ["a-1.cast", "b-2.cast", "c-3.cast"]);
    }

    #[test]
    fn write_meta_merges_links_by_file_with_the_widest_span() {
        let home = tempfile::tempdir().unwrap();
        let mut m = meta("s1", &home.path().join("s1.jsonl"));
        m.recordings = vec![link("a-1.cast", 100, 200), link("b-2.cast", 300, 300)];
        write_meta(home.path(), &m).unwrap();
        // The same file again: one entry, the earliest first and the latest last; the order stays.
        m.recordings = vec![link("b-2.cast", 250, 400), link("a-1.cast", 150, 900)];
        write_meta(home.path(), &m).unwrap();
        let listed = read_meta(home.path(), "claude", "s1").unwrap().unwrap().recordings;
        assert_eq!(listed, [link("a-1.cast", 100, 900), link("b-2.cast", 250, 400)]);
    }

    #[test]
    fn an_old_sidecar_without_a_client_reads_as_marshell() {
        let home = tempfile::tempdir().unwrap();
        let (_, sidecar) = archive_paths(home.path(), "claude", "s1");
        std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
        std::fs::write(
            &sidecar,
            r#"{"version":2,"agent":"claude","session_id":"s1","tab_id":"t1","cwd":"c","transcript_path":"p","started_ms":1,"updated_ms":2,"previous_in_tab":null,"recordings":[]}"#,
        )
        .unwrap();
        let got = read_meta(home.path(), "claude", "s1").unwrap().unwrap();
        assert_eq!(got.client, Client::Marshell);
    }

    #[test]
    fn write_meta_keeps_a_marshell_owner() {
        let home = tempfile::tempdir().unwrap();
        let mut owned = meta("s1", &home.path().join("s1.jsonl"));
        owned.started_ms = 5;
        owned.updated_ms = 10;
        write_meta(home.path(), &owned).unwrap();
        let mut imported = owned.clone();
        imported.tab_id = String::new();
        imported.client = Client::Terminal;
        imported.started_ms = 3;
        imported.updated_ms = 20;
        write_meta(home.path(), &imported).unwrap();
        let got = read_meta(home.path(), "claude", "s1").unwrap().unwrap();
        assert_eq!(got.tab_id, "t1");
        assert_eq!(got.client, Client::Marshell);
        assert_eq!(got.updated_ms, 20);
        assert_eq!(got.started_ms, 3);
    }

    #[test]
    fn write_meta_lets_marshell_take_over_an_imported_session() {
        let home = tempfile::tempdir().unwrap();
        let mut imported = meta("s1", &home.path().join("s1.jsonl"));
        imported.tab_id = String::new();
        imported.client = Client::Terminal;
        write_meta(home.path(), &imported).unwrap();
        let owned = meta("s1", &home.path().join("s1.jsonl"));
        write_meta(home.path(), &owned).unwrap();
        let got = read_meta(home.path(), "claude", "s1").unwrap().unwrap();
        assert_eq!(got.tab_id, "t1");
        assert_eq!(got.client, Client::Marshell);
    }

    #[test]
    fn write_meta_lets_a_resume_in_another_tab_take_the_session() {
        let home = tempfile::tempdir().unwrap();
        let owned = meta("s1", &home.path().join("s1.jsonl"));
        write_meta(home.path(), &owned).unwrap();
        let mut resumed = owned.clone();
        resumed.tab_id = "t2".into();
        resumed.previous_in_tab = Some("s0".into());
        write_meta(home.path(), &resumed).unwrap();
        let got = read_meta(home.path(), "claude", "s1").unwrap().unwrap();
        assert_eq!(got.tab_id, "t2");
        assert_eq!(got.previous_in_tab.as_deref(), Some("s0"));
    }

    #[test]
    fn write_meta_keeps_the_last_hundred_recordings() {
        let home = tempfile::tempdir().unwrap();
        let mut m = meta("s1", &home.path().join("s1.jsonl"));
        m.recordings = (0..120).map(|n| link(&format!("t-{n}.cast"), n, n)).collect();
        write_meta(home.path(), &m).unwrap();
        let listed = read_meta(home.path(), "claude", "s1").unwrap().unwrap().recordings;
        assert_eq!(listed.len(), MAX_RECORDINGS);
        assert_eq!(listed.first().map(|l| l.file.as_str()), Some("t-20.cast"));
        assert_eq!(listed.last().map(|l| l.file.as_str()), Some("t-119.cast"));
        // One more drops the oldest.
        m.recordings = vec![link("t-120.cast", 120, 120)];
        write_meta(home.path(), &m).unwrap();
        let listed = read_meta(home.path(), "claude", "s1").unwrap().unwrap().recordings;
        assert_eq!(listed.len(), MAX_RECORDINGS);
        assert_eq!(listed.first().map(|l| l.file.as_str()), Some("t-21.cast"));
    }

    #[test]
    fn an_old_sidecar_without_a_list_reads_as_empty_and_is_upgraded() {
        let home = tempfile::tempdir().unwrap();
        let (_, sidecar) = archive_paths(home.path(), "claude", "s1");
        std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
        std::fs::write(
            &sidecar,
            r#"{"version":1,"agent":"claude","session_id":"s1","tab_id":"t1","cwd":"c","transcript_path":"p","started_ms":1,"updated_ms":2,"previous_in_tab":null}"#,
        )
        .unwrap();
        let got = read_meta(home.path(), "claude", "s1").unwrap().unwrap();
        assert!(got.recordings.is_empty());
        let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&sidecar).unwrap()).unwrap();
        assert_eq!(written["version"], 2);
    }

    #[test]
    fn missing_transcript_keeps_the_old_copy() {
        let home = tempfile::tempdir().unwrap();
        let src = home.path().join("s1.jsonl");
        std::fs::write(&src, b"hello\n").unwrap();
        let m = meta("s1", &src);
        let archived = copy_transcript(home.path(), &m).unwrap();
        let before = std::fs::read(&archived).unwrap();
        std::fs::remove_file(&src).unwrap();
        assert!(copy_transcript(home.path(), &m).is_err());
        assert_eq!(std::fs::read(&archived).unwrap(), before);
        let leftovers = std::fs::read_dir(archived.parent().unwrap())
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn read_archive_round_trips_and_caps() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(read_archive(home.path(), "claude", "s1").unwrap(), None);
        let src = home.path().join("s1.jsonl");
        std::fs::write(
            &src,
            "{\"a\":1}
{\"b\":2}
",
        )
        .unwrap();
        copy_transcript(home.path(), &meta("s1", &src)).unwrap();
        assert_eq!(
            read_archive(home.path(), "claude", "s1").unwrap().as_deref(),
            Some(
                "{\"a\":1}
{\"b\":2}
"
            )
        );
        assert!(read_archive_capped(home.path(), "claude", "s1", 16).unwrap().is_some());
        assert!(read_archive_capped(home.path(), "claude", "s1", 15).is_err());
        let (archive, _) = archive_paths(home.path(), "claude", "s1");
        std::fs::write(&archive, b"not zstd").unwrap();
        assert!(read_archive(home.path(), "claude", "s1").is_err());
    }

    #[test]
    fn read_all_meta_skips_broken_sidecars() {
        let home = tempfile::tempdir().unwrap();
        write_meta(home.path(), &meta("a", &home.path().join("a.jsonl"))).unwrap();
        write_meta(home.path(), &meta("b", &home.path().join("b.jsonl"))).unwrap();
        let (_, broken) = archive_paths(home.path(), "claude", "c");
        std::fs::write(&broken, b"garbage \xff not json").unwrap();
        assert_eq!(read_all_meta(home.path()).len(), 2);
    }

    #[test]
    fn rebuild_skips_unsafe_sidecars() {
        let home = tempfile::tempdir().unwrap();
        let dir = std::path::Path::new(if cfg!(windows) {
            r"C:\Users\u\.claude\p"
        } else {
            "/home/u/.claude/p"
        });
        write_meta(home.path(), &meta("good-1", &dir.join("good-1.jsonl"))).unwrap();
        // Sidecars as something that skipped the hook-time check would have left them.
        write_meta(home.path(), &meta("x; calc", &dir.join("x; calc.jsonl"))).unwrap();
        write_meta(home.path(), &meta("-rf", &dir.join("-rf.jsonl"))).unwrap();
        write_meta(
            home.path(),
            &meta("unc-1", std::path::Path::new(r"\\host\share\unc-1.jsonl")),
        )
        .unwrap();
        write_meta(home.path(), &meta("other-1", &dir.join("elsewhere.jsonl"))).unwrap();
        let ids: Vec<String> = read_all_meta(home.path()).into_iter().map(|m| m.session_id).collect();
        assert_eq!(ids, vec!["good-1".to_string()]);
    }
}
