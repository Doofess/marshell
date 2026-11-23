//! The Changes panel's engine: what differs between a checkout and the branch it is measured against.
//!
//! `check` is the cheap question (what changed, as a stamp) and `read` the full answer (the patch and the new
//! files' diffs). Both run git only through `gitbox::GitBox`, and both keep what they learned per checkout
//! in `cache`.

pub mod cache;
pub mod raw;

pub use cache::{CHECK_FRESH, ChangesCache};
pub use raw::{RawEntry, parse_raw};

use crate::git::Stopped;
use crate::gitbox::refs::{self, Base, Head, RefError};
use crate::gitbox::worktree::{self, Reach, Tracked};
use crate::gitbox::{BoxError, GitBox, Origin};
use crate::server::{Ignores, walk_unignored};
use crate::tasks::{BINARY_SNIFF, MAX_DIFF, MAX_UNTRACKED, cut_diff, untracked_diff};
use cache::{Entry, FileStamp};
use marshell_protocol::api::{ChangesKind, ChangesReply, DiffFile, DiffNote, DiffStatus};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How long a check may take.
pub const CHECK_BUDGET: Duration = Duration::from_secs(2);
/// How long a full read may take.
pub const READ_BUDGET: Duration = Duration::from_secs(5);
/// The most untracked files that get a section.
pub const MAX_NEW_FILES: usize = 200;

pub(crate) const TOO_LONG: &str = "Git took too long to read the changes.";
/// The sentence for a repository that keeps its branches as reftable (the box says the same).
const REFTABLE: &str = "This repository stores its branches in a format Changes can't read yet.";
/// The sentence for a work tree git would follow to a network or device path (`gitbox::worktree`).
const SUBMODULE_NETWORK: &str =
    "A submodule of this repository points to a network folder, so Changes doesn't read it.";
/// The sentence for an index holding a path that isn't UTF-8 (`gitbox::worktree`).
const ODD_NAME: &str = "A file name in this repository isn't UTF-8, so Changes doesn't read it.";
/// A list of changed files cut at `MAX_RAW` would show part of the changes as all of them.
const TOO_MANY_CHANGED: &str = "This checkout has too many changed files for Changes.";
/// The most `diff --raw` output that is read.
const MAX_RAW: usize = 8 * 1024 * 1024;
/// The most `ls-files` output that is read.
const MAX_TRACKED_BYTES: usize = 64 * 1024 * 1024;

/// The checkout to read, and what the caller knows about it.
#[derive(Debug, Clone)]
pub struct Target {
    pub checkout: PathBuf,
    /// The checkout is not the launch folder's: the shell moved.
    pub moved: bool,
    /// The base branch of the task whose worktree this is.
    pub task_base: Option<String>,
}

