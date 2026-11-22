//! POST /v1/sessions/{tab}/search: plain text in the session's project files.
//!
//! The text comes from the webview and goes to a program, so it is never put where git reads options:
//! it follows `-e`, and `--` ends the options after it. `-F` makes it plain text, not a pattern. A
//! search runs in `spawn_blocking` with a cancel flag a drop guard sets, so a request the app aborts
//! (a newer search) kills its `git grep` at once, and a 3,000 ms deadline kills one that is slow.
//! Nothing keeps more than 4,096 bytes of a line.

use super::AppState;
use super::files::{
    Root, SKIPPED_DIRS, links_refused, open_regular, project_root, row_of, strip_verbatim, under_plain_folders,
};
use crate::git::{self, Stopped};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{SearchCut, SearchFile, SearchLine, SearchPart, SearchReply, SearchRequest};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The search stops at this many matching lines.
const MAX_LINES: usize = 1_000;
/// The search stops after this long.
const BUDGET: Duration = Duration::from_millis(3_000);
/// A file over this many bytes gives no lines.
const FILE_CAP: u64 = 1_048_576;
/// A file with a NUL byte in its first bytes is binary, as git decides it.
const BINARY_PROBE: usize = 8_000;
/// The most bytes of a line's text a record keeps.
const KEEP: usize = 4_096;
/// A shown line is at most this many characters, before the "…" marks.
const SHOWN: usize = 240;
/// A cut line starts this many characters before its first hit.
const LEAD: usize = 40;
/// The longest path a record takes.
const MAX_PATH: usize = 32 * 1024;

/// The refusal for a text the route won't search, if any.
fn check_query(q: &str) -> Option<&'static str> {
    if !(2..=200).contains(&q.chars().count()) {
        return Some("Search for 2 to 200 characters.");
    }
    if q.contains(['\n', '\r', '\0']) {
        return Some("Search for text on one line.");
    }
    None
}

/// Case matters when the text has a capital letter.
fn is_sensitive(q: &str) -> bool {
    q.chars().any(char::is_uppercase)
}

