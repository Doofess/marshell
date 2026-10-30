//! A repo's `marshell.json`: three scripts (`setup`, `run`, `teardown`) that Marshell runs only after
//! the person has allowed that repo's exact file. Trust is the pair (main repo root, SHA-256 of the
//! file's bytes) and lives in `<home>/scripts-trust.json`.

use crate::config::hash::hash_bytes;
use crate::profiles::detect;
use crate::projects;
use crate::store::{self, Versioned};
use marshell_protocol::api::RepoScripts;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const FILE_NAME: &str = "marshell.json";
/// The most of the file that is read.
pub const MAX_BYTES: u64 = 64 * 1024;

/// A script is one string, or one string per OS.
#[derive(Deserialize)]
#[serde(untagged)]
enum ScriptValue {
    Text(String),
    PerOs {
        windows: Option<String>,
        default: Option<String>,
    },
}

/// The file's three scripts. Unknown fields are ignored.
#[derive(Deserialize)]
struct ScriptFile {
    setup: Option<ScriptValue>,
    run: Option<ScriptValue>,
    teardown: Option<ScriptValue>,
}

/// The script for this OS: on Windows `windows`, or `default` when that is missing; elsewhere
/// `default`. A blank script, or none for this OS, is absent.
fn resolve(value: &ScriptValue, windows: bool) -> Option<String> {
    let text = match value {
        ScriptValue::Text(text) => Some(text),
        ScriptValue::PerOs { windows: win, default } => {
            if windows {
                win.as_ref().or(default.as_ref())
            } else {
                default.as_ref()
            }
        }
    }?;
    (!text.trim().is_empty()).then(|| text.clone())
}

/// The nearest folder at or above `dir` that holds a `.git` file or folder.
pub fn checkout_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors().find(|d| d.join(".git").exists()).map(Path::to_path_buf)
}

/// The file's bytes with CRLF line endings as LF. Git's autocrlf checks a worktree's copy out with CRLF while the main
/// checkout's may have LF, and a line ending can't change a script: a newline inside a JSON string is always escaped.
fn lf_endings(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for (i, &b) in bytes.iter().enumerate() {
        if !(b == b'\r' && bytes.get(i + 1) == Some(&b'\n')) {
            out.push(b);
        }
    }
    out
}

/// Everything `source` holds, or None when that is more than `MAX_BYTES`. It never reads past the limit, so a file
/// that keeps growing (or a device that never ends) can't hold the read.
fn read_limited(source: impl std::io::Read) -> std::io::Result<Option<Vec<u8>>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    source.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= MAX_BYTES).then_some(bytes))
}

/// The first character in `script` that can hide what it does on the card: a control character other than a tab or
/// a line break, a direction mark or override, or a zero-width character.
fn hidden_character(script: &str) -> Option<char> {
    script.chars().find(|&c| {
        (c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
            || matches!(c, '\u{061C}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2069}' | '\u{FEFF}')
    })
}

fn problem(why: impl std::fmt::Display) -> String {
    format!("{FILE_NAME} can't be read: {why}")
}

/// Reads, resolves and hashes the `marshell.json` of the checkout holding `dir`, and says whether
/// the repo's scripts are allowed.
pub fn read(dir: &Path, home: &Path) -> RepoScripts {
    let root = checkout_root(dir).unwrap_or_else(|| dir.to_path_buf());
    let file_path = root.join(FILE_NAME);
    let mut out = RepoScripts {
        root: root.to_string_lossy().into_owned(),
        file: file_path.to_string_lossy().into_owned(),
        hash: None,
        setup: None,
        run: None,
        teardown: None,
        trusted: false,
        problem: None,
    };
    // Checked before the open: opening a pipe would wait for a writer.
    match std::fs::metadata(&file_path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
        Err(e) => {
            out.problem = Some(problem(e));
            return out;
        }
        Ok(meta) if !meta.is_file() => {
            out.problem = Some(problem("it isn't a file."));
            return out;
        }
        Ok(_) => {}
    }
    let bytes = match std::fs::File::open(&file_path).and_then(read_limited) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            out.problem = Some(problem("it is larger than 64 KiB."));
            return out;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
        Err(e) => {
            out.problem = Some(problem(e));
            return out;
        }
    };
    // Windows PowerShell 5.1 writes UTF-8 with a byte order mark, which JSON doesn't allow and which changes no script.
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let hash = hash_bytes(&lf_endings(bytes));
    match serde_json::from_slice::<ScriptFile>(bytes) {
        Ok(file) => {
            let windows = cfg!(windows);
            let pick = |v: &Option<ScriptValue>| v.as_ref().and_then(|v| resolve(v, windows));
            let scripts = [
                ("setup", pick(&file.setup)),
                ("run", pick(&file.run)),
                ("teardown", pick(&file.teardown)),
            ];
            let hidden = scripts
                .iter()
                .find_map(|(name, script)| script.as_deref().and_then(hidden_character).map(|c| (name, c)));
            if let Some((name, c)) = hidden {
                out.problem = Some(problem(format_args!(
                    "its {name} script has a hidden character (U+{:04X}).",
                    c as u32
                )));
            } else {
                let [(_, setup), (_, run), (_, teardown)] = scripts;
                (out.setup, out.run, out.teardown) = (setup, run, teardown);
                // A linked worktree is trusted under the main checkout, so one Allow covers a branch's copy of the same file.
                let repo = projects::repo_root(&root).unwrap_or(root);
                out.trusted = is_trusted(home, &repo, &hash);
            }
        }
        Err(e) => out.problem = Some(problem(e)),
    }
    out.hash = Some(hash);
    out
}