/// What a check found out.
#[derive(Debug)]
pub struct Check {
    /// A hash of everything below, and of the files' lengths and modified times.
    pub stamp: u64,
    pub head: Head,
    /// HEAD's full commit id; None on an unborn branch.
    pub head_oid: Option<String>,
    /// Where `origin` points, as the sandbox read it. Not part of the stamp.
    pub origin: Origin,
    pub base: Option<Base>,
    /// What the files are compared with: the merge base, else HEAD, else the empty tree.
    pub from: String,
    /// The base shares no history with HEAD.
    pub unrelated: bool,
    pub entries: Vec<RawEntry>,
    /// The first `MAX_NEW_FILES` untracked files, sorted.
    pub new_files: Vec<String>,
    pub more_new: u32,
    /// The walk for new files was cut by the deadline: some may be missing.
    pub new_cut: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangesError {
    NoRepo,
    /// Changes won't or can't read this repository; the sentence says why.
    Refused(String),
    /// Git or the disk failed, or took too long; the sentence says what.
    Failed(String),
}

/// Why a git step stopped.
#[derive(Debug)]
pub(crate) enum Halt {
    /// The deadline came, or the caller cancelled.
    Late,
    Err(ChangesError),
}

impl Halt {
    pub(crate) fn into_error(self) -> ChangesError {
        match self {
            Halt::Late => ChangesError::Failed(TOO_LONG.to_string()),
            Halt::Err(e) => e,
        }
    }
}

impl From<BoxError> for Halt {
    fn from(e: BoxError) -> Halt {
        Halt::Err(match e {
            BoxError::NoRepo => ChangesError::NoRepo,
            BoxError::Refused(s) | BoxError::Unsupported(s) => ChangesError::Refused(s.to_string()),
            BoxError::Git(s) => ChangesError::Failed(s),
        })
    }
}

impl From<RefError> for Halt {
    fn from(e: RefError) -> Halt {
        Halt::Err(match e {
            RefError::Reftable => ChangesError::Refused(REFTABLE.to_string()),
            RefError::Unreadable(s) => ChangesError::Failed(format!("{s}.")),
        })
    }
}

fn failed(sentence: &str) -> Halt {
    Halt::Err(ChangesError::Failed(sentence.to_string()))
}

/// The changed files `git diff --raw -z` listed, refused when its output reached the cap.
fn raw_entries(raw: &[u8], capped: bool) -> Result<Vec<RawEntry>, Halt> {
    if capped {
        return Err(failed(TOO_MANY_CHANGED));
    }
    Ok(parse_raw(raw))
}

/// Runs git and collects at most `max` bytes; true when there was more.
pub(crate) fn collect(
    git: &GitBox,
    args: &[&str],
    deadline: Instant,
    cancel: &AtomicBool,
    max: usize,
) -> Result<(Vec<u8>, bool), Halt> {
    let (bytes, stopped) = git.output(args, deadline, cancel, max)?;
    match stopped {
        Stopped::Deadline | Stopped::Cancelled => Err(Halt::Late),
        Stopped::Asked => Ok((bytes, true)),
        Stopped::Done => Ok((bytes, false)),
    }
}

/// A file's length and modified time as it is now, None when it can't be looked at.
fn stat(path: &Path) -> Option<FileStamp> {
    std::fs::symlink_metadata(path)
        .ok()
        .map(|m| (m.len(), m.modified().ok()))
}

/// The checkout's tracked paths, from the index. Cached by the stamp of the sandbox's copy, which is what
/// `ls-files` reads: the checkout's own index may have changed since the last refresh copied it.
fn tracked_paths(
    git: &GitBox,
    cache: &mut Option<(Option<FileStamp>, Arc<Tracked>)>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Arc<Tracked>, Halt> {
    let key = std::fs::metadata(git.index())
        .ok()
        .filter(|m| m.is_file())
        .map(|m| (m.len(), m.modified().ok()));
    if let Some((had, set)) = cache
        && *had == key
    {
        return Ok(set.clone());
    }
    let (bytes, capped) = collect(
        git,
        &["ls-files", "--cached", "-z"],
        deadline,
        cancel,
        MAX_TRACKED_BYTES,
    )?;
    if capped {
        return Err(failed("This repository has too many files for Changes."));
    }
    let set = Arc::new(Tracked::parse(&bytes));
    *cache = Some((key, set.clone()));
    Ok(set)
}

/// The link safety check (`worktree::reach`), or its last clear answer while `reached` says that one stands.
fn looked(
    checkout: &Path,
    tracked: &Arc<Tracked>,
    reached: &mut cache::Reached,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Reach {
    if reached.stands(tracked) {
        return Reach::Clear;
    }
    let started = Instant::now();
    let reach = worktree::reach(checkout, tracked, deadline, cancel);
    match reach {
        Reach::Clear => reached.set(tracked, started),
        _ => *reached = cache::Reached::default(),
    }
    reach
}

/// The check: heads, base, the changed files and the new ones, and a stamp of all of it. Runs under the
/// checkout's `work` lock.
fn compute(
    home: &Path,
    target: &Target,
    entry: &mut Entry,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Check, Halt> {
    if Instant::now() >= deadline || cancel.load(Ordering::Relaxed) {
        return Err(Halt::Late);
    }
    match entry.git.as_mut() {
        Some(git) => git.refresh()?,
        None => entry.git = Some(GitBox::open(home, &target.checkout)?),
    }
    let Entry {
        git,
        merge_base,
        tracked,
        reached,
        walk,
    } = entry;
    let git = git.as_ref().expect("the box was just opened");
    let head = refs::head(git.git_dir(), git.common())?;
    let base = refs::base(git.git_dir(), git.common(), target.task_base.as_deref())?;

    let (from, unrelated) = match (&head.oid, &base) {
        (None, _) => (git.empty_tree().to_string(), false),
        (Some(head), None) => (head.as_str().to_string(), false),
        (Some(head), Some(base)) => {
            let key = (head.as_str().to_string(), base.oid.as_str().to_string());
            let found = match merge_base {
                Some((had, found)) if *had == key => found.clone(),
                _ => {
                    let (bytes, _) = collect(
                        git,
                        &["merge-base", head.as_str(), base.oid.as_str()],
                        deadline,
                        cancel,
                        4096,
                    )?;
                    let text = String::from_utf8_lossy(&bytes);
                    let text = text.trim();
                    let found = if text.is_empty() {
                        None
                    } else {
                        Some(
                            refs::Oid::parse(text)
                                .ok_or_else(|| failed("Git gave an answer Changes can't read."))?
                                .as_str()
                                .to_string(),
                        )
                    };
                    *merge_base = Some((key, found.clone()));
                    found
                }
            };
            match found {
                Some(found) => (found, false),
                None => (head.as_str().to_string(), true),
            }
        }
    };

    // Git is given the work tree only once nothing in it leads git to a network or device path. On a big
    // checkout that look is most of a check, so a slow one's clear answer stands for 10 s while the index is the
    // same: a link made in that time is seen by the next look (PLAN §9, slice 6u decision 1).
    let tracked = tracked_paths(git, tracked, deadline, cancel)?;
    let checkout = git.checkout().to_path_buf();
    match looked(&checkout, &tracked, reached, deadline, cancel) {
        Reach::Clear => {}
        Reach::Refused => return Err(Halt::Err(ChangesError::Refused(SUBMODULE_NETWORK.to_string()))),
        Reach::OddName => return Err(Halt::Err(ChangesError::Refused(ODD_NAME.to_string()))),
        Reach::Late => return Err(Halt::Late),
    }

    let (raw, capped) = collect(
        git,
        &[
            "diff",
            "--raw",
            "-z",
            "-M",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=all",
            &from,
            "--",
        ],
        deadline,
        cancel,
        MAX_RAW,
    )?;
    let entries = raw_entries(&raw, capped)?;
    let stats: Vec<Option<FileStamp>> = entries
        .iter()
        .map(|e| {
            (e.status != DiffStatus::Deleted)
                .then(|| stat(&checkout.join(&e.path)))
                .flatten()
        })
        .collect();

    let (mut new_files, more_new, new_cut) = walk.update(&tracked.paths, |each| {
        walk_unignored(
            &mut Ignores::new(&checkout, &checkout),
            &checkout,
            deadline,
            cancel,
            &|rel| tracked.paths.contains(rel),
            each,
        )
    });
    // A file that is gone since the walk is not new any more.
    let new_stats: Vec<FileStamp> = {
        let mut kept = Vec::new();
        new_files.retain(|path| match stat(&checkout.join(path)) {
            Some(s) => {
                kept.push(s);
                true
            }
            None => false,
        });
        kept
    };

    let mut hasher = DefaultHasher::new();
    // Two checkouts in the same state are still two answers: the reply names its folder.
    target.checkout.hash(&mut hasher);
    head.branch.hash(&mut hasher);
    head.oid.as_ref().map(refs::Oid::as_str).hash(&mut hasher);
    base.as_ref()
        .map(|b| (b.label.as_str(), b.oid.as_str()))
        .hash(&mut hasher);
    from.hash(&mut hasher);
    unrelated.hash(&mut hasher);
    raw.hash(&mut hasher);
    stats.hash(&mut hasher);
    new_files.hash(&mut hasher);
    new_stats.hash(&mut hasher);
    more_new.hash(&mut hasher);
    new_cut.hash(&mut hasher);

    Ok(Check {
        stamp: hasher.finish(),
        head_oid: head.oid.as_ref().map(|oid| oid.as_str().to_string()),
        origin: git.origin().clone(),
        head,
        base,
        from,
        unrelated,
        entries,
        new_files,
        more_new,
        new_cut,
    })
}

/// Why an untracked file has no diff: it is over the cap, or binary.
fn untracked_note(dir: &Path, path: &str) -> Option<DiffNote> {
    let full = dir.join(path);
    if std::fs::metadata(&full).ok()?.len() > MAX_UNTRACKED {
        return Some(DiffNote::Large);
    }
    let mut sniff = Vec::new();
    std::fs::File::open(&full)
        .ok()?
        .take(BINARY_SNIFF as u64)
        .read_to_end(&mut sniff)
        .ok()?;
    sniff.contains(&0).then_some(DiffNote::Binary)
}

/// The full reply for a check: the patch, the new files' diffs, the notes and the counts. Runs under `work`.
fn build_reply(
    entry: &Entry,
    target: &Target,
    check: &Check,
    hide_whitespace: bool,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ChangesReply, Halt> {
    let git = entry
        .git
        .as_ref()
        .ok_or_else(|| failed("Changes lost track of this repository."))?;
    let checkout = git.checkout();

    // The patch.
    let mut patch = String::new();
    if !check.entries.is_empty() {
        let mut args = vec![
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=all",
            "-M",
            "--src-prefix=a/",
            "--dst-prefix=b/",
        ];
        if hide_whitespace {
            args.push("-w");
        }
        args.push(&check.from);
        args.push("--");
        let (bytes, _) = collect(git, &args, deadline, cancel, MAX_DIFF + 1)?;
        patch = String::from_utf8_lossy(&bytes).into_owned();
    }

    // Which files git calls binary, by the header of the section that says so.
    let headers: HashMap<String, usize> = check
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| (format!("{} b/{}", e.old_path.as_deref().unwrap_or(&e.path), e.path), i))
        .collect();
    let mut binary: HashSet<usize> = HashSet::new();
    // Where each section starts, and which file it is.
    let mut starts: Vec<(Option<usize>, usize)> = Vec::new();
    let mut pos = 0;
    for line in patch.split_inclusive('\n') {
        let bare = line.trim_end_matches('\n');
        if let Some(rest) = bare.strip_prefix("diff --git a/") {
            starts.push((headers.get(rest).copied(), pos));
        } else if bare.starts_with("Binary files ") && bare.ends_with(" differ") {
            binary.extend(starts.last().and_then(|(file, _)| *file));
        }
        pos += line.len();
    }
    // Git leaves a file out of `-w` output altogether when only whitespace changed in it. The list and the
    // patch keep one order, so such a file gets a section of just its header (unless the patch was cut).
    let present: HashSet<usize> = starts.iter().filter_map(|(file, _)| *file).collect();
    if present.len() < check.entries.len() && patch.len() <= MAX_DIFF {
        let header = |i: usize| {
            let e = &check.entries[i];
            format!(
                "diff --git a/{} b/{}\n",
                e.old_path.as_deref().unwrap_or(&e.path),
                e.path
            )
        };
        let mut whole = String::with_capacity(patch.len());
        let mut next = 0;
        for (k, (file, start)) in starts.iter().enumerate() {
            let end = starts.get(k + 1).map_or(patch.len(), |s| s.1);
            if let Some(file) = file.filter(|file| *file >= next) {
                (next..file).for_each(|i| whole.push_str(&header(i)));
                next = file + 1;
            }
            whole.push_str(&patch[*start..end]);
        }
        (next..check.entries.len()).for_each(|i| whole.push_str(&header(i)));
        patch = whole;
    }

    let mut truncated = false;
    if patch.len() > MAX_DIFF {
        cut_diff(&mut patch, MAX_DIFF);
        truncated = true;
    }

    let mut files: Vec<DiffFile> = check
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| DiffFile {
            path: e.path.clone(),
            old_path: e.old_path.clone(),
            status: e.status,
            note: binary.contains(&i).then_some(DiffNote::Binary),
        })
        .collect();
    for path in &check.new_files {
        // The sections go in the order of the list, and stop at the first that doesn't fit. A file past the
        // cut has no section and no note: that is how the panel tells it was left out.
        let mut note = None;
        if !truncated {
            let section = untracked_diff(checkout, path);
            if patch.len() + section.len() > MAX_DIFF {
                truncated = true;
            } else {
                patch.push_str(&section);
                note = untracked_note(checkout, path);
            }
        }
        files.push(DiffFile {
            path: path.clone(),
            old_path: None,
            status: DiffStatus::Untracked,
            note,
        });
    }

    // The commits ahead of the base, when the files are compared with where the branch left it.
    let mut ahead = 0;
    if let (Some(head), Some(_), false) = (&check.head.oid, &check.base, check.unrelated) {
        let range = format!("{}..{}", check.from, head.as_str());
        let (bytes, _) = collect(git, &["rev-list", "--count", &range], deadline, cancel, 64)?;
        ahead = String::from_utf8_lossy(&bytes).trim().parse::<u32>().unwrap_or(0);
    }

    let uncommitted = if !check.new_files.is_empty() || check.more_new > 0 {
        true
    } else if let Some(head) = &check.head.oid {
        let mut any = false;
        let stopped = git.stream(
            &[
                "diff",
                "--raw",
                "-z",
                "--no-ext-diff",
                "--no-textconv",
                "--ignore-submodules=all",
                head.as_str(),
                "--",
            ],
            deadline,
            cancel,
            |chunk| {
                any |= !chunk.is_empty();
                !any
            },
        )?;
        if matches!(stopped, Stopped::Deadline | Stopped::Cancelled) {
            return Err(Halt::Late);
        }
        any
    } else {
        !check.entries.is_empty()
    };

    Ok(ChangesReply {
        kind: ChangesKind::Listed,
        stamp: reply_stamp(check, hide_whitespace),
        folder: target.checkout.to_string_lossy().into_owned(),
        moved: target.moved,
        branch: check.head.branch.clone(),
        detached: match (&check.head.branch, &check.head.oid) {
            (None, Some(oid)) => Some(oid.short().to_string()),
            _ => None,
        },
        base: check.base.as_ref().map(|b| b.label.clone()),
        unrelated: check.unrelated,
        ahead,
        uncommitted,
        files,
        diff: patch,
        truncated,
        more_new: check.more_new,
        new_cut: check.new_cut,
        reason: None,
        head: check.head_oid.clone(),
        pr_base_unfetched: None,
    })
}

/// The reply's stamp: the check's, and whether whitespace is hidden (the patch differs).
pub fn reply_stamp(check: &Check, hide_whitespace: bool) -> String {
    format!("{:016x}{}", check.stamp, if hide_whitespace { "w" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitbox::git_vars;
    use marshell_protocol::api::{ChangesKind, ChangesReply, DiffNote, DiffStatus};
    use std::path::Path;
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

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

        fn target(&self) -> Target {
            Target {
                checkout: self.repo.clone(),
                moved: false,
                task_base: None,
            }
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

    /// A repository with three files and one commit on `main`; the commit's id.
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

    /// Stages and commits everything in the repository.
    fn commit_all(scratch: &Scratch, message: &str) -> String {
        let git = plain(scratch);
        git(&scratch.repo, &["add", "-A"]);
        git(&scratch.repo, &["commit", "-q", "-m", message]);
        git(&scratch.repo, &["rev-parse", "HEAD"])
    }

    fn far() -> Instant {
        Instant::now() + Duration::from_secs(60)
    }

    /// Not a test: prints what each part of a check costs on the checkout `MARSHELL_BENCH_CHECKOUT` names
    /// (`docs/research/2026-11-23-changes-big-checkout.md` says how to make one). Run it with
    /// `cargo test --release -p marshell-core -j 4 --lib timing_on_a_big_checkout -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn timing_on_a_big_checkout() {
        let Some(checkout) = std::env::var_os("MARSHELL_BENCH_CHECKOUT") else {
            return;
        };
        let checkout = crate::projects::canonical(Path::new(&checkout));
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let target = Target {
            checkout: checkout.clone(),
            moved: false,
            task_base: None,
        };
        let never = AtomicBool::new(false);
        let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;

        let mut entry = Entry::default();
        let mut from = String::new();
        for n in 1..=6 {
            let t = Instant::now();
            let check = compute(&home, &target, &mut entry, far(), &never);
            println!("check #{n}: {:.0} ms, ok={}", ms(t), check.is_ok());
            if let Ok(check) = check {
                from = check.from;
            }
        }
        let git = entry.git.as_ref().unwrap();
        let t = Instant::now();
        let tracked = tracked_paths(git, &mut None, far(), &never).unwrap();
        println!("ls-files and its parse: {:.0} ms, {} paths", ms(t), tracked.paths.len());
        for n in 1..=3 {
            let t = Instant::now();
            let reach = worktree::reach(&checkout, &tracked, far(), &never);
            println!("link safety check #{n}: {:.0} ms, {reach:?}", ms(t));
        }
        let t = Instant::now();
        let args = [
            "diff",
            "--raw",
            "-z",
            "-M",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=all",
            &from,
            "--",
        ];
        let (raw, _) = collect(git, &args, far(), &never, MAX_RAW).unwrap();
        println!("git diff --raw: {:.0} ms, {} bytes", ms(t), raw.len());
        let t = Instant::now();
        let mut new_files = 0usize;
        let stopped = walk_unignored(
            &mut Ignores::new(&checkout, &checkout),
            &checkout,
            far(),
            &never,
            &|rel| tracked.paths.contains(rel),
            &mut |_, _| {
                new_files += 1;
                true
            },
        );
        println!("look for new files: {:.0} ms, {new_files} new, {stopped:?}", ms(t));
    }

    fn check_of(cache: &ChangesCache, target: &Target) -> Arc<Check> {
        // Zero age: every call is a new check, whatever the clock says.
        cache
            .check_within(
                &target.checkout.parent().unwrap().join("home"),
                target,
                Duration::ZERO,
                far(),
                &AtomicBool::new(false),
            )
            .unwrap()
    }

    fn read_of(cache: &ChangesCache, scratch: &Scratch, hide: bool) -> ChangesReply {
        cache
            .read(&scratch.home, &scratch.target(), hide, far(), &AtomicBool::new(false))
            .unwrap()
    }

    fn paths(reply: &ChangesReply) -> Vec<(&str, DiffStatus)> {
        reply.files.iter().map(|f| (f.path.as_str(), f.status)).collect()
    }

    /// Git opens a submodule's `.git` on every work-tree diff, `--ignore-submodules=all` or not, so one that
    /// names a network or device path refuses the check before git is given the work tree.
    #[test]
    fn a_submodule_whose_git_file_names_a_network_path_is_refused() {
        let scratch = Scratch::new();
        let head = repo_with_commit(&scratch);
        let git = plain(&scratch);
        let entry = format!("160000,{head},vendor/lib");
        git(&scratch.repo, &["update-index", "--add", "--cacheinfo", &entry]);
        // A device path: harmless if git does follow it, and `refused` reads only its text.
        write(
            &scratch.repo.join("vendor").join("lib").join(".git"),
            "gitdir: //?/C:/marshell-no-such-folder\n",
        );
        write(&scratch.repo.join("a.txt"), "changed\n");
        let cache = ChangesCache::new();
        let err = cache
            .read(&scratch.home, &scratch.target(), false, far(), &AtomicBool::new(false))
            .unwrap_err();
        assert_eq!(err, ChangesError::Refused(SUBMODULE_NETWORK.to_string()));
    }

    /// A fast clear look is not kept and a refusal never is; a slow clear look stands, so a refused folder made
    /// after it is not seen, until the index gives a new set of paths.
    #[test]
    fn a_slow_clear_look_stands_until_the_index_changes() {
        let scratch = Scratch::new();
        let never = AtomicBool::new(false);
        let tracked = Arc::new(Tracked::parse(b"vendor/lib\0a.txt\0"));
        let dot_git = scratch.repo.join("vendor").join("lib").join(".git");
        let mut reached = cache::Reached::default();
        let look = |tracked: &Arc<Tracked>, reached: &mut cache::Reached| {
            looked(&scratch.repo, tracked, reached, far(), &never)
        };

        assert_eq!(look(&tracked, &mut reached), Reach::Clear);
        // A fast look is not kept: the refused folder is seen on the next check.
        write(&dot_git, "gitdir: //?/C:/marshell-no-such-folder\n");
        assert_eq!(look(&tracked, &mut reached), Reach::Refused);
        assert!(reached.of.is_none(), "a refusal is never kept");

        std::fs::remove_file(&dot_git).unwrap();
        assert_eq!(look(&tracked, &mut reached), Reach::Clear);
        reached.took = Duration::from_millis(251);
        write(&dot_git, "gitdir: //?/C:/marshell-no-such-folder\n");
        assert_eq!(look(&tracked, &mut reached), Reach::Clear, "a slow clear look stands");

        // A new index is a new set of paths: looked at again at once.
        let again = Arc::new(Tracked::parse(b"vendor/lib\0a.txt\0"));
        assert_eq!(look(&again, &mut reached), Reach::Refused);
        assert!(reached.of.is_none());
    }

    /// The same through a whole check: the paths stay one set between checks, and a changed index ends the reuse.
    #[test]
    fn a_check_reuses_a_slow_clear_look_only_while_the_index_is_the_same() {
        let scratch = Scratch::new();
        let head = repo_with_commit(&scratch);
        let git = plain(&scratch);
        let entry_line = format!("160000,{head},vendor/lib");
        git(&scratch.repo, &["update-index", "--add", "--cacheinfo", &entry_line]);
        let never = AtomicBool::new(false);
        let target = scratch.target();
        let mut entry = Entry::default();
        let refused = |entry: &mut Entry| {
            matches!(
                compute(&scratch.home, &target, entry, far(), &never),
                Err(Halt::Err(ChangesError::Refused(_)))
            )
        };

        assert!(!refused(&mut entry));
        let first = entry.tracked.as_ref().unwrap().1.clone();
        assert!(!refused(&mut entry));
        assert!(
            Arc::ptr_eq(&first, &entry.tracked.as_ref().unwrap().1),
            "one set of paths between checks"
        );

        // As if that look had been slow: the folder that turned up since is not seen yet.
        entry.reached.took = Duration::from_millis(251);
        write(
            &scratch.repo.join("vendor").join("lib").join(".git"),
            "gitdir: //?/C:/marshell-no-such-folder\n",
        );
        assert!(!refused(&mut entry), "a slow clear look stands");

        write(&scratch.repo.join("new.txt"), "new\n");
        git(&scratch.repo, &["add", "new.txt"]);
        assert!(refused(&mut entry), "a changed index is looked at again at once");
    }

    #[test]
    fn check_on_a_branch_lists_commits_and_uncommitted_against_origin_main() {
        let scratch = Scratch::new();
        let first = repo_with_commit(&scratch);
        let git = plain(&scratch);
        git(&scratch.repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(&scratch.repo, &["checkout", "-q", "-b", "feat/login"]);
        write(&scratch.repo.join("d.txt"), "four\n");
        commit_all(&scratch, "add d");
        write(&scratch.repo.join("b.txt"), "two!\n");
        commit_all(&scratch, "change b");
        write(&scratch.repo.join("a.txt"), "one!\n");
        write(&scratch.repo.join("e.txt"), "five\n");

        let cache = ChangesCache::new();
        let check = check_of(&cache, &scratch.target());
        assert_eq!(check.head.branch.as_deref(), Some("feat/login"));
        assert_eq!(check.base.as_ref().map(|b| b.label.as_str()), Some("origin/main"));
        assert_eq!(check.from, first);
        assert!(!check.unrelated);
        assert_eq!(check.new_files, ["e.txt"]);

        let reply = read_of(&cache, &scratch, false);
        assert_eq!(reply.kind, ChangesKind::Listed);
        assert_eq!(reply.branch.as_deref(), Some("feat/login"));
        assert_eq!(reply.detached, None);
        assert_eq!(reply.base.as_deref(), Some("origin/main"));
        assert_eq!(reply.ahead, 2);
        assert!(reply.uncommitted);
        assert_eq!(
            paths(&reply),
            [
                ("a.txt", DiffStatus::Modified),
                ("b.txt", DiffStatus::Modified),
                ("d.txt", DiffStatus::Added),
                ("e.txt", DiffStatus::Untracked),
            ]
        );
        assert!(reply.diff.contains("diff --git a/a.txt b/a.txt"));
        assert!(reply.diff.contains("diff --git a/e.txt b/e.txt\nnew file mode"));
        assert!(!reply.truncated);
        assert_eq!(reply.stamp, reply_stamp(&check, false));
        assert_eq!(reply.folder, scratch.repo.to_string_lossy());
    }

    #[test]
    fn check_on_main_shows_unpushed_and_uncommitted() {
        let scratch = Scratch::new();
        let first = repo_with_commit(&scratch);
        let git = plain(&scratch);
        git(&scratch.repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        write(&scratch.repo.join("b.txt"), "two!\n");
        commit_all(&scratch, "local");
        write(&scratch.repo.join("a.txt"), "one!\n");

        let cache = ChangesCache::new();
        let check = check_of(&cache, &scratch.target());
        assert_eq!(check.from, first);
        let reply = read_of(&cache, &scratch, false);
        assert_eq!(reply.branch.as_deref(), Some("main"));
        assert_eq!(reply.base.as_deref(), Some("origin/main"));
        assert_eq!(reply.ahead, 1);
        assert!(reply.uncommitted);
        assert_eq!(
            paths(&reply),
            [("a.txt", DiffStatus::Modified), ("b.txt", DiffStatus::Modified)]
        );
    }

    #[test]
    fn check_with_no_base_compares_with_head() {
        let scratch = Scratch::new();
        let git = plain(&scratch);
        git(&scratch.repo, &["init", "-q", "-b", "trunk"]);
        write(&scratch.repo.join("a.txt"), "one\n");
        let head = commit_all(&scratch, "first");
        write(&scratch.repo.join("a.txt"), "one!\n");

        let cache = ChangesCache::new();
        let check = check_of(&cache, &scratch.target());
        assert!(check.base.is_none());
        assert_eq!(check.from, head);
        assert!(!check.unrelated);
        let reply = read_of(&cache, &scratch, false);
        assert_eq!(reply.base, None);
        assert_eq!(reply.ahead, 0);
        assert!(reply.uncommitted);
        assert_eq!(paths(&reply), [("a.txt", DiffStatus::Modified)]);
    }

    #[test]
    fn a_check_carries_the_head_id_and_origin_and_the_stamp_ignores_origin() {
        let scratch = Scratch::new();
        let head = repo_with_commit(&scratch);
        let cache = ChangesCache::new();
        let before = check_of(&cache, &scratch.target());
        assert_eq!(before.head_oid.as_deref(), Some(head.as_str()));
        assert!(before.origin.urls.is_empty());

        let git = plain(&scratch);
        git(
            &scratch.repo,
            &["remote", "add", "origin", "https://github.com/octo-org/widgets.git"],
        );
        let after = check_of(&cache, &scratch.target());
        assert_eq!(
            after.origin.urls,
            vec!["https://github.com/octo-org/widgets.git".to_string()]
        );
        assert_eq!(before.stamp, after.stamp);

        let unborn = Scratch::new();
        plain(&unborn)(&unborn.repo, &["init", "-q", "-b", "main"]);
        assert_eq!(check_of(&ChangesCache::new(), &unborn.target()).head_oid, None);
    }

    #[test]
    fn unborn_repo_shows_every_staged_and_new_file_as_new() {
        let scratch = Scratch::new();
        let git = plain(&scratch);
        git(&scratch.repo, &["init", "-q", "-b", "main"]);
        write(&scratch.repo.join("a.txt"), "one\n");
        git(&scratch.repo, &["add", "a.txt"]);
        write(&scratch.repo.join("b.txt"), "two\n");

        let cache = ChangesCache::new();
        let check = check_of(&cache, &scratch.target());
        assert_eq!(check.head.oid, None);
        assert!(!check.unrelated);
        let reply = read_of(&cache, &scratch, false);
        assert_eq!(reply.branch.as_deref(), Some("main"));
        assert_eq!(reply.detached, None);
        assert_eq!(reply.ahead, 0);
        assert!(reply.uncommitted);
        assert_eq!(
            paths(&reply),
            [("a.txt", DiffStatus::Added), ("b.txt", DiffStatus::Untracked)]
        );
        assert!(reply.diff.contains("+one\n"));
        assert!(reply.diff.contains("+two\n"));
    }

    #[test]
    fn unrelated_histories_compare_with_head_and_say_so() {
        let scratch = Scratch::new();
        let head = repo_with_commit(&scratch);
        let git = plain(&scratch);
        git(&scratch.repo, &["checkout", "-q", "--orphan", "other"]);
        git(&scratch.repo, &["rm", "-q", "-rf", "."]);
        write(&scratch.repo.join("x.txt"), "x\n");
        commit_all(&scratch, "unrelated");
        git(&scratch.repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(&scratch.repo, &["checkout", "-q", "main"]);
        write(&scratch.repo.join("a.txt"), "one!\n");

        let cache = ChangesCache::new();
        let check = check_of(&cache, &scratch.target());
        assert!(check.unrelated);
        assert_eq!(check.from, head);
        let reply = read_of(&cache, &scratch, false);
        assert!(reply.unrelated);
        assert_eq!(reply.base.as_deref(), Some("origin/main"));
        assert_eq!(reply.ahead, 0);
        assert_eq!(paths(&reply), [("a.txt", DiffStatus::Modified)]);
    }

    #[test]
    fn stamp_moves_when_a_file_is_touched_and_not_otherwise() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        write(&scratch.repo.join("a.txt"), "1ne\n");
        write(&scratch.repo.join("new.txt"), "new\n");
        let cache = ChangesCache::new();
        let first = check_of(&cache, &scratch.target());
        let second = check_of(&cache, &scratch.target());
        assert_eq!(first.stamp, second.stamp);

        // The same bytes with a newer modified time: the stamp moves.
        for name in ["a.txt", "new.txt"] {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(scratch.repo.join(name))
                .unwrap();
            file.set_modified(std::time::SystemTime::now() + Duration::from_secs(5))
                .unwrap();
            let after = check_of(&cache, &scratch.target());
            assert_ne!(after.stamp, second.stamp, "{name}");
            let again = check_of(&cache, &scratch.target());
            assert_eq!(after.stamp, again.stamp, "{name}");
            // Back to the first state of this loop, for the next file.
            drop(file);
            let back = std::fs::OpenOptions::new()
                .write(true)
                .open(scratch.repo.join(name))
                .unwrap();
            back.set_modified(std::time::SystemTime::now() + Duration::from_secs(60))
                .unwrap();
        }
        let reply = reply_stamp(&first, true);
        assert_eq!(reply, format!("{:016x}w", first.stamp));
        assert_eq!(reply_stamp(&first, false).len(), 16);
    }

    #[test]
    fn two_checkouts_in_the_same_state_have_different_stamps() {
        // A shell that moves between two clean clones must get the new folder, not a Same reply for the old one.
        let scratch = Scratch::new();
        let git = plain(&scratch);
        let twin = scratch.root.join("twin");
        std::fs::create_dir(&twin).unwrap();
        git(&scratch.repo, &["init", "-q", "-b", "main"]);
        git(&twin, &["init", "-q", "-b", "main"]);
        let cache = ChangesCache::new();
        let here = check_of(&cache, &scratch.target());
        let there = check_of(
            &cache,
            &Target {
                checkout: twin,
                ..scratch.target()
            },
        );
        assert_ne!(here.stamp, there.stamp);
    }

    #[test]
    fn new_files_are_capped_at_200_and_counted() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        for n in 0..300 {
            write(&scratch.repo.join(format!("new/f{n:03}.txt")), "x\n");
        }
        let cache = ChangesCache::new();
        let check = check_of(&cache, &scratch.target());
        assert_eq!(check.new_files.len(), 200);
        assert_eq!(check.more_new, 100);
        assert_eq!(check.new_files[0], "new/f000.txt");
        assert_eq!(check.new_files[199], "new/f199.txt");
        let reply = read_of(&cache, &scratch, false);
        assert_eq!(reply.files.len(), 200);
        assert_eq!(reply.more_new, 100);
    }

    #[test]
    fn read_cuts_at_4_mib_on_a_file_boundary() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let big = |c: char| format!("{}\n", c.to_string().repeat(63)).repeat(16 * 1024);
        for n in 0..5 {
            write(&scratch.repo.join(format!("big{n}.txt")), &big('x'));
        }
        commit_all(&scratch, "big files");
        for n in 0..5 {
            write(&scratch.repo.join(format!("big{n}.txt")), &big('y'));
        }
        let cache = ChangesCache::new();
        let reply = read_of(&cache, &scratch, false);
        assert_eq!(reply.files.len(), 5);
        assert!(reply.truncated);
        assert!(reply.diff.len() <= crate::tasks::MAX_DIFF);
        assert!(reply.diff.ends_with('\n'));
        let sections = reply.diff.lines().filter(|l| l.starts_with("diff --git ")).count();
        assert!((1..5).contains(&sections), "{sections}");
        // The last section is whole: as many lines added as removed.
        let last = reply.diff.rsplit("\ndiff --git ").next().unwrap();
        let removed = last
            .lines()
            .filter(|l| l.starts_with('-') && !l.starts_with("---"))
            .count();
        let added = last
            .lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
            .count();
        assert_eq!(removed, 16 * 1024);
        assert_eq!(added, 16 * 1024);
    }

    #[test]
    fn read_notes_binary_and_large_new_files() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        std::fs::write(scratch.repo.join("tracked.bin"), [0u8, 1, 2, 3]).unwrap();
        commit_all(&scratch, "binary");
        std::fs::write(scratch.repo.join("tracked.bin"), [0u8, 9, 9, 9, 9]).unwrap();
        std::fs::write(scratch.repo.join("new.bin"), [1u8, 0, 2]).unwrap();
        write(&scratch.repo.join("big.txt"), &"line\n".repeat(60_000));
        write(&scratch.repo.join("small.txt"), "small\n");

        let cache = ChangesCache::new();
        let reply = read_of(&cache, &scratch, false);
        let note = |path: &str| reply.files.iter().find(|f| f.path == path).unwrap().note;
        assert_eq!(note("tracked.bin"), Some(DiffNote::Binary));
        assert_eq!(note("new.bin"), Some(DiffNote::Binary));
        assert_eq!(note("big.txt"), Some(DiffNote::Large));
        assert_eq!(note("small.txt"), None);
        assert!(
            reply
                .diff
                .contains("Binary files a/tracked.bin and b/tracked.bin differ")
        );
        assert!(reply.diff.contains("+small\n"));
        assert!(!reply.diff.contains("+line\n"));
    }

    /// The panel tells the files a cut leaves out by their having no section and no note, so a new file past
    /// the cut has no note either.
    #[test]
    fn a_new_file_past_the_cut_has_no_note() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        // 18 new files of 250,000 bytes each pass 4 MiB; `zz.bin` comes last and is over the new-file cap.
        let text = format!("{}\n", "x".repeat(99)).repeat(2_500);
        for i in 0..18 {
            write(&scratch.repo.join(format!("big{i:02}.txt")), &text);
        }
        write(&scratch.repo.join("zz.bin"), &"y".repeat(300_000));

        let cache = ChangesCache::new();
        let reply = read_of(&cache, &scratch, false);
        assert!(reply.truncated);
        let last = reply.files.last().unwrap();
        assert_eq!(last.path, "zz.bin");
        assert_eq!(last.note, None, "a file past the cut has a note");
        for f in reply.files.iter().filter(|f| f.note.is_some()) {
            let header = format!("diff --git a/{0} b/{0}\n", f.path);
            assert!(reply.diff.contains(&header), "{} has a note and no section", f.path);
        }
    }

    #[test]
    fn hide_whitespace_drops_whitespace_only_hunks() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        write(&scratch.repo.join("a.txt"), "one \n");
        write(&scratch.repo.join("b.txt"), "twenty\n");
        let cache = ChangesCache::new();
        let shown = read_of(&cache, &scratch, false);
        assert!(shown.diff.contains("diff --git a/a.txt b/a.txt"));
        assert!(shown.diff.contains("@@"));
        let hidden = read_of(&cache, &scratch, true);
        assert_eq!(hidden.files.len(), 2);
        let section = |text: &str, name: &str| -> String {
            let start = text.find(&format!("diff --git a/{name} ")).unwrap();
            let rest = &text[start + 1..];
            rest.find("\ndiff --git ")
                .map_or(&text[start..], |end| &text[start..start + 1 + end])
                .to_string()
        };
        let a = section(&hidden.diff, "a.txt");
        assert!(a.starts_with("diff --git a/a.txt b/a.txt"));
        assert!(!a.contains("@@"), "{a}");
        assert!(section(&hidden.diff, "b.txt").contains("@@"));
        assert_ne!(hidden.stamp, shown.stamp);
        assert!(hidden.stamp.ends_with('w'));
    }

    #[test]
    fn a_task_checkout_compares_with_its_base_branch() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let git = plain(&scratch);
        git(&scratch.repo, &["checkout", "-q", "-b", "dev"]);
        write(&scratch.repo.join("dev.txt"), "dev\n");
        let dev = commit_all(&scratch, "dev work");
        let task = scratch.root.join("task");
        git(
            &scratch.repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "marshell/t",
                &task.to_string_lossy(),
                "dev",
            ],
        );
        write(&task.join("task.txt"), "task\n");
        git(&task, &["add", "-A"]);
        git(&task, &["commit", "-q", "-m", "task work"]);
        write(&task.join("a.txt"), "one!\n");

        let target = Target {
            checkout: task.clone(),
            moved: true,
            task_base: Some("dev".to_string()),
        };
        let cache = ChangesCache::new();
        let reply = cache
            .read(&scratch.home, &target, false, far(), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(reply.base.as_deref(), Some("dev"));
        assert!(reply.moved);
        assert_eq!(reply.ahead, 1);
        assert_eq!(reply.branch.as_deref(), Some("marshell/t"));
        assert_eq!(
            paths(&reply),
            [("a.txt", DiffStatus::Modified), ("task.txt", DiffStatus::Added)]
        );
        let check = cache
            .check(&scratch.home, &target, far(), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(check.from, dev);
    }

    #[test]
    fn cache_reuses_a_check_younger_than_a_second() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        write(&scratch.repo.join("a.txt"), "one!\n");
        let cache = ChangesCache::new();
        let target = scratch.target();
        let cancel = AtomicBool::new(false);
        let first = cache.check(&scratch.home, &target, far(), &cancel).unwrap();
        let runs = |cache: &ChangesCache| {
            let slot = cache.slot(&target.checkout);
            let work = slot.work.lock().unwrap();
            work.git.as_ref().unwrap().runs()
        };
        let before = runs(&cache);
        assert!(before > 0);
        let second = cache.check(&scratch.home, &target, far(), &cancel).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(runs(&cache), before);
        // A different spelling of the same folder, on Windows, is the same slot.
        if cfg!(windows) {
            let upper = PathBuf::from(target.checkout.to_string_lossy().to_uppercase());
            assert!(Arc::ptr_eq(&cache.slot(&target.checkout), &cache.slot(&upper)));
        }
        // An older one is made again.
        let third = cache
            .check_within(&scratch.home, &target, Duration::ZERO, far(), &cancel)
            .unwrap();
        assert!(!Arc::ptr_eq(&first, &third));
        assert!(runs(&cache) > before);
    }

    #[test]
    fn the_cache_keeps_eight_checkouts() {
        let cache = ChangesCache::new();
        let first = cache.slot(Path::new("/c/0"));
        let second = cache.slot(Path::new("/c/1"));
        for n in 2..8 {
            cache.slot(Path::new(&format!("/c/{n}")));
        }
        // Using the first again makes it the newest, so the next one drops "/c/1".
        assert!(Arc::ptr_eq(&first, &cache.slot(Path::new("/c/0"))));
        cache.slot(Path::new("/c/8"));
        assert!(Arc::ptr_eq(&first, &cache.slot(Path::new("/c/0"))));
        assert_eq!(Arc::strong_count(&second), 1);
        assert!(!Arc::ptr_eq(&second, &cache.slot(Path::new("/c/1"))));
    }

    #[test]
    fn a_detached_head_compares_with_the_base() {
        let scratch = Scratch::new();
        let first = repo_with_commit(&scratch);
        let git = plain(&scratch);
        git(&scratch.repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        write(&scratch.repo.join("d.txt"), "four\n");
        let second = commit_all(&scratch, "second");
        write(&scratch.repo.join("e.txt"), "five\n");
        let third = commit_all(&scratch, "third");
        git(&scratch.repo, &["checkout", "-q", "--detach", &third]);

        let cache = ChangesCache::new();
        let check = check_of(&cache, &scratch.target());
        assert_eq!(check.head.branch, None);
        assert_eq!(check.from, first);
        let reply = read_of(&cache, &scratch, false);
        assert_eq!(reply.branch, None);
        assert_eq!(reply.detached.as_deref(), Some(&third[..7]));
        assert_eq!(reply.ahead, 2);
        assert!(!reply.uncommitted);
        assert_eq!(
            paths(&reply),
            [("d.txt", DiffStatus::Added), ("e.txt", DiffStatus::Added)]
        );
        let _ = second;
    }

    #[test]
    fn a_late_check_says_git_took_too_long() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let cache = ChangesCache::new();
        let cancel = AtomicBool::new(false);
        let late = Instant::now();
        assert_eq!(
            cache.check(&scratch.home, &scratch.target(), late, &cancel).err(),
            Some(ChangesError::Failed(
                "Git took too long to read the changes.".to_string()
            ))
        );
        assert_eq!(
            cache.read(&scratch.home, &scratch.target(), false, late, &cancel).err(),
            Some(ChangesError::Failed(
                "Git took too long to read the changes.".to_string()
            ))
        );
    }

    #[test]
    fn a_caller_waiting_on_the_lock_gets_the_last_check() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let cache = ChangesCache::new();
        let target = scratch.target();
        let cancel = AtomicBool::new(false);
        let last = cache.check(&scratch.home, &target, far(), &cancel).unwrap();
        let slot = cache.slot(&target.checkout);
        let held = slot.work.lock().unwrap();
        let started = Instant::now();
        let got = cache
            .check_within(
                &scratch.home,
                &target,
                Duration::ZERO,
                Instant::now() + Duration::from_millis(50),
                &cancel,
            )
            .unwrap();
        assert!(Arc::ptr_eq(&last, &got));
        assert!(started.elapsed() < Duration::from_secs(5));
        // A read has no old reply to give.
        let read = cache.read(
            &scratch.home,
            &target,
            false,
            Instant::now() + Duration::from_millis(50),
            &cancel,
        );
        assert_eq!(
            read.err(),
            Some(ChangesError::Failed(
                "Git took too long to read the changes.".to_string()
            ))
        );
        drop(held);
    }

