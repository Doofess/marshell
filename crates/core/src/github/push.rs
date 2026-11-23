//! A confirmed push: the user's own `git push` for a session's checkout, started in a process tree of its own,
//! its output kept in a bounded transcript the panel polls, and stoppable whole.
//!
//! One push runs per checkout. It runs on a thread of its own through `git::run_watched`, in a process tree that
//! Cancel and `stop_pushes` end whole. A push's own lock is taken for one chunk of output at a time, never while a pipe
//! is read or a process waited for; and the lock of `GithubState` is never held while a push's lock is taken by
//! the thread, so the two can't deadlock. The end of a push (its state, its reason, `mark_stale` after a push that
//! landed) is `EndGuard`'s: it happens when git can't start and when the thread panics too, so a push never stays
//! "running" for good.

use super::remote::{RemoteRepo, parse_remote};
use super::{GithubState, plain, same, valid_branch};
use crate::git::{Ended, Pipe, run_watched};
use marshell_protocol::api::{PushProgress, PushState};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The most lines a push's transcript keeps; the oldest go first.
pub const PUSH_LINES: usize = 2_000;
/// The most characters a transcript line keeps.
pub const PUSH_LINE_CHARS: usize = 1_000;
/// How long a finished push stays readable.
pub const PUSH_KEPT: Duration = Duration::from_secs(10 * 60);
/// The most bytes an operating system command may run to before it is taken for text.
const OSC_MAX: usize = 4_096;
/// The most of one standard output line that is looked at for `To <address>`.
const TO_LINE_MAX: usize = 4_096;
/// The most `To` lines kept.
const TO_LINES_MAX: usize = 16;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Text,
    /// After an escape.
    Esc,
    /// In a control sequence (`ESC [`).
    Csi,
    /// In an operating system command (`ESC ]`), this many bytes in.
    Osc(usize),
    /// After an escape inside an operating system command.
    OscEsc(usize),
}

/// What one pipe has half read.
#[derive(Debug, Default)]
struct PipeState {
    mode: Mode,
    /// The start of a UTF-8 character that the chunk ended in.
    carry: Vec<u8>,
    /// How many bytes the character in `carry` has.
    need: usize,
    /// A carriage return came, and what comes next says whether it was a redraw.
    cr: bool,
}

#[derive(Debug)]
struct Line {
    text: String,
    chars: usize,
    pipe: Pipe,
}

fn slot(pipe: Pipe) -> usize {
    match pipe {
        Pipe::Out => 0,
        Pipe::Err => 1,
    }
}

/// What a push printed: at most `PUSH_LINES` lines of `PUSH_LINE_CHARS` characters, the oldest dropped. Escape
/// sequences are dropped, a carriage return redraws the last line when it came from the same pipe, and anything a
/// terminal would act on or that hides text becomes U+FFFD.
#[derive(Debug, Default)]
pub struct Transcript {
    lines: VecDeque<Line>,
    /// How many lines were dropped from the front: the number of `lines[0]`.
    dropped: u64,
    /// The last line has no line end yet.
    open: bool,
    pipes: [PipeState; 2],
}

impl Transcript {
    /// Takes what `pipe` gave next. A chunk may end inside an escape sequence or a character.
    pub fn feed(&mut self, pipe: Pipe, bytes: &[u8]) {
        for &b in bytes {
            self.byte(pipe, b);
        }
    }

    /// The lines from `from - 1` on (only the last line a reader has seen can have been redrawn), as
    /// (the number of the first, the lines, the number of lines so far). A `from` before the oldest line kept starts
    /// at that one; one beyond the total starts at the total, with no lines.
    pub fn since(&self, from: u64) -> (u64, Vec<String>, u64) {
        let total = self.dropped + self.lines.len() as u64;
        let start = from.saturating_sub(1).clamp(self.dropped, total);
        let skip = usize::try_from(start - self.dropped).unwrap_or(usize::MAX);
        let lines = self.lines.iter().skip(skip).map(|l| l.text.clone()).collect();
        (start, lines, total)
    }

    /// A line of Marshell's own, after what is there.
    fn note(&mut self, text: &str) {
        self.start_line(Pipe::Err);
        self.open = false;
        let line = self.lines.back_mut().expect("a line was just started");
        line.text = text.chars().take(PUSH_LINE_CHARS).collect();
        line.chars = line.text.chars().count();
    }

