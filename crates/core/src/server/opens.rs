//! `marshell open`: a web address or a file for a session's side pane. The app is the only writer of the pane's
//! tabs, so the core adds nothing itself: a request waits here, `/v1/events` shows the app what is waiting, and the
//! app answers each one through `POST /v1/pane/opens/done` once the tab is in (or it refused it). The route that
//! took the request waits for that answer for a few seconds, so the command can say how it went.

use super::{AppState, files};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use marshell_protocol::api::{OpenRequest, PaneOpen, PaneOpenDone, PaneOpensDone};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::{oneshot, watch};

/// How many requests may wait for the app. One more is refused.
pub const MAX_WAITING: usize = 64;
/// The longest address or path, in characters.
pub const TARGET_MAX: usize = 4096;
/// How long the route waits for the app's answer.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(5);

const BAD_ADDRESS: &str = "That address can't be opened in the side pane.";
const BAD_PATH: &str = "That path can't be opened in the side pane.";
const NO_PAGES: &str = "Browser tabs aren't available on this system yet.";
const FULL: &str = "Too many tabs are waiting to open. Try again when Marshell's window is back.";
const LATE: &str = "Marshell hasn't answered yet. The tab opens when its window does.";

/// How a waiting request ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The app added the tab, or marked the one that was there.
    Opened,
    /// The app refused it: "limit" (the session has its 32 tabs), "address" (the shell would not open it) or
    /// "session" (the app has no such session).
    Refused(String),
    /// Its session closed first.
    Closed,
}

/// 64 requests are already waiting.
#[derive(Debug, PartialEq, Eq)]
pub struct Full;

#[derive(Default)]
struct Waiting {
    last_id: u64,
    /// Oldest first, each with the route that is waiting for its answer (if it still is).
    list: Vec<(PaneOpen, oneshot::Sender<Outcome>)>,
}

/// The open requests the app has not answered yet.
pub struct Opens {
    waiting: Mutex<Waiting>,
    shown: watch::Sender<Vec<PaneOpen>>,
}

impl Default for Opens {
    fn default() -> Self {
        Self {
            waiting: Mutex::default(),
            shown: watch::channel(Vec::new()).0,
        }
    }
}

impl Opens {
    /// Queues a request. The id it got, and where its answer arrives.
    pub fn push(
        &self,
        tab_id: &str,
        kind: &str,
        target: String,
        by: String,
    ) -> Result<(u64, oneshot::Receiver<Outcome>), Full> {
        let mut waiting = self.waiting.lock().unwrap();
        if waiting.list.len() >= MAX_WAITING {
            return Err(Full);
        }
        waiting.last_id += 1;
        let open = PaneOpen {
            id: waiting.last_id,
            tab_id: tab_id.to_string(),
            kind: kind.to_string(),
            target,
            by,
        };
        let (tx, rx) = oneshot::channel();
        waiting.list.push((open, tx));
        self.show(&waiting);
        Ok((waiting.last_id, rx))
    }

    /// The app's answers. One for a request that isn't waiting (answered already, dropped, never there) is ignored.
    pub fn done(&self, done: &[PaneOpenDone]) {
        let mut waiting = self.waiting.lock().unwrap();
        let before = waiting.list.len();
        for answer in done {
            let Some(at) = waiting.list.iter().position(|(open, _)| open.id == answer.id) else {
                continue;
            };
            let (_, tx) = waiting.list.remove(at);
            // Err: the route stopped waiting. The tab is in either way.
            let _ = tx.send(match &answer.refused {
                None => Outcome::Opened,
                Some(why) => Outcome::Refused(why.clone()),
            });
        }
        if waiting.list.len() != before {
            self.show(&waiting);
        }
    }

    /// A session closed: nothing will open in its pane.
    pub fn drop_session(&self, tab_id: &str) {
        let mut waiting = self.waiting.lock().unwrap();
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut waiting.list)
            .into_iter()
            .partition(|(open, _)| open.tab_id == tab_id);
        waiting.list = kept;
        if gone.is_empty() {
            return;
        }
        for (_, tx) in gone {
            let _ = tx.send(Outcome::Closed);
        }
        self.show(&waiting);
    }

    /// What is waiting, oldest first, and every change to it.
    pub fn subscribe(&self) -> watch::Receiver<Vec<PaneOpen>> {
        self.shown.subscribe()
    }

    /// Publishes the list. Called with the lock held, so the lists go out in the order they came to be.
    fn show(&self, waiting: &Waiting) {
        self.shown
            .send_replace(waiting.list.iter().map(|(open, _)| open.clone()).collect());
    }
}

