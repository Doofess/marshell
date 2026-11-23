//! GitHub, as the Changes panel's pull request bar knows it.

pub mod gh;
pub mod plan;
pub mod query;
pub mod remote;
pub mod status;
pub mod template;

use crate::changes::{Check, Halt, READ_BUDGET, collect};
use crate::gitbox::GitBox;
use crate::gitbox::refs::{self, Oid};
use gh::GhError;
use marshell_protocol::api::GithubKind;
use status::{NEXT_IDLE_MS, Snapshot};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::sync::watch;

/// How long the path of gh is kept before PATH is searched again.
pub const GH_PATH_AGE: Duration = Duration::from_secs(60);
/// How long the hosts gh is signed in to are kept.
pub const AUTH_AGE: Duration = Duration::from_secs(10 * 60);
/// A snapshot younger than this answers a fresh ask without asking GitHub.
pub const FRESH_GAP: Duration = Duration::from_secs(3);
/// The least time to wait before asking again after GitHub's rate limit.
pub const RATE_MIN_MS: u64 = 60_000;
/// The most outgoing commits a push plan lists.
pub const COMMITS_MAX: usize = 100;
/// The most (checkout, branch) keys kept; a key with an ask in flight is never dropped.
const MAX_KEYS: usize = 64;

/// Whether `b` is a branch name Marshell hands to git or GitHub: a valid ref under `refs/heads/` (which
/// `refs::valid_name` checks as a whole ref, never a bare branch), not starting with `-` (an option) or
/// `+` (a forced refspec), and holding nothing git reads as a revision (`~`, `^`, `@{`, a lone `@`), so
/// `refs/heads/<b>` on a push's left side can only ever name that branch.
pub fn valid_branch(b: &str) -> bool {
    !b.starts_with(['-', '+'])
        && !b.contains(['~', '^'])
        && !b.contains("@{")
        && b != "@"
        && !b.ends_with('.')
        && refs::valid_name(&format!("refs/heads/{b}"))
}

/// Whether a name may be written into a command a person or an agent may run:
/// `^[A-Za-z0-9._/][A-Za-z0-9._/-]*$`.
pub fn command_safe(name: &str) -> bool {
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/');
    let mut chars = name.chars();
    chars.next().is_some_and(plain) && chars.all(|c| plain(c) || c == '-')
}

/// `text` as one line of plain characters, for anything from GitHub or from git that Marshell shows: every control
/// character, and every character that hides or reorders text (soft hyphen, Arabic letter mark, zero-width and
/// direction marks, line and paragraph separators and embeddings, word joiners and isolates, the byte order mark,
/// interlinear annotation marks, and the tag characters), becomes one space, and the result is cut at `max`
/// characters (not bytes).
pub fn plain(text: &str, max: usize) -> String {
    let hides = |c: char| {
        matches!(
            c,
            '\u{ad}'
                | '\u{61c}'
                | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
                | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{e0001}'
                | '\u{e0020}'..='\u{e007f}'
        )
    };
    text.chars()
        .take(max)
        .map(|c| if c.is_control() || hides(c) { ' ' } else { c })
        .collect()
}

/// Why GitHub gave no status: the kind the bar shows, a sentence for it when GitHub or gh had one, and when to ask again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: GithubKind,
    pub reason: Option<String>,
    pub next_ms: u64,
}

/// What one ask of GitHub came to.
pub type Outcome = Result<Snapshot, Failure>;

/// The time to wait after the rate limit: until `reset_ms` (milliseconds since 1970) when gh said when, and at least
/// `RATE_MIN_MS`.
fn rate_next_ms(reset_ms: Option<u64>, now_ms: u64) -> u64 {
    reset_ms
        .map_or(0, |reset| reset.saturating_sub(now_ms))
        .max(RATE_MIN_MS)
}

fn failure(kind: GithubKind, reason: Option<String>, next_ms: u64) -> Failure {
    Failure { kind, reason, next_ms }
}

