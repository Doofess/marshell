//! Codex's library copies (slice 8a): tagged blocks in the user `AGENTS.md`, and tagged sections inside
//! `developer_instructions` in a project's `.codex/config.toml`. Text outside Marshell's lines survives byte for byte
//! (`AGENTS.md`) or by decoded value (the TOML string); every other TOML key and comment stays.

use crate::instructions::copy::{Judged, Unwanted, normalized, parse_end, parse_marker, payload_hash};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Value};

const BEGIN: &str = "<!-- marshell:begin doc ";
const UNREADABLE: &str = "Codex's .codex/config.toml can't be read, so Marshell doesn't change it.";
const NOT_TEXT: &str = "developer_instructions in .codex/config.toml isn't text, so Marshell doesn't change it.";
const KEY: &str = "developer_instructions";
const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// One block: the begin marker's fields and the text between the begin and end lines (`\n` line ends).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Block {
    pub name: String,
    pub hash: String,
    pub source: String,
    pub body: String,
}

/// The begin line (with the hash of the body), the body and the end line, all with `\n`. A body that doesn't end in
/// a line end gets one first, so what is read back hashes the same.
// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn block_text(name: &str, source: &str, body: &str) -> String {
    let name = name.to_lowercase();
    let mut body = normalized(body);
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    format!(
        "{BEGIN}{name} {} from {source}; edit it in Marshell -->\n{body}<!-- marshell:end doc {name} -->\n",
        payload_hash(&body)
    )
}

/// A block in a text, by line numbers (the end line is included).
struct Span {
    start: usize,
    end: usize,
    block: Block,
}

fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

fn blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn scan(lines: &[&str]) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = if i == 0 {
            lines[0].strip_prefix('\u{feff}').unwrap_or(lines[0])
        } else {
            lines[i]
        };
        let begin = if line.starts_with(BEGIN) {
            parse_marker(line)
        } else {
            None
        };
        if let Some(m) = begin {
            let found = lines[i + 1..]
                .iter()
                .position(|l| parse_end(l).is_some_and(|n| n.eq_ignore_ascii_case(&m.name)));
            if let Some(off) = found {
                let end = i + 1 + off;
                spans.push(Span {
                    start: i,
                    end,
                    block: Block {
                        name: m.name,
                        hash: m.hash,
                        source: m.source,
                        body: normalized(&lines[i + 1..end].concat()),
                    },
                });
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    spans
}

/// The blocks in a text, judged with `\r\n` as `\n`. A begin line with no end line of its name is not a block.
pub(crate) fn blocks_in(text: &str) -> Vec<Block> {
    scan(&lines_of(text)).into_iter().map(|s| s.block).collect()
}

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn judge_block(expected_body: &str, found: &Block) -> Judged {
    if normalized(&found.body) == normalized(expected_body) {
        Judged::InSync
    } else if payload_hash(&found.body) == found.hash {
        Judged::OutOfDate
    } else {
        Judged::ChangedOutside
    }
}

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn judge_unwanted_block(found: &Block) -> Unwanted {
    if payload_hash(&found.body) == found.hash {
        Unwanted::Ours
    } else {
        Unwanted::Edited
    }
}

fn named(list: &[String], name: &str) -> bool {
    list.iter().any(|k| k.eq_ignore_ascii_case(name))
}

/// The text with the wanted blocks in place of their old ones (or added at the end), the kept ones as they are, and
/// every other block gone. The person's lines are copied as they are, line ends included.
fn rewrite(text: &str, wanted: &[(String, String)], keep: &[String]) -> String {
    let lines = lines_of(text);
    let spans = scan(&lines);
    let mut drop = vec![false; lines.len()];
    let mut replace: Vec<(usize, &str)> = Vec::new();
    let mut placed: HashSet<String> = HashSet::new();
    let mut existing: HashSet<String> = HashSet::new();
    for s in &spans {
        let name = s.block.name.to_lowercase();
        existing.insert(name.clone());
        if named(keep, &name) {
            continue;
        }
        for d in &mut drop[s.start..=s.end] {
            *d = true;
        }
        let new = wanted.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name));
        if let (Some((_, t)), true) = (new, placed.insert(name)) {
            replace.push((s.start, t));
        } else if s.start > 0 && !drop[s.start - 1] && blank(lines[s.start - 1]) {
            drop[s.start - 1] = true;
        } else if lines.get(s.end + 1).is_some_and(|l| blank(l)) {
            drop[s.end + 1] = true;
        }
    }
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some((_, t)) = replace.iter().find(|(at, _)| *at == i) {
            out.push_str(t);
        } else if !drop[i] {
            out.push_str(line);
        }
    }
    let mut added: Vec<&(String, String)> = Vec::new();
    for w in wanted {
        let name = w.0.to_lowercase();
        if !existing.contains(&name) && !added.iter().any(|a| a.0.eq_ignore_ascii_case(&name)) {
            added.push(w);
        }
    }
    added.sort_by_key(|w| w.0.to_lowercase());
    for (_, t) in added {
        if !out.is_empty() {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.lines().next_back().is_some_and(blank) {
                out.push('\n');
            }
        }
        out.push_str(t);
    }
    out
}