/// What a target is: "browser" for an address, "file" for anything else. Only text that starts with `http://` or
/// `https://` (in any case) is an address, so `localhost:3000` and `example.com` are file names. The line to answer
/// when it is neither one that can be opened. `pages` is whether this system has browser tabs.
fn classify(target: &str, pages: bool) -> Result<&'static str, &'static str> {
    let Some(rest) = after_scheme(target) else {
        let fine = target.chars().count() <= TARGET_MAX
            && !files::refused(target)
            && std::path::Path::new(target).is_absolute();
        return if fine { Ok("file") } else { Err(BAD_PATH) };
    };
    if !pages {
        return Err(NO_PAGES);
    }
    // The site's part ends at the first of these. A backslash counts: browsers read it as a slash here.
    let site = rest.split(['/', '?', '#', '\\']).next().unwrap_or_default();
    let fine = target.chars().count() <= TARGET_MAX
        && !target.chars().any(|c| c.is_control() || c.is_whitespace())
        && !site.is_empty()
        && !site.starts_with(':')
        // A name and password before the site: `http://bank.example@other.example/` reads like the first.
        && !site.contains('@');
    if fine { Ok("browser") } else { Err(BAD_ADDRESS) }
}

/// The text after `http://` or `https://`. None when the target starts with neither.
fn after_scheme(target: &str) -> Option<&str> {
    ["http://", "https://"].iter().find_map(|scheme| {
        let start = target.get(..scheme.len())?;
        start.eq_ignore_ascii_case(scheme).then(|| &target[scheme.len()..])
    })
}

