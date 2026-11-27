//! Library docs: a markdown file with a small frontmatter, read and written by a parser of its own
//! (no YAML crate, decision 37).

use crate::config::hash::hash_bytes;
use marshell_protocol::instructions::{DocScope, LibraryDoc};
use std::path::{Path, PathBuf};

/// CLI ids, in the order every list uses.
pub const CLIS: [&str; 5] = ["claude", "codex", "antigravity", "qwen", "copilot"];

const NAME_ERROR: &str = "This file's name can't name a doc: use letters, digits, - and _.";
const MARKER_TAIL: &str = " marks Marshell's copies, so a doc can't hold it.";
const NOT_UTF8: &str = "This doc isn't UTF-8 text, so Marshell doesn't read it.";

/// Why a doc's body can't be copied: its line `line` (counted in the body, as the page shows it) is one of the lines
/// that mark Marshell's copies, which would end a Codex block early.
pub fn marker_error(line: usize) -> String {
    format!("Line {line}{MARKER_TAIL}")
}

/// Whether `error` is the one `marker_error` makes.
pub fn is_marker_error(error: &str) -> bool {
    error.ends_with(MARKER_TAIL)
}

/// One library doc, read from its file.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub name: String,
    pub scope: DocScope,
    pub path: PathBuf,
    pub description: String,
    pub clis: Vec<&'static str>,
    pub globs: Vec<String>,
    pub enabled: bool,
    pub body: String,
    /// The lines of keys Marshell doesn't know, kept verbatim for a save.
    pub extra: Vec<String>,
    /// The config service's hash of the file's raw bytes.
    pub hash: String,
    pub error: Option<String>,
    /// Decision 53: a second file whose name equals another's without case.
    pub duplicate: bool,
}

impl Doc {
    /// Why the doc's frontmatter can't be read, when it can't (a file that isn't UTF-8 among them): its keys are then
    /// the defaults. A file name that can't name a doc, a second doc of the same name, or a body holding a marker line
    /// leaves the frontmatter read.
    pub fn frontmatter_error(&self) -> Option<&str> {
        self.error
            .as_deref()
            .filter(|e| !self.duplicate && *e != NAME_ERROR && !is_marker_error(e))
    }
}

struct Fm {
    description: String,
    clis: Vec<&'static str>,
    globs: Vec<String>,
    enabled: bool,
    extra: Vec<String>,
}

impl Fm {
    fn defaults() -> Fm {
        Fm {
            description: String::new(),
            clis: CLIS.to_vec(),
            globs: Vec::new(),
            enabled: true,
            extra: Vec::new(),
        }
    }
}