    #[test]
    fn marks_answer_at_once_and_refresh_in_the_background() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        write(&scratch.repo.join("a.txt"), "one!\n");
        let cache = Arc::new(ChangesCache::new());
        let target = scratch.target();
        assert!(cache.marks(scratch.home.clone(), target.clone()).is_none());
        let started = Instant::now();
        let check = loop {
            if let Some(check) = cache.marks(scratch.home.clone(), target.clone()) {
                break check;
            }
            assert!(started.elapsed() < Duration::from_secs(30), "no check arrived");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(check.entries.len(), 1);
        assert_eq!(check.entries[0].path, "a.txt");
        // A fresh check answers again without a new run.
        let again = cache.marks(scratch.home.clone(), target.clone()).unwrap();
        assert!(Arc::ptr_eq(&check, &again));
        assert!(
            !cache
                .slot(&target.checkout)
                .running
                .load(std::sync::atomic::Ordering::SeqCst)
        );
    }

    fn walked(walk: &mut cache::Walk, files: &[&str], stopped: crate::git::Stopped) -> (Vec<String>, u32, bool) {
        walk.update(&Default::default(), |each| {
            for file in files {
                each(file, false);
            }
            stopped
        })
    }

    #[test]
    fn a_cut_walk_keeps_the_last_list_and_the_stamp() {
        use crate::git::Stopped;
        let mut walk = cache::Walk::default();
        // The first walk was cut: nothing known yet, and the reply says so.
        assert_eq!(walked(&mut walk, &["z.txt"], Stopped::Deadline), (vec![], 0, true));
        let full = walked(&mut walk, &["b.txt", "a.txt"], Stopped::Done);
        assert_eq!(full, (vec!["a.txt".to_string(), "b.txt".to_string()], 0, false));
        // A cut walk that saw other files, or fewer, leaves the list as it was, and says it was cut.
        let (list, more, _) = full.clone();
        assert_eq!(
            walked(&mut walk, &["c.txt"], Stopped::Deadline),
            (list.clone(), more, true)
        );
        assert_eq!(walked(&mut walk, &[], Stopped::Cancelled), (list, more, true));
        // The cap is on the list, and the rest is counted.
        let many: Vec<String> = (0..250).map(|n| format!("f{n:03}")).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        let mut walk = cache::Walk::default();
        let (list, more, cut) = walked(&mut walk, &many, Stopped::Done);
        assert_eq!((list.len(), more, cut), (200, 50, false));
    }