    fn byte(&mut self, pipe: Pipe, b: u8) {
        let i = slot(pipe);
        match self.pipes[i].mode {
            Mode::Text => self.text_byte(pipe, b),
            Mode::Esc => match b {
                b'[' => self.pipes[i].mode = Mode::Csi,
                b']' => self.pipes[i].mode = Mode::Osc(0),
                _ => {
                    // An escape that starts no sequence is a control character like any other.
                    self.pipes[i].mode = Mode::Text;
                    self.put(pipe, '\u{fffd}');
                    self.byte(pipe, b);
                }
            },
            Mode::Csi => match b {
                0x40..=0x7e => self.pipes[i].mode = Mode::Text,
                0x20..=0x3f => {}
                _ => {
                    self.pipes[i].mode = Mode::Text;
                    self.byte(pipe, b);
                }
            },
            Mode::Osc(n) => match b {
                0x07 => self.pipes[i].mode = Mode::Text,
                0x1b => self.pipes[i].mode = Mode::OscEsc(n),
                b'\n' => {
                    self.pipes[i].mode = Mode::Text;
                    self.byte(pipe, b);
                }
                _ if n >= OSC_MAX => {
                    // Never ended: what follows is text.
                    self.pipes[i].mode = Mode::Text;
                    self.byte(pipe, b);
                }
                _ => self.pipes[i].mode = Mode::Osc(n + 1),
            },
            Mode::OscEsc(_) => {
                if b == b'\\' {
                    self.pipes[i].mode = Mode::Text;
                } else {
                    self.pipes[i].mode = Mode::Esc;
                    self.byte(pipe, b);
                }
            }
        }
    }

    fn text_byte(&mut self, pipe: Pipe, b: u8) {
        let i = slot(pipe);
        if b < 0x80 {
            if !self.pipes[i].carry.is_empty() {
                self.pipes[i].carry.clear();
                self.put(pipe, '\u{fffd}');
            }
            match b {
                0x1b => self.pipes[i].mode = Mode::Esc,
                b'\n' => self.newline(pipe),
                b'\r' => self.pipes[i].cr = true,
                b'\t' => self.put(pipe, '\t'),
                0..=0x1f | 0x7f => self.put(pipe, '\u{fffd}'),
                _ => self.put(pipe, char::from(b)),
            }
            return;
        }
        if (0x80..=0xbf).contains(&b) && !self.pipes[i].carry.is_empty() {
            self.pipes[i].carry.push(b);
            if self.pipes[i].carry.len() == self.pipes[i].need {
                let carry = std::mem::take(&mut self.pipes[i].carry);
                let ch = std::str::from_utf8(&carry)
                    .ok()
                    .and_then(|s| s.chars().next())
                    .unwrap_or('\u{fffd}');
                self.put(pipe, ch);
            }
            return;
        }
        if !self.pipes[i].carry.is_empty() {
            self.pipes[i].carry.clear();
            self.put(pipe, '\u{fffd}');
        }
        let need = match b {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => 0,
        };
        if need == 0 {
            self.put(pipe, '\u{fffd}');
        } else {
            self.pipes[i].carry = vec![b];
            self.pipes[i].need = need;
        }
    }

    fn start_line(&mut self, pipe: Pipe) {
        self.lines.push_back(Line {
            text: String::new(),
            chars: 0,
            pipe,
        });
        if self.lines.len() > PUSH_LINES {
            self.lines.pop_front();
            self.dropped += 1;
        }
        self.open = true;
    }

    /// Whether the last line is open and `pipe`'s.
    fn open_of(&self, pipe: Pipe) -> bool {
        self.open && self.lines.back().is_some_and(|l| l.pipe == pipe)
    }

    fn newline(&mut self, pipe: Pipe) {
        self.pipes[slot(pipe)].cr = false;
        if !self.open_of(pipe) {
            self.start_line(pipe);
        }
        self.open = false;
    }

    fn put(&mut self, pipe: Pipe, ch: char) {
        let mut buf = [0u8; 4];
        let shown = ch.encode_utf8(&mut buf);
        let ch = if ch as u32 >= 0x80 && plain(shown, 1) != *shown {
            '\u{fffd}'
        } else {
            ch
        };
        if std::mem::take(&mut self.pipes[slot(pipe)].cr)
            && self.open_of(pipe)
            && let Some(line) = self.lines.back_mut()
        {
            line.text.clear();
            line.chars = 0;
        }
        if !self.open_of(pipe) {
            self.start_line(pipe);
        }
        let line = self.lines.back_mut().expect("a line is open");
        if line.chars < PUSH_LINE_CHARS {
            line.text.push(ch);
            line.chars += 1;
        }
    }
}

