//! A library copy in a CLI's folder: a marker line that names the doc and holds a hash of the copy's text.

use super::doc::Doc;
use marshell_protocol::instructions::DocScope;
use sha2::{Digest, Sha256};
use std::path::Path;

const TAIL: &str = "; edit it in Marshell -->";

#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub name: String,
    pub hash: String,
    pub source: String,
}

pub fn marker_line(name: &str, hash: &str, source: &str) -> String {
    format!("<!-- marshell:doc {name} {hash} from {source}{TAIL}")
}

pub fn parse_marker(line: &str) -> Option<Marker> {
    let line = line.trim_end();
    let rest = line
        .strip_prefix("<!-- marshell:doc ")
        .or_else(|| line.strip_prefix("<!-- marshell:begin doc "))?;
    let rest = rest.strip_suffix(TAIL)?;
    let (name, rest) = rest.split_once(' ')?;
    let (hash, rest) = rest.split_once(' ')?;
    let source = rest.strip_prefix("from ")?;
    let hash_ok = hash.len() == 12 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if name.is_empty() || !hash_ok {
        return None;
    }
    Some(Marker {
        name: name.to_string(),
        hash: hash.to_string(),
        source: source.to_string(),
    })
}

/// `<!-- marshell:end doc <name> -->`.
pub fn parse_end(line: &str) -> Option<String> {
    let name = line
        .trim_end()
        .strip_prefix("<!-- marshell:end doc ")?
        .strip_suffix(" -->")?;
    (!name.is_empty() && !name.contains(' ')).then(|| name.to_string())
}

/// The first line of `body` that Marshell's copies use as a marker, counted from 1. A doc can't hold one: in Codex's
/// files it would end or start a block.
pub fn first_marker(body: &str) -> Option<usize> {
    body.lines()
        .position(|line| parse_marker(line).is_some() || parse_end(line).is_some())
        .map(|i| i + 1)
}

/// BOM dropped, `\r\n` taken as `\n`.
pub fn normalized(text: &str) -> String {
    text.strip_prefix('\u{feff}').unwrap_or(text).replace("\r\n", "\n")
}

/// The first 12 hex digits of the SHA-256 of the normalized text.
pub fn payload_hash(payload: &str) -> String {
    let digest = Sha256::digest(normalized(payload).as_bytes());
    hex::encode(digest)[..12].to_string()
}