/// The line for a path that can't be a file tab: missing, a folder, something that isn't a file, or reached through
/// a link to a network share. It reads the disk, so it runs off the async threads.
fn file_refusal(path: &str) -> Option<String> {
    let on_disk = std::path::Path::new(path);
    if files::links_refused(on_disk) {
        return Some(BAD_PATH.into());
    }
    match std::fs::metadata(on_disk) {
        Ok(meta) if meta.is_file() => None,
        Ok(meta) if meta.is_dir() => Some(format!("{path} is a folder, not a file.")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(format!("No file {path}.")),
        _ => Some(BAD_PATH.into()),
    }
}

/// Who asked, for the mark on the tab: the agent of the session the command ran in, else `shell`. A session launched as
/// a shell names the agent typed at its prompt (its foreground program), as the sidebar does.
fn opener(st: &AppState, from: Option<&str>) -> String {
    from.and_then(|tab| st.registry().row(tab))
        .and_then(|row| row.agent.or_else(|| row.foreground.and_then(|f| f.agent)))
        // The layout keeps the id, and refuses one it can't: a mark is not worth a layout that won't save.
        .filter(|agent| crate::layout::is_opener(agent))
        .unwrap_or_else(|| "shell".into())
}

fn text(status: StatusCode, line: impl Into<String>) -> Response {
    (status, line.into()).into_response()
}

fn closed(number: u32) -> Response {
    text(StatusCode::CONFLICT, format!("Session {number} has closed."))
}

/// POST /v1/sessions/{tab}/open
pub async fn open(State(st): State<AppState>, Path(tab): Path<String>, Json(req): Json<OpenRequest>) -> Response {
    let Some(row) = st.registry().row(&tab) else {
        return text(StatusCode::NOT_FOUND, "No such session.");
    };
    let kind = match classify(&req.target, cfg!(windows)) {
        Ok(kind) => kind,
        Err(line) => return text(StatusCode::BAD_REQUEST, line),
    };
    if kind == "file" {
        let path = req.target.clone();
        match tokio::task::spawn_blocking(move || file_refusal(&path)).await {
            Ok(None) => {}
            Ok(Some(line)) => return text(StatusCode::BAD_REQUEST, line),
            Err(_) => return text(StatusCode::INTERNAL_SERVER_ERROR, "The path could not be checked."),
        }
    }
    let by = opener(&st, req.from.as_deref());
    let Ok((_, answer)) = st.opens().push(&tab, kind, req.target, by) else {
        return text(StatusCode::SERVICE_UNAVAILABLE, FULL);
    };
    // The session may have closed while the file was checked, after its requests were dropped.
    if st.registry().row(&tab).is_none() {
        st.opens().drop_session(&tab);
    }
    match tokio::time::timeout(st.open_wait(), answer).await {
        Ok(Ok(Outcome::Opened)) => StatusCode::NO_CONTENT.into_response(),
        Ok(Ok(Outcome::Refused(why))) if why == "limit" => text(
            StatusCode::CONFLICT,
            format!(
                "Session {} already has {} tabs in its side pane.",
                row.number,
                crate::layout::MAX_PANE_TABS
            ),
        ),
        Ok(Ok(Outcome::Refused(why))) if why == "session" => closed(row.number),
        Ok(Ok(Outcome::Refused(_))) => text(StatusCode::CONFLICT, BAD_ADDRESS),
        // Err: the queue went with the core. The session is as gone as a closed one.
        Ok(Ok(Outcome::Closed) | Err(_)) => closed(row.number),
        // The request stays in the queue: the tab opens when the app next looks.
        Err(_) => text(StatusCode::ACCEPTED, LATE),
    }
}

/// POST /v1/pane/opens/done
pub async fn done(State(st): State<AppState>, Json(body): Json<PaneOpensDone>) -> StatusCode {
    st.opens().done(&body.done);
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(name: &str) -> String {
        if cfg!(windows) {
            format!(r"C:\proj\{name}")
        } else {
            format!("/proj/{name}")
        }
    }

    #[test]
    fn only_http_and_https_text_is_an_address() {
        for address in [
            "http://example.com",
            "https://example.com/a?b=c#d",
            "HTTP://EXAMPLE.COM/",
            "hTtPs://example.com/",
            "http://localhost:3000/",
            "http://[::1]:3000/",
            "https://example.com/?next=a@b",
            "https://example.com/#a@b",
        ] {
            assert_eq!(classify(address, true), Ok("browser"), "{address}");
        }
        // Everything else is a path, and these are not ones that can be opened.
        for path in [
            "",
            "example.com",
            "localhost:3000",
            "javascript:alert(1)",
            "file:///C:/x",
            "ftp://example.com/",
            "http:/example.com",
            "http:example.com",
            "..\\x.md",
            "a b.md",
        ] {
            assert_eq!(classify(path, true), Err(BAD_PATH), "{path}");
        }
        assert_eq!(classify(&abs("a b.md"), true), Ok("file"));
    }

    #[test]
    fn an_address_that_hides_its_site_is_refused() {
        let long = format!("https://example.com/{}", "a".repeat(TARGET_MAX));
        for bad in [
            "http://",
            "https:///x",
            "http://?x",
            "http://#x",
            "http://:80/",
            "http://user:pw@example.com/",
            "http://user@example.com/",
            "http://@example.com/",
            "http://\\evil.example/",
            "https://exa mple.com/",
            "https://example.com/a b",
            "https://example.com/a\tb",
            "https://example.com/a\nb",
            "https://example.com/\u{0}",
            "https://example.com/\u{7f}",
            "https://example.com/\u{a0}",
            long.as_str(),
        ] {
            assert_eq!(classify(bad, true), Err(BAD_ADDRESS), "{bad:?}");
        }
        let at_the_limit = format!("https://example.com/{}", "é".repeat(TARGET_MAX - 20));
        assert_eq!(at_the_limit.chars().count(), TARGET_MAX);
        assert_eq!(classify(&at_the_limit, true), Ok("browser"));
        // A backslash ends the site as a slash does (browsers read it so), so this one shows where it goes.
        assert_eq!(classify("http://good.example\\@other.example/", true), Ok("browser"));
    }

    #[test]
    fn a_system_without_pages_says_so_for_every_address() {
        for address in ["https://example.com/", "http://user@example.com/", "HTTP://"] {
            assert_eq!(classify(address, false), Err(NO_PAGES), "{address}");
        }
        // Files open everywhere.
        assert_eq!(classify(&abs("a.md"), false), Ok("file"));
    }

    #[test]
    fn a_path_that_must_not_reach_the_disk_is_refused() {
        let long = abs(&"a".repeat(TARGET_MAX));
        for bad in [
            r"\\server\share\x.md",
            "//server/share/x.md",
            r"\\.\COM1",
            r"\\?\C:\x.md",
            r"\??\C:\x.md",
            "a.md",
            "./a.md",
            long.as_str(),
        ] {
            assert_eq!(classify(bad, true), Err(BAD_PATH), "{bad:?}");
        }
        let with_nul = abs("a\0.md");
        assert_eq!(classify(&with_nul, true), Err(BAD_PATH));
    }

    #[test]
    fn a_file_tab_needs_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.md");
        std::fs::write(&file, "x").unwrap();
        let text = |p: &std::path::Path| p.to_string_lossy().into_owned();
        assert_eq!(file_refusal(&text(&file)), None);
        let folder = text(tmp.path());
        assert_eq!(
            file_refusal(&folder),
            Some(format!("{folder} is a folder, not a file."))
        );
        let missing = text(&tmp.path().join("b.md"));
        assert_eq!(file_refusal(&missing), Some(format!("No file {missing}.")));
        // A folder that isn't there is a missing file too, not a crash.
        let deeper = text(&tmp.path().join("nope").join("b.md"));
        assert_eq!(file_refusal(&deeper), Some(format!("No file {deeper}.")));
    }

    fn pushed(opens: &Opens, tab: &str) -> (u64, oneshot::Receiver<Outcome>) {
        opens
            .push(tab, "file", abs("a.md"), "shell".into())
            .expect("the queue has room")
    }

    fn answer(id: u64, refused: Option<&str>) -> PaneOpenDone {
        PaneOpenDone {
            id,
            refused: refused.map(str::to_string),
        }
    }

    #[test]
    fn the_queue_shows_what_waits_oldest_first() {
        let opens = Opens::default();
        let mut shown = opens.subscribe();
        assert!(shown.borrow_and_update().is_empty());
        let (first, _a) = pushed(&opens, "t1");
        let (second, _b) = pushed(&opens, "t2");
        assert!(shown.has_changed().unwrap());
        let list = shown.borrow_and_update().clone();
        assert_eq!(list.iter().map(|o| o.id).collect::<Vec<_>>(), [first, second]);
        assert_eq!(
            (list[0].tab_id.as_str(), list[0].kind.as_str(), list[0].by.as_str()),
            ("t1", "file", "shell")
        );
        assert_ne!(first, second);
        // A later subscriber sees the same list at once.
        assert_eq!(opens.subscribe().borrow().len(), 2);
    }

    #[test]
    fn an_answer_reaches_the_request_it_names() {
        let opens = Opens::default();
        let (first, mut a) = pushed(&opens, "t1");
        let (second, mut b) = pushed(&opens, "t1");
        let (third, mut c) = pushed(&opens, "t1");
        opens.done(&[answer(second, Some("limit")), answer(third, None)]);
        assert_eq!(b.try_recv(), Ok(Outcome::Refused("limit".into())));
        assert_eq!(c.try_recv(), Ok(Outcome::Opened));
        assert!(a.try_recv().is_err());
        assert_eq!(
            opens.subscribe().borrow().iter().map(|o| o.id).collect::<Vec<_>>(),
            [first]
        );
    }

    #[test]
    fn an_answer_nobody_waits_for_changes_nothing() {
        let opens = Opens::default();
        let (first, a) = pushed(&opens, "t1");
        let mut shown = opens.subscribe();
        shown.borrow_and_update();
        opens.done(&[answer(first + 1, None), answer(0, Some("limit"))]);
        opens.done(&[]);
        assert!(!shown.has_changed().unwrap());
        // The route gave up: the app's answer still takes the request off the queue.
        drop(a);
        opens.done(&[answer(first, None)]);
        opens.done(&[answer(first, None)]);
        assert!(shown.borrow_and_update().is_empty());
    }

    #[test]
    fn a_closed_session_takes_its_requests_with_it() {
        let opens = Opens::default();
        let (_, mut a) = pushed(&opens, "t1");
        let (kept, mut b) = pushed(&opens, "t2");
        let (_, mut c) = pushed(&opens, "t1");
        opens.drop_session("t1");
        assert_eq!(a.try_recv(), Ok(Outcome::Closed));
        assert_eq!(c.try_recv(), Ok(Outcome::Closed));
        assert!(b.try_recv().is_err());
        assert_eq!(
            opens.subscribe().borrow().iter().map(|o| o.id).collect::<Vec<_>>(),
            [kept]
        );
        // A session with nothing waiting publishes nothing.
        let mut shown = opens.subscribe();
        shown.borrow_and_update();
        opens.drop_session("t3");
        assert!(!shown.has_changed().unwrap());
    }

    #[test]
    fn the_queue_holds_64_and_takes_more_once_one_is_answered() {
        let opens = Opens::default();
        let held: Vec<_> = (0..MAX_WAITING).map(|_| pushed(&opens, "t1")).collect();
        assert_eq!(opens.push("t1", "file", abs("a.md"), "shell".into()).err(), Some(Full));
        assert_eq!(opens.subscribe().borrow().len(), MAX_WAITING);
        opens.done(&[answer(held[0].0, None)]);
        let (next, _rx) = pushed(&opens, "t1");
        // An id is never given twice.
        assert!(held.iter().all(|(id, _)| *id != next));
    }
}