/// The failure a gh error is to the bar. A refusal, a call that took too long and an answer that can't be read are
/// all "GitHub can't be reached" with a sentence.
pub fn failure_of(error: GhError) -> Failure {
    let unreachable = |sentence: String| failure(GithubKind::Unreachable, Some(sentence), NEXT_IDLE_MS);
    match error {
        GhError::Missing => failure(GithubKind::NoGh, None, NEXT_IDLE_MS),
        GhError::SignedOut => failure(GithubKind::SignedOut, None, NEXT_IDLE_MS),
        GhError::NotFound => failure(GithubKind::NotFound, None, NEXT_IDLE_MS),
        GhError::RateLimited { reset_ms } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
            failure(GithubKind::RateLimited, None, rate_next_ms(reset_ms, now))
        }
        GhError::Unreachable(sentence) => unreachable(plain(&sentence, 300)),
        GhError::Refused { message, .. } => unreachable(plain(&message, 300)),
        GhError::TooLong => unreachable("GitHub took too long to answer.".to_string()),
        GhError::Unreadable => unreachable("GitHub's answer couldn't be read.".to_string()),
    }
}

/// `text` with every byte but `A-Z a-z 0-9 - . _ ~ /` as `%XX`.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The page that opens a pull request from `branch` of the fork owned by `owner` to the parent's default branch.
/// `fork_of` is GitHub's text for the parent: it is used only when it is exactly `owner/name`. None without a
/// parent, a parent default branch, or a parent that passes.
pub fn compare_url(
    host: &str,
    owner: &str,
    branch: &str,
    fork_of: Option<&str>,
    parent_default: Option<&str>,
) -> Option<String> {
    let parent = fork_of?;
    remote::slug(parent)?;
    Some(format!(
        "https://{host}/{parent}/compare/{}...{owner}:{}?expand=1",
        percent_encode(parent_default?),
        percent_encode(branch)
    ))
}

/// What one ask in flight brings the others waiting for it.
pub type Flight = watch::Receiver<Option<Outcome>>;

/// The ask for one (checkout, branch): what GitHub last said, the generation it was asked under, the ask in flight.
struct Key {
    checkout: PathBuf,
    branch: String,
    /// Raised by `mark_stale`: an answer asked for under an older one is not kept.
    generation: u64,
    snapshot: Option<(Instant, Snapshot)>,
    flight: Option<Flight>,
    /// (node id, number) of the pull request last reported for this branch.
    seen: Option<(String, u32)>,
}

#[derive(Default)]
struct Inner {
    keys: Vec<Key>,
    /// (checkout, (branch, base)) of the open pull request last seen at the checkout.
    pr_bases: Vec<(PathBuf, (String, String))>,
    /// The path of gh and when it was looked for (None: not found).
    gh: Option<(Instant, Option<PathBuf>)>,
    /// The hosts gh is signed in to and when it was asked.
    auth: Option<(Instant, Vec<String>)>,
    /// (checkout, push id) of the pushes that run. Nothing fills it yet.
    pushes: Vec<(PathBuf, String)>,
}

fn same(a: &Path, b: &Path) -> bool {
    crate::projects::same_path(&a.to_string_lossy(), &b.to_string_lossy())
}

impl Inner {
    fn position(&self, checkout: &Path, branch: &str) -> Option<usize> {
        self.keys
            .iter()
            .position(|k| k.branch == branch && same(&k.checkout, checkout))
    }

    /// The key's index, made when there is none (the oldest key without an ask in flight goes when there are too many).
    fn key(&mut self, checkout: &Path, branch: &str) -> usize {
        if let Some(at) = self.position(checkout, branch) {
            return at;
        }
        if self.keys.len() >= MAX_KEYS
            && let Some(at) = self.keys.iter().position(|k| k.flight.is_none())
        {
            self.keys.remove(at);
        }
        self.keys.push(Key {
            checkout: checkout.to_path_buf(),
            branch: branch.to_string(),
            generation: 0,
            snapshot: None,
            flight: None,
            seen: None,
        });
        self.keys.len() - 1
    }

    /// Drops what GitHub said for the key, with the pull request seen and the base kept for it. True when the
    /// checkout's pull request base was the key's, and is gone.
    fn drop_snapshot(&mut self, at: usize) -> bool {
        let key = &mut self.keys[at];
        key.snapshot = None;
        key.seen = None;
        let (checkout, branch) = (key.checkout.clone(), key.branch.clone());
        self.remove_pr_base(&checkout, &branch)
    }

    fn remove_pr_base(&mut self, checkout: &Path, branch: &str) -> bool {
        let before = self.pr_bases.len();
        self.pr_bases.retain(|(c, (b, _))| !(b == branch && same(c, checkout)));
        self.pr_bases.len() != before
    }
}

/// What GitHub said, by (checkout, branch), and what the core keeps around asking it: the path of gh, the hosts it is
/// signed in to, and the pushes that run. Its lock is never held across a call to gh or to git, or across an `.await`.
#[derive(Default)]
pub struct GithubState {
    /// gh's path in a test: the PATH is not searched.
    fixed_gh: Option<PathBuf>,
    inner: Mutex<Inner>,
}