#[derive(Serialize, Deserialize)]
struct Entry {
    root: String,
    hash: String,
}

#[derive(Serialize, Deserialize, Default)]
struct TrustDoc {
    entries: Vec<Entry>,
}

impl Versioned for TrustDoc {
    const VERSION: u32 = 1;
}

/// `allow` reads, changes and writes the file; two at once would lose an entry.
static TRUST_LOCK: Mutex<()> = Mutex::new(());

fn trust_file(home: &Path) -> PathBuf {
    home.join("scripts-trust.json")
}

/// Allows the repo's scripts as they are now. A newer hash for the same root replaces the old entry.
/// Refuses when the trust file can't be read, so a file the person may want to repair is never overwritten.
pub fn allow(home: &Path, repo_root: &Path, hash: &str) -> anyhow::Result<()> {
    let _guard = TRUST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = trust_file(home);
    let mut doc = store::read::<TrustDoc>(&path)?.unwrap_or_default();
    let root = repo_root.to_string_lossy().into_owned();
    doc.entries.retain(|e| !projects::same_path(&e.root, &root));
    doc.entries.push(Entry {
        root,
        hash: hash.to_string(),
    });
    store::write(&path, &doc)
}

/// Whether this exact file was allowed for this repo.
pub fn is_trusted(home: &Path, repo_root: &Path, hash: &str) -> bool {
    let root = repo_root.to_string_lossy();
    store::read::<TrustDoc>(&trust_file(home))
        .ok()
        .flatten()
        .is_some_and(|doc| {
            doc.entries
                .iter()
                .any(|e| e.hash == hash && projects::same_path(&e.root, &root))
        })
}

/// The command line that runs `script` in `shell_program`. The script is one argument and is never
/// pasted into a command string.
pub fn argv(shell_program: &str, script: &str) -> Vec<String> {
    let flags: &[&str] = if cfg!(windows) {
        &["-NoLogo", "-Command"]
    } else {
        &["-lc"]
    };
    std::iter::once(shell_program)
        .chain(flags.iter().copied())
        .chain(std::iter::once(script))
        .map(str::to_string)
        .collect()
}