/// The frontmatter, the marker line (with the hash of frontmatter plus body), then the body.
pub fn file_copy(frontmatter: &str, name: &str, source: &str, body: &str) -> String {
    let hash = payload_hash(&format!("{frontmatter}{body}"));
    format!(
        "{frontmatter}{}\n{body}",
        marker_line(&name.to_lowercase(), &hash, source)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judged {
    InSync,
    OutOfDate,
    ChangedOutside,
    Taken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unwanted {
    Ours,
    Edited,
    NotOurs,
}

/// A copy's parts: its marker, the text before the marker line (the frontmatter) and the text after it.
struct Split {
    marker: Marker,
    before: String,
    after: String,
}

fn split(text: &str) -> Option<Split> {
    let text = normalized(text);
    let lines: Vec<&str> = text.split('\n').collect();
    let at = if lines.first().is_some_and(|l| parse_marker(l).is_some()) {
        0
    } else if lines.first().is_some_and(|l| l.trim_end() == "---") {
        lines.iter().skip(1).position(|l| l.trim_end() == "---")? + 2
    } else {
        return None;
    };
    let marker = parse_marker(lines.get(at)?)?;
    let before: String = lines[..at].iter().map(|l| format!("{l}\n")).collect();
    let after = lines[at + 1..].join("\n");
    Some(Split { marker, before, after })
}

impl Split {
    fn intact(&self) -> bool {
        payload_hash(&format!("{}{}", self.before, self.after)) == self.marker.hash
    }
}

pub fn judge_file(expected: &str, actual: &str, name: &str) -> Judged {
    let Some(s) = split(actual) else {
        return Judged::Taken;
    };
    if !s.marker.name.eq_ignore_ascii_case(name) {
        return Judged::Taken;
    }
    if !s.intact() {
        return Judged::ChangedOutside;
    }
    if normalized(actual) == normalized(expected) {
        Judged::InSync
    } else {
        Judged::OutOfDate
    }
}

pub fn judge_unwanted(actual: &str, own_name: &str) -> Unwanted {
    match split(actual) {
        Some(s) if s.marker.name.eq_ignore_ascii_case(own_name) => {
            if s.intact() {
                Unwanted::Ours
            } else {
                Unwanted::Edited
            }
        }
        _ => Unwanted::NotOurs,
    }
}

/// The text after the marker line, normalized.
pub fn copy_body(actual: &str) -> Option<String> {
    split(actual).map(|s| s.after)
}

/// The name in `marshell-<name><suffix>`, when the file is such a copy's.
pub fn own_name_of(file_name: &str, suffix: &str) -> Option<String> {
    let name = file_name.strip_prefix("marshell-")?.strip_suffix(suffix)?;
    let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then(|| name.to_string())
}

/// `'text'`, a quote inside doubled.
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Where a doc's text came from, as the marker shows it.
pub fn source_of(doc: &Doc, project: Option<&Path>, user_home: &Path) -> String {
    let project = if doc.scope == DocScope::Project { project } else { None };
    crate::doctor::instructions::display(&doc.path, project, user_home)
}

/// A copy file as it stands now.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Disk {
    Missing,
    /// A link, a folder, a file over 4 MiB, one that isn't UTF-8 text, or one that can't be read: never read or
    /// written, because a link could point anywhere and Marshell would edit the target.
    Refused,
    /// The file's text and the config service's hash of its raw bytes.
    Text {
        text: String,
        hash: String,
    },
}

/// Reads a file only when `symlink_metadata` says it is a plain file of at most 4 MiB (as the docs are read).
pub(crate) fn read_disk(path: &Path) -> Disk {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Disk::Missing,
        Err(_) => return Disk::Refused,
    };
    if meta.file_type().is_symlink() || !meta.is_file() || meta.len() > super::doc::MAX_BYTES {
        return Disk::Refused;
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            let hash = crate::config::hash::hash_bytes(&bytes);
            match String::from_utf8(bytes) {
                Ok(text) => Disk::Text { text, hash },
                Err(_) => Disk::Refused,
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Disk::Missing,
        Err(_) => Disk::Refused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_round_trips() {
        let line = marker_line("security", "0123456789ab", ".marshell/instructions/security.md");
        assert_eq!(
            line,
            "<!-- marshell:doc security 0123456789ab from .marshell/instructions/security.md; edit it in Marshell -->"
        );
        let m = parse_marker(&line).unwrap();
        assert_eq!(m.name, "security");
        assert_eq!(m.hash, "0123456789ab");
        assert_eq!(m.source, ".marshell/instructions/security.md");
        let begin = line.replace("marshell:doc", "marshell:begin doc") + "\r";
        assert_eq!(parse_marker(&begin).unwrap(), m);
        let spaced = parse_marker(&marker_line("a", "0123456789ab", "~/My Docs/x.md")).unwrap();
        assert_eq!(spaced.source, "~/My Docs/x.md");
        assert_eq!(parse_end("<!-- marshell:end doc a -->"), Some("a".into()));
    }

    #[test]
    fn marker_rejects_other_comments() {
        assert_eq!(parse_marker("<!-- note -->"), None);
        assert_eq!(parse_marker(&marker_line("a", "0123456789a", "s")), None);
        assert_eq!(parse_marker(&marker_line("a", "0123456789AB", "s")), None);
    }

    #[test]
    fn payload_hash_ignores_crlf() {
        assert_eq!(payload_hash("a\r\nb\n"), payload_hash("\u{feff}a\nb\n"));
        assert_eq!(payload_hash("a").len(), 12);
    }

    #[test]
    fn states() {
        let expected = file_copy("", "security", "s", "Body\n");
        assert_eq!(judge_file(&expected, &expected, "security"), Judged::InSync);
        assert_eq!(judge_file(&expected, &expected, "SECURITY"), Judged::InSync);
        let old = file_copy("", "security", "s", "Old\n");
        assert_eq!(judge_file(&expected, &old, "security"), Judged::OutOfDate);
        let mine = old.replace("Old", "Mine");
        assert_eq!(judge_file(&expected, &mine, "security"), Judged::ChangedOutside);
        assert_eq!(judge_file(&expected, "Body\n", "security"), Judged::Taken);
        let other = file_copy("", "other", "s", "Body\n");
        assert_eq!(judge_file(&expected, &other, "security"), Judged::Taken);
    }

    #[test]
    fn crlf_copy_is_in_sync() {
        let expected = file_copy("---\npaths:\n  - 'a'\n---\n", "security", "s", "Body\n");
        let crlf = expected.replace('\n', "\r\n");
        assert_eq!(judge_file(&expected, &crlf, "security"), Judged::InSync);
        let plain = file_copy("", "security", "s", "Body\n");
        assert_eq!(
            judge_file(&plain, &plain.replace('\n', "\r\n"), "security"),
            Judged::InSync
        );
    }

    #[test]
    fn frontmatter_edit_is_changed_outside() {
        let fm = crate::agents::antigravity::library::frontmatter(&[]);
        let expected = file_copy(&fm, "security", "s", "Body\n");
        let edited = expected.replace("trigger: always_on", "trigger: manual");
        assert_ne!(edited, expected);
        assert_eq!(judge_file(&expected, &edited, "security"), Judged::ChangedOutside);
    }

    #[test]
    fn unwanted() {
        let copy = file_copy("", "gone", "s", "Body\n");
        assert_eq!(judge_unwanted(&copy, "gone"), Unwanted::Ours);
        assert_eq!(judge_unwanted(&copy.replace("Body", "Mine"), "gone"), Unwanted::Edited);
        assert_eq!(judge_unwanted(&copy, "gone-old"), Unwanted::NotOurs);
        assert_eq!(judge_unwanted("Body\n", "gone"), Unwanted::NotOurs);
    }

    #[test]
    fn copy_body_takes_the_text_after_the_marker() {
        let c = file_copy("---\npaths:\n  - 'a'\n---\n", "x", "s", "Line\n");
        assert_eq!(copy_body(&c), Some("Line\n".to_string()));
        assert_eq!(copy_body("Line\n"), None);
    }

    #[test]
    fn own_names() {
        assert_eq!(own_name_of("marshell-a.md", ".md"), Some("a".into()));
        assert_eq!(own_name_of("style.md", ".md"), None);
        assert_eq!(own_name_of("marshell-.md", ".md"), None);
    }
}