    /// `ls-files` reads the sandbox's copy of the index, so the cached paths are keyed by that copy: an index
    /// that changes after it was copied is read again once the next check copies it.
    #[test]
    fn the_tracked_paths_follow_the_index_git_read() {
        let scratch = Scratch::new();
        repo_with_commit(&scratch);
        let mut git = GitBox::open(&scratch.home, &scratch.repo).unwrap();
        git.refresh().unwrap();
        // `git add` lands between the copy and the read.
        write(&scratch.repo.join("d.txt"), "four\n");
        plain(&scratch)(&scratch.repo, &["add", "d.txt"]);
        let mut cache = None;
        let first = tracked_paths(&git, &mut cache, far(), &AtomicBool::new(false)).unwrap();
        assert!(!first.paths.contains("d.txt"));
        git.refresh().unwrap();
        let next = tracked_paths(&git, &mut cache, far(), &AtomicBool::new(false)).unwrap();
        assert!(next.paths.contains("d.txt"));
    }

    /// A list of changed files cut at its cap would show part of the changes as all of them.
    #[test]
    fn a_raw_diff_past_its_cap_is_refused() {
        let record = b":100644 100644 aaaa bbbb M\0a.txt\0";
        assert_eq!(raw_entries(record, false).map(|e| e.len()).ok(), Some(1));
        let Err(Halt::Err(ChangesError::Failed(why))) = raw_entries(record, true) else {
            panic!("a capped list is refused");
        };
        assert_eq!(why, "This checkout has too many changed files for Changes.");
    }

