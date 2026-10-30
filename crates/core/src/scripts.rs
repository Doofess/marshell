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
    match std::fs::metadata(&file_path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
        Err(e) => {
            out.problem = Some(problem(e));
            return out;
        }
        Ok(meta) if meta.len() > MAX_BYTES => {
            out.problem = Some(problem("it is larger than 64 KiB."));
            return out;
        }
        Ok(_) => {}
    }
    let bytes = match std::fs::read(&file_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
        Err(e) => {
            out.problem = Some(problem(e));
            return out;
        }
    };
    let hash = hash_bytes(&bytes);
    match serde_json::from_slice::<ScriptFile>(&bytes) {
        Ok(file) => {
            let windows = cfg!(windows);
            let pick = |v: &Option<ScriptValue>| v.as_ref().and_then(|v| resolve(v, windows));
            out.setup = pick(&file.setup);
            out.run = pick(&file.run);
            out.teardown = pick(&file.teardown);
            // A linked worktree is trusted under the main checkout, so one Allow covers a branch's copy of the same file.
            let repo = projects::repo_root(&root).unwrap_or(root);
            out.trusted = is_trusted(home, &repo, &hash);
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