/// CON, PRN, AUX, NUL, COM0 to COM9 and LPT0 to LPT9, in any case: Windows 10 opens the device for `nul.md`.
fn reserved_on_windows(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    matches!(n.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (n.len() == 4 && (n.starts_with("COM") || n.starts_with("LPT")) && n.as_bytes()[3].is_ascii_digit())
}

fn name_chars_ok(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Reads a doc's file. Never fails: a problem sets `error` and leaves the keys at their defaults.
pub fn parse(name: &str, scope: DocScope, path: PathBuf, bytes: &[u8]) -> Doc {
    // Bytes that aren't UTF-8 aren't decoded at all, as the copies aren't (`copy::read_disk`): read lossily, U+FFFD
    // would go into every copy, and a save would replace the file's own bytes. The doc is broken and its body empty.
    let (text, mut error) = match std::str::from_utf8(bytes) {
        Ok(text) => (text, None),
        Err(_) => ("", Some(NOT_UTF8.to_string())),
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut fm = Fm::defaults();
    let mut body_start = 0;
    if lines[0].trim_end() == "---" {
        match lines.iter().skip(1).position(|l| l.trim_end() == "---").map(|i| i + 1) {
            Some(close) => {
                match parse_frontmatter(&lines[1..close]) {
                    Ok(f) => fm = f,
                    Err(e) => error = Some(e),
                }
                body_start = close + 1;
            }
            None => error = Some("The frontmatter has no closing --- line.".to_string()),
        }
    }
    if error.is_none() && !name_chars_ok(name) {
        error = Some(NAME_ERROR.to_string());
    }
    let body = clean_body(&lines[body_start..]);
    // A save from the page refuses such a body; a file written by hand or committed by a teammate is caught here.
    if error.is_none()
        && let Some(n) = super::copy::first_marker(&body)
    {
        error = Some(marker_error(n));
    }
    Doc {
        name: name.to_string(),
        scope,
        path,
        description: fm.description,
        clis: fm.clis,
        globs: fm.globs,
        enabled: fm.enabled,
        body,
        extra: fm.extra,
        hash: hash_bytes(bytes),
        error,
        duplicate: false,
    }
}

fn clean_body(lines: &[&str]) -> String {
    let mut end = lines.len();
    while end > 0 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    if end == 0 {
        return String::new();
    }
    let mut out = lines[..end].join("\n");
    out.push('\n');
    out
}

fn skipped(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

fn continues(line: &str) -> bool {
    line.starts_with([' ', '\t']) || line == "-" || line.starts_with("- ") || skipped(line)
}

fn unreadable(n: usize) -> String {
    format!("Line {n} of the frontmatter can't be read.")
}

/// `lines` are the frontmatter's lines; the first is line 2 of the file.
fn parse_frontmatter(lines: &[&str]) -> Result<Fm, String> {
    let mut fm = Fm::defaults();
    let mut seen: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let n = i + 2;
        if skipped(line) {
            // A comment before the first key is the person's: a save keeps it, after Marshell's keys.
            if !line.trim().is_empty() {
                fm.extra.push(line.to_string());
            }
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < lines.len() && continues(lines[j]) {
            j += 1;
        }
        // Continuation lines with their file line numbers; blanks and comments dropped.
        let cont: Vec<(usize, &str)> = (i + 1..j)
            .filter(|&k| !skipped(lines[k]))
            .map(|k| (k + 2, lines[k]))
            .collect();
        // A line that is no `key:` (git's conflict markers among them), or one of Marshell's keys set again, is a
        // frontmatter nobody can read the person's intent from: the doc is broken, so its copies are left as they are.
        let Some((key, value)) = line.split_once(':').map(|(k, v)| (k, v.trim())) else {
            return Err(unreadable(n));
        };
        if ["description", "enabled", "clis", "globs"].contains(&key) {
            if seen.contains(&key) {
                return Err(unreadable(n));
            }
            seen.push(key);
        }
        match key {
            "description" => {
                if let Some((m, _)) = cont.first() {
                    return Err(unreadable(*m));
                }
                fm.description = scalar(value).ok_or_else(|| unreadable(n))?;
            }
            "enabled" => {
                if let Some((m, _)) = cont.first() {
                    return Err(unreadable(*m));
                }
                fm.enabled = match value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(format!("Line {n}: enabled must be true or false.")),
                };
            }
            "clis" | "globs" => {
                let items = list(key, value, &cont, n)?;
                if key == "globs" {
                    fm.globs = items;
                } else {
                    for item in &items {
                        if !CLIS.contains(&item.as_str()) {
                            return Err(format!(
                                "Line {n}: clis names {item}, which isn't one of claude, codex, antigravity, qwen, copilot."
                            ));
                        }
                    }
                    fm.clis = CLIS.iter().copied().filter(|c| items.iter().any(|x| x == c)).collect();
                }
            }
            _ => {
                // Kept as written: a blank line inside a `|` block is part of its value, and a comment is the person's.
                fm.extra.extend(lines[i..j].iter().map(|l| (*l).to_string()));
            }
        }
        if ["description", "enabled", "clis", "globs"].contains(&key) {
            // Marshell writes its own keys again, but a comment among their lines is the person's and stays.
            fm.extra.extend(
                lines[i + 1..j]
                    .iter()
                    .filter(|l| l.trim().starts_with('#'))
                    .map(|l| (*l).to_string()),
            );
        }
        i = j;
    }
    Ok(fm)
}

fn list(key: &str, value: &str, cont: &[(usize, &str)], n: usize) -> Result<Vec<String>, String> {
    if value.is_empty() {
        let mut items = Vec::new();
        for (m, l) in cont {
            let item = l
                .trim_start()
                .strip_prefix('-')
                .filter(|r| r.is_empty() || r.starts_with([' ', '\t']))
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .ok_or_else(|| unreadable(*m))?;
            items.push(scalar(item).ok_or_else(|| unreadable(*m))?);
        }
        return Ok(items);
    }
    let inner = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| format!("Line {n}: {key} must be a list, such as [claude, codex]."))?;
    if let Some((m, _)) = cont.first() {
        return Err(unreadable(*m));
    }
    let mut items = Vec::new();
    for raw in split_flow(inner) {
        let raw = raw.trim();
        if !raw.is_empty() {
            items.push(scalar(raw).ok_or_else(|| unreadable(n))?);
        }
    }
    Ok(items)
}

/// Splits a flow list's inside at the commas outside quotes.
fn split_flow(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in inner.char_indices() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '\'' || c == '"' => quote = Some(c),
            None if c == ',' => {
                parts.push(&inner[start..i]);
                start = i + 1;
            }
            None => {}
        }
    }
    parts.push(&inner[start..]);
    parts
}

