//! Git for the Changes panel, run so that nothing the repository controls is read or run (spike S15).
//!
//! A repository's own config can run a program (`core.fsmonitor`, a `filter.*.clean`, `diff.external`) and
//! can include a file from a network path, and no `-c` flag switches off every such setting. So git runs
//! against a git folder of Marshell's own, one per checkout, under Marshell's home: its `HEAD` and `config`
//! are written here, and the repository contributes only a copy of its index, its shallow file and its
//! `info/attributes`, plus its object folder by path. The few settings that change how a file compares
//! come from `config::ALLOWED`. Git is never given a file it could treat as config but this one.
//!
//! The index is a copy because git may refresh and rewrite the index it reads; the repository's own index
//! is never written.

pub mod config;
pub mod refs;
pub mod sweep;
pub mod worktree;

pub use crate::git::Stopped;
use crate::git::stream_command;
use crate::server::{common_dir, git_dir, links_refused, read_capped, refused};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

const NETWORK: &str = "This repository reads its objects from a network folder, so Changes doesn't read it.";
const NEEDS_NEWER_GIT: &str = "Changes needs git 2.32 or later.";
const REFTABLE: &str = "This repository stores its branches in a format Changes can't read yet.";
const NEWER_FORMAT: &str = "This repository uses a git format Changes can't read yet.";
const TOO_LARGE: &str = "This repository's history file is too large for Changes.";

/// The largest `shallow` or `info/attributes` file that is copied, and the largest `alternates` file read.
const SMALL_FILE_CAP: u64 = 1024 * 1024;
/// How many `alternates` files deep git follows (its own limit).
const ALTERNATES_DEPTH: usize = 5;
/// The most `alternates` files read in one check, so a tangle of them can't keep it busy.
const ALTERNATES_BUDGET: usize = 256;
/// How long the system and user config files are trusted before they are read again.
const CONFIG_MAX_AGE: Duration = Duration::from_secs(60);
/// How long one `git config` read may take.
const CONFIG_DEADLINE: Duration = Duration::from_secs(10);
/// The most output of one `git config` read that is kept.
const CONFIG_MAX_BYTES: usize = 4 * 1024 * 1024;

/// The commands that look into a submodule's work tree unless told not to; the box always tells them.
const SUBMODULE_BLIND: &[&str] = &["diff", "diff-files", "diff-index", "status"];

const EMPTY_TREE_SHA1: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const EMPTY_TREE_SHA256: &str = "6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321";

/// Why a checkout can't be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoxError {
    /// There is no git folder.
    NoRepo,
    /// Reading it would reach a network folder or a device.
    Refused(&'static str),
    /// A kind of repository or a git that Changes can't read, in the sentence the panel shows.
    Unsupported(&'static str),
    /// Git or the filesystem failed; the text is git's own sentence where it gave one.
    Git(String),
}

impl std::fmt::Display for BoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoxError::NoRepo => f.write_str("There is no git repository here."),
            BoxError::Refused(s) | BoxError::Unsupported(s) => f.write_str(s),
            BoxError::Git(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for BoxError {}

/// What a file looked like when it was last copied or read: its length and modified time.
type FileStamp = (u64, Option<SystemTime>);

/// What `GitBox` last synced. `None` means "not yet"; `Some(None)` means "the file was missing".
#[derive(Default)]
struct Stamps {
    config: Option<(Option<FileStamp>, Option<FileStamp>)>,
    config_read: Option<Instant>,
    index: Option<Option<FileStamp>>,
    shallow: Option<Option<FileStamp>>,
    attributes: Option<Option<FileStamp>>,
}

/// What the config says about where `origin` points, as written and in the order it was read (the system
/// file, the user's, the repository's, then the worktree's). Only read: `GitBox` never passes any of it on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Origin {
    /// `remote.origin.url`, every value.
    pub urls: Vec<String>,
    /// `remote.origin.pushurl`, every value.
    pub push_urls: Vec<String>,
    /// `url.<base>.insteadOf` as `(base, value)`.
    pub instead_of: Vec<(String, String)>,
    /// `url.<base>.pushInsteadOf` as `(base, value)`.
    pub push_instead_of: Vec<(String, String)>,
}

impl Origin {
    /// Takes in the entries of one file's read that name origin or rewrite an address.
    fn take(&mut self, entries: &[(String, String)]) {
        for (key, value) in entries {
            // Git prints the section and variable in lowercase and the remote's name as written, so only
            // `origin` itself matches.
            match key.as_str() {
                "remote.origin.url" => self.urls.push(value.clone()),
                "remote.origin.pushurl" => self.push_urls.push(value.clone()),
                _ => match config::rewrite_entry(key, value) {
                    Some((config::Rewrite::Instead, base, value)) => self.instead_of.push((base, value)),
                    Some((config::Rewrite::Push, base, value)) => self.push_instead_of.push((base, value)),
                    None => {}
                },
            }
        }
    }
}

/// One checkout's git, in a sandbox.
pub struct GitBox {
    checkout: PathBuf,
    git_dir: PathBuf,
    common: PathBuf,
    sandbox: PathBuf,
    /// `empty-config`, next to the sandboxes.
    empty: PathBuf,
    /// The allowed settings read from the system, user and repository files, later files last.
    pairs: Vec<(String, String)>,
    /// Where `origin` points, from the same files. Read only: none of it is ever passed on to git.
    origin: Origin,
    sha256: bool,
    stamps: Stamps,
    #[cfg(test)]
    runs: std::sync::atomic::AtomicUsize,
}

/// The names of the inherited variables git reads: every `GIT_*` one. Windows names have no case.
pub fn git_vars(vars: impl Iterator<Item = (OsString, OsString)>) -> Vec<OsString> {
    vars.map(|(name, _)| name)
        .filter(|name| {
            let name = name.to_string_lossy();
            name.get(..4).is_some_and(|p| {
                if cfg!(windows) {
                    p.eq_ignore_ascii_case("GIT_")
                } else {
                    p == "GIT_"
                }
            })
        })
        .collect()
}

/// Removes every inherited `GIT_*` variable from the child's environment.
fn scrub(cmd: &mut Command) {
    for name in git_vars(std::env::vars_os()) {
        cmd.env_remove(name);
    }
}

/// No console window flashes for a git run (as `git::command` does).
fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// FNV-1a, 64 bits. Written out because `DefaultHasher` isn't stable across Rust releases and the
/// sandbox folder must keep its name.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// The sandbox's folder name: 16 hex digits of the hash of the checkout's path (lowercased on Windows).
fn sandbox_name(checkout: &Path) -> String {
    let text = checkout.to_string_lossy().into_owned();
    let text = if cfg!(windows) { text.to_lowercase() } else { text };
    format!("{:016x}", fnv1a64(text.as_bytes()))
}

/// The folder of `checkout`'s sandbox: `git-sandbox` under `home`, made absolute, and the name of the
/// checkout's absolute path in it.
pub fn sandbox_dir(home: &Path, checkout: &Path) -> PathBuf {
    let sandboxes = crate::paths::git_sandbox_dir(home);
    let sandboxes = std::path::absolute(&sandboxes).unwrap_or(sandboxes);
    let checkout = std::path::absolute(checkout).unwrap_or_else(|_| checkout.to_path_buf());
    sandboxes.join(sandbox_name(&checkout))
}

fn stamp_of(meta: &std::fs::Metadata) -> FileStamp {
    (meta.len(), meta.modified().ok())
}

/// A file's stamp, or None when it is missing or isn't a regular file.
fn stamp(path: &Path) -> Option<FileStamp> {
    std::fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| stamp_of(&m))
}

