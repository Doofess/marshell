//! Recorded CLI fixtures must be scrubbed (`pnpm scrub:fixtures <dir>`) before they are committed.

use std::fs;
use std::path::{Path, PathBuf};

const UUID_PREFIX: &str = "00000000-0000-4000-8000-";

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn is_email_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "._%+-".contains(c)
}

/// Every `local@domain.tld` in `text` that is not the placeholder.
fn stray_emails(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (at, _) in text.match_indices('@') {
        let local: String = {
            let rev: String = text[..at].chars().rev().take_while(|c| is_email_char(*c)).collect();
            rev.chars().rev().collect()
        };
        let domain: String = text[at + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '.')
            .collect();
        let domain = domain.trim_end_matches('.');
        if !local.is_empty() && domain.contains('.') {
            let email = format!("{local}@{domain}");
            if email != "user@example.com" {
                found.push(email);
            }
        }
    }
    found
}

/// Every UUID-shaped token (8-4-4-4-12 hex) in `text`.
fn uuids(text: &str) -> Vec<String> {
    const SHAPE: [usize; 5] = [8, 4, 4, 4, 12];
    const LEN: usize = 36;
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i + LEN <= bytes.len() {
        let mut pos = i;
        let mut ok = true;
        for (n, len) in SHAPE.iter().enumerate() {
            if n > 0 {
                if bytes[pos] != b'-' {
                    ok = false;
                    break;
                }
                pos += 1;
            }
            if !bytes[pos..pos + len].iter().all(u8::is_ascii_hexdigit) {
                ok = false;
                break;
            }
            pos += len;
        }
        if ok {
            found.push(text[i..i + LEN].to_string());
            i += LEN;
        } else {
            i += 1;
        }
    }
    found
}

#[test]
fn fixtures_are_scrubbed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    assert!(root.is_dir(), "no fixtures yet: {} does not exist", root.display());
    let mut files = Vec::new();
    walk(&root, &mut files);

    let claude_json = files.iter().any(|f| {
        f.extension().is_some_and(|e| e == "json") && f.strip_prefix(&root).is_ok_and(|p| p.starts_with("claude"))
    });
    assert!(claude_json, "no .json file under fixtures/claude/");

    for file in &files {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        let name = file.display();
        assert!(!text.contains("Home Office"), "{name} holds the user name");
        assert!(!text.contains("Home-Office"), "{name} holds the user name slug");
        let emails = stray_emails(&text);
        assert!(emails.is_empty(), "{name} holds e-mail addresses: {emails:?}");
        for id in uuids(&text) {
            assert!(id.starts_with(UUID_PREFIX), "{name} holds an unscrubbed id: {id}");
        }
    }
}