/// A plain, single-quoted or double-quoted scalar; `None` when it is malformed.
fn scalar(v: &str) -> Option<String> {
    let v = v.trim();
    if let Some(rest) = v.strip_prefix('\'') {
        let inner = rest.strip_suffix('\'')?;
        if inner.replace("''", "").contains('\'') {
            return None;
        }
        Some(inner.replace("''", "'"))
    } else if let Some(rest) = v.strip_prefix('"') {
        let inner = rest.strip_suffix('"')?;
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next()? {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    _ => return None,
                },
                '"' => return None,
                c => out.push(c),
            }
        }
        Some(out)
    } else {
        Some(v.to_string())
    }
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The doc file's text, as Marshell writes it: `\n` line ends, no BOM.
pub fn render(doc: &Doc) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("description: {}\n", quote(&doc.description)));
    out.push_str(&format!("clis: [{}]\n", doc.clis.join(", ")));
    let globs: Vec<String> = doc.globs.iter().map(|g| quote(g)).collect();
    out.push_str(&format!("globs: [{}]\n", globs.join(", ")));
    if doc.scope == DocScope::Project {
        out.push_str(&format!("enabled: {}\n", doc.enabled));
    }
    for line in &doc.extra {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("---\n");
    out.push_str(&doc.body);
    out
}

/// A new doc's name: letters, digits, `-` and `_`, at most 64, and not one of `taken` without case.
pub fn check_name(name: &str, taken: &[&str]) -> Result<(), String> {
    if name.is_empty() {
        return Err("A doc needs a name.".into());
    }
    if !name_chars_ok(name) {
        return Err("A name holds letters, digits, - and _ only.".into());
    }
    if name.chars().count() > 64 {
        return Err("A name is at most 64 characters.".into());
    }
    if reserved_on_windows(name) {
        return Err(format!("A name can't be {name}: Windows keeps it for a device."));
    }
    if let Some(t) = taken.iter().find(|t| t.eq_ignore_ascii_case(name)) {
        return Err(format!("Another doc here is named {t}."));
    }
    Ok(())
}