fn io_error(what: &str, e: std::io::Error) -> BoxError {
    BoxError::Git(format!("Couldn't {what}: {e}."))
}

/// Writes `bytes` to a temp file in `dir`, gives it `modified` when asked, and renames it over `name`.
fn write_file(dir: &Path, name: &str, bytes: &[u8], modified: Option<SystemTime>) -> std::io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{name}.{}-{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        if let Some(time) = modified {
            file.set_modified(time)?;
        }
        drop(file);
        std::fs::rename(&tmp, dir.join(name))
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// `write_file`, skipped when the file already says exactly this.
fn write_if_different(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    if std::fs::read(dir.join(name)).is_ok_and(|have| have == bytes) {
        return Ok(());
    }
    write_file(dir, name, bytes, None)
}

fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The hex id of the shared index a split index names, read from its `link` extension. The entries are
/// walked to find where the extensions start; a file this can't make sense of gives None.
fn shared_index_id(bytes: &[u8], hash_len: usize) -> Option<String> {
    let be32 = |at: usize| -> Option<usize> {
        let b = bytes.get(at..at.checked_add(4)?)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    if bytes.get(..4)? != b"DIRC" {
        return None;
    }
    let version = be32(4)?;
    if !(2..=4).contains(&version) {
        return None;
    }
    let count = be32(8)?;
    let end = bytes.len().checked_sub(hash_len)?;
    let fixed = 40 + hash_len + 2;
    let mut pos = 12;
    for _ in 0..count {
        let entry = pos;
        let flags = u16::from_be_bytes([*bytes.get(entry + fixed - 2)?, *bytes.get(entry + fixed - 1)?]);
        let mut at = entry + fixed;
        if version >= 3 && flags & 0x4000 != 0 {
            at += 2;
        }
        if version == 4 {
            // A varint (how much of the previous name to cut), then the rest of the name and a NUL.
            while bytes.get(at)? & 0x80 != 0 {
                at += 1;
            }
            at += 1;
            at += bytes.get(at..end)?.iter().position(|b| *b == 0)? + 1;
            pos = at;
        } else {
            let mut name_len = usize::from(flags & 0x0fff);
            if name_len == 0x0fff {
                name_len = bytes.get(at..end)?.iter().position(|b| *b == 0)?;
            }
            // The entry is padded with 1 to 8 NULs to a multiple of 8 bytes.
            pos = entry + ((at - entry + name_len + 8) & !7);
        }
        if pos > end {
            return None;
        }
    }
    while pos + 8 <= end {
        let size = be32(pos + 4)?;
        let data = pos + 8;
        if &bytes[pos..pos + 4] == b"link" {
            let id = bytes.get(data..data.checked_add(hash_len)?)?;
            if id.iter().all(|b| *b == 0) {
                return None;
            }
            return Some(id.iter().map(|b| format!("{b:02x}")).collect());
        }
        pos = data.checked_add(size)?;
    }
    None
}

/// The git version, read once per process; an error when git can't be run or is older than 2.32, the
/// release that brought `GIT_CONFIG_GLOBAL`.
fn check_version() -> Result<(), BoxError> {
    static VERSION: OnceLock<(u32, u32)> = OnceLock::new();
    let version = match VERSION.get() {
        Some(v) => *v,
        None => {
            let out =
                crate::git::run(&std::env::temp_dir(), &["--version"]).map_err(|e| BoxError::Git(e.to_string()))?;
            let found = parse_version(&out.stdout).ok_or(BoxError::Unsupported(NEEDS_NEWER_GIT))?;
            *VERSION.get_or_init(|| found)
        }
    };
    if version < (2, 32) {
        return Err(BoxError::Unsupported(NEEDS_NEWER_GIT));
    }
    Ok(())
}

/// `(major, minor)` from `git version 2.55.0.windows.3`.
fn parse_version(text: &str) -> Option<(u32, u32)> {
    let rest = text.trim().strip_prefix("git version ")?;
    let mut parts = rest.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// Whether the `info/alternates` of `objects`, or of a folder it lists, names a network or device path, a
/// quoted path or a link `links_refused` refuses. Git follows these `ALTERNATES_DEPTH` levels deep, each
/// line relative to the objects folder its file is in.
fn alternates_refused(objects: &Path, depth: usize, budget: &mut usize) -> bool {
    if depth > ALTERNATES_DEPTH {
        return false;
    }
    let file = objects.join("info").join("alternates");
    if links_refused(&file) {
        return true;
    }
    match std::fs::metadata(&file) {
        Ok(meta) if meta.is_file() => {
            if meta.len() > SMALL_FILE_CAP || *budget == 0 {
                return true;
            }
            *budget -= 1;
        }
        _ => return false,
    }
    let Some(bytes) = read_capped(&file, SMALL_FILE_CAP) else {
        return true;
    };
    let text = String::from_utf8_lossy(&bytes).into_owned();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('"') || refused(line) {
            return true;
        }
        let target = objects.join(line);
        if links_refused(&target) || alternates_refused(&target, depth + 1, budget) {
            return true;
        }
    }
    false
}

/// Copies `src` into `dir` as `name` when it changed since `slot` was set, or deletes the copy when `src`
/// is gone. A file over the cap is refused.
fn sync_copy(src: &Path, dir: &Path, name: &str, slot: &mut Option<Option<FileStamp>>) -> Result<(), BoxError> {
    let now = stamp(src);
    let dest = dir.join(name);
    if *slot == Some(now) && now.is_some() == dest.is_file() {
        return Ok(());
    }
    let Some(seen) = now else {
        remove_if_present(&dest).map_err(|e| io_error("remove a copy in Changes' git folder", e))?;
        *slot = Some(None);
        return Ok(());
    };
    if seen.0 > SMALL_FILE_CAP {
        return Err(BoxError::Unsupported(TOO_LARGE));
    }
    let bytes = read_capped(src, SMALL_FILE_CAP).ok_or(BoxError::Unsupported(TOO_LARGE))?;
    write_file(dir, name, &bytes, None).map_err(|e| io_error("copy into Changes' git folder", e))?;
    *slot = (stamp(src) == now).then_some(now);
    Ok(())
}

impl GitBox {
    /// Opens the sandbox of `checkout`, making it when it isn't there, and refuses what Changes can't read.
    pub fn open(home: &Path, checkout: &Path) -> Result<GitBox, BoxError> {
        let checkout = std::path::absolute(checkout).unwrap_or_else(|_| checkout.to_path_buf());
        let git_dir = git_dir(&checkout).ok_or(BoxError::NoRepo)?;
        check_version()?;
        let common = common_dir(&git_dir);
        let sandboxes = crate::paths::git_sandbox_dir(home);
        let sandboxes = std::path::absolute(&sandboxes).unwrap_or(sandboxes);
        let mut git_box = GitBox {
            sandbox: sandbox_dir(home, &checkout),
            empty: sandboxes.join("empty-config"),
            checkout,
            git_dir,
            common,
            pairs: Vec::new(),
            origin: Origin::default(),
            sha256: false,
            stamps: Stamps::default(),
            #[cfg(test)]
            runs: Default::default(),
        };
        git_box.reload()?;
        Ok(git_box)
    }

    /// Reads the config again when the repository's or worktree's config file changed (length, modified
    /// time), or 60 s passed for the system and global files; copies the index, the shallow file and
    /// `info/attributes` again when their sources changed; and checks the repository again for what it
    /// may not reach.
    pub fn refresh(&mut self) -> Result<(), BoxError> {
        self.reload()
    }

    /// All of it runs under `sweep::GATE` shared, so a sweep never removes the sandbox while it is written. The
    /// lock is held across file work and a `git config` run on purpose: it is a plain thread lock like
    /// `Slot::work` (`cache.rs`), never held in async code.
    fn reload(&mut self) -> Result<(), BoxError> {
        let _gate = sweep::GATE.read().unwrap_or_else(|e| e.into_inner());
        self.preflight()?;
        self.write_static()?;
        self.write_owner();
        let seen = (
            stamp(&self.common.join("config")),
            stamp(&self.git_dir.join("config.worktree")),
        );
        let stale =
            self.stamps.config != Some(seen) || self.stamps.config_read.is_none_or(|at| at.elapsed() >= CONFIG_MAX_AGE);
        if stale {
            self.read_config()?;
            self.stamps.config = Some(seen);
            self.stamps.config_read = Some(Instant::now());
            self.write_static()?;
        }
        self.sync_index()?;
        sync_copy(
            &self.common.join("shallow"),
            &self.sandbox,
            "shallow",
            &mut self.stamps.shallow,
        )?;
        sync_copy(
            &self.common.join("info").join("attributes"),
            &self.sandbox.join("info"),
            "attributes",
            &mut self.stamps.attributes,
        )
    }

    /// Writes `OWNER_FILE`, which names the checkout for the sweep and dates the sandbox's last use, when the
    /// file isn't there, isn't a regular file, is older than `TOUCH_EVERY` or is dated in the future. A
    /// checkout whose path isn't UTF-8 gets no file. A failed write is left for the next check: the file is
    /// bookkeeping, and Changes works without it.
    fn write_owner(&self) {
        let Some(text) = self.checkout.to_str() else {
            return;
        };
        let file = self.sandbox.join(sweep::OWNER_FILE);
        let fresh = std::fs::symlink_metadata(&file)
            .ok()
            .filter(|meta| meta.file_type().is_file())
            .and_then(|meta| meta.modified().ok())
            .is_some_and(|modified| {
                SystemTime::now()
                    .duration_since(modified)
                    .is_ok_and(|age| age <= sweep::TOUCH_EVERY)
            });
        if !fresh {
            let _ = write_file(&self.sandbox, sweep::OWNER_FILE, text.as_bytes(), None);
        }
    }

    /// What must hold before git is given any path of the repository: no link to a network folder on the way
    /// to what git reads, no alternate object folder on the network, and refs that aren't reftable.
    fn preflight(&self) -> Result<(), BoxError> {
        let common = &self.common;
        let objects = common.join("objects");
        let mut paths = vec![
            objects.clone(),
            objects.join("info"),
            objects.join("pack"),
            objects.join("info").join("alternates"),
            self.git_dir.join("index"),
            common.join("shallow"),
            common.join("packed-refs"),
            common.join("info").join("attributes"),
            common.join("config"),
            self.git_dir.join("config.worktree"),
            common.join("reftable"),
        ];
        for folder in [&self.git_dir, common] {
            if let Ok(entries) = std::fs::read_dir(folder) {
                paths.extend(
                    entries
                        .flatten()
                        .filter(|e| e.file_name().to_string_lossy().starts_with("sharedindex."))
                        .map(|e| e.path()),
                );
            }
        }
        if paths.iter().any(|p| links_refused(p)) || alternates_refused(&objects, 0, &mut ALTERNATES_BUDGET.clone()) {
            return Err(BoxError::Refused(NETWORK));
        }
        if common.join("reftable").is_dir() {
            return Err(BoxError::Unsupported(REFTABLE));
        }
        Ok(())
    }

    /// `HEAD`, `refs/`, `config` and the empty config file: the same bytes every time, so they are written
    /// only when they differ.
    fn write_static(&self) -> Result<(), BoxError> {
        let config: &[u8] = if self.sha256 {
            b"[core]\n\trepositoryformatversion = 1\n\tbare = false\n[extensions]\n\tobjectformat = sha256\n"
        } else {
            b"[core]\n\trepositoryformatversion = 0\n\tbare = false\n"
        };
        let written = (|| {
            std::fs::create_dir_all(self.sandbox.join("refs"))?;
            write_if_different(&self.sandbox, "HEAD", b"ref: refs/heads/none\n")?;
            write_if_different(&self.sandbox, "config", config)?;
            match self.empty.parent() {
                Some(dir) => write_if_different(dir, "empty-config", b""),
                None => Ok(()),
            }
        })();
        written.map_err(|e| io_error("prepare Changes' git folder", e))
    }

    /// One `git config -z --get-regexp` read, with the working folder at the sandbox and no `NOSYSTEM`: the
    /// `scope` says which file. Nothing of the repository is included, and no program runs.
    fn config_entries(&self, scope: &[&OsStr], re: &str) -> Result<Vec<(String, String)>, String> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.sandbox)
            .arg("config")
            .args(scope)
            .args(["--no-includes", "-z", "--get-regexp", re]);
        scrub(&mut cmd);
        cmd.env("GIT_DIR", &self.sandbox);
        no_window(&mut cmd);
        self.count_run();
        let mut out = Vec::new();
        let cancel = AtomicBool::new(false);
        let stopped = stream_command(cmd, Instant::now() + CONFIG_DEADLINE, &cancel, |chunk| {
            out.extend_from_slice(chunk);
            out.len() <= CONFIG_MAX_BYTES
        })
        .map_err(|e| e.to_string())?;
        if stopped != Stopped::Done {
            return Err("Git took too long to read its settings.".to_string());
        }
        Ok(config::parse_z(&out))
    }

    /// Reads the format and the allowed settings. The system and user files are the person's own, so a file
    /// git can't read there is skipped; the repository's own file failing is an error.
    fn read_config(&mut self) -> Result<(), BoxError> {
        let re = config::key_regex();
        let mut pairs = Vec::new();
        let mut origin = Origin::default();
        let mut keep = |entries: Vec<(String, String)>| {
            pairs.extend(entries.iter().filter_map(|(k, v)| config::allowed_pair(k, v)));
            origin.take(&entries);
        };
        for scope in ["--system", "--global"] {
            if let Ok(entries) = self.config_entries(&[OsStr::new(scope)], &re) {
                keep(entries);
            }
        }
        let repo_file = self.common.join("config");
        let mut version = 0u32;
        let (mut object_format, mut ref_storage, mut worktree_config) = (None, None, false);
        if repo_file.is_file() {
            let entries = self
                .config_entries(&[OsStr::new("--file"), repo_file.as_os_str()], &re)
                .map_err(BoxError::Git)?;
            for (key, value) in &entries {
                match key.to_ascii_lowercase().as_str() {
                    "core.repositoryformatversion" => {
                        version = value.trim().parse().map_err(|_| BoxError::Unsupported(NEWER_FORMAT))?;
                    }
                    "extensions.objectformat" => object_format = Some(value.trim().to_ascii_lowercase()),
                    "extensions.refstorage" => ref_storage = Some(value.trim().to_ascii_lowercase()),
                    "extensions.worktreeconfig" => worktree_config = config::is_true(value.trim()),
                    _ => {}
                }
            }
            keep(entries);
        }
        let worktree_file = self.git_dir.join("config.worktree");
        if worktree_config && worktree_file.is_file() {
            keep(
                self.config_entries(&[OsStr::new("--file"), worktree_file.as_os_str()], &re)
                    .map_err(BoxError::Git)?,
            );
        }
        if ref_storage.as_deref() == Some("reftable") {
            return Err(BoxError::Unsupported(REFTABLE));
        }
        if version > 1 {
            return Err(BoxError::Unsupported(NEWER_FORMAT));
        }
        // Git reads the object format only from a version 1 repository.
        let sha256 = match object_format.as_deref().filter(|_| version >= 1) {
            None | Some("sha1") => false,
            Some("sha256") => true,
            Some(_) => return Err(BoxError::Unsupported(NEWER_FORMAT)),
        };
        self.pairs = pairs;
        self.origin = origin;
        self.sha256 = sha256;
        Ok(())
    }

    /// Copies the index (and the shared index a split index names) into the sandbox when the source changed.
    /// The copy gets the source's modified time, so git's racy-clean check judges it as it would the source.
    fn sync_index(&mut self) -> Result<(), BoxError> {
        let src = self.git_dir.join("index");
        let dest = self.sandbox.join("index");
        let now = stamp(&src);
        if self.stamps.index == Some(now) && now.is_some() == dest.is_file() {
            return Ok(());
        }
        let copy = || -> std::io::Result<()> {
            let Some(seen) = now else {
                remove_if_present(&dest)?;
                return self.keep_shared(None);
            };
            let bytes = std::fs::read(&src)?;
            write_file(&self.sandbox, "index", &bytes, seen.1)?;
            let hash_len = if self.sha256 { 32 } else { 20 };
            let shared = shared_index_id(&bytes, hash_len).map(|id| format!("sharedindex.{id}"));
            if let Some(name) = &shared
                && !self.sandbox.join(name).is_file()
                && let Some(from) = [&self.git_dir, &self.common]
                    .into_iter()
                    .map(|d| d.join(name))
                    .find(|p| p.is_file())
            {
                let modified = std::fs::metadata(&from).and_then(|m| m.modified()).ok();
                write_file(&self.sandbox, name, &std::fs::read(&from)?, modified)?;
            }
            self.keep_shared(shared.as_deref())
        };
        copy().map_err(|e| io_error("copy the index into Changes' git folder", e))?;
        // A source that changed while it was being copied is copied again next time.
        self.stamps.index = (stamp(&src) == now).then_some(now);
        Ok(())
    }

    /// Deletes every `sharedindex.*` copy in the sandbox but `keep`.
    fn keep_shared(&self, keep: Option<&str>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(&self.sandbox)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("sharedindex.") && Some(name.as_str()) != keep {
                remove_if_present(&entry.path())?;
            }
        }
        Ok(())
    }

    fn count_run(&self) {
        #[cfg(test)]
        self.runs.fetch_add(1, Ordering::Relaxed);
    }

    /// The git command: every inherited `GIT_*` variable gone, and git pointed at the sandbox, the checkout's
    /// files and its object folder, with no config file but the sandbox's.
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.checkout).args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "core.quotepath=off",
            // A dirty submodule is checked by a git of its own, which reads the submodule's config, filters
            // and all. Submodules are never looked into.
            "-c",
            "diff.ignoreSubmodules=all",
            "-c",
            "status.submoduleSummary=false",
            "-c",
            "submodule.recurse=false",
        ]);
        for (key, value) in &self.pairs {
            cmd.arg("-c").arg(format!("{key}={value}"));
        }
        match args.split_first() {
            // The config covers only porcelain `git diff`; the flag covers the plumbing too.
            Some((first, rest)) if SUBMODULE_BLIND.contains(first) => {
                cmd.arg(first).arg("--ignore-submodules=all").args(rest);
            }
            _ => {
                cmd.args(args);
            }
        }
        scrub(&mut cmd);
        cmd.env("GIT_DIR", &self.sandbox)
            .env("GIT_WORK_TREE", &self.checkout)
            .env("GIT_INDEX_FILE", self.sandbox.join("index"))
            .env("GIT_OBJECT_DIRECTORY", self.common.join("objects"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &self.empty)
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0");
        #[cfg(debug_assertions)]
        if let Some(trace) = std::env::var_os(marshell_protocol::brand::env_var("GIT_TRACE")) {
            cmd.env("GIT_TRACE2_EVENT", trace);
        }
        no_window(&mut cmd);
        cmd
    }

    /// Runs git and hands `each` its stdout in chunks; see `git::stream` for when it stops. Git's own
    /// failure is `BoxError::Git` with the sentence it gave.
    pub fn stream(
        &self,
        args: &[&str],
        deadline: Instant,
        cancel: &AtomicBool,
        each: impl FnMut(&[u8]) -> bool,
    ) -> Result<Stopped, BoxError> {
        self.count_run();
        stream_command(self.command(args), deadline, cancel, each).map_err(|e| BoxError::Git(e.to_string()))
    }

    /// Collects at most `max` bytes; past that the run is stopped (`Stopped::Asked`) and the bytes so far kept.
    pub fn output(
        &self,
        args: &[&str],
        deadline: Instant,
        cancel: &AtomicBool,
        max: usize,
    ) -> Result<(Vec<u8>, Stopped), BoxError> {
        let mut bytes = Vec::new();
        let stopped = self.stream(args, deadline, cancel, |chunk| {
            let room = max.saturating_sub(bytes.len());
            if chunk.len() > room {
                bytes.extend_from_slice(&chunk[..room]);
                return false;
            }
            bytes.extend_from_slice(chunk);
            true
        })?;
        Ok((bytes, stopped))
    }

    pub fn checkout(&self) -> &Path {
        &self.checkout
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn common(&self) -> &Path {
        &self.common
    }

    /// Where `origin` points, as of the last config read. Values only: nothing in it reaches a command line.
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    /// The sandbox's copy of the index: the one git reads, as it was at the last refresh.
    pub fn index(&self) -> PathBuf {
        self.sandbox.join("index")
    }

    /// The id of the empty tree in this repository's hash.
    pub fn empty_tree(&self) -> &'static str {
        if self.sha256 {
            EMPTY_TREE_SHA256
        } else {
            EMPTY_TREE_SHA1
        }
    }

    /// How many git processes this box started (a config read counts), for the tests of what is cached.
    #[cfg(test)]
    pub fn runs(&self) -> usize {
        self.runs.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    const MISSING_OID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const EMPTY_SHA1: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
    const EMPTY_SHA256: &str = "6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321";

    /// A scratch folder with a `repo/` to work in, an empty git config to point plain git at, and a Marshell `home/`.
    struct Scratch {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        repo: PathBuf,
        home: PathBuf,
    }

    impl Scratch {
        fn new() -> Scratch {
            let tmp = tempfile::tempdir().unwrap();
            let root = crate::projects::canonical(tmp.path());
            let repo = root.join("repo");
            std::fs::create_dir(&repo).unwrap();
            std::fs::write(root.join("empty.gitconfig"), "").unwrap();
            Scratch {
                home: root.join("home"),
                repo,
                root,
                _tmp: tmp,
            }
        }

        fn marker(&self) -> PathBuf {
            self.root.join("marker.log")
        }
    }

    /// Plain git, never reading the person's own config: for building and inspecting the scratch repositories.
    fn plain(scratch: &Scratch) -> impl Fn(&Path, &[&str]) -> String + use<> {
        let empty = scratch.root.join("empty.gitconfig");
        move |dir: &Path, args: &[&str]| {
            let mut cmd = Command::new("git");
            cmd.current_dir(dir)
                .args([
                    "-c",
                    "user.name=T",
                    "-c",
                    "user.email=t@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args);
            for name in git_vars(std::env::vars_os()) {
                cmd.env_remove(name);
            }
            cmd.env("GIT_CONFIG_GLOBAL", &empty)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_TERMINAL_PROMPT", "0");
            let out = cmd.output().expect("git runs");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn append(path: &Path, text: &str) {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    /// A repository with three files and one commit on `main`.
    fn repo_with_commit(scratch: &Scratch) -> String {
        let git = plain(scratch);
        git(&scratch.repo, &["init", "-q", "-b", "main"]);
        write(&scratch.repo.join("a.txt"), "one\n");
        write(&scratch.repo.join("b.txt"), "two\n");
        write(&scratch.repo.join("c.md"), "three\n");
        git(&scratch.repo, &["add", "-A"]);
        git(&scratch.repo, &["commit", "-q", "-m", "first"]);
        git(&scratch.repo, &["rev-parse", "HEAD"])
    }

    fn far() -> Instant {
        Instant::now() + Duration::from_secs(30)
    }

    fn run(b: &GitBox, args: &[&str]) -> String {
        let cancel = AtomicBool::new(false);
        let (bytes, stopped) = b.output(args, far(), &cancel, 4 * 1024 * 1024).unwrap();
        assert_eq!(stopped, Stopped::Done, "{args:?}");
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// A path as text that two spellings of the same place compare equal on.
    fn norm(path: &Path) -> String {
        flat(&crate::projects::canonical(path).to_string_lossy())
    }

    /// Text with one kind of separator, no quotes and no doubled slashes (git quotes and escapes a path
    /// with a space in `--show-origin`), and lowercase on Windows.
    fn flat(text: &str) -> String {
        let mut text = text.replace('"', "").replace('\\', "/");
        while text.contains("//") {
            text = text.replace("//", "/");
        }
        if cfg!(windows) { text.to_lowercase() } else { text }
    }

    fn sandbox_of(b: &GitBox) -> PathBuf {
        b.sandbox.clone()
    }

    // The S15 trap: a repository whose own config would run a program at every turn. Each program appends to a
    // marker file; the commands run relative to the work tree, so `../marker.log` is the marker.
    fn set_the_trap(scratch: &Scratch) {
        let git = plain(scratch);
        write(&scratch.repo.join(".gitattributes"), "*.txt filter=evil\n");
        git(&scratch.repo, &["add", ".gitattributes"]);
        git(&scratch.repo, &["commit", "-q", "-m", "attributes"]);
        let include = if cfg!(windows) {
            r"\\\\127.0.0.1\\marshell-no-share\\x.cfg"
        } else {
            "/marshell-no-such-folder/x.cfg"
        };
        append(
            &scratch.repo.join(".git").join("config"),
            &format!(
                "[core]\n\tfsmonitor = echo fsmonitor >> ../marker.log\n\
                 [filter \"evil\"]\n\tclean = cat >> ../marker.log\n\
                 [diff]\n\texternal = echo external >> ../marker.log\n\
                 [include]\n\tpath = {include}\n"
            ),
        );
        std::fs::write(scratch.marker(), "").unwrap();
    }

    fn marker_text(scratch: &Scratch) -> String {
        std::fs::read_to_string(scratch.marker()).unwrap_or_default()
    }

    #[test]
    fn hostile_config_runs_nothing() {
        let scratch = Scratch::new();
        let head = repo_with_commit(&scratch);
        set_the_trap(&scratch);
        write(&scratch.repo.join("a.txt"), "changed one\n");

        // The trap works: plain git runs the repository's programs.
        let mut plain_diff = Command::new("git");
        plain_diff
            .current_dir(&scratch.repo)
            .arg("diff")
            .env("GIT_CONFIG_GLOBAL", scratch.root.join("empty.gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
        let _ = plain_diff.output().unwrap();
        let ran = marker_text(&scratch);
        assert!(
            ran.contains("external") || ran.contains("fsmonitor") || ran.contains("changed one"),
            "plain git ran none of the hostile programs, so this test proves nothing: {ran:?}"
        );
        std::fs::write(scratch.marker(), "").unwrap();

        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let raw = run(&b, &["diff", "--raw"]);
        assert!(raw.contains("a.txt"), "{raw}");
        let patch = run(&b, &["diff"]);
        assert!(patch.contains("+changed one"), "{patch}");
        let since_head = run(&b, &["diff", "--raw", &head]);
        assert!(since_head.contains("a.txt"), "{since_head}");
        let files = run(&b, &["ls-files"]);
        assert!(files.contains("a.txt") && files.contains("c.md"), "{files}");
        assert_eq!(run(&b, &["rev-list", "--count", &head]).trim(), "1");
        assert_eq!(marker_text(&scratch), "", "a program of the repository ran");
    }

    /// A diff of a checkout with a dirty submodule would have git check the submodule with a git of its own,
    /// which reads the submodule's config: a filter there must not run either.
    #[test]
    fn a_submodule_config_runs_nothing() {
        let scratch = Scratch::new();
        let git = plain(&scratch);
        let source = scratch.root.join("subsource");
        std::fs::create_dir(&source).unwrap();
        git(&source, &["init", "-q", "-b", "main"]);
        write(&source.join(".gitattributes"), "*.txt filter=evil\n");
        write(&source.join("s.txt"), "inside\n");
        git(&source, &["add", "-A"]);
        git(&source, &["commit", "-q", "-m", "sub"]);
        repo_with_commit(&scratch);
        let source_arg = source.to_string_lossy().replace('\\', "/");
        git(
            &scratch.repo,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                &source_arg,
                "sub",
            ],
        );
        git(&scratch.repo, &["commit", "-q", "-m", "submodule"]);
        let marker = scratch.marker().to_string_lossy().replace('\\', "/");
        append(
            &scratch.repo.join(".git").join("modules").join("sub").join("config"),
            &format!("[filter \"evil\"]\n\tclean = cat >> '{marker}'\n"),
        );
        // Same bytes, a new modified time: git has to hash the file again, through its filter.
        let file = scratch.repo.join("sub").join("s.txt");
        let later = SystemTime::now() + Duration::from_secs(120);
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(later)
            .unwrap();
        write(&scratch.repo.join("a.txt"), "changed one\n");
        std::fs::write(scratch.marker(), "").unwrap();

        // The trap works: plain git runs the submodule's filter.
        git(&scratch.repo, &["diff"]);
        assert!(
            marker_text(&scratch).contains("inside"),
            "plain git ran no filter in the submodule, so this test proves nothing: {:?}",
            marker_text(&scratch)
        );
        std::fs::write(scratch.marker(), "").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(later + Duration::from_secs(60))
            .unwrap();

        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert!(run(&b, &["diff", "--raw"]).contains("a.txt"));
        assert!(run(&b, &["diff"]).contains("+changed one"));
        run(&b, &["status", "--porcelain"]);
        assert_eq!(marker_text(&scratch), "", "a filter of the submodule ran");
    }

    #[test]
    fn only_the_sandbox_and_the_command_line_are_read() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        set_the_trap(&scratch);
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let listing = run(&b, &["config", "--list", "--show-origin"]);
        let sandbox = norm(&sandbox_of(&b));
        let mut lines = 0;
        for line in listing.lines() {
            let line = flat(line);
            lines += 1;
            assert!(
                line.starts_with(&format!("file:{sandbox}/config")) || line.starts_with("command line:"),
                "{line}"
            );
        }
        assert!(lines > 0, "{listing}");
        assert!(
            !listing.contains("marker.log") && !listing.contains("evil"),
            "{listing}"
        );
    }

    #[test]
    fn allowlisted_values_pass_and_others_do_not() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        append(
            &scratch.repo.join(".git").join("config"),
            "[core]\n\tautocrlf = input\n\tpager = x\n[diff]\n\talgorithm = evil\n",
        );
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        // The system and global files come first; the repository's own file is last, and wins.
        assert_eq!(
            b.pairs.last(),
            Some(&("core.autocrlf".to_string(), "input".to_string())),
            "{:?}",
            b.pairs
        );
        assert!(
            b.pairs
                .iter()
                .all(|(k, v)| k != "core.pager" && v != "evil" && v != "x"),
            "{:?}",
            b.pairs
        );
    }

    #[test]
    fn crlf_file_is_unchanged_under_autocrlf_true() {
        let scratch = Scratch::new();
        let git = plain(&scratch);
        git(&scratch.repo, &["init", "-q", "-b", "main"]);
        git(&scratch.repo, &["config", "core.autocrlf", "true"]);
        write(&scratch.repo.join("crlf.txt"), "one\r\ntwo\r\n");
        git(&scratch.repo, &["add", "-A"]);
        git(&scratch.repo, &["commit", "-q", "-m", "crlf"]);
        // The same bytes again, so the file's stat no longer matches the index and git has to compare content.
        std::thread::sleep(Duration::from_millis(30));
        write(&scratch.repo.join("crlf.txt"), "one\r\ntwo\r\n");
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert_eq!(run(&b, &["diff", "--raw"]).trim(), "");
        // Without the setting passed on, the same file would differ (the box's `-c` is what decides).
        let mut cmd = Command::new("git");
        cmd.current_dir(&scratch.repo)
            .args(["-c", "core.autocrlf=false", "diff", "--raw"])
            .env("GIT_CONFIG_GLOBAL", scratch.root.join("empty.gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
        let out = cmd.output().unwrap();
        assert!(!out.stdout.is_empty(), "the control shows a change without autocrlf");
    }

    #[test]
    fn a_linked_worktree_uses_its_own_index() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let git = plain(&scratch);
        let wt = scratch.root.join("wt");
        git(
            &scratch.repo,
            &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "feat"],
        );
        write(&wt.join("b.txt"), "changed two\n");
        let b = GitBox::open(&scratch.home, &wt).unwrap();
        assert_eq!(norm(b.common()), norm(&scratch.repo.join(".git")));
        assert!(
            norm(b.git_dir()).starts_with(&norm(&scratch.repo.join(".git").join("worktrees"))),
            "{:?}",
            b.git_dir()
        );
        assert_eq!(
            std::fs::read(sandbox_of(&b).join("index")).unwrap(),
            std::fs::read(b.git_dir().join("index")).unwrap()
        );
        let raw = run(&b, &["diff", "--raw"]);
        assert!(raw.contains("b.txt") && !raw.contains("a.txt"), "{raw}");
        assert_eq!(run(&b, &["hash-object", "-t", "tree", "--stdin"]).trim(), EMPTY_SHA1);
        let main = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert_ne!(sandbox_of(&main), sandbox_of(&b));
        assert_eq!(run(&main, &["diff", "--raw"]).trim(), "");
    }

    #[test]
    fn sha256_repo_gets_objectformat_in_the_sandbox() {
        let scratch = Scratch::new();
        let git = plain(&scratch);
        git(&scratch.repo, &["init", "-q", "--object-format=sha256", "-b", "main"]);
        write(&scratch.repo.join("a.txt"), "one\n");
        git(&scratch.repo, &["add", "-A"]);
        git(&scratch.repo, &["commit", "-q", "-m", "first"]);
        write(&scratch.repo.join("a.txt"), "two\n");
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let config = std::fs::read_to_string(sandbox_of(&b).join("config")).unwrap();
        assert!(config.contains("objectformat = sha256"), "{config}");
        assert!(config.contains("repositoryformatversion = 1"), "{config}");
        assert_eq!(b.empty_tree(), EMPTY_SHA256);
        assert_eq!(run(&b, &["hash-object", "-t", "tree", "--stdin"]).trim(), EMPTY_SHA256);
        assert!(run(&b, &["diff", "--raw"]).contains("a.txt"));
    }

    #[test]
    fn index_is_never_written() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        // Two tracked files get new modified times and the same bytes: the case where git would like to
        // rewrite the index with the refreshed stat data.
        std::thread::sleep(Duration::from_millis(30));
        write(&scratch.repo.join("a.txt"), "one\n");
        write(&scratch.repo.join("b.txt"), "two\n");
        let index = scratch.repo.join(".git").join("index");
        let bytes = std::fs::read(&index).unwrap();
        let modified = std::fs::metadata(&index).unwrap().modified().unwrap();
        let mut b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        for _ in 0..3 {
            b.refresh().unwrap();
            run(&b, &["status", "--porcelain"]);
            assert_eq!(run(&b, &["diff", "--raw"]).trim(), "");
            run(&b, &["ls-files", "-s"]);
            assert!(!scratch.repo.join(".git").join("index.lock").exists());
        }
        assert_eq!(std::fs::read(&index).unwrap(), bytes);
        assert_eq!(std::fs::metadata(&index).unwrap().modified().unwrap(), modified);
        assert!(!scratch.repo.join(".git").join("index.lock").exists());
    }

    #[test]
    fn a_split_index_is_copied_with_its_shared_file() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let git = plain(&scratch);
        git(&scratch.repo, &["update-index", "--split-index"]);
        write(&scratch.repo.join("d.txt"), "four\n");
        git(&scratch.repo, &["add", "d.txt"]);
        let shared: Vec<String> = std::fs::read_dir(scratch.repo.join(".git"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("sharedindex."))
            .collect();
        assert!(!shared.is_empty(), "git wrote no shared index");
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let named = shared
            .iter()
            .find(|n| sandbox_of(&b).join(n).is_file())
            .unwrap_or_else(|| panic!("the sandbox has none of {shared:?}"));
        assert_eq!(
            std::fs::read(sandbox_of(&b).join(named)).unwrap(),
            std::fs::read(scratch.repo.join(".git").join(named)).unwrap()
        );
        let files = run(&b, &["ls-files"]);
        assert!(files.contains("a.txt") && files.contains("d.txt"), "{files}");
    }

    #[test]
    fn alternates_on_a_network_path_are_refused() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        write(
            &scratch.repo.join(".git/objects/info/alternates"),
            "\\\\server\\share\\objects\n",
        );
        let err = GitBox::open(&scratch.home, &scratch.repo).err().unwrap();
        assert!(matches!(err, BoxError::Refused(_)), "{err:?}");
    }

    #[test]
    fn nested_alternates_are_followed_and_refused() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let alt = scratch.root.join("alt").join("objects");
        write(&alt.join("info/alternates"), "\\\\server\\share\\objects\n");
        write(
            &scratch.repo.join(".git/objects/info/alternates"),
            &format!("{}\n", alt.display()),
        );
        let err = GitBox::open(&scratch.home, &scratch.repo).err().unwrap();
        assert!(matches!(err, BoxError::Refused(_)), "{err:?}");
        // A local alternate that leads nowhere is fine.
        write(&alt.join("info/alternates"), "");
        assert!(GitBox::open(&scratch.home, &scratch.repo).is_ok());
    }

    #[test]
    fn reftable_in_the_config_is_unsupported() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let git = plain(&scratch);
        git(&scratch.repo, &["config", "core.repositoryformatversion", "1"]);
        append(
            &scratch.repo.join(".git").join("config"),
            "[extensions]\n\trefstorage = reftable\n",
        );
        let err = GitBox::open(&scratch.home, &scratch.repo).err().unwrap();
        assert_eq!(
            err,
            BoxError::Unsupported("This repository stores its branches in a format Changes can't read yet.")
        );
    }

    #[test]
    fn shallow_is_copied_and_rev_list_stops_there() {
        let scratch = Scratch::new();
        let git = plain(&scratch);
        repo_with_commit(&scratch);
        for n in ["second", "third"] {
            write(&scratch.repo.join("a.txt"), n);
            git(&scratch.repo, &["commit", "-q", "-am", n]);
        }
        let head = git(&scratch.repo, &["rev-parse", "HEAD"]);
        let middle = git(&scratch.repo, &["rev-parse", "HEAD~1"]);
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert_eq!(run(&b, &["rev-list", "--count", &head]).trim(), "3");
        write(&scratch.repo.join(".git").join("shallow"), &format!("{middle}\n"));
        let mut b = b;
        b.refresh().unwrap();
        assert_eq!(
            std::fs::read_to_string(sandbox_of(&b).join("shallow")).unwrap(),
            format!("{middle}\n")
        );
        assert_eq!(run(&b, &["rev-list", "--count", &head]).trim(), "2");
    }

    #[test]
    fn a_stale_shallow_copy_is_removed() {
        let scratch = Scratch::new();
        let head = repo_with_commit(&scratch);
        write(&scratch.repo.join(".git").join("shallow"), &format!("{head}\n"));
        let mut b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert!(sandbox_of(&b).join("shallow").is_file());
        std::fs::remove_file(scratch.repo.join(".git").join("shallow")).unwrap();
        b.refresh().unwrap();
        assert!(!sandbox_of(&b).join("shallow").exists());
    }

    #[test]
    fn renamelimit_over_1000_is_dropped() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        append(
            &scratch.repo.join(".git").join("config"),
            "[diff]\n\trenameLimit = 5000\n",
        );
        let mut b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert!(b.pairs.iter().all(|(_, v)| v != "5000"), "{:?}", b.pairs);
        append(&scratch.repo.join(".git").join("config"), "\trenameLimit = 800\n");
        b.refresh().unwrap();
        assert_eq!(
            b.pairs.last(),
            Some(&("diff.renamelimit".to_string(), "800".to_string())),
            "{:?}",
            b.pairs
        );
    }

    #[test]
    fn the_origin_url_is_read_without_includes() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let included = scratch.root.join("included.cfg");
        write(
            &included,
            "[remote \"origin\"]\n\turl = https://github.com/evil/included.git\n\tpushurl = https://github.com/evil/pushed.git\n",
        );
        let path = included.to_string_lossy().replace('\\', "/");
        append(
            &scratch.repo.join(".git").join("config"),
            &format!(
                "[remote \"origin\"]\n\turl = https://github.com/octo-org/widgets.git\n[include]\n\tpath = {path}\n"
            ),
        );
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert_eq!(
            b.origin().urls,
            vec!["https://github.com/octo-org/widgets.git".to_string()]
        );
        assert!(b.origin().push_urls.is_empty(), "{:?}", b.origin());
        // The included file is real: plain git, which follows includes, sees its values.
        let seen = plain(&scratch)(&scratch.repo, &["config", "--get-all", "remote.origin.url"]);
        assert!(seen.contains("evil/included"), "{seen}");
    }

    #[test]
    fn a_linked_worktree_reads_origin_from_the_common_config() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let git = plain(&scratch);
        git(
            &scratch.repo,
            &["remote", "add", "origin", "https://github.com/octo-org/widgets.git"],
        );
        let wt = scratch.root.join("wt");
        git(
            &scratch.repo,
            &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "feat"],
        );
        let b = GitBox::open(&scratch.home, &wt).unwrap();
        assert_eq!(
            b.origin().urls,
            vec!["https://github.com/octo-org/widgets.git".to_string()]
        );
    }

    #[test]
    fn the_url_never_becomes_a_dash_c_pair() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        append(
            &scratch.repo.join(".git").join("config"),
            "[remote \"origin\"]\n\turl = https://github.com/octo-org/widgets.git\n\tpushurl = git@github.com:octo-org/widgets.git\n\
             [url \"https://github.com/\"]\n\tinsteadOf = gh:\n\tpushInsteadOf = git@github.com:\n",
        );
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert!(!b.origin().urls.is_empty());
        let cmd = b.command(&["status"]);
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        for arg in &args {
            let lower = arg.to_ascii_lowercase();
            assert!(
                !lower.contains("remote.origin") && !lower.starts_with("url.") && !lower.contains("insteadof"),
                "{arg}"
            );
        }
        assert!(
            b.pairs
                .iter()
                .all(|(k, _)| !k.starts_with("remote.") && !k.starts_with("url."))
        );
    }

    #[test]
    fn pushurl_and_push_instead_of_are_read() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        append(
            &scratch.repo.join(".git").join("config"),
            "[remote \"origin\"]\n\turl = https://github.com/octo-org/widgets.git\n\
             \tpushurl = git@github.com:octo-org/widgets.git\n\tpushurl = git@github.com:octo-org/second.git\n\
             [url \"https://github.com/\"]\n\tinsteadOf = gh:\n\tinsteadOf = github:\n\tpushInsteadOf = ssh:\n\
             [url \"git@github.com:\"]\n\tpushInsteadOf = https://github.com/\n\
             [remote \"Origin\"]\n\turl = https://github.com/octo-org/not-origin.git\n",
        );
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let origin = b.origin();
        assert_eq!(origin.urls, vec!["https://github.com/octo-org/widgets.git".to_string()]);
        assert_eq!(
            origin.push_urls,
            vec![
                "git@github.com:octo-org/widgets.git".to_string(),
                "git@github.com:octo-org/second.git".to_string()
            ]
        );
        let at = |list: &[(String, String)], base: &str, value: &str| {
            list.iter()
                .position(|(b, v)| b == base && v == value)
                .unwrap_or_else(|| panic!("{base} {value} in {list:?}"))
        };
        let first = at(&origin.instead_of, "https://github.com/", "gh:");
        let second = at(&origin.instead_of, "https://github.com/", "github:");
        assert!(first < second, "{:?}", origin.instead_of);
        let ssh = at(&origin.push_instead_of, "https://github.com/", "ssh:");
        let back = at(&origin.push_instead_of, "git@github.com:", "https://github.com/");
        assert!(ssh < back, "{:?}", origin.push_instead_of);
        assert!(!origin.instead_of.iter().any(|(_, v)| v == "ssh:"));
    }

    #[test]
    fn inherited_git_env_is_removed() {
        let vars = [
            ("GIT_EXTERNAL_DIFF", "x"),
            ("PATH", "y"),
            ("GIT_DIR", "z"),
            ("GITHUB_TOKEN", "t"),
            ("HOME", "h"),
        ]
        .map(|(k, v)| (OsString::from(k), OsString::from(v)));
        assert_eq!(
            git_vars(vars.into_iter()),
            vec![OsString::from("GIT_EXTERNAL_DIFF"), OsString::from("GIT_DIR")]
        );
        if cfg!(windows) {
            let lower = [(OsString::from("git_dir"), OsString::from("z"))];
            assert_eq!(git_vars(lower.into_iter()), vec![OsString::from("git_dir")]);
        }
    }

    #[test]
    fn exit_128_reports_gits_sentence() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let cancel = AtomicBool::new(false);
        let err = b
            .output(&["rev-list", "--count", MISSING_OID], far(), &cancel, 1024)
            .unwrap_err();
        let BoxError::Git(text) = err else { panic!("{err:?}") };
        assert!(
            text.contains("bad object") || text.contains("unknown revision"),
            "{text}"
        );
        assert!(text.starts_with("Git stopped with exit code 128: "), "{text}");
    }

    #[test]
    fn the_hash_is_fnv_1a_and_the_folder_keeps_its_name() {
        // Published FNV-1a 64 test vectors.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
        assert_eq!(sandbox_name(Path::new("a")), "af63dc4c8601ec8c");
        assert_eq!(sandbox_name(Path::new("")).len(), 16);
        let same = sandbox_name(Path::new("C:/Repo")) == sandbox_name(Path::new("c:/repo"));
        assert_eq!(same, cfg!(windows), "paths compare without case only on Windows");
    }

    #[test]
    fn the_version_is_read_from_gits_line() {
        assert_eq!(parse_version("git version 2.55.0.windows.3\n"), Some((2, 55)));
        assert_eq!(parse_version("git version 2.32.0"), Some((2, 32)));
        assert_eq!(parse_version("something else"), None);
        assert_eq!(parse_version("git version x.y"), None);
    }

    #[test]
    fn an_index_that_makes_no_sense_names_no_shared_file() {
        assert_eq!(shared_index_id(b"", 20), None);
        assert_eq!(shared_index_id(b"DIRC", 20), None);
        let mut header = b"DIRC".to_vec();
        header.extend_from_slice(&2u32.to_be_bytes());
        header.extend_from_slice(&1000u32.to_be_bytes());
        header.extend_from_slice(&[0; 20]);
        assert_eq!(shared_index_id(&header, 20), None);
        header.extend_from_slice(&[7; 300]);
        assert_eq!(shared_index_id(&header, 20), None);
    }

    #[test]
    fn nothing_runs_when_nothing_changed() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let mut b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let before = b.runs();
        assert!(before > 0, "the first open reads the config");
        b.refresh().unwrap();
        assert_eq!(b.runs(), before);
        // A change to the repository's config is read again.
        std::thread::sleep(Duration::from_millis(30));
        append(
            &scratch.repo.join(".git").join("config"),
            "[core]\n\tfilemode = false\n",
        );
        b.refresh().unwrap();
        assert!(b.runs() > before);
        assert!(b.pairs.contains(&("core.filemode".to_string(), "false".to_string())));
    }

    #[test]
    fn a_changed_index_is_copied_again_with_its_modified_time() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let git = plain(&scratch);
        let mut b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        write(&scratch.repo.join("d.txt"), "four\n");
        git(&scratch.repo, &["add", "d.txt"]);
        b.refresh().unwrap();
        let source = scratch.repo.join(".git").join("index");
        let copy = sandbox_of(&b).join("index");
        assert_eq!(std::fs::read(&copy).unwrap(), std::fs::read(&source).unwrap());
        assert_eq!(
            std::fs::metadata(&copy).unwrap().modified().unwrap(),
            std::fs::metadata(&source).unwrap().modified().unwrap()
        );
        assert!(run(&b, &["ls-files"]).contains("d.txt"));
    }

    #[test]
    fn a_checkout_with_no_git_folder_is_no_repo() {
        let scratch = Scratch::new();
        assert_eq!(GitBox::open(&scratch.home, &scratch.repo).err(), Some(BoxError::NoRepo));
    }

    fn date(path: &Path, modified: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    fn modified(path: &Path) -> SystemTime {
        std::fs::metadata(path).unwrap().modified().unwrap()
    }

    /// Whether `time` is within a minute of now.
    fn recent(time: SystemTime) -> bool {
        let now = SystemTime::now();
        let apart = now.duration_since(time).unwrap_or_else(|e| e.duration());
        apart < Duration::from_secs(60)
    }

    #[test]
    fn open_names_the_checkout_in_the_sandbox() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let text = std::fs::read_to_string(sandbox_of(&b).join(sweep::OWNER_FILE)).unwrap();
        assert_eq!(Path::new(&text), b.checkout());
        assert_eq!(sandbox_of(&b), sandbox_dir(&scratch.home, &scratch.repo));
        assert_eq!(
            sweep::sweep(&scratch.home, SystemTime::now()),
            sweep::Swept { removed: 0, kept: 1 }
        );
        assert!(sandbox_of(&b).is_dir());
    }

    #[test]
    fn a_sandbox_without_the_file_gets_one_when_it_is_opened() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let file = sandbox_of(&b).join(sweep::OWNER_FILE);
        std::fs::remove_file(&file).unwrap();
        GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert!(file.is_file());
    }

    #[test]
    fn the_file_is_written_again_after_an_hour() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let mut b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        let file = sandbox_of(&b).join(sweep::OWNER_FILE);
        let minutes = |m: u64| Duration::from_secs(m * 60);
        date(&file, SystemTime::now() - minutes(59));
        let before = modified(&file);
        b.refresh().unwrap();
        assert_eq!(modified(&file), before, "59 minutes is not old enough");
        date(&file, SystemTime::now() - minutes(61));
        b.refresh().unwrap();
        assert!(recent(modified(&file)), "{:?}", modified(&file));
        date(&file, SystemTime::now() + 24 * 60 * minutes(1));
        b.refresh().unwrap();
        assert!(recent(modified(&file)), "{:?}", modified(&file));
    }

    #[test]
    fn a_sandbox_removed_under_a_live_box_is_rebuilt_by_refresh() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let mut b = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        assert_eq!(sweep::remove_all(&scratch.home), 1);
        assert!(!sandbox_of(&b).exists());
        b.refresh().unwrap();
        let sandbox = sandbox_of(&b);
        assert!(sandbox.join("index").is_file());
        assert!(sandbox.join("HEAD").is_file());
        assert!(sandbox.join(sweep::OWNER_FILE).is_file());
        assert!(run(&b, &["ls-files"]).contains("a.txt"));
    }
}
