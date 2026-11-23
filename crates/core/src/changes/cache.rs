//! What Changes keeps per checkout: the sandboxed git, the last check and what makes the next one cheap.
//!
//! The outer lock is held only to find or make a slot, and `last` only to read or swap it. `work` is the one
//! lock held across git runs; it serializes the runs for one checkout. The tree never waits on it: it asks
//! `marks`, which answers from the last check and refreshes it in the background.

use super::{CHECK_BUDGET, ChangesError, Check, Halt, MAX_NEW_FILES, TOO_LONG, Target};
use crate::git::Stopped;
use crate::gitbox::GitBox;
use crate::gitbox::worktree::Tracked;
use marshell_protocol::api::ChangesReply;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant, SystemTime};

/// A check younger than this is reused.
pub const CHECK_FRESH: Duration = Duration::from_secs(1);
/// The most checkouts kept; the least recently used one is dropped.
const MAX_SLOTS: usize = 8;
/// A walk that took longer than this runs at most every `SLOW_WALK_EVERY`.
const SLOW_WALK: Duration = Duration::from_millis(250);
const SLOW_WALK_EVERY: Duration = Duration::from_secs(10);
/// How long a caller waiting for `work` sleeps between tries.
const LOCK_TICK: Duration = Duration::from_millis(10);

/// A file's length and modified time.
pub(super) type FileStamp = (u64, Option<SystemTime>);

/// The walk for untracked files: the last full list, when it ran and how long it took.
#[derive(Default)]
pub(super) struct Walk {
    /// The first `MAX_NEW_FILES` untracked files, sorted.
    pub list: Vec<String>,
    /// How many more there were.
    pub more: u32,
    pub at: Option<Instant>,
    pub took: Duration,
    /// The last walk was cut, so `list` may be missing files.
    pub cut: bool,
}

impl Walk {
    /// The untracked files, how many more there are, and whether the last walk was cut. `run` walks the
    /// checkout, handing each file and whether it is a link to the closure it is given; it says how the walk
    /// ended. A walk that was cut (deadline or cancel) keeps the last full list, so a cut never moves the
    /// list. When the last walk was slow, finished or cut, it isn't repeated for 10 s.
    pub(super) fn update(
        &mut self,
        tracked: &HashSet<String>,
        run: impl FnOnce(&mut dyn FnMut(&str, bool) -> bool) -> Stopped,
    ) -> (Vec<String>, u32, bool) {
        let reuse = self.took > SLOW_WALK && self.at.is_some_and(|at| at.elapsed() < SLOW_WALK_EVERY);
        if !reuse {
            let started = Instant::now();
            let mut found: Vec<String> = Vec::new();
            let stopped = run(&mut |rel, link| {
                if !link {
                    found.push(rel.to_string());
                }
                true
            });
            if stopped == Stopped::Done {
                found.sort_unstable();
                self.more = u32::try_from(found.len().saturating_sub(MAX_NEW_FILES)).unwrap_or(u32::MAX);
                found.truncate(MAX_NEW_FILES);
                self.list = found;
            }
            self.cut = stopped != Stopped::Done;
            self.at = Some(started);
            self.took = started.elapsed();
        }
        // What was tracked since the list was made is not new any more.
        let list = self
            .list
            .iter()
            .filter(|path| !tracked.contains(*path))
            .cloned()
            .collect();
        (list, self.more, self.cut)
    }
}

/// Everything one checkout's git work keeps between checks; held through `Slot::work`.
#[derive(Default)]
pub(super) struct Entry {
    pub git: Option<GitBox>,
    /// `(head, base)` → their merge base, None when the histories share none.
    pub merge_base: Option<((String, String), Option<String>)>,
    /// The tracked paths, with the index file's stamp they were read at.
    pub tracked: Option<(Option<FileStamp>, Arc<Tracked>)>,
    pub walk: Walk,
}

pub(super) struct Slot {
    pub work: Mutex<Entry>,
    pub last: Mutex<Option<(Instant, Arc<Check>)>>,
    pub running: AtomicBool,
}

impl Slot {
    fn new() -> Slot {
        Slot {
            work: Mutex::new(Entry::default()),
            last: Mutex::new(None),
            running: AtomicBool::new(false),
        }
    }