/// Trims the globs, drops empty and repeated ones, and refuses what can't be a project glob.
pub fn check_globs(globs: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for g in globs {
        let g = g.trim();
        if g.is_empty() {
            continue;
        }
        if g.contains(['\n', '\r']) {
            return Err("A glob is one line.".into());
        }
        if g.contains(',') {
            return Err(format!("{g}: a glob can't hold a comma. Write it as two globs."));
        }
        if g.starts_with('/') {
            return Err(format!(
                "{g}: a glob is relative to the project, so it can't start with /."
            ));
        }
        if g.split(['/', '\\']).any(|s| s == "..") {
            return Err(format!("{g}: a glob can't go above the project with .."));
        }
        if !out.iter().any(|o| o == g) {
            out.push(g.to_string());
        }
    }
    Ok(out)
}

pub fn check_description(text: &str) -> Result<(), String> {
    if text.contains(['\n', '\r']) {
        return Err("A description is one line.".into());
    }
    if text.chars().count() > 200 {
        return Err("A description is at most 200 characters.".into());
    }
    Ok(())
}

/// The largest library file Marshell reads, as the Doctor's instruction readers cap theirs.
pub const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Why a library file isn't read: a link could point anywhere (a cloned repo's link to a private file would
/// otherwise be copied into committed rule files), and a file over `MAX_BYTES` is too big to hold.
fn refusal(is_link: bool, is_file: bool, len: u64) -> Option<&'static str> {
    if is_link {
        Some("This doc is a link to another file, so Marshell doesn't read it.")
    } else if !is_file {
        Some("This doc isn't a file, so Marshell doesn't read it.")
    } else if len > MAX_BYTES {
        Some("This doc is over 4 MiB, so Marshell doesn't read it.")
    } else {
        None
    }
}

/// A doc's bytes, or why they aren't read. The link test uses the entry's own metadata, never the target's.
pub(crate) fn read_plain(path: &Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("This doc can't be read: {e}."))?;
    if let Some(why) = refusal(meta.file_type().is_symlink(), meta.is_file(), meta.len()) {
        return Err(why.to_string());
    }
    std::fs::read(path).map_err(|e| format!("This doc can't be read: {e}."))
}

/// The `*.md` files directly in `dir`, sorted by name without case. A missing folder is an empty list. A file that
/// isn't read (a link, over 4 MiB, locked, or not UTF-8) is listed with its error, so its copies are left as they are.
pub fn read_dir(dir: &Path, scope: DocScope) -> Vec<Doc> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut docs: Vec<Doc> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let is_md = path.extension().is_some_and(|x| x.eq_ignore_ascii_case("md"));
            let kind = e.file_type().ok()?;
            if !is_md || kind.is_dir() {
                return None;
            }
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            Some(match read_plain(&path) {
                Ok(bytes) => parse(&stem, scope, path, &bytes),
                Err(why) => {
                    let mut doc = parse(&stem, scope, path, b"");
                    doc.error = Some(why);
                    doc
                }
            })
        })
        .collect();
    docs.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    let mut first: Vec<(String, String)> = Vec::new();
    for d in &mut docs {
        let lower = d.name.to_lowercase();
        match first.iter().find(|(l, _)| *l == lower) {
            Some((_, f)) => {
                d.duplicate = true;
                d.error = Some(format!("Another doc here is named {f}."));
            }
            None => first.push((lower, d.name.clone())),
        }
    }
    docs
}

/// The doc named `name`, equal without case; a `duplicate` doc is never found.
pub fn find<'a>(docs: &'a [Doc], name: &str) -> Option<&'a Doc> {
    docs.iter().find(|d| !d.duplicate && d.name.eq_ignore_ascii_case(name))
}