/// The text with its blocks changed as `wanted` (name and full `block_text`) and `keep` say; None when only
/// whitespace is left, so the file goes.
// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn with_blocks(text: &str, wanted: &[(String, String)], keep: &[String]) -> Option<String> {
    let out = rewrite(text, wanted, keep);
    (!blank(&out)).then_some(out)
}

/// The text with every Marshell block taken out.
pub(crate) fn person_text(text: &str) -> String {
    rewrite(text, &[], &[])
}

/// The person's text when the file is theirs to judge: a link or a file over 4 MiB counts as the person's text
/// (and is never read), a missing or unreadable file as empty.
fn override_text(path: &Path) -> String {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return String::new();
    };
    if meta.file_type().is_symlink() || meta.len() > MAX_BYTES {
        return "x".to_string();
    }
    std::fs::read_to_string(path).unwrap_or_default()
}

/// `AGENTS.override.md` when its text, with Marshell's blocks taken out, holds more than whitespace, else `AGENTS.md`
/// (decision 45).
// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn user_file(codex_home: &Path) -> PathBuf {
    let over = codex_home.join("AGENTS.override.md");
    if blank(&person_text(&override_text(&over))) {
        codex_home.join("AGENTS.md")
    } else {
        over
    }
}

/// The user file `user_file` didn't pick, whose blocks are unwanted.
// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn other_user_file(codex_home: &Path) -> PathBuf {
    if user_file(codex_home).ends_with("AGENTS.md") {
        codex_home.join("AGENTS.override.md")
    } else {
        codex_home.join("AGENTS.md")
    }
}

// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn project_config(project: &Path) -> PathBuf {
    project.join(".codex").join("config.toml")
}

/// The `developer_instructions` text, `\r\n` as `\n`: None when there is no such key.
fn instructions_of(doc: &DocumentMut) -> Result<Option<String>, String> {
    match doc.get(KEY) {
        None => Ok(None),
        Some(item) => item
            .as_str()
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| NOT_TEXT.to_string()),
    }
}

/// The sections in the project's `developer_instructions`.
// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn sections_in(config_text: &str) -> Result<Vec<Block>, String> {
    let doc: DocumentMut = config_text.parse().map_err(|_| UNREADABLE.to_string())?;
    Ok(instructions_of(&doc)?
        .map(|s| blocks_in(&normalized(&s)))
        .unwrap_or_default())
}

