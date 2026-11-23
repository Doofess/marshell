//! `git diff --raw -z` output: one record per changed file.

use marshell_protocol::api::DiffStatus;

/// One changed file, as `diff --raw` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEntry {
    pub status: DiffStatus,
    pub path: String,
    /// The path before a rename, or the file a copy was made from.
    pub old_path: Option<String>,
}

/// Reads records of the form `:<mode> <mode> <oid> <oid> <letter>[score]\0<path>\0`, with a second
/// `<path>\0` for a rename or copy. A record of a kind that isn't known (`X`), a cut record and a path that
/// isn't UTF-8 are skipped.
pub fn parse_raw(bytes: &[u8]) -> Vec<RawEntry> {
    let mut tokens: Vec<&[u8]> = bytes.split(|b| *b == 0).collect();
    // The text after the last NUL is the start of a record that was cut (or nothing).
    tokens.pop();
    let mut entries = Vec::new();
    let mut at = 0;
    while at < tokens.len() {
        let meta = tokens[at];
        at += 1;
        if meta.first() != Some(&b':') {
            // Not the start of a record: look for the next one.
            continue;
        }
        // `:<mode> <mode> <oid> <oid> <letter>[score]`
        let letter = meta
            .split(|b| *b == b' ')
            .nth(4)
            .and_then(|field| field.first())
            .copied();
        let two = matches!(letter, Some(b'R' | b'C'));
        let want = if two { 2 } else { 1 };
        if at + want > tokens.len() {
            break;
        }
        let first = std::str::from_utf8(tokens[at]).ok();
        let second = if two {
            std::str::from_utf8(tokens[at + 1]).ok()
        } else {
            None
        };
        at += want;
        let (Some(first), true) = (first, !two || second.is_some()) else {
            continue;
        };
        let entry = match letter {
            Some(b'A') => (DiffStatus::Added, first, None),
            Some(b'D') => (DiffStatus::Deleted, first, None),
            Some(b'M' | b'T' | b'U') => (DiffStatus::Modified, first, None),
            Some(b'R') => (DiffStatus::Renamed, second.unwrap_or(first), Some(first)),
            Some(b'C') => (DiffStatus::Added, second.unwrap_or(first), Some(first)),
            _ => continue,
        };
        entries.push(RawEntry {
            status: entry.0,
            path: entry.1.to_string(),
            old_path: entry.2.map(str::to_string),
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(letter: &str, paths: &[&str]) -> Vec<u8> {
        let mut out = format!(":100644 100644 {0} {0} {letter}\0", "1".repeat(40)).into_bytes();
        for path in paths {
            out.extend_from_slice(path.as_bytes());
            out.push(0);
        }
        out
    }

    fn entry(status: DiffStatus, path: &str, old: Option<&str>) -> RawEntry {
        RawEntry {
            status,
            path: path.to_string(),
            old_path: old.map(str::to_string),
        }
    }

    #[test]
    fn raw_parses_modify_add_delete_rename_and_copy() {
        let mut bytes = Vec::new();
        bytes.extend(record("M", &["a.txt"]));
        bytes.extend(record("A", &["dir/new file.txt"]));
        bytes.extend(record("D", &["gone.txt"]));
        bytes.extend(record("R087", &["old.txt", "new.txt"]));
        bytes.extend(record("C100", &["from.txt", "copy.txt"]));
        bytes.extend(record("T", &["link"]));
        bytes.extend(record("U", &["merging.txt"]));
        assert_eq!(
            parse_raw(&bytes),
            vec![
                entry(DiffStatus::Modified, "a.txt", None),
                entry(DiffStatus::Added, "dir/new file.txt", None),
                entry(DiffStatus::Deleted, "gone.txt", None),
                entry(DiffStatus::Renamed, "new.txt", Some("old.txt")),
                entry(DiffStatus::Added, "copy.txt", Some("from.txt")),
                entry(DiffStatus::Modified, "link", None),
                entry(DiffStatus::Modified, "merging.txt", None),
            ]
        );
        assert!(parse_raw(b"").is_empty());
    }

    #[test]
    fn raw_skips_a_cut_record_and_non_utf8_paths() {
        let mut bytes = Vec::new();
        bytes.extend(record("M", &["a.txt"]));
        bytes.extend(record("X", &["unknown.txt"]));
        let mut bad = record("M", &[]);
        bad.extend_from_slice(&[b'b', 0xff, 0xfe, 0]);
        bytes.extend(bad);
        bytes.extend(record("R100", &["old.txt", "new.txt"]));
        bytes.extend(record("M", &["z.txt"]));
        let whole = parse_raw(&bytes);
        assert_eq!(
            whole.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
            ["a.txt", "new.txt", "z.txt"]
        );
        // The output was cut inside the last record's path.
        let mut cut = bytes.clone();
        cut.extend(record("M", &["partial"]));
        cut.pop();
        cut.truncate(cut.len() - 3);
        assert_eq!(parse_raw(&cut), whole);
        // ... and after the first path of a rename.
        let mut rename = bytes.clone();
        rename.extend(record("R100", &["only-the-old-path"]));
        assert_eq!(parse_raw(&rename), whole);
    }
}