pub fn to_protocol(doc: &Doc) -> LibraryDoc {
    LibraryDoc {
        name: doc.name.clone(),
        scope: doc.scope,
        path: doc.path.to_string_lossy().into_owned(),
        description: doc.description.clone(),
        clis: doc.clis.iter().map(|c| c.to_string()).collect(),
        globs: doc.globs.clone(),
        enabled: doc.enabled,
        body: doc.body.clone(),
        hash: doc.hash.clone(),
        error: doc.error.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> Doc {
        parse("x", DocScope::Project, PathBuf::from("x.md"), text.as_bytes())
    }

    #[test]
    fn parses_the_four_keys() {
        let d = p(
            "---\ndescription: 'Security rules'\nclis: [claude, codex]\nglobs:\n  - 'src/**'\n  - tests/**\nenabled: false\n---\nBody line\n",
        );
        assert_eq!(d.description, "Security rules");
        assert_eq!(d.clis, vec!["claude", "codex"]);
        assert_eq!(d.globs, vec!["src/**", "tests/**"]);
        assert!(!d.enabled);
        assert_eq!(d.body, "Body line\n");
        assert_eq!(d.error, None);
    }

    #[test]
    fn no_frontmatter_takes_defaults() {
        let d = p("Just text\n");
        assert_eq!(d.clis, CLIS.to_vec());
        assert!(d.globs.is_empty());
        assert!(d.enabled);
        assert_eq!(d.description, "");
        assert_eq!(d.body, "Just text\n");
    }

    #[test]
    fn crlf_and_bom_read_as_lf() {
        let d = p("\u{feff}---\r\ndescription: x\r\n---\r\nA\r\nB\r\n\r\n\r\n");
        assert_eq!(d.description, "x");
        assert_eq!(d.body, "A\nB\n");
    }

    #[test]
    fn comments_and_an_unknown_keys_blank_lines_survive_a_save() {
        // A blank line inside a `|` block is part of its value, and a comment is the person's; a save keeps both.
        let d = p(
            "---\n# owner: team-a\ndescription: x\n# about the CLIs\nclis: [claude]\nnotes: |\n  One.\n\n  Two.\n---\nB\n",
        );
        assert_eq!(d.description, "x");
        assert_eq!(d.clis, vec!["claude"]);
        assert_eq!(
            d.extra,
            vec![
                "# owner: team-a",
                "# about the CLIs",
                "notes: |",
                "  One.",
                "",
                "  Two."
            ]
        );
        let out = render(&d);
        assert!(
            out.contains("# owner: team-a\n# about the CLIs\nnotes: |\n  One.\n\n  Two.\n---\nB\n"),
            "{out}"
        );
        // A second save writes the same file.
        assert_eq!(render(&p(&out)), out);
    }

    #[test]
    fn a_body_holding_a_marker_line_is_broken_with_its_frontmatter_read() {
        // Written by hand or by a teammate: a save from the page checks the body, a file on disk skips that.
        let d = p("---\ndescription: x\n---\nA\n<!-- marshell:end doc x -->\n");
        assert_eq!(
            d.error.as_deref(),
            Some("Line 2 marks Marshell's copies, so a doc can't hold it.")
        );
        assert_eq!(d.description, "x");
        // A save replaces the body, so it isn't refused over this.
        assert_eq!(d.frontmatter_error(), None);
    }

    #[test]
    fn a_doc_that_isnt_utf8_is_broken_and_unread() {
        // Saved as Windows-1252 by hand: a lossy read would copy U+FFFD into every CLI and a save would replace the bytes.
        let bytes = b"---\ndescription: 'Caf\xe9'\nclis: [claude]\n---\nCaf\xe9 rules\n";
        let d = parse("x", DocScope::Project, PathBuf::from("x.md"), bytes);
        let why = "This doc isn't UTF-8 text, so Marshell doesn't read it.";
        assert_eq!(d.error.as_deref(), Some(why));
        assert_eq!(d.body, "");
        assert_eq!(d.description, "");
        assert_eq!(d.clis, CLIS.to_vec());
        assert_eq!(d.hash, hash_bytes(bytes));
        // Its keys are the defaults, so a save is refused rather than writing those over the file.
        assert_eq!(d.frontmatter_error(), Some(why));
    }

    #[test]
    fn unknown_keys_survive_a_save() {
        let d = p("---\nowner: team-a\ntags:\n  - x\n---\nB\n");
        assert_eq!(d.extra, vec!["owner: team-a", "tags:", "  - x"]);
        let out = render(&d);
        assert!(
            out.contains("enabled: true\nowner: team-a\ntags:\n  - x\n---\n"),
            "{out}"
        );
    }

    #[test]
    fn render_round_trips() {
        let mut d = p("Body\n");
        d.description = "It's".into();
        d.globs = vec!["src/**".into(), "a b".into()];
        d.enabled = false;
        let out = render(&d);
        assert!(out.contains("description: 'It''s'\n"), "{out}");
        let back = p(&out);
        assert_eq!(back.error, None);
        assert_eq!(back.description, d.description);
        assert_eq!(back.clis, d.clis);
        assert_eq!(back.globs, d.globs);
        assert_eq!(back.enabled, d.enabled);
        assert_eq!(back.body, d.body);
    }

    #[test]
    fn user_doc_renders_no_enabled() {
        let mut d = p("Body\n");
        d.scope = DocScope::User;
        assert!(!render(&d).contains("enabled:"));
    }

    #[test]
    fn bad_values_name_their_line() {
        let err = |t: &str| p(t).error;
        assert_eq!(
            err("---\nclis: claude\n---\n").as_deref(),
            Some("Line 2: clis must be a list, such as [claude, codex].")
        );
        assert_eq!(
            err("---\nenabled: maybe\n---\n").as_deref(),
            Some("Line 2: enabled must be true or false.")
        );
        assert_eq!(
            err("---\nclis: [claude, gemini]\n---\n").as_deref(),
            Some("Line 2: clis names gemini, which isn't one of claude, codex, antigravity, qwen, copilot.")
        );
        assert_eq!(
            err("---\ndescription: x\n").as_deref(),
            Some("The frontmatter has no closing --- line.")
        );
    }

    #[test]
    fn a_merge_conflict_in_the_frontmatter_is_unreadable() {
        // A teammate's bad merge: the frontmatter still closes, but holds git's markers and the key twice.
        let d = p(
            "---\ndescription: x\n<<<<<<< HEAD\nclis: [claude, codex]\n=======\nclis: [claude]\n>>>>>>> theirs\n---\nB\n",
        );
        assert_eq!(d.error.as_deref(), Some("Line 3 of the frontmatter can't be read."));
        // The same key twice, with no markers left, is as unreadable: which one the person meant isn't known.
        let twice = p("---\nclis: [claude, codex]\nclis: [claude]\n---\nB\n");
        assert_eq!(twice.error.as_deref(), Some("Line 3 of the frontmatter can't be read."));
    }

    #[test]
    fn names() {
        assert_eq!(check_name("security", &[]), Ok(()));
        // Windows 10 opens the device for `nul.md`; a project doc is shared with teammates on any Windows.
        for reserved in ["nul", "CON", "Aux", "prn", "com1", "LPT9", "com0"] {
            assert_eq!(
                check_name(reserved, &[]),
                Err(format!("A name can't be {reserved}: Windows keeps it for a device.")),
            );
        }
        for fine in ["console", "com10", "null", "lpt", "nul_rules"] {
            assert_eq!(check_name(fine, &[]), Ok(()), "{fine}");
        }
        assert_eq!(check_name("", &[]), Err("A doc needs a name.".into()));
        assert_eq!(
            check_name("Sec Ret", &[]),
            Err("A name holds letters, digits, - and _ only.".into())
        );
        assert_eq!(
            check_name(&"a".repeat(65), &[]),
            Err("A name is at most 64 characters.".into())
        );
        assert_eq!(
            check_name("SECURITY", &["security"]),
            Err("Another doc here is named security.".into())
        );
    }

    #[test]
    fn globs() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            check_globs(&v(&["src/**", " ", "src/**", " tests/*.rs "])),
            Ok(v(&["src/**", "tests/*.rs"]))
        );
        assert_eq!(
            check_globs(&v(&["src/{a,b}"])),
            Err("src/{a,b}: a glob can't hold a comma. Write it as two globs.".into())
        );
        assert_eq!(
            check_globs(&v(&["/abs/**"])),
            Err("/abs/**: a glob is relative to the project, so it can't start with /.".into())
        );
        assert_eq!(
            check_globs(&v(&["../x"])),
            Err("../x: a glob can't go above the project with ..".into())
        );
        // The local API can send one; the doc's `globs:` line and each copy's frontmatter would break.
        assert_eq!(check_globs(&v(&["src/a\nb"])), Err("A glob is one line.".into()));
        assert_eq!(check_globs(&v(&["src/a\rb"])), Err("A glob is one line.".into()));
    }

    #[test]
    fn description() {
        assert_eq!(check_description("ok"), Ok(()));
        assert_eq!(check_description("a\nb"), Err("A description is one line.".into()));
        assert_eq!(
            check_description(&"a".repeat(201)),
            Err("A description is at most 200 characters.".into())
        );
    }

    #[test]
    fn read_dir_lists_md_files() {
        let t = tempfile::tempdir().unwrap();
        for f in ["b.md", "A.md", "notes.txt", "bad name.md"] {
            std::fs::write(t.path().join(f), "x\n").unwrap();
        }
        std::fs::create_dir(t.path().join("sub")).unwrap();
        let docs = read_dir(t.path(), DocScope::User);
        let names: Vec<_> = docs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["A", "b", "bad name"]);
        assert_eq!(
            docs[2].error.as_deref(),
            Some("This file's name can't name a doc: use letters, digits, - and _.")
        );
        assert_eq!(docs[0].hash, hash_bytes(b"x\n"));
        assert!(read_dir(&t.path().join("missing"), DocScope::User).is_empty());
    }

    #[test]
    fn refusals() {
        assert_eq!(refusal(false, true, 10), None);
        assert_eq!(
            refusal(true, false, 10),
            Some("This doc is a link to another file, so Marshell doesn't read it.")
        );
        assert_eq!(refusal(false, true, MAX_BYTES), None);
        assert_eq!(
            refusal(false, true, MAX_BYTES + 1),
            Some("This doc is over 4 MiB, so Marshell doesn't read it.")
        );
    }

    #[test]
    fn oversized_doc_is_listed_unread() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("big.md"), vec![b'a'; 4 * 1024 * 1024 + 1]).unwrap();
        std::fs::write(t.path().join("edge.md"), vec![b'a'; 4 * 1024 * 1024]).unwrap();
        let docs = read_dir(t.path(), DocScope::User);
        assert_eq!(docs[0].name, "big");
        assert_eq!(
            docs[0].error.as_deref(),
            Some("This doc is over 4 MiB, so Marshell doesn't read it.")
        );
        assert_eq!(docs[0].body, "");
        assert_eq!(docs[1].error, None);
    }

    #[test]
    fn linked_doc_is_listed_unread() {
        let t = tempfile::tempdir().unwrap();
        let secret = t.path().join("secret.txt");
        std::fs::write(&secret, "private\n").unwrap();
        let dir = t.path().join("docs");
        std::fs::create_dir(&dir).unwrap();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&secret, dir.join("leak.md"));
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(&secret, dir.join("leak.md"));
        if let Err(e) = made {
            eprintln!("skipped: cannot create a symlink here: {e}");
            return;
        }
        let docs = read_dir(&dir, DocScope::Project);
        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].error.as_deref(),
            Some("This doc is a link to another file, so Marshell doesn't read it.")
        );
        assert!(!docs[0].body.contains("private"));
    }

    #[cfg(not(windows))]
    #[test]
    fn case_twins_mark_the_second() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("Sec.md"), "a\n").unwrap();
        std::fs::write(t.path().join("sec.md"), "b\n").unwrap();
        let docs = read_dir(t.path(), DocScope::User);
        assert!(!docs[0].duplicate);
        assert!(docs[1].duplicate);
        assert_eq!(docs[1].error.as_deref(), Some("Another doc here is named Sec."));
        assert_eq!(find(&docs, "SEC").unwrap().body, "a\n");
    }
}