    /// A walk the deadline cuts is as slow as a finished one: it isn't run again on every check.
    #[test]
    fn a_slow_cut_walk_is_not_run_again_at_once() {
        use crate::git::Stopped;
        let mut walk = cache::Walk::default();
        let (_, _, cut) = walk.update(&Default::default(), |_| {
            std::thread::sleep(Duration::from_millis(300));
            Stopped::Deadline
        });
        assert!(cut);
        let mut ran = false;
        let (list, _, cut) = walk.update(&Default::default(), |_| {
            ran = true;
            Stopped::Done
        });
        assert!(!ran, "a walk that ran 300 ms before it was cut waits 10 s");
        assert_eq!((list, cut), (vec![], true));
    }

    #[test]
    fn a_slow_walk_is_not_repeated_for_ten_seconds() {
        use crate::git::Stopped;
        let mut walk = cache::Walk::default();
        walked(&mut walk, &["a.txt"], Stopped::Done);
        // A fast walk runs every time.
        assert_eq!(walked(&mut walk, &["b.txt"], Stopped::Done).0, ["b.txt"]);
        // A slow one is reused, less what has been tracked since.
        walk.took = Duration::from_millis(400);
        let mut tracked = std::collections::HashSet::new();
        tracked.insert("b.txt".to_string());
        walk.list = vec!["b.txt".to_string(), "c.txt".to_string()];
        walk.at = Some(Instant::now());
        let (list, _, _) = walk.update(&tracked, |_| panic!("the walk is reused"));
        assert_eq!(list, ["c.txt"]);
        // After ten seconds it runs again.
        walk.at = Some(Instant::now() - Duration::from_secs(11));
        assert_eq!(walked(&mut walk, &["d.txt"], Stopped::Done).0, ["d.txt"]);
    }
}