    fn last(&self) -> Option<(Instant, Arc<Check>)> {
        self.last.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set_last(&self, check: Option<Arc<Check>>) {
        *self.last.lock().unwrap_or_else(|e| e.into_inner()) = check.map(|c| (Instant::now(), c));
    }

    fn younger_than(&self, fresh: Duration) -> Option<Arc<Check>> {
        self.last()
            .filter(|(at, _)| at.elapsed() < fresh)
            .map(|(_, check)| check)
    }

    /// `work`, waiting for it until `deadline`; None when the deadline came first.
    fn lock_work(&self, deadline: Instant) -> Option<MutexGuard<'_, Entry>> {
        loop {
            match self.work.try_lock() {
                Ok(guard) => return Some(guard),
                Err(TryLockError::Poisoned(p)) => return Some(p.into_inner()),
                Err(TryLockError::WouldBlock) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return None;
                    }
                    std::thread::sleep(LOCK_TICK.min(deadline - now));
                }
            }
        }
    }

    /// `work`, with a check no older than `fresh` (the last one, or a new one made under `work`).
    fn lock_check(
        &self,
        home: &Path,
        target: &Target,
        fresh: Duration,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<(MutexGuard<'_, Entry>, Arc<Check>), Halt> {
        let Some(mut entry) = self.lock_work(deadline) else {
            return Err(Halt::Late);
        };
        // Another caller may have finished a check while this one waited.
        if let Some(check) = self.younger_than(fresh) {
            return Ok((entry, check));
        }
        match super::compute(home, target, &mut entry, deadline, cancel) {
            Ok(check) => {
                let check = Arc::new(check);
                self.set_last(Some(check.clone()));
                Ok((entry, check))
            }
            Err(Halt::Err(e)) => {
                // The old answer isn't one any more.
                self.set_last(None);
                Err(Halt::Err(e))
            }
            Err(Halt::Late) => Err(Halt::Late),
        }
    }

    /// A check no older than `fresh`, else a new one. When the deadline comes first, the last check of any
    /// age if there is one.
    fn check(
        &self,
        home: &Path,
        target: &Target,
        fresh: Duration,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<Arc<Check>, ChangesError> {
        if let Some(check) = self.younger_than(fresh) {
            return Ok(check);
        }
        match self.lock_check(home, target, fresh, deadline, cancel) {
            Ok((_, check)) => Ok(check),
            Err(Halt::Late) => self
                .last()
                .map(|(_, check)| check)
                .ok_or_else(|| ChangesError::Failed(TOO_LONG.to_string())),
            Err(Halt::Err(e)) => Err(e),
        }
    }

    fn read(
        &self,
        home: &Path,
        target: &Target,
        hide_whitespace: bool,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<ChangesReply, ChangesError> {
        let (entry, check) = self
            .lock_check(home, target, CHECK_FRESH, deadline, cancel)
            .map_err(Halt::into_error)?;
        super::build_reply(&entry, target, &check, hide_whitespace, deadline, cancel).map_err(Halt::into_error)
    }
}

/// Clears `running` when the background check is over, however it ended (or never began).
struct Running(Arc<Slot>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::SeqCst);
    }
}

pub struct ChangesCache {
    slots: Mutex<Vec<(PathBuf, Arc<Slot>)>>,
}

impl Default for ChangesCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangesCache {
    pub fn new() -> Self {
        ChangesCache {
            slots: Mutex::new(Vec::new()),
        }
    }

    /// The slot of `checkout`, made when there is none, and now the most recently used.
    pub(super) fn slot(&self, checkout: &Path) -> Arc<Slot> {
        let same = |a: &Path| crate::projects::same_path(&a.to_string_lossy(), &checkout.to_string_lossy());
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(at) = slots.iter().position(|(path, _)| same(path)) {
            let found = slots.remove(at);
            let slot = found.1.clone();
            slots.push(found);
            return slot;
        }
        let slot = Arc::new(Slot::new());
        slots.push((checkout.to_path_buf(), slot.clone()));
        if slots.len() > MAX_SLOTS {
            slots.remove(0);
        }
        slot
    }

    /// A check younger than `CHECK_FRESH` for this checkout is reused.
    pub fn check(
        &self,
        home: &Path,
        target: &Target,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<Arc<Check>, ChangesError> {
        self.check_within(home, target, CHECK_FRESH, deadline, cancel)
    }

    /// `check` with the age a reused check may have (the tests use zero).
    pub(super) fn check_within(
        &self,
        home: &Path,
        target: &Target,
        fresh: Duration,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<Arc<Check>, ChangesError> {
        self.slot(&target.checkout).check(home, target, fresh, deadline, cancel)
    }

    /// The whole reply: the check, then the patch, the new files' diffs and the counts.
    pub fn read(
        &self,
        home: &Path,
        target: &Target,
        hide_whitespace: bool,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<ChangesReply, ChangesError> {
        self.slot(&target.checkout)
            .read(home, target, hide_whitespace, deadline, cancel)
    }

    /// For the tree: the last finished check of any age, at once. When it is older than `CHECK_FRESH`, or there
    /// is none, and no check runs, one starts in the background with `CHECK_BUDGET`, and the next poll gets its
    /// result.
    pub fn marks(self: &Arc<Self>, home: PathBuf, target: Target) -> Option<Arc<Check>> {
        let slot = self.slot(&target.checkout);
        let last = slot.last();
        let stale = last.as_ref().is_none_or(|(at, _)| at.elapsed() >= CHECK_FRESH);
        if stale && !slot.running.swap(true, Ordering::SeqCst) {
            let running = Running(slot);
            let job = move || {
                let cancel = AtomicBool::new(false);
                let deadline = Instant::now() + CHECK_BUDGET;
                // The result is the slot's last check, which the next poll reads; an error leaves none.
                let _ = running.0.check(&home, &target, CHECK_FRESH, deadline, &cancel);
            };
            match tokio::runtime::Handle::try_current() {
                Ok(runtime) => {
                    runtime.spawn_blocking(job);
                }
                Err(_) => {
                    std::thread::spawn(job);
                }
            }
        }
        last.map(|(_, check)| check)
    }
}