/// The config with its `developer_instructions` sections changed as `wanted` and `keep` say, as `with_blocks` does for
/// a text. The string's other text stays; every other key and comment is untouched. Ok(None) deletes the file: the key
/// held nothing else and no other content is left.
// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn with_sections(
    config_text: Option<&str>,
    wanted: &[(String, String)],
    keep: &[String],
) -> Result<Option<String>, String> {
    let mut doc: DocumentMut = match config_text {
        Some(t) => t.parse().map_err(|_| UNREADABLE.to_string())?,
        None => DocumentMut::new(),
    };
    let raw = instructions_of(&doc)?;
    let current = normalized(raw.as_deref().unwrap_or(""));
    let mut text = rewrite(&current, wanted, keep);
    // A single line of the person's text with no end line of its own gets none back once the sections are gone.
    let single = text.trim_end_matches('\n').find('\n').is_none();
    if text.ends_with('\n') && single && blocks_in(&text).is_empty() && !blocks_in(&current).is_empty() {
        text.pop();
    }
    if raw.as_deref() == Some(text.as_str()) || (raw.is_none() && blank(&text)) {
        return Ok(config_text.map(str::to_string));
    }
    if blank(&text) {
        doc.remove(KEY);
        let left = doc.to_string();
        return Ok((!blank(&left)).then_some(left));
    }
    let mut value = Value::from(text);
    if let Some(old) = doc.get(KEY).and_then(Item::as_value) {
        *value.decor_mut() = old.decor().clone();
    }
    doc[KEY] = Item::Value(value);
    Ok(Some(doc.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trust {
    Trusted,
    Untrusted,
    NoEntry,
}

/// What Codex's user config says about the project: the folder's own `[projects.'<key>']` entry, else its git root's,
/// compared as Codex compares them (`trust_key`). The first entry found decides.
// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn project_trust(user_config: Option<&str>, project_key: &str, git_root_key: Option<&str>) -> Trust {
    use super::extensions::trust_key;
    let Some(doc) = user_config.and_then(|t| t.parse::<DocumentMut>().ok()) else {
        return Trust::NoEntry;
    };
    let Some(table) = doc.get("projects").and_then(Item::as_table_like) else {
        return Trust::NoEntry;
    };
    let wanted = std::iter::once(project_key).chain(git_root_key).map(trust_key);
    for want in wanted {
        if let Some((_, item)) = table.iter().find(|(key, _)| trust_key(key) == want) {
            let level = item
                .as_table_like()
                .and_then(|t| t.get("trust_level"))
                .and_then(Item::as_str);
            return match level {
                Some("trusted") => Trust::Trusted,
                Some("untrusted") => Trust::Untrusted,
                _ => Trust::NoEntry,
            };
        }
    }
    Trust::NoEntry
}

/// Whether the user's Codex config sets its own `developer_instructions` (a project's value replaces it).
// Used by the library planner (slice 8a Task 5).
#[allow(dead_code)]
pub(crate) fn user_sets_developer_instructions(user_config: Option<&str>) -> bool {
    let Some(doc) = user_config.and_then(|t| t.parse::<DocumentMut>().ok()) else {
        return false;
    };
    match doc.get(KEY) {
        Some(item) => item.as_str().is_none_or(|s| !blank(s)),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sec(body: &str) -> (String, String) {
        ("security".to_string(), block_text("security", "~/s.md", body))
    }

    fn decoded(text: &str) -> String {
        let doc: DocumentMut = text.parse().unwrap();
        doc[KEY].as_str().unwrap().to_string()
    }

    #[test]
    fn adds_a_block_after_the_persons_text() {
        let h = payload_hash("Body\n");
        let got = with_blocks("Mine\n", &[sec("Body\n")], &[]);
        assert_eq!(
            got.unwrap(),
            format!(
                "Mine\n\n<!-- marshell:begin doc security {h} from ~/s.md; edit it in Marshell -->\nBody\n<!-- marshell:end doc security -->\n"
            )
        );
    }

    #[test]
    fn replaces_a_block_in_place() {
        let old = block_text("security", "~/s.md", "Old\n");
        let new = block_text("security", "~/s.md", "New\n");
        let text = format!("A\n\n{old}\nB\n");
        assert_eq!(
            with_blocks(&text, &[sec("New\n")], &[]),
            Some(format!("A\n\n{new}\nB\n"))
        );
    }

    #[test]
    fn removes_a_block_and_its_blank_line() {
        let b = block_text("security", "~/s.md", "Old\n");
        assert_eq!(
            with_blocks(&format!("Mine\n\n{b}"), &[], &[]),
            Some("Mine\n".to_string())
        );
        assert_eq!(with_blocks(&b, &[], &[]), None);
        assert_eq!(
            with_blocks(&format!("A\n\n{b}\nB\n"), &[], &[]),
            Some("A\n\nB\n".to_string())
        );
        let b2 = block_text("other", "~/o.md", "O\n");
        assert_eq!(with_blocks(&format!("{b}\n{b2}"), &[], &[]), None);
    }

    #[test]
    fn keeps_a_block_it_is_told_to_keep() {
        let kept = block_text("zeta", "~/z.md", "Z\n");
        let text = format!("Mine\n\n{kept}");
        let got = with_blocks(&text, &[sec("Body\n")], &["ZETA".to_string()]).unwrap();
        assert!(got.starts_with(&text));
        assert!(got.ends_with(&sec("Body\n").1));
        assert_eq!(got, format!("{text}\n{}", sec("Body\n").1));
    }

    #[test]
    fn new_blocks_go_last_in_name_order() {
        let a = ("alpha".to_string(), block_text("alpha", "s", "A\n"));
        let b = ("beta".to_string(), block_text("beta", "s", "B\n"));
        let got = with_blocks("Mine\n", &[b.clone(), a.clone()], &[]).unwrap();
        assert_eq!(got, format!("Mine\n\n{}\n{}", a.1, b.1));
    }

    #[test]
    fn crlf_text_outside_the_blocks_survives() {
        let b = block_text("security", "~/s.md", "Old\n");
        let text = format!("A\r\nB\r\n\r\n{b}\r\nC\r\n");
        let got = with_blocks(&text, &[sec("New\n")], &[]).unwrap();
        assert!(got.starts_with("A\r\nB\r\n\r\n<!-- marshell:begin doc security "));
        assert!(got.ends_with("-->\n\r\nC\r\n"));
        assert_eq!(person_text(&text), "A\r\nB\r\n\r\nC\r\n");
        let crlf_block = b.replace('\n', "\r\n");
        let found = blocks_in(&format!("x\r\n{crlf_block}"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].body, "Old\n");
    }

    #[test]
    fn a_begin_without_an_end_is_the_persons_text() {
        let b = block_text("security", "~/s.md", "Old\n");
        let begin = b.lines().next().unwrap().to_string();
        let text = format!("{begin}\nText after\n");
        assert!(blocks_in(&text).is_empty());
        assert_eq!(person_text(&text), text);
        let two = format!("{begin}\nMine\n{b}");
        assert_eq!(blocks_in(&two).len(), 1);
        assert_eq!(
            blocks_in("<!-- marshell:begin doc x nothash from s; edit it in Marshell -->\n").len(),
            0
        );
    }

    #[test]
    fn block_states() {
        let h = |b: &str| blocks_in(&block_text("security", "s", b)).remove(0);
        let fresh = h("Body\n");
        assert_eq!(judge_block("Body\n", &fresh), Judged::InSync);
        assert_eq!(judge_unwanted_block(&fresh), Unwanted::Ours);
        let mut edited = fresh.clone();
        edited.body = "Mine\n".to_string();
        assert_eq!(judge_block("Body\n", &edited), Judged::ChangedOutside);
        assert_eq!(judge_unwanted_block(&edited), Unwanted::Edited);
        let old = h("Old\n");
        assert_eq!(judge_block("Body\n", &old), Judged::OutOfDate);
    }

    #[test]
    fn override_with_only_marshell_blocks_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        assert_eq!(user_file(home), home.join("AGENTS.md"));
        assert_eq!(other_user_file(home), home.join("AGENTS.override.md"));
        std::fs::write(home.join("AGENTS.override.md"), "  \n").unwrap();
        assert_eq!(user_file(home), home.join("AGENTS.md"));
        std::fs::write(home.join("AGENTS.override.md"), block_text("security", "s", "B\n")).unwrap();
        assert_eq!(user_file(home), home.join("AGENTS.md"));
        std::fs::write(home.join("AGENTS.override.md"), "x\n").unwrap();
        assert_eq!(user_file(home), home.join("AGENTS.override.md"));
        assert_eq!(other_user_file(home), home.join("AGENTS.md"));
    }

    #[test]
    fn section_lives_in_developer_instructions() {
        let (_, text) = sec("Body\n");
        let got = with_sections(Some("model = \"o3\"\n"), &[sec("Body\n")], &[])
            .unwrap()
            .unwrap();
        let doc: DocumentMut = got.parse().unwrap();
        assert_eq!(doc["model"].as_str(), Some("o3"));
        assert_eq!(decoded(&got), text);
        assert_eq!(sections_in(&got).unwrap().len(), 1);
    }

    #[test]
    fn keeps_the_persons_developer_text() {
        let got = with_sections(Some("developer_instructions = \"Be brief.\"\n"), &[sec("Body\n")], &[])
            .unwrap()
            .unwrap();
        assert!(decoded(&got).starts_with("Be brief.\n\n<!-- marshell:begin doc security "));
        let back = with_sections(Some(&got), &[], &[]).unwrap().unwrap();
        assert_eq!(decoded(&back), "Be brief.");
    }

    #[test]
    fn keeps_comments_and_other_keys() {
        let text = "# top\nmodel = \"o3\" # mine\ndeveloper_instructions = \"Be brief.\" # why\n[tools]\nx = 1\n";
        let got = with_sections(Some(text), &[sec("Body\n")], &[]).unwrap().unwrap();
        assert!(got.contains("# top"));
        assert!(got.contains("model = \"o3\" # mine"));
        assert!(got.contains("# why"));
        assert!(got.contains("[tools]\nx = 1"));
        let doc: DocumentMut = got.parse().unwrap();
        assert_eq!(doc["tools"]["x"].as_integer(), Some(1));
    }

    #[test]
    fn body_with_backslashes_round_trips() {
        let body = "Use C:\\\\tmp and \"\"\" quotes\n";
        let got = with_sections(None, &[sec(body)], &[]).unwrap().unwrap();
        let blocks = sections_in(&got).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].body, body);
    }

    #[test]
    fn crlf_config_is_normalized() {
        let b = block_text("security", "~/s.md", "Old\n");
        let text = format!("developer_instructions = \"\"\"\r\nMine\r\n\r\n{b}\"\"\"\r\nmodel = \"o3\"\r\n");
        let got = with_sections(Some(&text), &[sec("New\n")], &[]).unwrap().unwrap();
        let d = decoded(&got);
        assert!(!d.contains('\r'));
        assert!(d.starts_with("Mine\n\n<!-- marshell:begin doc security "));
        assert!(d.contains("New\n"));
    }

    #[test]
    fn last_section_out_removes_the_key_and_the_file() {
        let only = with_sections(None, &[sec("Body\n")], &[]).unwrap().unwrap();
        assert_eq!(with_sections(Some(&only), &[], &[]), Ok(None));
        let with_model = format!("model = \"o3\"\n{only}");
        let got = with_sections(Some(&with_model), &[], &[]).unwrap().unwrap();
        let doc: DocumentMut = got.parse().unwrap();
        assert!(doc.get(KEY).is_none());
        assert_eq!(doc["model"].as_str(), Some("o3"));
    }

    #[test]
    fn new_file_is_created() {
        let got = with_sections(None, &[sec("Body\n")], &[]).unwrap().unwrap();
        assert_eq!(decoded(&got), sec("Body\n").1);
        assert_eq!(with_sections(None, &[], &[]), Ok(None));
    }

    #[test]
    fn bad_toml_is_refused() {
        assert_eq!(
            with_sections(Some("x = ["), &[], &[]),
            Err("Codex's .codex/config.toml can't be read, so Marshell doesn't change it.".to_string())
        );
        assert_eq!(
            with_sections(Some("developer_instructions = 3\n"), &[sec("B\n")], &[]),
            Err("developer_instructions in .codex/config.toml isn't text, so Marshell doesn't change it.".to_string())
        );
        assert!(sections_in("x = [").is_err());
        assert!(sections_in("developer_instructions = 3\n").is_err());
    }

    #[test]
    fn trust() {
        let cfg = |level: &str, key: &str| format!("[projects.'{key}']\ntrust_level = \"{level}\"\n");
        let p = r"C:\p";
        assert_eq!(project_trust(Some(&cfg("trusted", p)), p, None), Trust::Trusted);
        assert_eq!(project_trust(Some(&cfg("untrusted", p)), p, None), Trust::Untrusted);
        assert_eq!(
            project_trust(Some(&cfg("trusted", r"C:\other")), p, None),
            Trust::NoEntry
        );
        assert_eq!(project_trust(Some("model = 'x'\n"), p, None), Trust::NoEntry);
        assert_eq!(project_trust(None, p, None), Trust::NoEntry);
        assert_eq!(project_trust(Some("x = ["), p, None), Trust::NoEntry);
        assert_eq!(
            project_trust(Some(&cfg("trusted", r"C:\")), p, Some(r"C:\")),
            Trust::Trusted
        );
        assert_eq!(
            project_trust(Some(&cfg("untrusted", r"C:\")), p, Some(r"C:\")),
            Trust::Untrusted
        );
        let both = format!("{}{}", cfg("untrusted", p), cfg("trusted", r"C:\"));
        assert_eq!(project_trust(Some(&both), p, Some(r"C:\")), Trust::Untrusted);
        assert_eq!(project_trust(Some(&cfg("trusted", "C:/P/")), p, None), Trust::Trusted);
    }

    #[test]
    fn user_developer_instructions() {
        assert!(user_sets_developer_instructions(Some(
            "developer_instructions = \"x\"\n"
        )));
        assert!(!user_sets_developer_instructions(Some("")));
        assert!(!user_sets_developer_instructions(None));
    }
}