/// How an ask for a status goes on.
pub enum Take {
    /// The snapshot kept for the key answers.
    Stored(Snapshot),
    /// Another ask for the key is in flight: wait for it.
    Wait(Flight),
    /// This ask goes to GitHub; the others for the key wait for it.
    Lead(Lead),
}

impl GithubState {
    /// gh at `path`, for a test: it is used without a search of PATH.
    pub fn with_gh(mut self, path: PathBuf) -> Self {
        self.fixed_gh = Some(path);
        self
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// (branch, base) of the open pull request last seen at `checkout`.
    pub fn pr_base(&self, checkout: &Path) -> Option<(String, String)> {
        self.lock()
            .pr_bases
            .iter()
            .find(|(c, _)| same(c, checkout))
            .map(|(_, pair)| pair.clone())
    }

    /// After a push, a create, a merge or a pull action at `checkout`: what GitHub said for each of its branches is no
    /// longer kept, and an ask that is in flight for one brings an answer that is not kept either.
    pub fn mark_stale(&self, checkout: &Path) {
        for key in self.lock().keys.iter_mut().filter(|k| same(&k.checkout, checkout)) {
            key.generation += 1;
            key.snapshot = None;
        }
    }

    /// (node id, number) of the pull request last reported for `branch` at `checkout`.
    pub fn pull_seen(&self, checkout: &Path, branch: &str) -> Option<(String, u32)> {
        let inner = self.lock();
        inner
            .position(checkout, branch)
            .and_then(|at| inner.keys[at].seen.clone())
    }

    /// The id of the push that runs at `checkout`.
    pub(crate) fn running_push(&self, checkout: &Path) -> Option<String> {
        self.lock()
            .pushes
            .iter()
            .find(|(c, _)| same(c, checkout))
            .map(|(_, id)| id.clone())
    }

    /// Notes what the answer showed for `branch` at `checkout`: the pull request seen (node id, number) and the base of
    /// its open one. A branch without an open pull request has no base noted. True when the base noted for the
    /// checkout changed, which is when the Changes panel must compare with another one.
    pub(crate) fn note_pull(
        &self,
        checkout: &Path,
        branch: &str,
        seen: Option<(String, u32)>,
        open_base: Option<&str>,
    ) -> bool {
        let mut inner = self.lock();
        let at = inner.key(checkout, branch);
        inner.keys[at].seen = seen;
        match open_base {
            Some(base) => {
                let wanted = (branch.to_string(), base.to_string());
                match inner.pr_bases.iter_mut().find(|(c, _)| same(c, checkout)) {
                    Some((_, have)) if *have == wanted => false,
                    Some((_, have)) => {
                        *have = wanted;
                        true
                    }
                    None => {
                        inner.pr_bases.push((checkout.to_path_buf(), wanted));
                        true
                    }
                }
            }
            None => inner.remove_pr_base(checkout, branch),
        }
    }

    /// The path of gh: the test's, else the last search of PATH when it is under `GH_PATH_AGE` old, else `find`.
    pub(crate) fn gh_path(&self, find: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
        if let Some(fixed) = &self.fixed_gh {
            return Some(fixed.clone());
        }
        let cached = self.lock().gh.clone();
        if let Some((at, found)) = cached
            && at.elapsed() < GH_PATH_AGE
        {
            return found;
        }
        let found = find();
        self.lock().gh = Some((Instant::now(), found.clone()));
        found
    }

    /// gh was not there when it was run: the next ask searches PATH again.
    pub(crate) fn forget_gh_path(&self) {
        self.lock().gh = None;
    }

    /// The hosts gh is signed in to: the last answer when it is under `AUTH_AGE` old, else `ask`'s (kept when it is one).
    pub(crate) fn auth_hosts(
        &self,
        ask: impl FnOnce() -> Result<Vec<String>, GhError>,
    ) -> Result<Vec<String>, GhError> {
        let cached = self.lock().auth.clone();
        if let Some((at, hosts)) = cached
            && at.elapsed() < AUTH_AGE
        {
            return Ok(hosts);
        }
        let hosts = ask()?;
        self.lock().auth = Some((Instant::now(), hosts.clone()));
        Ok(hosts)
    }

    /// How the ask for `branch` at `checkout` goes on. `fresh: false` answers from the snapshot kept, and asks
    /// GitHub only when there is none; `fresh: true` does when the snapshot is `FRESH_GAP` old. One ask runs per key:
    /// the others wait for it.
    pub fn take(self: &Arc<Self>, checkout: &Path, branch: &str, fresh: bool) -> Take {
        let mut inner = self.lock();
        let at = inner.key(checkout, branch);
        let key = &mut inner.keys[at];
        if let Some((asked, snapshot)) = &key.snapshot
            && (!fresh || asked.elapsed() < FRESH_GAP)
        {
            return Take::Stored(snapshot.clone());
        }
        if let Some(flight) = &key.flight {
            return Take::Wait(flight.clone());
        }
        let (tx, rx) = watch::channel(None);
        key.flight = Some(rx);
        Take::Lead(Lead {
            state: self.clone(),
            checkout: checkout.to_path_buf(),
            branch: branch.to_string(),
            tx: Some(tx),
            generation: key.generation,
        })
    }
}

/// The right to ask GitHub for one key, and the duty to answer the asks waiting for it. When it is dropped without
/// having answered (the task failed, panicked or never ran), they get an `Unreachable` answer, and the key is free.
pub struct Lead {
    state: Arc<GithubState>,
    checkout: PathBuf,
    branch: String,
    tx: Option<watch::Sender<Option<Outcome>>>,
    generation: u64,
}

impl Lead {
    /// The receiver of the answer, for the asker that leads.
    pub fn flight(&self) -> Flight {
        self.tx.as_ref().expect("a lead that has not answered").subscribe()
    }

