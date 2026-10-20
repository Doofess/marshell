//! Compressed copies of agent transcripts, each with a small `.meta.json` sidecar.

use super::{SessionMeta, transcript_allowed};
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

/// Writes the sidecar for a session.
pub fn write_meta(home: &Path, meta: &SessionMeta) -> anyhow::Result<()> {
    let (_, sidecar) = archive_paths(home, &meta.agent, &meta.session_id);
    store::write(&sidecar, meta)
}

/// Reads the whole transcript, compresses it, and replaces the archive copy and the sidecar.
/// If the transcript can't be read, returns an error and leaves the old copy as it was.
pub fn copy_transcript(home: &Path, meta: &SessionMeta) -> anyhow::Result<PathBuf> {
    let bytes = std::fs::read(&meta.transcript_path).with_context(|| format!("reading {}", meta.transcript_path))?;
    let packed = zstd::encode_all(bytes.as_slice(), 3).context("compressing the transcript")?;
    let (archive, _) = archive_paths(home, &meta.agent, &meta.session_id);
    store::write_bytes_atomic(&archive, &packed)?;
    write_meta(home, meta)?;
    Ok(archive)
}

/// Every readable sidecar in the archive. One that can't be read, or whose id or transcript path fails
/// `transcript_allowed`, is skipped with a warning, so a rebuild never indexes it.
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
                Ok(Some(meta)) if transcript_allowed(&meta.transcript_path, &meta.session_id) => found.push(meta),
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
    use super::super::SessionMeta;
    use super::*;
    use crate::store;

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
        }
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