/// `git -C <checkout> -c protocol.allow=never -c protocol.https.allow=always -c protocol.ssh.allow=always [-c <extra>]…
/// push --progress --porcelain -u origin refs/heads/<branch>:refs/heads/<branch>`. Settings on the command line win
/// over the repository's, so no `file`, `ext` or other helper transport can run. The refspec is always the full one:
/// no force, no tags, no other remote. `env` is Marshell's own environment, less what points git somewhere else
/// (`GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`,
/// `GIT_COMMON_DIR`, `GIT_NAMESPACE`, every `GIT_CONFIG*`), with the prompt for a password off. `extra` is empty
/// outside tests. Panics for a branch `valid_branch` refuses: callers check it first.
pub fn push_command(
    git: &Path,
    checkout: &Path,
    branch: &str,
    env: impl Iterator<Item = (OsString, OsString)>,
    extra: &[String],
) -> Command {
    assert!(
        valid_branch(branch),
        "{branch:?} is never a branch Marshell pushes: callers check it first"
    );
    const POINTERS: [&str; 8] = [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_NAMESPACE",
        "GIT_TERMINAL_PROMPT",
    ];
    let mut cmd = Command::new(git);
    cmd.arg("-C").arg(checkout);
    for setting in [
        "protocol.allow=never",
        "protocol.https.allow=always",
        "protocol.ssh.allow=always",
    ] {
        cmd.arg("-c").arg(setting);
    }
    for setting in extra {
        cmd.arg("-c").arg(setting);
    }
    cmd.args(["push", "--progress", "--porcelain", "-u", "origin"])
        .arg(format!("refs/heads/{branch}:refs/heads/{branch}"));
    cmd.env_clear();
    for (name, value) in env {
        let upper = name.to_string_lossy().to_ascii_uppercase();
        if !POINTERS.contains(&upper.as_str()) && !upper.starts_with("GIT_CONFIG") {
            cmd.env(name, value);
        }
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd
}

/// The address in git's `To <address>` line, which `--porcelain` writes to standard output once per remote.
pub fn pushed_to(stdout_line: &str) -> Option<&str> {
    let address = stdout_line.strip_prefix("To ")?.trim();
    (!address.is_empty()).then_some(address)
}

/// The `To` lines of a standard output read in chunks, bounded.
#[derive(Default)]
struct ToLines {
    line: Vec<u8>,
    found: Vec<String>,
}

impl ToLines {
    fn feed(&mut self, chunk: &[u8]) {
        for &b in chunk {
            if b == b'\n' {
                self.end_line();
            } else if self.line.len() < TO_LINE_MAX {
                self.line.push(b);
            }
        }
    }

    fn end_line(&mut self) {
        let text = String::from_utf8_lossy(&self.line).into_owned();
        self.line.clear();
        if self.found.len() < TO_LINES_MAX
            && let Some(address) = pushed_to(&text)
        {
            self.found.push(address.to_string());
        }
    }

    fn done(mut self) -> Vec<String> {
        self.end_line();
        self.found
    }
}

struct PushData {
    state: PushState,
    reason: Option<String>,
    transcript: Transcript,
    /// When it left `Running`.
    ended: Option<Instant>,
}

/// What a push's thread and the routes share.
pub(super) struct PushShared {
    cancel: AtomicBool,
    data: Mutex<PushData>,
}

impl PushShared {
    fn new() -> PushShared {
        PushShared {
            cancel: AtomicBool::new(false),
            data: Mutex::new(PushData {
                state: PushState::Running,
                reason: None,
                transcript: Transcript::default(),
                ended: None,
            }),
        }
    }

    fn data(&self) -> MutexGuard<'_, PushData> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn running(&self) -> bool {
        self.data().state == PushState::Running
    }
}

/// A push `GithubState` knows: its id, its checkout and what it shares with its thread.
pub(super) struct PushSlot {
    id: String,
    checkout: PathBuf,
    shared: Arc<PushShared>,
}

/// What a push needs to start.
pub struct PushLaunch {
    pub checkout: PathBuf,
    pub branch: String,
    /// The repository the bar shows, which git's `To` line is checked against.
    pub repo: RemoteRepo,
    pub git: PathBuf,
    /// Extra `-c` settings; empty outside tests.
    pub extra: Vec<String>,
}

/// A push of the checkout runs already.
#[derive(Debug, PartialEq, Eq)]
pub struct AlreadyRunning;

/// Ends a push: when the thread says how, or, when it didn't get to (a panic, or a thread that never ran), as
/// failed.
struct EndGuard {
    github: Arc<GithubState>,
    shared: Arc<PushShared>,
    checkout: PathBuf,
    done: bool,
}

impl EndGuard {
    fn new(github: Arc<GithubState>, shared: Arc<PushShared>, checkout: PathBuf) -> EndGuard {
        EndGuard {
            github,
            shared,
            checkout,
            done: false,
        }
    }

    fn end(mut self, state: PushState, reason: Option<String>) {
        self.done = true;
        self.finish(state, reason);
    }