    /// Asks with `ask`. An answer that comes back under a later generation than it was asked under is dropped, and
    /// `ask` runs once more; what that second run brings is given to those waiting, and kept when the generation still
    /// stands. An error drops the snapshot, and `forget` is called with the checkout when that took the base of its
    /// pull request away. Blocking, and no lock is held while `ask` runs.
    pub fn run(mut self, ask: impl Fn() -> Outcome, forget: impl Fn(&Path)) {
        let mut generation = self.generation;
        for attempt in 0..2 {
            let out = ask();
            if matches!(&out, Err(f) if f.kind == GithubKind::NoGh) {
                self.state.forget_gh_path();
            }
            let mut inner = self.state.lock();
            let at = inner.position(&self.checkout, &self.branch);
            let current = at.is_none_or(|at| inner.keys[at].generation == generation);
            if !current && attempt == 0 {
                generation = at.map_or(generation, |at| inner.keys[at].generation);
                continue;
            }
            let mut base_gone = false;
            if let Some(at) = at {
                match &out {
                    Ok(snapshot) if current => inner.keys[at].snapshot = Some((Instant::now(), snapshot.clone())),
                    Ok(_) => {}
                    Err(_) => base_gone = inner.drop_snapshot(at),
                }
                inner.keys[at].flight = None;
            }
            drop(inner);
            if base_gone {
                forget(&self.checkout);
            }
            if let Some(tx) = self.tx.take() {
                tx.send_replace(Some(out));
            }
            return;
        }
    }
}

impl Drop for Lead {
    fn drop(&mut self) {
        let Some(tx) = self.tx.take() else {
            return;
        };
        {
            let mut inner = self.state.lock();
            if let Some(at) = inner.position(&self.checkout, &self.branch) {
                inner.keys[at].flight = None;
                inner.drop_snapshot(at);
            }
        }
        tx.send_replace(Some(Err(failure(
            GithubKind::Unreachable,
            Some("GitHub's answer didn't arrive.".to_string()),
            NEXT_IDLE_MS,
        ))));
    }
}

/// What the checkout holds of a branch, set against what GitHub has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local {
    /// GitHub has the branch.
    pub on_github: bool,
    /// The commits here that GitHub's branch lacks; for a branch that isn't on GitHub, the ones past its base.
    pub to_push: u32,
    /// GitHub's branch holds a commit this checkout lacks.
    pub github_ahead: bool,
}

/// How many commits `to` holds that `from` doesn't. Both are commit ids.
fn count(git: &GitBox, from: &Oid, to: &Oid) -> Result<u32, Halt> {
    let range = format!("{}..{}", from.as_str(), to.as_str());
    let cancel = AtomicBool::new(false);
    let (bytes, _) = collect(
        git,
        &["rev-list", "--count", &range],
        Instant::now() + READ_BUDGET,
        &cancel,
        64,
    )?;
    Ok(String::from_utf8_lossy(&bytes).trim().parse().unwrap_or(0))
}