/// The program of the default profile's shell, or `/bin/sh` when there is none.
pub fn default_shell() -> String {
    detect::default_shell()
        .into_iter()
        .next()
        .unwrap_or_else(|| "/bin/sh".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_with(file: Option<&[u8]>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        if let Some(bytes) = file {
            std::fs::write(dir.path().join(FILE_NAME), bytes).unwrap();
        }
        dir
    }

    fn value(json: &str) -> ScriptValue {
        serde_json::from_str(json).unwrap()
    }

    const SAMPLE: &str = r#"{"setup":"pnpm i","run":{"windows":"pnpm dev:win","default":"pnpm dev"},"teardown":{"default":"rm -rf .cache"}}"#;

    #[test]
    fn reads_strings_and_per_os_values() {
        let repo = repo_with(Some(SAMPLE.as_bytes()));
        let home = tempfile::tempdir().unwrap();
        let got = read(repo.path(), home.path());
        assert_eq!(got.problem, None);
        assert_eq!(got.setup.as_deref(), Some("pnpm i"));
        if cfg!(windows) {
            assert_eq!(got.run.as_deref(), Some("pnpm dev:win"));
        } else {
            assert_eq!(got.run.as_deref(), Some("pnpm dev"));
        }
        assert_eq!(got.teardown.as_deref(), Some("rm -rf .cache"));
        assert_eq!(got.hash, Some(hash_bytes(SAMPLE.as_bytes())));
        assert!(!got.trusted);

        let run = value(r#"{"windows":"pnpm dev:win","default":"pnpm dev"}"#);
        assert_eq!(resolve(&run, true).as_deref(), Some("pnpm dev:win"));
        assert_eq!(resolve(&run, false).as_deref(), Some("pnpm dev"));
        // Windows falls back to `default`; a plain string is for every OS.
        let teardown = value(r#"{"default":"rm -rf .cache"}"#);
        assert_eq!(resolve(&teardown, true).as_deref(), Some("rm -rf .cache"));
        assert_eq!(resolve(&value(r#""pnpm i""#), false).as_deref(), Some("pnpm i"));
    }

    #[test]
    fn a_script_with_no_value_for_this_os_is_absent() {
        assert_eq!(resolve(&value(r#"{"windows":"x"}"#), false), None);
        assert_eq!(resolve(&value(r#""   ""#), false), None);
        assert_eq!(resolve(&value(r#""""#), true), None);
        assert_eq!(resolve(&value(r#"{"windows":" ","default":"y"}"#), true), None);
    }

    #[test]
    fn no_file_is_no_scripts_and_no_problem() {
        let repo = repo_with(None);
        let home = tempfile::tempdir().unwrap();
        let got = read(repo.path(), home.path());
        assert_eq!((got.hash, got.problem), (None, None));
        assert_eq!((got.setup, got.run, got.teardown), (None, None, None));
        assert!(!got.trusted);
    }

    #[test]
    fn a_broken_file_names_its_problem() {
        let home = tempfile::tempdir().unwrap();
        for bytes in [&b"{ nope"[..], br#"{"setup": 3}"#, b"[]"] {
            let repo = repo_with(Some(bytes));
            let got = read(repo.path(), home.path());
            let problem = got.problem.expect("a problem");
            assert!(problem.starts_with("marshell.json can't be read:"), "{problem}");
            assert_eq!((got.setup, got.run, got.teardown), (None, None, None));
            assert!(!got.trusted);
        }
        let big = vec![b' '; MAX_BYTES as usize + 1];
        let repo = repo_with(Some(&big));
        let got = read(repo.path(), home.path());
        assert_eq!(
            got.problem.as_deref(),
            Some("marshell.json can't be read: it is larger than 64 KiB.")
        );
        assert_eq!(got.hash, None);
        assert_eq!((got.setup, got.run, got.teardown), (None, None, None));
    }

    /// Windows PowerShell 5.1's `Set-Content -Encoding UTF8` starts the file with a byte order mark.
    #[test]
    fn a_byte_order_mark_is_skipped() {
        let home = tempfile::tempdir().unwrap();
        let bom = [b"\xEF\xBB\xBF", SAMPLE.as_bytes()].concat();
        let got = read(repo_with(Some(&bom)).path(), home.path());
        assert_eq!(got.problem, None);
        assert_eq!(got.setup.as_deref(), Some("pnpm i"));
        assert_eq!(
            got.hash,
            read(repo_with(Some(SAMPLE.as_bytes())).path(), home.path()).hash
        );
    }

    #[test]
    fn the_read_stops_after_the_limit() {
        // A source that never ends, as /dev/zero or a file still growing would be.
        assert_eq!(read_limited(std::io::repeat(b' ')).unwrap(), None);
        let small = read_limited(&b"{}"[..]).unwrap();
        assert_eq!(small.as_deref(), Some(&b"{}"[..]));
    }

    #[test]
    fn a_folder_is_not_a_file() {
        let home = tempfile::tempdir().unwrap();
        let repo = repo_with(None);
        std::fs::create_dir(repo.path().join(FILE_NAME)).unwrap();
        let got = read(repo.path(), home.path());
        assert_eq!(
            got.problem.as_deref(),
            Some("marshell.json can't be read: it isn't a file.")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_device_is_not_read() {
        let home = tempfile::tempdir().unwrap();
        let repo = repo_with(None);
        std::os::unix::fs::symlink("/dev/zero", repo.path().join(FILE_NAME)).unwrap();
        let got = read(repo.path(), home.path());
        assert_eq!(
            got.problem.as_deref(),
            Some("marshell.json can't be read: it isn't a file.")
        );
    }

    /// Direction marks and zero-width characters can make the card show a command other than the one that runs.
    #[test]
    fn a_hidden_character_is_a_problem() {
        let home = tempfile::tempdir().unwrap();
        for (ch, code) in [
            ('\u{202E}', "U+202E"),
            ('\u{2066}', "U+2066"),
            ('\u{200B}', "U+200B"),
            ('\u{1B}', "U+001B"),
        ] {
            let body = serde_json::json!({ "setup": format!("echo hi{ch}; rm x") }).to_string();
            let got = read(repo_with(Some(body.as_bytes())).path(), home.path());
            assert_eq!(
                got.problem,
                Some(format!(
                    "marshell.json can't be read: its setup script has a hidden character ({code})."
                )),
            );
            assert_eq!((got.setup, got.run, got.teardown), (None, None, None));
        }
        // Tabs and newlines are part of ordinary scripts.
        let body = serde_json::json!({ "run": "a\tb\nc\r\nd" }).to_string();
        let got = read(repo_with(Some(body.as_bytes())).path(), home.path());
        assert_eq!(got.problem, None);
        assert_eq!(got.run.as_deref(), Some("a\tb\nc\r\nd"));
    }

    #[test]
    fn checkout_root_stops_at_a_git_file_or_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("a");
        let file = dir.path().join("b");
        std::fs::create_dir_all(folder.join(".git")).unwrap();
        std::fs::create_dir_all(&file).unwrap();
        std::fs::write(file.join(".git"), "gitdir: elsewhere").unwrap();
        for root in [&folder, &file] {
            let deep = root.join("x").join("y");
            std::fs::create_dir_all(&deep).unwrap();
            assert_eq!(checkout_root(&deep).as_deref(), Some(root.as_path()));
            assert_eq!(checkout_root(root).as_deref(), Some(root.as_path()));
        }
    }

    #[test]
    fn trust_is_per_repo_and_hash() {
        let home = tempfile::tempdir().unwrap();
        let (a, b) = (Path::new("/repos/a"), Path::new("/repos/b"));
        assert!(!is_trusted(home.path(), a, "h1"));
        allow(home.path(), a, "h1").unwrap();
        assert!(is_trusted(home.path(), a, "h1"));
        assert!(!is_trusted(home.path(), a, "h2"));
        assert!(!is_trusted(home.path(), b, "h1"));
        allow(home.path(), a, "h2").unwrap();
        assert!(is_trusted(home.path(), a, "h2"));
        assert!(!is_trusted(home.path(), a, "h1"));
        assert!(!is_trusted(home.path(), b, "h2"));
        if cfg!(windows) {
            assert!(is_trusted(home.path(), Path::new("/REPOS/A"), "h2"));
        }
    }

    /// Git's autocrlf checks a worktree's copy out with CRLF while the main checkout's file may have LF; the same
    /// scripts are the same file.
    #[test]
    fn line_endings_dont_change_the_hash() {
        let home = tempfile::tempdir().unwrap();
        let lf = repo_with(Some(b"{\n  \"setup\": \"pnpm i\"\n}\n"));
        let crlf = repo_with(Some(b"{\r\n  \"setup\": \"pnpm i\"\r\n}\r\n"));
        let (a, b) = (read(lf.path(), home.path()), read(crlf.path(), home.path()));
        assert_eq!(a.hash, b.hash);
        let other = repo_with(Some(b"{\n  \"setup\": \"pnpm ci\"\n}\n"));
        assert_ne!(a.hash, read(other.path(), home.path()).hash);
    }

    #[test]
    fn read_reports_trust_under_the_main_repo() {
        let dir = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let main = dir.path().join("main");
        let work = dir.path().join("work");
        let admin = main.join(".git").join("worktrees").join("work");
        std::fs::create_dir_all(&admin).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(admin.join("commondir"), "../..").unwrap();
        std::fs::write(work.join(".git"), format!("gitdir: {}", admin.display())).unwrap();
        std::fs::write(main.join(FILE_NAME), SAMPLE).unwrap();
        std::fs::write(work.join(FILE_NAME), SAMPLE).unwrap();

        let main_scripts = read(&main, home.path());
        let main_root = projects::repo_root(&main).unwrap();
        allow(home.path(), &main_root, main_scripts.hash.as_deref().unwrap()).unwrap();
        assert!(read(&main, home.path()).trusted);
        let got = read(&work, home.path());
        assert!(got.trusted);
        assert_eq!(PathBuf::from(&got.root), work);

        std::fs::write(work.join(FILE_NAME), SAMPLE.replace("pnpm i", "pnpm x")).unwrap();
        assert!(!read(&work, home.path()).trusted);
    }

    #[test]
    fn argv_keeps_the_script_as_one_argument() {
        let script = "echo \"a & b\"; echo 'c'\necho d";
        if cfg!(windows) {
            assert_eq!(argv("pwsh.exe", script), ["pwsh.exe", "-NoLogo", "-Command", script]);
        } else {
            assert_eq!(argv("/bin/sh", script), ["/bin/sh", "-lc", script]);
        }
    }
}
