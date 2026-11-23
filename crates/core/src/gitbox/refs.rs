//! Refs read from a repository's own files, never by running git.
//!
//! Git is run only on a copy of the index in a sandbox that reads no repository config (spike S15), so
//! Marshell finds HEAD, a branch and the main branch itself: a loose ref file first, then `packed-refs`.
//! Every read is capped: a single-ref file at 4 KiB, `packed-refs` line by line.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::server::{MAX_POINTER_BYTES, links_refused, read_capped};

/// The most symbolic refs followed in a row.
const MAX_HOPS: usize = 5;
/// The longest ref name that is read.
const MAX_NAME_BYTES: usize = 1024;
/// The longest `packed-refs` line that is read; longer ones are skipped.
const MAX_PACKED_LINE: usize = 4096;
/// The largest `packed-refs` file that is read.
const MAX_PACKED_BYTES: u64 = 256 * 1024 * 1024;

/// An object id: lowercase hex, exactly 40 (SHA-1) or 64 (SHA-256) characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Oid(String);

impl Oid {
    pub fn parse(s: &str) -> Option<Oid> {
        let hex = matches!(s.len(), 40 | 64) && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        hex.then(|| Oid(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first 7 characters.
    pub fn short(&self) -> &str {
        &self.0[..7]
    }
}

/// Where HEAD points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The branch HEAD is on, as `feat/login` (no `refs/heads/`); None when HEAD is detached.
    pub branch: Option<String>,
    /// The commit HEAD resolves to; None on an unborn branch.
    pub oid: Option<Oid>,
}

/// The branch a diff is taken against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    pub label: String,
    pub oid: Oid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefError {
    /// The repository stores its refs as reftable, which Marshell doesn't read.
    Reftable,
    Unreadable(&'static str),
}

/// Whether `name` is a ref name Marshell reads: `HEAD` or one under `refs/`, with no component that
/// is empty, starts with `.` or ends with `.lock`, and none of the characters git forbids.
pub(crate) fn valid_name(name: &str) -> bool {
    if name.len() > MAX_NAME_BYTES || !(name == "HEAD" || name.starts_with("refs/")) {
        return false;
    }
    if name.contains("..") || name.chars().any(|c| c.is_control() || "\\:?*[ ".contains(c)) {
        return false;
    }
    name.split('/')
        .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
}

/// Whether `name` lives in the worktree's own git dir rather than the common one.
fn per_worktree(name: &str) -> bool {
    name == "HEAD"
        || ["refs/bisect/", "refs/worktree/", "refs/rewritten/"]
            .iter()
            .any(|p| name.starts_with(p))
}

fn check_reftable(common: &Path) -> Result<(), RefError> {
    if common.join("reftable").is_dir() {
        Err(RefError::Reftable)
    } else {
        Ok(())
    }
}

/// The first line of the loose ref file `name` in `dir`, trimmed. None when there is no such regular
/// file, it is over 4 KiB, or a link on its way is refused.
fn loose(dir: &Path, name: &str) -> Option<String> {
    let mut path = dir.to_path_buf();
    path.extend(name.split('/'));
    if links_refused(&path) {
        return None;
    }
    let bytes = read_capped(&path, MAX_POINTER_BYTES)?;
    let text = String::from_utf8_lossy(&bytes);
    Some(text.lines().next().unwrap_or("").trim().to_string())
}

/// Reads up to one line, at most `MAX_PACKED_LINE` bytes of it, into `line`. Returns the bytes read.
fn read_line_capped(reader: &mut impl BufRead, line: &mut Vec<u8>) -> Result<usize, RefError> {
    line.clear();
    // One byte past the limit tells a line that fits (it ends in a newline) from one that doesn't.
    reader
        .by_ref()
        .take(MAX_PACKED_LINE as u64 + 1)
        .read_until(b'\n', line)
        .map_err(|_| RefError::Unreadable("packed-refs can't be read"))
}

/// The id in `<common>/packed-refs` for `name`. Lines over 4,096 bytes, `#` lines and `^` lines are
/// skipped, and the read stops at the first match.
fn packed(common: &Path, name: &str) -> Result<Option<Oid>, RefError> {
    let path = common.join("packed-refs");
    if links_refused(&path) {
        return Ok(None);
    }
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(RefError::Unreadable("packed-refs can't be read")),
    };
    let meta = file
        .metadata()
        .map_err(|_| RefError::Unreadable("packed-refs can't be read"))?;
    if !meta.is_file() {
        return Ok(None);
    }
    if meta.len() > MAX_PACKED_BYTES {
        return Err(RefError::Unreadable("packed-refs is over 256 MiB"));
    }
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    loop {
        let read = read_line_capped(&mut reader, &mut line)?;
        if read == 0 {
            return Ok(None);
        }
        if line.last() != Some(&b'\n') && read > MAX_PACKED_LINE {
            // Too long: drop the rest of it without keeping it.
            loop {
                let more = read_line_capped(&mut reader, &mut line)?;
                if more == 0 || line.last() == Some(&b'\n') {
                    break;
                }
            }
            continue;
        }
        let Ok(text) = std::str::from_utf8(&line) else {
            continue;
        };
        let text = text.trim_end_matches(['\r', '\n']);
        if text.starts_with(['#', '^']) {
            continue;
        }
        if let Some((id, found)) = text.split_once(' ')
            && found == name
        {
            return Ok(Oid::parse(id));
        }
    }
}

/// `name` followed through symbolic refs (at most 5) to an id. A name that isn't valid, a loop and a
/// ref that isn't there are all `None`.
fn resolve_in(git_dir: &Path, common: &Path, name: &str) -> Result<Option<Oid>, RefError> {
    let mut name = name.to_string();
    // The name asked for, then up to MAX_HOPS names it points on to.
    for _ in 0..=MAX_HOPS {
        if !valid_name(&name) {
            return Ok(None);
        }
        let own = per_worktree(&name);
        match loose(if own { git_dir } else { common }, &name) {
            Some(text) => match text.strip_prefix("ref:") {
                Some(target) => name = target.trim().to_string(),
                None => return Ok(Oid::parse(text.split_whitespace().next().unwrap_or(""))),
            },
            // Per-worktree refs are never packed.
            None if own => return Ok(None),
            None => return packed(common, &name),
        }
    }
    Ok(None)
}

/// HEAD: the branch it is on and the commit it resolves to.
pub fn head(git_dir: &Path, common: &Path) -> Result<Head, RefError> {
    check_reftable(common)?;
    let text = loose(git_dir, "HEAD").ok_or(RefError::Unreadable("HEAD can't be read"))?;
    if let Some(target) = text.strip_prefix("ref:") {
        let target = target.trim();
        if !valid_name(target) {
            return Err(RefError::Unreadable("HEAD names an invalid ref"));
        }
        return Ok(Head {
            branch: target.strip_prefix("refs/heads/").map(str::to_string),
            oid: resolve_in(git_dir, common, target)?,
        });
    }
    match Oid::parse(text.split_whitespace().next().unwrap_or("")) {
        Some(oid) => Ok(Head {
            branch: None,
            oid: Some(oid),
        }),
        None => Err(RefError::Unreadable("HEAD is neither a ref nor an id")),
    }
}

/// The commit the ref `name` (`refs/heads/main`, or `HEAD`) names, or None.
pub fn resolve(git_dir: &Path, common: &Path, name: &str) -> Result<Option<Oid>, RefError> {
    check_reftable(common)?;
    resolve_in(git_dir, common, name)
}

/// The branch a task's changes are measured against: the base of the open pull request, if its remote branch
/// is fetched; else the task's own base branch, else the remote's default, else `main` or `master`. The second
/// value is the pull request's base when it was given but isn't fetched here.
pub fn base(
    git_dir: &Path,
    common: &Path,
    task_base: Option<&str>,
    pr_base: Option<&str>,
) -> Result<(Option<Base>, Option<String>), RefError> {
    check_reftable(common)?;
    let found = |name: &str, label: String| -> Result<Option<Base>, RefError> {
        Ok(resolve_in(git_dir, common, name)?.map(|oid| Base { label, oid }))
    };
    let mut unfetched = None;
    if let Some(b) = pr_base.filter(|b| crate::github::valid_branch(b)) {
        // Only the remote branch: a local one of that name isn't what the pull request targets.
        if let Some(base) = found(&format!("refs/remotes/origin/{b}"), format!("origin/{b}"))? {
            return Ok((Some(base), None));
        }
        unfetched = Some(b.to_string());
    }
    let chosen = base_by_default(git_dir, common, task_base)?;
    Ok((chosen, unfetched))
}

fn base_by_default(git_dir: &Path, common: &Path, task_base: Option<&str>) -> Result<Option<Base>, RefError> {
    let found = |name: &str, label: String| -> Result<Option<Base>, RefError> {
        Ok(resolve_in(git_dir, common, name)?.map(|oid| Base { label, oid }))
    };
    if let Some(b) = task_base {
        if let Some(base) = found(&format!("refs/heads/{b}"), b.to_string())? {
            return Ok(Some(base));
        }
        if let Some(base) = found(&format!("refs/remotes/origin/{b}"), format!("origin/{b}"))? {
            return Ok(Some(base));
        }
    }
    if let Some(text) = loose(common, "refs/remotes/origin/HEAD")
        && let Some(target) = text.strip_prefix("ref:").map(str::trim)
        && let Some(x) = target.strip_prefix("refs/remotes/origin/")
        && !x.is_empty()
        && let Some(base) = found(target, format!("origin/{x}"))?
    {
        return Ok(Some(base));
    }
    for x in ["main", "master"] {
        if let Some(base) = found(&format!("refs/remotes/origin/{x}"), format!("origin/{x}"))? {
            return Ok(Some(base));
        }
    }
    for x in ["main", "master"] {
        if let Some(base) = found(&format!("refs/heads/{x}"), x.to_string())? {
            return Ok(Some(base));
        }
    }
    Ok(None)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{common_dir, git_dir};
    use std::path::PathBuf;

    const A: &str = "1111111111111111111111111111111111111111";
    const B: &str = "2222222222222222222222222222222222222222";
    const C: &str = "3333333333333333333333333333333333333333";

    fn oid(s: &str) -> Oid {
        Oid::parse(s).unwrap()
    }

    fn temp() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(tmp.path());
        (tmp, root)
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn head_on_a_branch_reads_the_loose_ref() {
        let (_tmp, dir) = temp();
        write(&dir.join("HEAD"), "ref: refs/heads/feat/login\n");
        write(&dir.join("refs/heads/feat/login"), &format!("{A}\n"));
        let head = head(&dir, &dir).unwrap();
        assert_eq!(head.branch.as_deref(), Some("feat/login"));
        assert_eq!(head.oid, Some(oid(A)));
        assert_eq!(head.oid.unwrap().short(), "1111111");
    }

    #[test]
    fn head_falls_back_to_packed_refs_and_skips_peeled_lines() {
        let (_tmp, dir) = temp();
        write(&dir.join("HEAD"), "ref: refs/heads/main\n");
        write(
            &dir.join("packed-refs"),
            &format!("# pack-refs with: peeled fully-peeled sorted \n{C} refs/tags/v1\n^{B}\n{A} refs/heads/main\n"),
        );
        let head = head(&dir, &dir).unwrap();
        assert_eq!(head.branch.as_deref(), Some("main"));
        assert_eq!(head.oid, Some(oid(A)));
        // A peeled line is never a ref, and a tag's own line is found.
        assert_eq!(resolve(&dir, &dir, "refs/tags/v1").unwrap(), Some(oid(C)));
        assert_eq!(resolve(&dir, &dir, &format!("^{B}")).unwrap(), None);
    }

    #[test]
    fn packed_refs_skips_a_line_over_4096_bytes_and_goes_on() {
        let (_tmp, dir) = temp();
        let long = "x".repeat(10_000);
        write(
            &dir.join("packed-refs"),
            &format!("{B} refs/heads/{long}\n{A} refs/heads/main\r\n"),
        );
        assert_eq!(resolve(&dir, &dir, "refs/heads/main").unwrap(), Some(oid(A)));
    }

    #[test]
    fn head_detached_has_no_branch() {
        let (_tmp, dir) = temp();
        write(&dir.join("HEAD"), &format!("{B}\n"));
        let head = head(&dir, &dir).unwrap();
        assert_eq!(head.branch, None);
        assert_eq!(head.oid, Some(oid(B)));
    }

    #[test]
    fn head_unborn_has_no_oid() {
        let (_tmp, dir) = temp();
        write(&dir.join("HEAD"), "ref: refs/heads/main\n");
        let head = head(&dir, &dir).unwrap();
        assert_eq!(head.branch.as_deref(), Some("main"));
        assert_eq!(head.oid, None);
    }

    #[test]
    fn worktree_head_lives_in_its_git_dir_and_branches_in_common() {
        let (_tmp, root) = temp();
        let common = root.join("main").join(".git");
        let linked = common.join("worktrees").join("wt");
        write(&linked.join("commondir"), "../..\n");
        write(&linked.join("HEAD"), "ref: refs/heads/wt-branch\n");
        write(&common.join("HEAD"), "ref: refs/heads/main\n");
        write(&common.join("refs/heads/main"), &format!("{A}\n"));
        write(&common.join("refs/heads/wt-branch"), &format!("{B}\n"));
        write(&linked.join("refs/bisect/bad"), &format!("{C}\n"));
        // The checkout's `.git` file points at the linked git dir, as `git worktree add` writes it.
        let checkout = root.join("wt");
        write(&checkout.join(".git"), &format!("gitdir: {}\n", linked.display()));

        let git = git_dir(&checkout).unwrap();
        let common_found = common_dir(&git);
        assert_eq!(common_found, linked.join("../.."));
        let head = head(&git, &common_found).unwrap();
        assert_eq!(head.branch.as_deref(), Some("wt-branch"));
        assert_eq!(head.oid, Some(oid(B)));
        // A branch lives in the common dir (not in the linked one) and `refs/bisect/` in the linked one.
        assert_eq!(resolve(&git, &common_found, "refs/heads/main").unwrap(), Some(oid(A)));
        assert_eq!(resolve(&git, &common_found, "refs/bisect/bad").unwrap(), Some(oid(C)));
        assert_eq!(resolve(&common, &common, "refs/bisect/bad").unwrap(), None);
        assert_eq!(resolve(&git, &common_found, "HEAD").unwrap(), Some(oid(B)));
    }

    #[test]
    fn base_prefers_origin_head_then_origin_main_then_local_main() {
        // origin/HEAD names origin/trunk, though origin/main and a local main exist.
        let (_tmp, dir) = temp();
        write(
            &dir.join("refs/remotes/origin/HEAD"),
            "ref: refs/remotes/origin/trunk\n",
        );
        write(&dir.join("refs/remotes/origin/trunk"), &format!("{A}\n"));
        write(&dir.join("refs/remotes/origin/main"), &format!("{B}\n"));
        write(&dir.join("refs/heads/main"), &format!("{C}\n"));
        let found = base(&dir, &dir, None, None).unwrap().0.unwrap();
        assert_eq!((found.label.as_str(), found.oid), ("origin/trunk", oid(A)));

        // No origin/HEAD: origin/main, then origin/master.
        let (_tmp2, dir) = temp();
        write(&dir.join("refs/remotes/origin/main"), &format!("{B}\n"));
        write(&dir.join("refs/heads/main"), &format!("{C}\n"));
        let found = base(&dir, &dir, None, None).unwrap().0.unwrap();
        assert_eq!((found.label.as_str(), found.oid), ("origin/main", oid(B)));
        let (_tmp3, dir) = temp();
        write(&dir.join("refs/remotes/origin/master"), &format!("{B}\n"));
        let found = base(&dir, &dir, None, None).unwrap().0.unwrap();
        assert_eq!(found.label, "origin/master");

        // Only local branches: main, then master.
        let (_tmp4, dir) = temp();
        write(&dir.join("refs/heads/main"), &format!("{C}\n"));
        write(&dir.join("refs/heads/master"), &format!("{A}\n"));
        let found = base(&dir, &dir, None, None).unwrap().0.unwrap();
        assert_eq!((found.label.as_str(), found.oid), ("main", oid(C)));
        let (_tmp5, dir) = temp();
        write(&dir.join("refs/heads/master"), &format!("{A}\n"));
        assert_eq!(base(&dir, &dir, None, None).unwrap().0.unwrap().label, "master");

        // An origin/HEAD that doesn't resolve is passed over.
        let (_tmp6, dir) = temp();
        write(&dir.join("refs/remotes/origin/HEAD"), "ref: refs/remotes/origin/gone\n");
        write(&dir.join("refs/remotes/origin/main"), &format!("{B}\n"));
        assert_eq!(base(&dir, &dir, None, None).unwrap().0.unwrap().label, "origin/main");

        // Nothing at all.
        let (_tmp7, dir) = temp();
        assert_eq!(base(&dir, &dir, None, None).unwrap().0, None);
    }

    #[test]
    fn base_for_a_task_uses_its_branch() {
        let (_tmp, dir) = temp();
        write(&dir.join("refs/heads/develop"), &format!("{A}\n"));
        write(&dir.join("refs/heads/main"), &format!("{C}\n"));
        let found = base(&dir, &dir, Some("develop"), None).unwrap().0.unwrap();
        assert_eq!((found.label.as_str(), found.oid), ("develop", oid(A)));

        // Without a local branch, the remote one; with neither, the usual order.
        let (_tmp2, dir) = temp();
        write(&dir.join("refs/remotes/origin/develop"), &format!("{B}\n"));
        write(&dir.join("refs/heads/main"), &format!("{C}\n"));
        let found = base(&dir, &dir, Some("develop"), None).unwrap().0.unwrap();
        assert_eq!((found.label.as_str(), found.oid), ("origin/develop", oid(B)));
        assert_eq!(base(&dir, &dir, Some("nope"), None).unwrap().0.unwrap().label, "main");
    }

    #[test]
    fn base_follows_an_open_pull_requests_fetched_base() {
        // The pull request's base is the remote branch, though a local one of that name and a task base exist.
        let (_tmp, dir) = temp();
        write(
            &dir.join("refs/remotes/origin/develop"),
            &format!(
                "{B}
"
            ),
        );
        write(
            &dir.join("refs/heads/develop"),
            &format!(
                "{A}
"
            ),
        );
        write(
            &dir.join("refs/heads/main"),
            &format!(
                "{C}
"
            ),
        );
        let (found, unfetched) = base(&dir, &dir, None, Some("develop")).unwrap();
        let found = found.unwrap();
        assert_eq!((found.label.as_str(), found.oid), ("origin/develop", oid(B)));
        assert_eq!(unfetched, None);
    }

    #[test]
    fn an_unfetched_pull_request_base_keeps_the_default_and_says_so() {
        // Only a local branch of that name: not the pull request's base.
        let (_tmp, dir) = temp();
        write(
            &dir.join("refs/heads/develop"),
            &format!(
                "{A}
"
            ),
        );
        write(
            &dir.join("refs/heads/main"),
            &format!(
                "{C}
"
            ),
        );
        let (found, unfetched) = base(&dir, &dir, None, Some("develop")).unwrap();
        assert_eq!(found.unwrap().label, "main");
        assert_eq!(unfetched.as_deref(), Some("develop"));

        // The task's base still comes next.
        let (found, unfetched) = base(&dir, &dir, Some("develop"), Some("gone")).unwrap();
        assert_eq!(found.unwrap().label, "develop");
        assert_eq!(unfetched.as_deref(), Some("gone"));

        // A name that is no branch is ignored.
        let (found, unfetched) = base(&dir, &dir, None, Some("-x")).unwrap();
        assert_eq!(found.unwrap().label, "main");
        assert_eq!(unfetched, None);
    }

    #[test]
    fn a_pull_request_base_comes_before_a_tasks_base() {
        let (_tmp, dir) = temp();
        write(
            &dir.join("refs/heads/task-base"),
            &format!(
                "{A}
"
            ),
        );
        write(
            &dir.join("refs/remotes/origin/develop"),
            &format!(
                "{B}
"
            ),
        );
        let (found, unfetched) = base(&dir, &dir, Some("task-base"), Some("develop")).unwrap();
        assert_eq!(found.unwrap().label, "origin/develop");
        assert_eq!(unfetched, None);
    }

    #[test]
    fn a_name_with_dotdot_or_lock_resolves_to_none() {
        let (_tmp, dir) = temp();
        write(&dir.join("x"), &format!("{A}\n"));
        write(&dir.join("refs/heads/a.lock"), &format!("{B}\n"));
        write(&dir.join("refs/heads/.hidden"), &format!("{B}\n"));
        write(&dir.join("refs/heads/ok"), &format!("{C}\n"));
        for name in [
            "refs/heads/../../x",
            "refs/heads/a.lock",
            "refs/heads/.hidden",
            "refs/heads/a b",
            "refs/heads/a:b",
            "refs/heads/a\\b",
            "refs/heads/a?",
            "refs/heads/a*",
            "refs/heads/a[",
            "refs/heads/a\u{7}",
            "refs/heads//ok",
            "heads/ok",
            "",
        ] {
            assert_eq!(resolve(&dir, &dir, name).unwrap(), None, "{name:?}");
        }
        assert_eq!(resolve(&dir, &dir, "refs/heads/ok").unwrap(), Some(oid(C)));
        let long = format!("refs/heads/{}", "a".repeat(1100));
        assert_eq!(resolve(&dir, &dir, &long).unwrap(), None);
    }

    #[test]
    fn symbolic_loops_stop_after_five_hops() {
        let (_tmp, dir) = temp();
        write(&dir.join("refs/heads/a"), "ref: refs/heads/b\n");
        write(&dir.join("refs/heads/b"), "ref: refs/heads/a\n");
        assert_eq!(resolve(&dir, &dir, "refs/heads/a").unwrap(), None);

        // A chain of five symbolic hops resolves; a sixth does not.
        for i in 0..6 {
            write(
                &dir.join(format!("refs/heads/c{i}")),
                &format!("ref: refs/heads/c{}\n", i + 1),
            );
        }
        write(&dir.join("refs/heads/c6"), &format!("{A}\n"));
        assert_eq!(resolve(&dir, &dir, "refs/heads/c1").unwrap(), Some(oid(A)));
        assert_eq!(resolve(&dir, &dir, "refs/heads/c0").unwrap(), None);
    }

    #[test]
    fn a_reftable_folder_is_refused() {
        let (_tmp, dir) = temp();
        write(&dir.join("HEAD"), "ref: refs/heads/.invalid\n");
        std::fs::create_dir(dir.join("reftable")).unwrap();
        assert_eq!(head(&dir, &dir), Err(RefError::Reftable));
        assert_eq!(base(&dir, &dir, None, None), Err(RefError::Reftable));
        assert_eq!(resolve(&dir, &dir, "refs/heads/main"), Err(RefError::Reftable));
    }

    #[test]
    fn sha256_ids_parse_and_others_do_not() {
        let sha256 = "a".repeat(64);
        assert_eq!(Oid::parse(&sha256).unwrap().as_str(), sha256);
        assert!(Oid::parse(A).is_some());
        assert!(Oid::parse(&"a".repeat(63)).is_none());
        assert!(Oid::parse(&"a".repeat(41)).is_none());
        assert!(Oid::parse(&"A".repeat(40)).is_none());
        assert!(Oid::parse(&"g".repeat(40)).is_none());
        assert!(Oid::parse("").is_none());
    }
}