/// Whether the commit `oid` is in this repository.
pub(crate) fn has_commit(git: &GitBox, oid: &Oid) -> Result<bool, Halt> {
    let object = format!("{}^{{commit}}", oid.as_str());
    let cancel = AtomicBool::new(false);
    let (bytes, _) = collect(
        git,
        &["rev-parse", "--verify", "--quiet", &object],
        Instant::now() + READ_BUDGET,
        &cancel,
        256,
    )?;
    Ok(!bytes.iter().all(u8::is_ascii_whitespace))
}

/// Whether `commit` is in the history of the checkout's `HEAD`: it is here, and `HEAD` holds all of it.
pub(crate) fn in_history(git: &GitBox, check: &Check, commit: &str) -> Result<bool, Halt> {
    let (Some(head), Some(commit)) = (check.head_oid.as_deref().and_then(Oid::parse), Oid::parse(commit)) else {
        return Ok(false);
    };
    Ok(has_commit(git, &commit)? && count(git, &head, &commit)? == 0)
}

/// The commits to push, and whether GitHub is ahead, from commit ids only (the sandbox has no refs): `branch_oid` is
/// GitHub's commit at the branch (None when it has none), `base_oid` the commit a branch that isn't on GitHub is
/// counted from (None counts none). When GitHub's commit isn't here, GitHub is ahead and nothing can be counted.
pub(crate) fn local_counts(
    git: &GitBox,
    check: &Check,
    branch_oid: Option<&str>,
    base_oid: Option<&str>,
) -> Result<Local, Halt> {
    let branch = branch_oid.and_then(Oid::parse);
    let Some(head) = check.head_oid.as_deref().and_then(Oid::parse) else {
        // A branch with no commit yet has nothing to push, and everything GitHub has is ahead of it.
        return Ok(Local {
            on_github: branch.is_some(),
            to_push: 0,
            github_ahead: branch.is_some(),
        });
    };
    let Some(branch) = branch else {
        let to_push = match base_oid.and_then(Oid::parse) {
            Some(base) => count(git, &base, &head)?,
            None => 0,
        };
        return Ok(Local {
            on_github: false,
            to_push,
            github_ahead: false,
        });
    };
    if !has_commit(git, &branch)? {
        return Ok(Local {
            on_github: true,
            to_push: 0,
            github_ahead: true,
        });
    }
    Ok(Local {
        on_github: true,
        to_push: count(git, &branch, &head)?,
        github_ahead: count(git, &head, &branch)? > 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_branch_is_percent_encoded_in_a_compare_link() {
        assert_eq!(percent_encode("fix/#12"), "fix/%2312");
        assert_eq!(percent_encode("a-b.c_d~e/f"), "a-b.c_d~e/f");
        assert_eq!(percent_encode("a b?c&d=\u{e9}"), "a%20b%3Fc%26d%3D%C3%A9");
    }

    #[test]
    fn a_compare_link_needs_a_parent_that_is_owner_slash_name() {
        let link =
            |fork: Option<&str>, default: Option<&str>| compare_url("github.com", "me", "fix/#12", fork, default);
        assert_eq!(
            link(Some("up-org/widgets"), Some("trunk")).as_deref(),
            Some("https://github.com/up-org/widgets/compare/trunk...me:fix/%2312?expand=1")
        );
        assert_eq!(link(None, Some("trunk")), None);
        assert_eq!(link(Some("up-org/widgets"), None), None);
        for bad in [
            "up-org",
            "up-org/widgets/x",
            "up-org/widgets.git",
            "up org/widgets",
            "../widgets",
            "a/b/",
        ] {
            assert_eq!(link(Some(bad), Some("trunk")), None, "{bad:?}");
        }
    }

    #[test]
    fn the_rate_limit_waits_until_the_reset_and_at_least_a_minute() {
        assert_eq!(rate_next_ms(None, 1_000), RATE_MIN_MS);
        assert_eq!(rate_next_ms(Some(1_500), 1_000), RATE_MIN_MS);
        assert_eq!(rate_next_ms(Some(401_000), 1_000), 400_000);
        assert_eq!(rate_next_ms(Some(10), 1_000), RATE_MIN_MS);
    }

    #[test]
    fn a_gh_error_is_the_kind_of_its_name_or_unreachable_with_a_sentence() {
        let kind = |e: GhError| failure_of(e).kind;
        assert_eq!(kind(GhError::Missing), GithubKind::NoGh);
        assert_eq!(kind(GhError::SignedOut), GithubKind::SignedOut);
        assert_eq!(kind(GhError::NotFound), GithubKind::NotFound);
        assert_eq!(kind(GhError::RateLimited { reset_ms: None }), GithubKind::RateLimited);
        assert_eq!(failure_of(GhError::RateLimited { reset_ms: None }).next_ms, RATE_MIN_MS);
        let refused = failure_of(GhError::Refused {
            status: Some(422),
            message: "Validation\nFailed".into(),
        });
        assert_eq!(refused.kind, GithubKind::Unreachable);
        assert_eq!(refused.reason.as_deref(), Some("Validation Failed"));
        assert_eq!(
            failure_of(GhError::TooLong).reason.as_deref(),
            Some("GitHub took too long to answer.")
        );
        assert_eq!(
            failure_of(GhError::Unreadable).reason.as_deref(),
            Some("GitHub's answer couldn't be read.")
        );
    }

    /// An ask whose answer arrives under a later generation is dropped and asked again once; a waiter gets the
    /// second answer; and a lead dropped unanswered frees the key and tells the waiters.
    #[test]
    fn an_answer_under_an_older_generation_is_asked_again_and_a_lost_lead_answers_the_waiters() {
        use std::cell::Cell;
        let state = Arc::new(GithubState::default());
        let checkout = Path::new("/r");
        let snap = |next_ms| Snapshot {
            default_branch: None,
            fork_of: None,
            parent_default: None,
            branch_oid: None,
            pulls: Vec::new(),
            can_write: false,
            merge_methods: Vec::new(),
            auto_merge_allowed: false,
            next_ms,
        };
        let Take::Lead(lead) = state.take(checkout, "b", true) else {
            panic!("the first ask leads");
        };
        let mut waiting = match state.take(checkout, "b", true) {
            Take::Wait(rx) => rx,
            _ => panic!("the second ask waits"),
        };
        let runs = Cell::new(0);
        lead.run(
            || {
                runs.set(runs.get() + 1);
                if runs.get() == 1 {
                    state.mark_stale(checkout);
                }
                Ok(snap(runs.get()))
            },
            |_| {},
        );
        assert_eq!(runs.get(), 2);
        let got = waiting.borrow_and_update().clone().expect("answered").unwrap();
        assert_eq!(got.next_ms, 2);
        let Take::Stored(kept) = state.take(checkout, "b", false) else {
            panic!("the second answer is kept");
        };
        assert_eq!(kept.next_ms, 2);

        // A lead that never answers.
        state.mark_stale(checkout);
        let Take::Lead(lead) = state.take(checkout, "b", true) else {
            panic!("no snapshot, no flight: leads");
        };
        let Take::Wait(mut waiting) = state.take(checkout, "b", true) else {
            panic!("waits");
        };
        drop(lead);
        let lost = waiting.borrow_and_update().clone().expect("answered").unwrap_err();
        assert_eq!(lost.kind, GithubKind::Unreachable);
        assert!(
            matches!(state.take(checkout, "b", true), Take::Lead(_)),
            "the key is free"
        );
    }

    #[test]
    fn valid_branch_refuses_a_leading_dash_or_plus_and_bad_ref_names() {
        for bad in [
            "-x", "+main", "a..b", "a b", "", "a/", "/a", "a//b", ".a", "a.lock", "a:b",
        ] {
            assert!(!valid_branch(bad), "{bad:?}");
        }
        for good in ["feat/login", "v1.2", "main", "release/1.2-rc"] {
            assert!(valid_branch(good), "{good:?}");
        }
    }

    /// `refs/heads/a~1` on a push's left side would name the commit before `a`, not a branch.
    #[test]
    fn valid_branch_refuses_what_git_reads_as_a_revision() {
        for bad in ["a~1", "a^", "a^{commit}", "a@{1}", "@", "a@{upstream}"] {
            assert!(!valid_branch(bad), "{bad:?}");
        }
        assert!(valid_branch("user@feature"));
    }

    #[test]
    fn command_safe_refuses_shell_characters() {
        for bad in [
            "main;x", "x$(y)", "a|b", "a&b", "a'b", "a>b", "-x", "", "a b", "a\"b", "a`b", "a\nb",
        ] {
            assert!(!command_safe(bad), "{bad:?}");
        }
        for good in ["release/1.2", "main", "v1.2_x", ".hidden", "/abs", "a-b"] {
            assert!(command_safe(good), "{good:?}");
        }
    }
}