fn fold(c: char, sensitive: bool) -> char {
    if sensitive {
        return c;
    }
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

/// Where `q` occurs in `line`, as byte ranges on char boundaries, left to right and not overlapping.
/// Characters compare one by one; when case is ignored each is lower-cased first.
fn hits(line: &str, q: &str, sensitive: bool) -> Vec<(usize, usize)> {
    let needle: Vec<char> = q.chars().map(|c| fold(c, sensitive)).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let hay: Vec<(usize, char)> = line.char_indices().map(|(i, c)| (i, fold(c, sensitive))).collect();
    let n = needle.len();
    let mut found = Vec::new();
    let mut i = 0;
    while i + n <= hay.len() {
        if hay[i..i + n].iter().map(|(_, c)| *c).eq(needle.iter().copied()) {
            let end = hay.get(i + n).map_or(line.len(), |(b, _)| *b);
            found.push((hay[i].0, end));
            i += n;
        } else {
            i += 1;
        }
    }
    found
}

/// Whether `q` occurs in `line`, compared as `hits` does but holding nothing, for lines too long to
/// hold a copy of.
fn has_hit(line: &str, q: &str, sensitive: bool) -> bool {
    let mut needle = q.chars().map(|c| fold(c, sensitive));
    let Some(first) = needle.next() else {
        return false;
    };
    let rest: Vec<char> = needle.collect();
    line.char_indices().any(|(i, c)| {
        fold(c, sensitive) == first
            && line[i + c.len_utf8()..]
                .chars()
                .map(|c| fold(c, sensitive))
                .take(rest.len())
                .eq(rest.iter().copied())
    })
}

/// A line as the reply shows it: no leading whitespace, split at every hit, and when it is over 240
/// characters the 240 from 40 before the first hit, with "…" at each cut end. A line without a hit
/// (git and `hits` can disagree about a character) is one part that is not a hit.
fn make_parts(text: &str, q: &str, sensitive: bool) -> Vec<SearchPart> {
    let text = text.trim_start();
    let ranges = hits(text, q, sensitive);
    let starts: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    let n = starts.len();
    let (from, to) = if n > SHOWN {
        let first = ranges.first().map_or(0, |(s, _)| starts.binary_search(s).unwrap_or(0));
        let from = first.saturating_sub(LEAD);
        (from, (from + SHOWN).min(n))
    } else {
        (0, n)
    };
    let begin = starts.get(from).copied().unwrap_or(text.len());
    let end = starts.get(to).copied().unwrap_or(text.len());
    let mut parts = Vec::new();
    let mut cursor = begin;
    for (s, e) in ranges {
        let (s, e) = (s.max(begin), e.min(end));
        if s >= e {
            continue;
        }
        if s > cursor {
            parts.push(SearchPart {
                text: text[cursor..s].to_string(),
                hit: false,
            });
        }
        parts.push(SearchPart {
            text: text[s..e].to_string(),
            hit: true,
        });
        cursor = e;
    }
    if cursor < end || parts.is_empty() {
        parts.push(SearchPart {
            text: text[cursor..end].to_string(),
            hit: false,
        });
    }
    if from > 0 {
        match parts.first_mut() {
            Some(p) if !p.hit => p.text.insert(0, '…'),
            _ => parts.insert(
                0,
                SearchPart {
                    text: "…".into(),
                    hit: false,
                },
            ),
        }
    }
    if to < n {
        match parts.last_mut() {
            Some(p) if !p.hit => p.text.push('…'),
            _ => parts.push(SearchPart {
                text: "…".into(),
                hit: false,
            }),
        }
    }
    parts
}

/// One line of `git grep -n -z` output.
#[derive(Debug)]
struct Record {
    path: String,
    line: u32,
    /// At most `KEEP` bytes of the line, without its line break.
    text: String,
}

#[derive(Clone, Copy)]
enum Field {
    Path,
    Line,
    Text,
}

/// Reads `path\0line\0text\n` records from chunks of any size. After the second NUL everything up to
/// the next `\n` is text, a NUL included (git calls a file binary only by its first 8,000 bytes), and
/// only the first `KEEP` bytes of it are kept, so a huge minified line is never held whole.
struct Parser {
    field: Field,
    path: Vec<u8>,
    line: Vec<u8>,
    text: Vec<u8>,
    /// The record being read has a path or a number that can't be used.
    bad: bool,
}

impl Parser {
    fn new() -> Self {
        Parser {
            field: Field::Path,
            path: Vec::new(),
            line: Vec::new(),
            text: Vec::new(),
            bad: false,
        }
    }

    /// The bytes held for the record being read.
    #[cfg(test)]
    fn held(&self) -> usize {
        self.path.len() + self.line.len() + self.text.len()
    }

    fn finish(&mut self) -> Option<Record> {
        let record = (!self.bad)
            .then(|| {
                let line = std::str::from_utf8(&self.line).ok()?.parse::<u32>().ok()?;
                let mut text = self.text.as_slice();
                if let Some(rest) = text.strip_suffix(b"\r") {
                    text = rest;
                }
                Some(Record {
                    path: String::from_utf8_lossy(&self.path).into_owned(),
                    line,
                    text: String::from_utf8_lossy(text).into_owned(),
                })
            })
            .flatten();
        self.path.clear();
        self.line.clear();
        self.text.clear();
        self.bad = false;
        record
    }

    /// Takes a chunk, calling `on` for every record it completes. False when `on` said to stop; the
    /// rest of the chunk is then dropped.
    fn feed(&mut self, mut chunk: &[u8], on: &mut impl FnMut(Record) -> bool) -> bool {
        while !chunk.is_empty() {
            let stop_at = match self.field {
                Field::Text => b'\n',
                _ => 0,
            };
            let at = chunk.iter().position(|b| *b == stop_at);
            let piece = &chunk[..at.unwrap_or(chunk.len())];
            match self.field {
                Field::Path if self.path.len() + piece.len() > MAX_PATH => self.bad = true,
                Field::Path => self.path.extend_from_slice(piece),
                Field::Line if self.line.len() + piece.len() > 10 => self.bad = true,
                Field::Line => self.line.extend_from_slice(piece),
                Field::Text => {
                    let room = KEEP.saturating_sub(self.text.len());
                    self.text.extend_from_slice(&piece[..piece.len().min(room)]);
                }
            }
            let Some(at) = at else {
                return true;
            };
            chunk = &chunk[at + 1..];
            match self.field {
                Field::Path => self.field = Field::Line,
                Field::Line => self.field = Field::Text,
                Field::Text => {
                    self.field = Field::Path;
                    if let Some(record) = self.finish()
                        && !on(record)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// The matching lines found so far, by file.
struct Found<'a> {
    q: &'a str,
    sensitive: bool,
    files: BTreeMap<String, Vec<SearchLine>>,
    lines: usize,
    cut: Option<SearchCut>,
}

impl<'a> Found<'a> {
    fn new(q: &'a str, sensitive: bool) -> Self {
        Found {
            q,
            sensitive,
            files: BTreeMap::new(),
            lines: 0,
            cut: None,
        }
    }

    /// Adds a line. False when the 1,000-line limit is reached and the line is one too many.
    fn add(&mut self, path: &str, line: u32, text: &str) -> bool {
        if self.lines >= MAX_LINES {
            self.cut = Some(SearchCut::Lines);
            return false;
        }
        self.files.entry(path.to_string()).or_default().push(SearchLine {
            line,
            parts: make_parts(text, self.q, self.sensitive),
        });
        self.lines += 1;
        true
    }

    fn clear(&mut self) {
        self.files.clear();
        self.lines = 0;
        self.cut = None;
    }

    fn reply(self, root: &FsPath, stopped: Stopped) -> SearchReply {
        let cut = match stopped {
            // An answer nobody waits for.
            Stopped::Cancelled => return empty(root),
            Stopped::Done => None,
            Stopped::Asked => self.cut,
            Stopped::Deadline => Some(SearchCut::Time),
        };
        SearchReply {
            root: strip_verbatim(root).unwrap_or_default(),
            files: self
                .files
                .into_iter()
                .map(|(path, lines)| SearchFile { path, lines })
                .collect(),
            cut,
        }
    }
}

fn empty(root: &FsPath) -> SearchReply {
    SearchReply {
        root: strip_verbatim(root).unwrap_or_default(),
        files: Vec::new(),
        cut: None,
    }
}

/// Whether a file git found may give lines: a regular file (not a link) of at most 1,048,576 bytes,
/// under no folder link (git for Windows walks into junctions). `folders` is `under_plain_folders`'s.
fn small_enough(root: &FsPath, rel: &str, folders: &mut HashMap<PathBuf, bool>) -> bool {
    let path = FsPath::new(rel);
    path.components().all(|c| matches!(c, std::path::Component::Normal(_)))
        && under_plain_folders(root, rel, folders)
        && std::fs::symlink_metadata(root.join(path)).is_ok_and(|m| m.is_file() && m.len() <= FILE_CAP)
}

/// `git grep` in a checkout, through `git::stream`. Err when git can't run or fails.
fn grep_git(
    root: &FsPath,
    q: &str,
    deadline: Instant,
    cancel: &AtomicBool,
    found: &mut Found,
) -> anyhow::Result<Stopped> {
    let mut args = vec!["grep", "-I", "-n", "-z", "--no-color", "--untracked", "-F"];
    if !found.sensitive {
        args.push("-i");
    }
    // `-e` makes a text that starts with a dash a pattern, and `--` ends the options.
    args.extend(["-e", q, "--"]);
    let mut parser = Parser::new();
    // Git prints a file's lines together, so one remembered answer per file is enough.
    let mut last: Option<(String, bool)> = None;
    let mut folders = HashMap::new();
    git::stream(root, &args, deadline, cancel, |chunk| {
        parser.feed(chunk, &mut |record| {
            let allowed = match &last {
                Some((path, ok)) if *path == record.path => *ok,
                _ => {
                    let ok = small_enough(root, &record.path, &mut folders);
                    last = Some((record.path.clone(), ok));
                    ok
                }
            };
            !allowed || found.add(&record.path, record.line, &record.text)
        })
    })
}

/// `path` relative to `root` with `/` separators. None when it is not under `root` or not UTF-8.
fn relative(root: &FsPath, path: &FsPath) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = rel.components().map(|c| c.as_os_str().to_str()).collect();
    Some(parts?.join("/"))
}

/// Reads one file of the walk and adds its matching lines. False when the line limit stopped the
/// search.
fn search_file(root: &FsPath, path: &FsPath, found: &mut Found) -> bool {
    let Some((file, meta)) = open_regular(path) else {
        return true;
    };
    if meta.len() > FILE_CAP {
        return true;
    }
    let mut bytes = Vec::new();
    if file.take(FILE_CAP + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > FILE_CAP {
        return true;
    }
    if bytes.iter().take(BINARY_PROBE).any(|b| *b == 0) {
        return true;
    }
    let Some(rel) = relative(root, path) else {
        return true;
    };
    for (i, raw) in bytes.split(|b| *b == b'\n').enumerate() {
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        // The match is looked for in the whole line, as git does, and only 4,096 bytes of it are kept.
        if !has_hit(&String::from_utf8_lossy(raw), found.q, found.sensitive) {
            continue;
        }
        let text = String::from_utf8_lossy(&raw[..raw.len().min(KEEP)]);
        if !found.add(&rel, i as u32 + 1, &text) {
            return false;
        }
    }
    true
}

/// The search outside a checkout: the folders under `root` except the skipped ones, and no folder
/// through a link. A linked file is read only when its target passes `links_refused`.
fn walk(root: &FsPath, deadline: Instant, cancel: &AtomicBool, found: &mut Found) -> Stopped {
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            if cancel.load(Ordering::Relaxed) {
                return Stopped::Cancelled;
            }
            if Instant::now() >= deadline {
                return Stopped::Deadline;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                let skipped = entry.file_name().to_str().is_some_and(|n| SKIPPED_DIRS.contains(&n));
                if !skipped {
                    folders.push(path);
                }
            } else if (kind.is_file()
                || (kind.is_symlink() && !links_refused(&path) && std::fs::metadata(&path).is_ok_and(|m| m.is_file())))
                && !search_file(root, &path, found)
            {
                return Stopped::Asked;
            }
        }
    }
    Stopped::Done
}

/// The search. Blocking. In a checkout it asks `git grep`; when git can't run, or outside one, it
/// walks the folders.
fn run_search(root: &FsPath, is_git: bool, q: &str, cancel: &AtomicBool) -> SearchReply {
    let deadline = Instant::now() + BUDGET;
    let mut found = Found::new(q, is_sensitive(q));
    let stopped = if is_git {
        match grep_git(root, q, deadline, cancel, &mut found) {
            Ok(stopped) => stopped,
            Err(_) => {
                found.clear();
                walk(root, deadline, cancel, &mut found)
            }
        }
    } else {
        walk(root, deadline, cancel, &mut found)
    };
    found.reply(root, stopped)
}

/// Sets the flag when it is dropped. The handler holds one while it waits, so a request the app
/// aborts (the future is dropped) stops the blocking work, which `spawn_blocking` alone would not.
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// POST /v1/sessions/{tab}/search
pub async fn search(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<SearchRequest>) -> Response {
    if let Some(refusal) = check_query(&req.q) {
        return (StatusCode::BAD_REQUEST, refusal).into_response();
    }
    let row = row_of(&st, &tab);
    let session = st.session(&tab);
    let reported = session.and_then(|s| s.current_cwd());
    let cancel = Arc::new(AtomicBool::new(false));
    let _stop = CancelOnDrop(cancel.clone());
    let done = tokio::task::spawn_blocking(move || {
        let found = project_root(row.as_ref(), reported)?;
        let is_git = matches!(found, Root::Checkout(_));
        let root = found.path().to_path_buf();
        Some(run_search(&root, is_git, &req.q, &cancel))
    })
    .await;
    match done {
        Ok(Some(reply)) => Json(reply).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "This session has no folder.").into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AppState, router};
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, header};
    use std::path::{Path as FsPath, PathBuf};
    use tower::ServiceExt;

    const TOKEN: &str = "test-token";
    const PORT: u16 = 5563;

    fn temp() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(tmp.path());
        (tmp, root)
    }

    fn git(root: &FsPath, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write(root: &FsPath, rel: &str, text: impl AsRef<[u8]>) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A temp checkout, and the folder's guard.
    fn repo() -> (tempfile::TempDir, PathBuf) {
        let (tmp, root) = temp();
        git(&root, &["init", "-q"]);
        (tmp, root)
    }

    fn find(root: &FsPath, is_git: bool, q: &str) -> SearchReply {
        run_search(root, is_git, q, &AtomicBool::new(false))
    }

    fn paths(reply: &SearchReply) -> Vec<&str> {
        reply.files.iter().map(|f| f.path.as_str()).collect()
    }

    fn texts(file: &SearchFile) -> Vec<String> {
        file.lines
            .iter()
            .map(|l| l.parts.iter().map(|p| p.text.as_str()).collect::<String>())
            .collect()
    }

    fn numbers(file: &SearchFile) -> Vec<u32> {
        file.lines.iter().map(|l| l.line).collect()
    }

    #[test]
    fn finds_plain_text_with_line_numbers() {
        let (_tmp, root) = repo();
        write(&root, "a.txt", "one\n  two needle here\nthree\nneedle\n");
        write(&root, "sub/b.txt", "nothing\n");
        write(&root, "sub/c.txt", "x\nneedle\n");
        let reply = find(&root, true, "needle");
        assert_eq!(reply.cut, None);
        assert_eq!(paths(&reply), ["a.txt", "sub/c.txt"]);
        assert_eq!(numbers(&reply.files[0]), [2, 4]);
        assert_eq!(texts(&reply.files[0]), ["two needle here", "needle"]);
        assert_eq!(numbers(&reply.files[1]), [2]);
        assert_eq!(reply.root, strip_verbatim(&root).unwrap());
        let parts = &reply.files[0].lines[0].parts;
        assert_eq!(
            parts,
            &[
                SearchPart {
                    text: "two ".into(),
                    hit: false
                },
                SearchPart {
                    text: "needle".into(),
                    hit: true
                },
                SearchPart {
                    text: " here".into(),
                    hit: false
                },
            ]
        );
    }

    #[test]
    fn ignores_case_unless_the_text_has_a_capital() {
        let (_tmp, root) = repo();
        write(&root, "a.txt", "TODO one\ntodo two\nTodo three\n");
        for is_git in [true, false] {
            let lower = find(&root, is_git, "todo");
            assert_eq!(numbers(&lower.files[0]), [1, 2, 3], "git {is_git}");
            let capital = find(&root, is_git, "Todo");
            assert_eq!(numbers(&capital.files[0]), [3], "git {is_git}");
        }
    }

    #[test]
    fn a_regex_character_is_plain_text() {
        let (_tmp, root) = repo();
        write(&root, "a.txt", "a.b\naxb\n[x]\n(a|b)\n");
        for is_git in [true, false] {
            let dot = find(&root, is_git, "a.b");
            assert_eq!(numbers(&dot.files[0]), [1], "git {is_git}");
            let bracket = find(&root, is_git, "[x]");
            assert_eq!(numbers(&bracket.files[0]), [3], "git {is_git}");
            let alt = find(&root, is_git, "(a|b)");
            assert_eq!(numbers(&alt.files[0]), [4], "git {is_git}");
        }
    }

    #[test]
    fn skips_ignored_binary_and_big_files() {
        let (_tmp, root) = repo();
        write(&root, ".gitignore", "dist/\n");
        write(&root, "dist/out.js", "needle\n");
        write(&root, "src/ok.txt", "needle\n");
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0needle".to_vec();
        png.extend_from_slice(&[0u8; 64]);
        write(&root, "logo.png", png);
        let mut big = b"needle\n".to_vec();
        big.resize(1_048_577, b'x');
        write(&root, "big.txt", big);
        let mut edge = b"needle\n".to_vec();
        edge.resize(1_048_576, b'x');
        write(&root, "edge.txt", edge);
        let reply = find(&root, true, "needle");
        assert_eq!(paths(&reply), ["edge.txt", "src/ok.txt"]);
    }

    #[test]
    fn finds_untracked_files_that_are_not_ignored() {
        let (_tmp, root) = repo();
        write(&root, ".gitignore", "*.log\n");
        write(&root, "tracked.txt", "needle\n");
        git(&root, &["add", "tracked.txt"]);
        write(&root, "new.txt", "needle\n");
        write(&root, "x.log", "needle\n");
        let reply = find(&root, true, "needle");
        assert_eq!(paths(&reply), ["new.txt", "tracked.txt"]);
    }

    #[test]
    fn stops_at_1000_lines_and_says_so() {
        let (_tmp, root) = repo();
        let text: String = (0..1_200).map(|i| format!("needle {i}\n")).collect();
        write(&root, "a.txt", text);
        for is_git in [true, false] {
            let reply = find(&root, is_git, "needle");
            let total: usize = reply.files.iter().map(|f| f.lines.len()).sum();
            assert_eq!(total, 1_000, "git {is_git}");
            assert_eq!(reply.cut, Some(SearchCut::Lines), "git {is_git}");
        }
    }

    #[test]
    fn exactly_1000_lines_is_not_a_cut() {
        let (_tmp, root) = repo();
        let text: String = (0..1_000).map(|i| format!("needle {i}\n")).collect();
        write(&root, "a.txt", text);
        for is_git in [true, false] {
            let reply = find(&root, is_git, "needle");
            assert_eq!(reply.files[0].lines.len(), 1_000, "git {is_git}");
            assert_eq!(reply.cut, None, "git {is_git}");
        }
    }

    /// Feeds the parser and collects every record it gives.
    fn parse(chunks: &[&[u8]]) -> Vec<Record> {
        let mut parser = Parser::new();
        let mut records = Vec::new();
        for chunk in chunks {
            parser.feed(chunk, &mut |r| {
                records.push(r);
                true
            });
        }
        records
    }

    #[test]
    fn a_record_parser_takes_a_nul_inside_the_text() {
        let records = parse(&[b"a.txt\x0012\x00x\x00y\n"]);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, "a.txt");
        assert_eq!(records[0].line, 12);
        assert_eq!(records[0].text, "x\0y");
        // The same bytes one at a time, and two records in one chunk.
        let bytes = b"a.txt\x0012\x00x\x00y\nb/c.txt\x003\x00z\r\n";
        let one_by_one: Vec<&[u8]> = bytes.chunks(1).collect();
        let records = parse(&one_by_one);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].text, "x\0y");
        assert_eq!(
            (records[1].path.as_str(), records[1].line, records[1].text.as_str()),
            ("b/c.txt", 3, "z")
        );
        assert_eq!(parse(&[bytes]).len(), 2);
    }

    #[test]
    fn a_record_parser_keeps_4096_bytes_of_a_huge_line() {
        let mut parser = Parser::new();
        let mut records = Vec::new();
        let mut on = |r: Record| {
            records.push(r);
            true
        };
        parser.feed(b"big.txt\x001\x00", &mut on);
        let chunk = vec![b'a'; 1 << 20];
        for _ in 0..8 {
            parser.feed(&chunk, &mut on);
            assert!(parser.held() <= 4_096 + 64, "{}", parser.held());
        }
        parser.feed(b"\nnext.txt\x002\x00ok\n", &mut on);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].text.len(), 4_096);
        assert_eq!(records[1].path, "next.txt");
        assert_eq!(records[1].text, "ok");
    }

    #[test]
    fn a_record_parser_stops_when_asked_and_drops_a_half_record() {
        let mut parser = Parser::new();
        let mut seen = 0;
        let go = parser.feed(b"a\x001\x00x\nb\x002\x00y\nc\x003\x00half", &mut |_| {
            seen += 1;
            false
        });
        assert!(!go);
        assert_eq!(seen, 1);
    }

    #[test]
    fn a_query_starting_with_a_dash_is_text() {
        let (_tmp, root) = repo();
        write(&root, "a.txt", "run it with --help now\nuse -e for more\nplain\n");
        let help = find(&root, true, "--help");
        assert_eq!(paths(&help), ["a.txt"]);
        assert_eq!(numbers(&help.files[0]), [1]);
        let e = find(&root, true, "-e");
        assert_eq!(numbers(&e.files[0]), [2]);
    }

    #[test]
    fn outside_git_walks_and_skips_node_modules() {
        let (_tmp, root) = temp();
        write(&root, "node_modules/p/a.js", "needle\n");
        write(&root, "target/a.txt", "needle\n");
        write(&root, ".git/config", "needle\n");
        write(&root, "src/b.txt", "needle\r\nsecond\r\n");
        write(&root, "src/deep/c.txt", "no\nNEEDLE\n");
        write(&root, "bin.dat", b"needle\0\0\0");
        let mut big = b"needle\n".to_vec();
        big.resize(1_048_577, b'x');
        write(&root, "big.txt", big);
        let reply = find(&root, false, "needle");
        assert_eq!(paths(&reply), ["src/b.txt", "src/deep/c.txt"]);
        assert_eq!(texts(&reply.files[0]), ["needle"]);
        assert_eq!(numbers(&reply.files[1]), [2]);
        assert_eq!(reply.cut, None);
    }

    #[test]
    fn outside_git_a_link_to_a_folder_is_not_entered() {
        let (_tmp, root) = temp();
        let (_other_tmp, other) = temp();
        write(&other, "secret.txt", "needle\n");
        write(&root, "ok.txt", "needle\n");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(&other, root.join("link")).is_ok();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&other, root.join("link")).is_ok();
        if !made {
            return;
        }
        let reply = find(&root, false, "needle");
        assert_eq!(paths(&reply), ["ok.txt"]);
    }

    /// A junction, which git for Windows walks into as if it were a folder.
    #[cfg(windows)]
    fn junction(target: &FsPath, link: &FsPath) {
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    }

    /// Live check 6r-2: `git grep --untracked` went through a junction to the folder above the
    /// checkout, and back into it, and showed lines from the scratch home.
    #[cfg(windows)]
    #[test]
    fn in_a_checkout_a_folder_link_is_not_searched_through() {
        let (_tmp, root) = repo();
        let (_other_tmp, other) = temp();
        write(&other, "secret.txt", "needle\n");
        write(&root, "src/ok.txt", "needle\n");
        junction(&other, &root.join("up"));
        junction(&root.join("src"), &root.join("again"));
        let reply = find(&root, true, "needle");
        assert_eq!(paths(&reply), ["src/ok.txt"]);
    }

    #[test]
    fn long_lines_are_cut_around_the_first_hit() {
        let line = format!("{}needle{}", "a".repeat(2_000), "b".repeat(3_000));
        let parts = make_parts(&line, "needle", true);
        let text: String = parts.iter().map(|p| p.text.as_str()).collect();
        assert!(text.starts_with('…'));
        assert!(text.ends_with('…'));
        // 240 characters, and the "…" at each cut end.
        assert_eq!(text.chars().count(), 242);
        let hit: Vec<&SearchPart> = parts.iter().filter(|p| p.hit).collect();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].text, "needle");
        // The window starts 40 characters before the hit.
        assert_eq!(parts[0].text.chars().count(), 41);
        // A hit at the very end cuts only the start.
        let tail = format!("{}needle", "a".repeat(5_000));
        let parts = make_parts(&tail, "needle", true);
        let text: String = parts.iter().map(|p| p.text.as_str()).collect();
        assert!(text.starts_with('…'));
        assert!(!text.ends_with('…'));
        assert!(text.ends_with("needle"));
        assert!(parts.last().unwrap().hit);
        // A hit near the start cuts only the end.
        let head = format!("needle{}", "b".repeat(5_000));
        let parts = make_parts(&head, "needle", true);
        assert!(parts[0].hit);
        assert!(parts.last().unwrap().text.ends_with('…'));
        assert!(!parts.last().unwrap().hit);
        // A hit exactly at the end of a window keeps "…" out of the hit.
        let edge = format!("{}needle{}", "a".repeat(40), "b".repeat(194));
        let parts = make_parts(&edge, "needle", true);
        assert!(parts.iter().filter(|p| p.hit).all(|p| p.text == "needle"));
    }

    #[test]
    fn a_line_of_240_characters_is_not_cut() {
        let line = format!("{}needle", "a".repeat(234));
        let parts = make_parts(&line, "needle", true);
        let text: String = parts.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(text, line);
    }

    #[test]
    fn parts_split_at_every_hit() {
        let parts = make_parts("a foo b foo", "foo", true);
        let got: Vec<(&str, bool)> = parts.iter().map(|p| (p.text.as_str(), p.hit)).collect();
        assert_eq!(got, [("a ", false), ("foo", true), (" b ", false), ("foo", true)]);
    }

    #[test]
    fn leading_whitespace_is_dropped() {
        let parts = make_parts("\t  foo bar", "foo", true);
        assert_eq!(parts[0].text, "foo");
        assert!(parts[0].hit);
    }

    #[test]
    fn a_line_git_found_without_a_hit_is_one_plain_part() {
        // `git grep -i` and `hits` can disagree on a character; the line still shows.
        let parts = make_parts("straße", "STRASSE", false);
        assert_eq!(
            parts,
            [SearchPart {
                text: "straße".into(),
                hit: false
            }]
        );
    }

    #[test]
    fn hits_compare_char_by_char_on_char_boundaries() {
        assert_eq!(hits("ÀB àb", "àb", false), [(0, 3), (4, 7)]);
        assert_eq!(hits("ÀB àb", "àb", true), [(4, 7)]);
        assert_eq!(hits("aaaa", "aa", true), [(0, 2), (2, 4)]);
        assert_eq!(hits("日本語 日本", "日本", true), [(0, 6), (10, 16)]);
        assert!(hits("abc", "xyz", false).is_empty());
    }

    #[test]
    fn has_hit_agrees_with_hits() {
        for (line, q, sensitive) in [
            ("ÀB àb", "àb", false),
            ("ÀB àb", "àb", true),
            ("aaaa", "aa", true),
            ("a", "aa", true),
            ("abc", "xyz", false),
            ("日本語 日本", "日本", true),
            ("", "ab", false),
        ] {
            assert_eq!(
                has_hit(line, q, sensitive),
                !hits(line, q, sensitive).is_empty(),
                "{line:?} {q:?}"
            );
        }
    }

    #[test]
    fn only_a_capital_makes_the_search_case_sensitive() {
        assert!(!is_sensitive("todo"));
        assert!(is_sensitive("Todo"));
        assert!(is_sensitive("tÀdo"));
        assert!(!is_sensitive("日本語"));
    }

    #[test]
    fn a_huge_line_in_a_file_gives_a_short_record() {
        let (_tmp, root) = repo();
        let mut text = vec![b'x'; 500_000];
        text.extend_from_slice(b"needle");
        text.push(b'\n');
        text.extend_from_slice(b"short needle\n");
        write(&root, "min.js", text);
        let git_reply = find(&root, true, "needle");
        let walk_reply = find(&root, false, "needle");
        for reply in [git_reply, walk_reply] {
            let lines = &reply.files[0].lines;
            assert_eq!(lines.len(), 2);
            for line in lines {
                let len: usize = line.parts.iter().map(|p| p.text.chars().count()).sum();
                assert!(len <= 242, "{len}");
            }
            // The match past 4,096 bytes is not shown as a hit, but the line is.
            assert!(lines[0].parts.iter().all(|p| !p.hit));
            assert_eq!(lines[1].line, 2);
        }
    }

    #[test]
    fn a_flag_set_before_the_search_gives_nothing() {
        let (_tmp, root) = repo();
        write(&root, "a.txt", "needle\n");
        for is_git in [true, false] {
            let cancel = AtomicBool::new(true);
            let reply = run_search(&root, is_git, "needle", &cancel);
            assert!(reply.files.is_empty(), "git {is_git}");
        }
    }

    #[test]
    fn dropping_the_guard_sets_the_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        {
            let _guard = CancelOnDrop(flag.clone());
            assert!(!flag.load(Ordering::Relaxed));
        }
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn a_query_is_checked_before_anything_runs() {
        assert!(check_query("ab").is_none());
        assert!(check_query(&"é".repeat(200)).is_none());
        assert_eq!(check_query("a"), Some("Search for 2 to 200 characters."));
        assert_eq!(check_query(""), Some("Search for 2 to 200 characters."));
        assert_eq!(check_query(&"é".repeat(201)), Some("Search for 2 to 200 characters."));
        for q in ["ab\ncd", "ab\rcd", "ab\0cd", "ab\r\n"] {
            assert_eq!(check_query(q), Some("Search for text on one line."), "{q:?}");
        }
    }

    async fn post(q: &str, tab: &str, bearer: bool) -> (StatusCode, Vec<u8>) {
        let home = tempfile::tempdir().unwrap().keep();
        let st = AppState::new(TOKEN.into(), PORT, Vec::new(), home, Vec::new());
        let body = serde_json::json!({ "q": q });
        let mut req = Request::builder()
            .method("POST")
            .uri(format!("/v1/sessions/{tab}/search"))
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::CONTENT_TYPE, "application/json");
        if bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
        }
        let res = router(st)
            .oneshot(req.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, bytes.to_vec())
    }

    #[tokio::test]
    async fn a_query_under_2_or_over_200_characters_is_refused() {
        for q in ["", "a", &"x".repeat(201)] {
            let (status, body) = post(q, "nope", true).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(String::from_utf8_lossy(&body), "Search for 2 to 200 characters.");
        }
        for q in ["ab\ncd", "ab\rcd", "ab\u{0}cd"] {
            let (status, body) = post(q, "nope", true).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(String::from_utf8_lossy(&body), "Search for text on one line.");
        }
    }

    #[tokio::test]
    async fn a_session_with_no_folder_is_not_found() {
        let (status, body) = post("needle", "nope", true).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(String::from_utf8_lossy(&body), "This session has no folder.");
    }

    #[tokio::test]
    async fn search_is_refused_without_the_bearer() {
        assert_eq!(post("needle", "nope", false).await.0, StatusCode::UNAUTHORIZED);
    }
}