    fn finish(&self, state: PushState, reason: Option<String>) {
        // Before the state shows: whoever sees "pushed" and asks GitHub must not be answered from before the push.
        if state == PushState::Pushed {
            self.github.mark_stale(&self.checkout);
        }
        let mut data = self.shared.data();
        data.state = state;
        data.reason = reason;
        data.ended = Some(Instant::now());
    }
}

impl Drop for EndGuard {
    fn drop(&mut self) {
        if !self.done {
            self.finish(PushState::Failed, Some("The push stopped unexpectedly.".to_string()));
        }
    }
}

fn new_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the OS random number generator failed");
    hex::encode(bytes)
}

/// Whether `id` has the shape of a push id: 32 lowercase hex digits.
fn id_shape(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Drops the pushes that ended more than `kept` ago.
fn prune(pushes: &mut Vec<PushSlot>, kept: Duration) {
    pushes.retain(|p| p.shared.data().ended.is_none_or(|at| at.elapsed() < kept));
}

/// The push's thread: runs git, feeds the transcript, and ends the push.
fn run(guard: EndGuard, launch: PushLaunch) {
    let shared = guard.shared.clone();
    let cmd = push_command(
        &launch.git,
        &launch.checkout,
        &launch.branch,
        std::env::vars_os(),
        &launch.extra,
    );
    let mut to = ToLines::default();
    let ended = run_watched(cmd, &shared.cancel, |pipe, chunk| {
        shared.data().transcript.feed(pipe, chunk);
        if pipe == Pipe::Out {
            to.feed(chunk);
        }
    });
    let (state, mut reason) = match ended {
        Ok(Ended::Exited(Some(0))) => (PushState::Pushed, None),
        Ok(Ended::Exited(Some(code))) => (PushState::Failed, Some(format!("git ended with code {code}."))),
        Ok(Ended::Exited(None)) => (PushState::Failed, Some("git ended without a code.".to_string())),
        Ok(Ended::Cancelled) => (PushState::Cancelled, None),
        Err(e) => (PushState::Failed, Some(plain(&format!("{e:#}"), 300))),
    };
    if state != PushState::Cancelled {
        let repo = format!("{}/{}", launch.repo.owner, launch.repo.name);
        for address in to.done() {
            if parse_remote(&address).as_ref() != Some(&launch.repo) {
                let note = format!("git pushed to {}, which isn't {repo}.", plain(&address, 200));
                shared.data().transcript.note(&note);
                if state == PushState::Pushed {
                    reason = Some(note);
                }
                break;
            }
        }
    }
    guard.end(state, reason);
}

impl GithubState {
    /// Starts a push of `launch.checkout`'s branch, unless one of that checkout runs. The check and the insert are
    /// one step under the lock. Returns the push's id; a push whose thread can't start is there, ended failed.
    pub fn start_push(self: &Arc<Self>, launch: PushLaunch) -> Result<String, AlreadyRunning> {
        let id = new_id();
        let shared = Arc::new(PushShared::new());
        {
            let mut inner = self.lock();
            prune(&mut inner.pushes, PUSH_KEPT);
            if inner
                .pushes
                .iter()
                .any(|p| same(&p.checkout, &launch.checkout) && p.shared.running())
            {
                return Err(AlreadyRunning);
            }
            inner.pushes.push(PushSlot {
                id: id.clone(),
                checkout: launch.checkout.clone(),
                shared: shared.clone(),
            });
        }
        let guard = EndGuard::new(self.clone(), shared.clone(), launch.checkout.clone());
        let spawned = std::thread::Builder::new()
            .name("marshell-push".to_string())
            .spawn(move || run(guard, launch));
        if spawned.is_err() {
            // The closure was dropped with its guard, which ended the push as failed; say why.
            shared.data().reason = Some("Marshell couldn't start the push.".to_string());
        }
        Ok(id)
    }

    /// The push `id`, when there is one: running, or ended less than `PUSH_KEPT` ago.
    fn find_push(&self, id: &str) -> Option<Arc<PushShared>> {
        if !id_shape(id) {
            return None;
        }
        let mut inner = self.lock();
        prune(&mut inner.pushes, PUSH_KEPT);
        inner.pushes.iter().find(|p| p.id == id).map(|p| p.shared.clone())
    }

    /// The push `id` as the panel polls it, with its lines from `from - 1` on (see `Transcript::since`).
    pub fn push_progress(&self, id: &str, from: u64) -> Option<PushProgress> {
        let shared = self.find_push(id)?;
        let data = shared.data();
        let (start, lines, total) = data.transcript.since(from);
        Some(PushProgress {
            state: data.state,
            start,
            lines,
            total,
            reason: data.reason.clone(),
        })
    }

    /// Asks the push `id` to end: git and everything it started are killed. False for an id that isn't known. A push
    /// that has ended already is left as it ended.
    pub fn cancel_push(&self, id: &str) -> bool {
        match self.find_push(id) {
            Some(shared) => {
                shared.cancel.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    /// The id of the push that runs at `checkout`.
    pub(crate) fn running_push(&self, checkout: &Path) -> Option<String> {
        let mut inner = self.lock();
        prune(&mut inner.pushes, PUSH_KEPT);
        inner
            .pushes
            .iter()
            .find(|p| same(&p.checkout, checkout) && p.shared.running())
            .map(|p| p.id.clone())
    }

    /// How many pushes run.
    pub fn pushes_running(&self) -> u32 {
        let mut inner = self.lock();
        prune(&mut inner.pushes, PUSH_KEPT);
        let running = inner.pushes.iter().filter(|p| p.shared.running()).count();
        u32::try_from(running).unwrap_or(u32::MAX)
    }

    /// Cancels every push that runs and waits for them to end, at most `wait`. The lock is not held while waiting.
    pub fn stop_pushes(&self, wait: Duration) {
        let running: Vec<Arc<PushShared>> = {
            let inner = self.lock();
            inner
                .pushes
                .iter()
                .filter(|p| p.shared.running())
                .map(|p| p.shared.clone())
                .collect()
        };
        for shared in &running {
            shared.cancel.store(true, Ordering::Relaxed);
        }
        let deadline = Instant::now() + wait;
        while running.iter().any(|s| s.running()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::Pipe;
    use std::ffi::OsString;
    use std::path::Path;

    fn text(t: &Transcript) -> Vec<String> {
        t.since(0).1
    }

    #[test]
    fn a_carriage_return_redraws_the_last_line() {
        let mut t = Transcript::default();
        // S17 finding 9: one line redrawn with carriage returns, erased to its end each time.
        t.feed(
            Pipe::Err,
            b"Counting objects:  14% (1/7)\rCounting objects:  28% (2/7)\x1b[K\r",
        );
        assert_eq!(text(&t), ["Counting objects:  28% (2/7)"]);
        t.feed(
            Pipe::Err,
            b"Counting objects: 100% (7/7)\rCounting objects: 100% (7/7), done.\n",
        );
        t.feed(Pipe::Err, b"remote: Resolving deltas\n");
        assert_eq!(
            text(&t),
            ["Counting objects: 100% (7/7), done.", "remote: Resolving deltas"]
        );
    }

    #[test]
    fn a_carriage_return_before_a_line_feed_is_a_line_end_not_a_redraw() {
        let mut t = Transcript::default();
        t.feed(Pipe::Out, b"one\r\ntwo\r");
        t.feed(Pipe::Out, b"\nthree\r\n\r\nfour");
        assert_eq!(text(&t), ["one", "two", "three", "", "four"]);
    }

    #[test]
    fn a_carriage_return_from_the_other_pipe_starts_a_line() {
        let mut t = Transcript::default();
        t.feed(Pipe::Err, b"progress 10%");
        t.feed(Pipe::Out, b"\rto stdout\n");
        assert_eq!(text(&t), ["progress 10%", "to stdout"]);
        // And the same pipe redraws only a line that is still open.
        let mut t = Transcript::default();
        t.feed(Pipe::Err, b"done\n\rnext");
        assert_eq!(text(&t), ["done", "next"]);
    }

    #[test]
    fn escape_sequences_are_dropped() {
        let mut t = Transcript::default();
        t.feed(
            Pipe::Err,
            b"\x1b[1;31mred\x1b[0m \x1b]0;a title\x07plain \x1b]8;;http://x\x1b\\link\x1b]8;;\x1b\\\n",
        );
        assert_eq!(text(&t), ["red plain link"]);
        // A sequence cut by the end of a chunk.
        let mut t = Transcript::default();
        for part in [&b"a\x1b"[..], b"[3", b"1;1", b"mb\x1b]0;ti", b"tle", b"\x1b", b"\\c\n"] {
            t.feed(Pipe::Out, part);
        }
        assert_eq!(text(&t), ["abc"]);
        // An escape that starts nothing, and control characters, become U+FFFD; a tab stays.
        let mut t = Transcript::default();
        t.feed(Pipe::Out, b"x\x1bQy\x00z\x07\t|\x7f\n");
        assert_eq!(text(&t), ["x\u{fffd}Qy\u{fffd}z\u{fffd}\t|\u{fffd}"]);
    }

    #[test]
    fn an_unterminated_operating_system_command_does_not_swallow_the_rest() {
        let mut t = Transcript::default();
        let mut bytes = b"\x1b]0;".to_vec();
        bytes.extend(std::iter::repeat_n(b'x', 10_000));
        bytes.extend_from_slice(b"after\n");
        t.feed(Pipe::Err, &bytes);
        t.feed(Pipe::Err, b"\x1b]0;never ended\nnext\n");
        let lines = text(&t);
        assert_eq!(lines.last().map(String::as_str), Some("next"), "{lines:?}");
    }

    #[test]
    fn a_split_utf8_character_is_kept_whole() {
        let mut t = Transcript::default();
        let word = "gr\u{fc}n \u{20ac} \u{1f680}\n".as_bytes();
        // One byte at a time: every character is split at every point.
        for byte in word {
            t.feed(Pipe::Out, std::slice::from_ref(byte));
        }
        assert_eq!(text(&t), ["gr\u{fc}n \u{20ac} \u{1f680}"]);
        // The halves of one character on two pipes are two broken ones, not one.
        let mut t = Transcript::default();
        t.feed(Pipe::Out, &"\u{20ac}".as_bytes()[..2]);
        t.feed(Pipe::Err, &"\u{20ac}".as_bytes()[2..]);
        t.feed(Pipe::Out, b"\n");
        let all = text(&t).concat();
        assert!(!all.contains('\u{20ac}'), "{all:?}");
        // Bytes that are not UTF-8 become U+FFFD.
        let mut t = Transcript::default();
        t.feed(Pipe::Out, b"a\xffb\xe2\x82c\n");
        assert_eq!(text(&t), ["a\u{fffd}b\u{fffd}c"]);
    }

    #[test]
    fn text_that_hides_or_reorders_other_text_is_replaced() {
        let mut t = Transcript::default();
        t.feed(Pipe::Out, "a\u{202e}b\u{200b}c\n".as_bytes());
        assert_eq!(text(&t), ["a\u{fffd}b\u{fffd}c"]);
    }

    #[test]
    fn a_line_is_cut_at_1000_characters() {
        let mut t = Transcript::default();
        t.feed(Pipe::Out, "\u{e9}".repeat(1500).as_bytes());
        t.feed(Pipe::Out, b"\nshort\n");
        let lines = text(&t);
        assert_eq!(lines[0].chars().count(), PUSH_LINE_CHARS);
        assert_eq!(lines[1], "short");
    }

    #[test]
    fn the_transcript_keeps_the_last_2000_lines() {
        let mut t = Transcript::default();
        assert_eq!(t.since(0), (0, Vec::new(), 0));
        assert_eq!(t.since(5), (0, Vec::new(), 0));
        for n in 0..2_500 {
            t.feed(Pipe::Out, format!("line {n}\n").as_bytes());
        }
        let (start, lines, total) = t.since(0);
        assert_eq!(total, 2_500);
        assert_eq!(lines.len(), PUSH_LINES);
        assert_eq!(start, 500, "the oldest 500 were dropped");
        assert_eq!(lines[0], "line 500");
        assert_eq!(lines[PUSH_LINES - 1], "line 2499");
        // A reader that has seen `from` lines gets the lines from `from - 1` on: the last one may be redrawn.
        let (start, lines, total) = t.since(2_490);
        assert_eq!((start, total), (2_489, 2_500));
        assert_eq!(lines.len(), 11);
        assert_eq!(lines[0], "line 2489");
        // Up to date: only the last line again.
        let (start, lines, total) = t.since(2_500);
        assert_eq!((start, total), (2_499, 2_500));
        assert_eq!(lines, ["line 2499"]);
        // Before the oldest line kept, or beyond the total: no panic, and a start inside what is kept.
        let (start, lines, _) = t.since(3);
        assert_eq!((start, lines.len()), (500, PUSH_LINES));
        let (start, lines, total) = t.since(u64::MAX);
        assert_eq!((start, lines.len(), total), (2_500, 0, 2_500));
    }

    #[test]
    fn the_open_line_is_in_the_answer_and_a_redraw_moves_no_count() {
        let mut t = Transcript::default();
        t.feed(Pipe::Err, b"first\nProgress 1%");
        assert_eq!(t.since(0), (0, vec!["first".to_string(), "Progress 1%".to_string()], 2));
        t.feed(Pipe::Err, b"\rProgress 2%");
        assert_eq!(t.since(2), (1, vec!["Progress 2%".to_string()], 2));
    }

    fn env(pairs: &[(&str, &str)]) -> impl Iterator<Item = (OsString, OsString)> + use<> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(*k), OsString::from(*v)))
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn args_of(cmd: &std::process::Command) -> Vec<String> {
        cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn the_command_pushes_a_full_refspec() {
        let cmd = push_command(
            Path::new("git"),
            Path::new("/work/repo"),
            "feat/login",
            env(&[("PATH", "/bin")]),
            &[],
        );
        assert_eq!(
            args_of(&cmd),
            [
                "-C",
                "/work/repo",
                "-c",
                "protocol.allow=never",
                "-c",
                "protocol.https.allow=always",
                "-c",
                "protocol.ssh.allow=always",
                "push",
                "--progress",
                "--porcelain",
                "-u",
                "origin",
                "refs/heads/feat/login:refs/heads/feat/login",
            ]
        );
        // A test's settings come after Marshell's own and before `push`, each behind its own `-c`.
        let extra = vec!["protocol.file.allow=always".to_string(), "x.y=z".to_string()];
        let cmd = push_command(
            Path::new("git"),
            Path::new("/work/repo"),
            "feat/login",
            env(&[]),
            &extra,
        );
        let args = args_of(&cmd);
        assert_eq!(
            args[8..14],
            ["-c", "protocol.file.allow=always", "-c", "x.y=z", "push", "--progress"]
        );
        assert_eq!(args.len(), 18);
    }

    #[test]
    fn the_commands_environment_has_none_of_gits_own_pointers() {
        let cmd = push_command(
            Path::new("git"),
            Path::new("/work/repo"),
            "feat/login",
            env(&[
                ("PATH", "/bin"),
                ("SystemRoot", "C:\\Windows"),
                ("GIT_DIR", "/elsewhere"),
                ("git_work_tree", "/elsewhere"),
                ("GIT_INDEX_FILE", "x"),
                ("GIT_OBJECT_DIRECTORY", "x"),
                ("GIT_ALTERNATE_OBJECT_DIRECTORIES", "x"),
                ("GIT_COMMON_DIR", "x"),
                ("GIT_NAMESPACE", "x"),
                ("GIT_CONFIG", "x"),
                ("GIT_CONFIG_GLOBAL", "x"),
                ("GIT_CONFIG_COUNT", "1"),
                ("GIT_CONFIG_KEY_0", "protocol.allow"),
                ("GIT_CONFIG_VALUE_0", "always"),
                ("GIT_TERMINAL_PROMPT", "1"),
                ("GIT_SSH_COMMAND", "ssh -i key"),
            ]),
            &[],
        );
        let got: Vec<(String, String)> = cmd
            .get_envs()
            .filter_map(|(k, v)| {
                Some((
                    k.to_string_lossy().to_ascii_uppercase(),
                    v?.to_string_lossy().into_owned(),
                ))
            })
            .collect();
        let names: Vec<&str> = got.iter().map(|(k, _)| k.as_str()).collect();
        for gone in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_COMMON_DIR",
            "GIT_NAMESPACE",
            "GIT_CONFIG",
            "GIT_CONFIG_GLOBAL",
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_KEY_0",
            "GIT_CONFIG_VALUE_0",
        ] {
            assert!(!names.contains(&gone), "{gone} is passed on: {names:?}");
        }
        assert!(names.contains(&"PATH") && names.contains(&"SYSTEMROOT"), "{names:?}");
        assert!(
            names.contains(&"GIT_SSH_COMMAND"),
            "the rest is Marshell's own: {names:?}"
        );
        let prompt: Vec<&str> = got
            .iter()
            .filter(|(k, _)| k == "GIT_TERMINAL_PROMPT")
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(prompt, ["0"], "{got:?}");
    }

    #[test]
    #[should_panic(expected = "never a branch")]
    fn the_command_is_never_built_for_a_branch_that_is_an_option_or_a_force() {
        let _ = push_command(Path::new("git"), Path::new("/r"), "+main", env(&[]), &[]);
    }

    #[test]
    fn pushed_to_reads_gits_to_line() {
        assert_eq!(
            pushed_to("To https://github.com/octo-org/widgets.git"),
            Some("https://github.com/octo-org/widgets.git")
        );
        assert_eq!(
            pushed_to("To git@github.com:octo-org/widgets.git\r"),
            Some("git@github.com:octo-org/widgets.git")
        );
        assert_eq!(pushed_to("To   C:/tmp/bare.git "), Some("C:/tmp/bare.git"));
        for not in [
            "",
            "To",
            "To ",
            "Total 3 (delta 0)",
            "*\trefs/heads/x:refs/heads/x\t[new branch]",
            "Done",
            " To x",
        ] {
            assert_eq!(pushed_to(not), None, "{not:?}");
        }
    }

    fn repo() -> RemoteRepo {
        RemoteRepo {
            host: "github.com".into(),
            owner: "octo-org".into(),
            name: "widgets".into(),
        }
    }

    fn launch(checkout: &str, git: &str) -> PushLaunch {
        PushLaunch {
            checkout: PathBuf::from(checkout),
            branch: "feat/login".into(),
            repo: repo(),
            git: PathBuf::from(git),
            extra: Vec::new(),
        }
    }

    fn wait_for_the_end(github: &GithubState, id: &str) -> PushProgress {
        let started = Instant::now();
        loop {
            let progress = github.push_progress(id, 0).expect("the push is known");
            if progress.state != PushState::Running || started.elapsed() > Duration::from_secs(20) {
                return progress;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A git that can't start ends the push as failed, with a sentence, and frees the checkout.
    #[test]
    fn a_push_whose_git_cannot_start_ends_failed() {
        let github = Arc::new(GithubState::default());
        let id = github
            .start_push(launch("/work/repo", "marshell-no-such-git-program"))
            .unwrap();
        let ended = wait_for_the_end(&github, &id);
        assert_eq!(ended.state, PushState::Failed);
        assert!(ended.reason.as_deref().is_some_and(|r| !r.is_empty()), "{ended:?}");
        assert_eq!(github.running_push(Path::new("/work/repo")), None);
        assert_eq!(github.pushes_running(), 0);
    }

    /// A thread that panics, or never runs, can't leave its push running for good.
    #[test]
    fn a_push_that_is_dropped_before_it_ended_ends_failed() {
        let github = Arc::new(GithubState::default());
        let shared = Arc::new(PushShared::new());
        let guard = EndGuard::new(github.clone(), shared.clone(), PathBuf::from("/r"));
        assert!(shared.running());
        let panicked = std::thread::spawn(move || {
            let _guard = guard;
            panic!("the push's thread failed");
        })
        .join();
        assert!(panicked.is_err());
        let data = shared.data();
        assert_eq!(data.state, PushState::Failed);
        assert_eq!(data.reason.as_deref(), Some("The push stopped unexpectedly."));
        assert!(data.ended.is_some());
    }

    /// A push that landed makes what GitHub said stale, before it shows as pushed; one that failed doesn't.
    #[test]
    fn a_push_that_landed_marks_the_checkout_stale() {
        let github = Arc::new(GithubState::default());
        let checkout = PathBuf::from("/r");
        let kept = |github: &Arc<GithubState>| github.lock().keys.iter().map(|k| k.generation).sum::<u64>();
        let _ = github.take(&checkout, "feat/login", true);
        let before = kept(&github);
        EndGuard::new(github.clone(), Arc::new(PushShared::new()), checkout.clone())
            .end(PushState::Failed, Some("git ended with code 1.".into()));
        assert_eq!(kept(&github), before);
        EndGuard::new(github.clone(), Arc::new(PushShared::new()), checkout).end(PushState::Pushed, None);
        assert_eq!(kept(&github), before + 1);
    }

    #[test]
    fn a_finished_push_is_kept_for_a_while_and_a_running_one_is_not_dropped() {
        let github = Arc::new(GithubState::default());
        let done = Arc::new(PushShared::new());
        let running = Arc::new(PushShared::new());
        EndGuard::new(github.clone(), done.clone(), PathBuf::from("/a")).end(PushState::Cancelled, None);
        let mut pushes = vec![
            PushSlot {
                id: "a".into(),
                checkout: PathBuf::from("/a"),
                shared: done,
            },
            PushSlot {
                id: "b".into(),
                checkout: PathBuf::from("/b"),
                shared: running,
            },
        ];
        prune(&mut pushes, PUSH_KEPT);
        assert_eq!(pushes.len(), 2);
        prune(&mut pushes, Duration::ZERO);
        assert_eq!(pushes.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["b"]);
    }

    #[test]
    fn a_push_id_is_32_lowercase_hex_digits_and_others_are_unknown() {
        let id = new_id();
        assert!(id_shape(&id), "{id}");
        assert_ne!(id, new_id());
        for bad in [
            "",
            "abc",
            &id.to_uppercase(),
            &format!("{id}0"),
            &id[..31],
            &"../".repeat(11),
        ] {
            assert!(!id_shape(bad), "{bad:?}");
        }
        let github = GithubState::default();
        assert!(github.push_progress(&id, 0).is_none());
        assert!(!github.cancel_push(&id));
        assert!(github.push_progress("../../x", 0).is_none());
    }

    #[test]
    fn the_to_lines_of_a_stdout_read_in_chunks() {
        let mut to = ToLines::default();
        for chunk in [
            &b"hook out
To https://github"[..],
            b".com/octo-org/widgets.git
*	r",
            b"efs/heads/x:refs/heads/x	[new branch]
To C:/x/bare.git",
        ] {
            to.feed(chunk);
        }
        assert_eq!(to.done(), ["https://github.com/octo-org/widgets.git", "C:/x/bare.git"]);
    }
}
