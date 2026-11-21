//! The browser tab's pages (slice 6q-3a): each one a WebView2 child webview of the main window, on one profile
//! under the Marshell home, apart from the person's own browser.
//!
//! A page gets no command, no plugin and no event of the app's: the capability names the app's own webview only
//! (main.rs's tests), and everything here is sent to that webview alone. Where a page may go is `rules`'s
//! allow-list, asked on every way in.

#[cfg(windows)]
mod com;
pub mod rules;

use rules::{
    Chord, CookieRule, Cookies, Emulated, Own, Placer, PopupGate, Rect, Registry, Report, Size, Viewports, fit,
};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::time::{Duration, Instant};
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, Webview, WebviewBuilder, WebviewUrl};

/// wry's default arguments without `msSmartScreenProtection`, so SmartScreen stays on for the pages (S12). Every
/// page must give WebView2 the same arguments: pages share one browser process, and a second set is refused.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI";
const UNAVAILABLE: &str = "Browser tabs aren't available on this system yet.";
/// What a command answers for an address the allow-list refuses; the app's webview hears `browser://refused` too.
const REFUSED: &str = "refused";
const NO_PAGE: &str = "The page isn't open.";
/// How long quitting waits for the pages' browser process to end, so it writes its cookies (S12 finding 12).
const QUIT_WAIT: Duration = Duration::from_secs(2);

type Job = Box<dyn FnOnce() + Send>;

/// The pages' state, managed by the app.
pub struct BrowserState {
    /// The WebView2 user-data folder every page shares: `<home>/browser-profile`.
    profile: PathBuf,
    /// The dev server the app itself is loaded from, in a dev build.
    dev_url: Option<String>,
    registry: Mutex<Registry>,
    placer: Mutex<Placer>,
    /// Each tab's viewport width, by its page's key, as the app last reported it.
    viewports: Mutex<Viewports>,
    /// What each page was last asked to emulate, by its webview's label.
    emulated: Mutex<Emulated>,
    /// The cookie rule the app told, and which pages keep data on disk.
    cookies: Mutex<Cookies>,
    /// Held while what the last run left is cleared ("until quit"): a page that keeps data is made after it.
    start_clear: tauri::async_runtime::Mutex<()>,
    /// The blank pages made so far: each gets a label of its own.
    blanks: AtomicU32,
    /// Marshell is quitting: a second ask to quit starts nothing.
    quitting: AtomicBool,
    /// The key the app last asked to show, which may not have its page yet.
    wanted: Mutex<Option<String>>,
    /// The chords the app's keymap claims: pressed in a page, they are the app's.
    claimed: Mutex<HashSet<Chord>>,
    /// The colour scheme the app asked pages to see; None until it has asked.
    dark: Mutex<Option<bool>>,
    /// The pages' browser process, 0 when there is none known.
    browser_pid: AtomicU32,
    started: Instant,
    /// Work for the main thread, from inside a WebView2 callback. `run_on_main_thread` runs its closure at once
    /// when it is called on the main thread, which is where those callbacks run; a thread in between makes it a
    /// real post, so nothing calls back into a webview from inside its own callback.
    poster: mpsc::Sender<Job>,
}

impl BrowserState {
    pub fn new(app: &AppHandle, home: &Path) -> Self {
        let (poster, jobs) = mpsc::channel::<Job>();
        let on_main = app.clone();
        std::thread::spawn(move || {
            for job in jobs {
                if on_main.run_on_main_thread(job).is_err() {
                    break;
                }
            }
        });
        let dev_url = app.config().build.dev_url.as_ref().filter(|_| tauri::is_dev());
        Self {
            profile: home.join("browser-profile"),
            dev_url: dev_url.map(Url::to_string),
            registry: Mutex::default(),
            placer: Mutex::default(),
            viewports: Mutex::default(),
            emulated: Mutex::default(),
            cookies: Mutex::default(),
            start_clear: tauri::async_runtime::Mutex::new(()),
            blanks: AtomicU32::new(0),
            quitting: AtomicBool::new(false),
            wanted: Mutex::default(),
            claimed: Mutex::default(),
            dark: Mutex::default(),
            browser_pid: AtomicU32::new(0),
            started: Instant::now(),
            poster,
        }
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The origins a page may never open: the dev server the app itself is loaded from, and the local API.
fn own_origins(dev_url: Option<&str>, api_port: Option<u16>) -> Vec<Own> {
    let dev = dev_url
        .and_then(|url| url.parse::<Url>().ok())
        .and_then(|url| Some((url.host_str()?.to_string(), url.port_or_known_default()?)));
    let api = api_port.map(|port| ("127.0.0.1".to_string(), port));
    dev.into_iter().chain(api).collect()
}

/// The allow-list, with the app's own origins as they are now: the core's port changes when the core restarts.
fn allowed(app: &AppHandle, url: &Url) -> bool {
    let state = app.state::<BrowserState>();
    let api_port = app
        .try_state::<crate::CoreHandle>()
        .and_then(|core| core.0.endpoint())
        .map(|endpoint| endpoint.port);
    rules::allowed_navigation(url, &own_origins(state.dev_url.as_deref(), api_port))
}

/// The page's address, title and history, as the app's webview hears them.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PageState {
    key: String,
    url: String,
    title: String,
    can_back: bool,
    can_forward: bool,
    loading: bool,
}

#[derive(Clone, Serialize)]
struct PageUrl {
    key: String,
    url: String,
    /// Why a refused address was refused, when it wasn't the allow-list: "popup" for a tab no person asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    why: Option<&'static str>,
}

#[derive(Clone, Serialize)]
struct PageKey {
    key: String,
}

#[derive(Clone, Serialize)]
struct Gone {}

#[derive(Clone, Serialize)]
struct PageFocus {
    key: String,
    focused: bool,
}

#[derive(Clone, Serialize)]
struct FocusOut {
    key: String,
    direction: &'static str,
}

/// A key the page gave up, with the page it was pressed in.
#[derive(Clone, Serialize)]
struct PageKeyEvent {
    key: String,
    #[serde(flatten)]
    event: rules::KeyEvent,
}

#[derive(Clone, Serialize)]
struct PageIcon {
    key: String,
    src: Option<String>,
}

/// A file drag over a page. `paths` only on a drop.
#[derive(Clone, Serialize)]
struct PageDrag {
    key: String,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    paths: Option<Vec<String>>,
}

/// Gives the keyboard to the app's own webview. On the main thread, never inside a page's callback.
fn focus_app(app: &AppHandle) {
    if let Some(main) = app.get_webview("main")
        && let Err(e) = main.set_focus()
    {
        eprintln!("Marshell: the window did not take the keyboard back from a page: {e}");
    }
}

/// Tells the app's webview about a file dragged over a page: the page's webview hears its own drops, and the app's
/// doesn't (S12 finding 19).
fn on_drag(app: &AppHandle, key: &str, event: &tauri::WebviewEvent) {
    let tauri::WebviewEvent::DragDrop(drag) = event else {
        return;
    };
    let (kind, paths) = match drag {
        tauri::DragDropEvent::Enter { .. } => ("enter", None),
        tauri::DragDropEvent::Drop { paths, .. } => {
            let paths = paths.iter().map(|path| path.to_string_lossy().into_owned()).collect();
            ("drop", Some(paths))
        }
        tauri::DragDropEvent::Leave => ("leave", None),
        // Over comes on every mouse move; enter, drop and leave say what happened.
        _ => return,
    };
    let on_main = app.clone();
    let payload = PageDrag {
        key: key.to_string(),
        kind,
        paths,
    };
    post(app, move || emit(&on_main, "browser://drag", payload));
}

/// Sends to the app's own webview only. A page has no listener and nothing to listen with.
fn emit<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(e) = app.emit_to("main", event, payload) {
        eprintln!("Marshell: the window did not hear {event}: {e}");
    }
}

/// Queues work for the main thread's next turn. Safe inside a WebView2 callback.
fn post(app: &AppHandle, job: impl FnOnce() + Send + 'static) {
    if let Some(state) = app.try_state::<BrowserState>() {
        let _ = state.poster.send(Box::new(job));
    }
}

/// The blank page.
fn blank() -> Url {
    "about:blank".parse().expect("about:blank is an address")
}

fn refuse(app: &AppHandle, key: &str, url: &str) {
    let on_main = app.clone();
    let payload = refusal(key, url, None);
    post(app, move || emit(&on_main, "browser://refused", payload));
}

/// What the app hears about a refused address: the address cut to what it keeps, since a page chose its length.
fn refusal(key: &str, url: &str, why: Option<&'static str>) -> PageUrl {
    PageUrl {
        key: key.to_string(),
        url: rules::clipped_address(url),
        why,
    }
}

/// A tab the page asked for, on the main thread: the app opens it only when a person asked (a click, a key). A
/// script that opens tabs on its own is told no, and the app says so under the bar.
fn popup(app: &AppHandle, key: &str, url: &Url, gate: &Mutex<PopupGate>) {
    let now = app.state::<BrowserState>().now_ms();
    let person = lock(gate).by_a_person(url, now);
    if !person {
        emit(app, "browser://refused", refusal(key, url.as_str(), Some("popup")));
    } else if url.as_str().chars().count() > rules::ADDRESS_MAX {
        // Longer than a tab's address may be: the app would drop it without a word.
        emit(app, "browser://refused", refusal(key, url.as_str(), None));
    } else {
        let payload = PageUrl {
            key: key.to_string(),
            url: url.to_string(),
            why: None,
        };
        emit(app, "browser://popup", payload);
    }
}

/// Runs `job` off the async runtime's own threads. Making a page waits for the main thread, and WebView2 can take
/// a while there: on a runtime worker that wait would hold up every command and every timer that shares it.
async fn off_the_runtime<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|e| e.to_string())
}

/// The typed or asked-for address as a URL the allow-list lets through; None, and the app is told, otherwise.
fn checked(app: &AppHandle, key: &str, text: &str) -> Option<Url> {
    let url = text.parse::<Url>().ok().filter(|url| allowed(app, url));
    if url.is_none() {
        refuse(app, key, text);
    }
    url
}

/// The key's page. The registry's lock is let go before the webview is touched.
fn page(app: &AppHandle, key: &str) -> Option<Webview> {
    let label = lock(&app.state::<BrowserState>().registry).label(key)?.to_string();
    app.get_webview(&label)
}

/// The page that is shown, and its key.
fn shown_page(app: &AppHandle) -> Option<(String, Webview)> {
    let state = app.state::<BrowserState>();
    let (key, label) = {
        let registry = lock(&state.registry);
        let key = registry.shown()?;
        (key.to_string(), registry.label(key)?.to_string())
    };
    Some((key, app.get_webview(&label)?))
}

/// Logical position and size, never negative.
fn set_bounds(page: &Webview, to: Rect) {
    let bounds = tauri::Rect {
        position: LogicalPosition::new(to.x.max(0.0), to.y.max(0.0)).into(),
        size: LogicalSize::new(to.width.max(0.0), to.height.max(0.0)).into(),
    };
    if let Err(e) = page.set_bounds(bounds) {
        eprintln!("Marshell: a browser page was not placed: {e}");
    }
}

/// Puts the key's page where its tab's viewport says it goes in `slot`, the page's whole place: centred at its own
/// width when that fits, and laid out at its width and drawn smaller when it doesn't (6r-1b decisions 15 to 17).
/// The page is asked to change its layout only when that differs from what it was last asked for, so a drag's
/// many calls send one request per change; a request that fails is forgotten, and the next call asks again.
fn settle(app: &AppHandle, key: &str, page: &Webview, slot: Rect) {
    let state = app.state::<BrowserState>();
    let fit = fit(slot, lock(&state.viewports).of(key));
    set_bounds(page, fit.rect);
    #[cfg(windows)]
    {
        let label = page.label().to_string();
        let (before, free) = {
            let mut emulated = lock(&state.emulated);
            let Some(before) = emulated.ask(&label, fit.emulate) else {
                return;
            };
            (before, emulated.begin(&label, fit.emulate))
        };
        // One call per page is in flight: while it is unanswered a newer want only waits, so a drag that asks for a
        // new scale at every pixel never leaves a thread parked on each of them, and answers apply in the order asked.
        if free {
            send_emulation(app.clone(), label, fit.emulate, before);
        }
    }
}

/// Sends one emulation and, when it is answered, the newest one that was asked meanwhile. `before` is what the page
/// had if this call fails.
#[cfg(windows)]
fn send_emulation(app: AppHandle, label: String, want: Option<rules::Emulate>, before: Option<rules::Emulate>) {
    let Some(page) = app.get_webview(&label) else {
        lock(&app.state::<BrowserState>().emulated).forget(&label);
        return;
    };
    com::emulate(&page, want, move |took| {
        let there = app.get_webview(&label).is_some();
        let next = {
            let state = app.state::<BrowserState>();
            let mut emulated = lock(&state.emulated);
            if !there {
                emulated.forget(&label);
                None
            } else {
                if !took {
                    emulated.failed(&label, want, before);
                }
                let has = if took { want } else { before };
                let next = emulated.finish(&label, has);
                if let Some(next) = next {
                    // What is asked now is the one in flight, so a failure of it is judged against it.
                    emulated.ask(&label, next);
                }
                next.map(|next| (next, has))
            }
        };
        if let Some((next, has)) = next {
            send_emulation(app, label, next, has);
        }
    });
}

/// The main window was resized: the page moves at once, inside the window's own resize, without a round trip to
/// the app's webview. Runs on the main thread, also during an edge drag's sizing loop.
pub fn on_resized(window: &tauri::Window, size: tauri::PhysicalSize<u32>) {
    let app = window.app_handle();
    let Some(state) = app.try_state::<BrowserState>() else {
        return;
    };
    let size = size.to_logical::<f64>(window.scale_factor().unwrap_or(1.0));
    let win = Size {
        width: size.width,
        height: size.height,
    };
    let to = lock(&state.placer).on_resize(win, state.now_ms());
    if let (Some(to), Some((key, page))) = (to, shown_page(app)) {
        settle(app, &key, &page, to);
    }
}

/// A report from the app, applied on the main thread: the window's size is read there without a round trip, and
/// the move can't land between a resize and its own move.
fn place(app: &AppHandle, report: Report) {
    let state = app.state::<BrowserState>();
    let Some(window) = crate::main_window(app) else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let Ok(size) = window.inner_size() else {
        return;
    };
    let size = size.to_logical::<f64>(scale);
    let win = Size {
        width: size.width,
        height: size.height,
    };
    // The report's viewport is its own page's, which is the shown page's unless the app has just changed tabs.
    if let Some(key) = &report.page {
        lock(&state.viewports).set(key, report.viewport);
    }
    let to = lock(&state.placer).on_report(report, win, scale, state.now_ms());
    if let Some((key, page)) = shown_page(app) {
        settle(app, &key, &page, to);
    }
}

/// Hides every page but the key's, and shows that one at the box last decided. On the main thread.
fn show(app: &AppHandle, key: Option<&str>) {
    let state = app.state::<BrowserState>();
    *lock(&state.wanted) = key.map(str::to_string);
    let (hide, show) = lock(&state.registry).show(key);
    for label in hide {
        if let Some(page) = app.get_webview(&label) {
            let _ = page.hide();
        }
    }
    // A page hidden while it had the keyboard would keep it, and nothing the person typed next would show anywhere:
    // the keyboard goes to the app, whose own focus says where it belongs.
    #[cfg(windows)]
    if com::keyboard_on_hidden_window() {
        focus_app(app);
    }
    let Some(page) = show.and_then(|label| app.get_webview(&label)) else {
        return;
    };
    let slot = lock(&state.placer).last();
    if let (Some(key), Some(slot)) = (key, slot) {
        settle(app, key, &page, slot);
    }
    if let Err(e) = page.show() {
        eprintln!("Marshell: a browser page was not shown: {e}");
    }
}

/// Whether the app's window is still the one in front. A page taking the keyboard inside the window takes the
/// focus from the window's own handle, and that is not the person leaving Marshell.
pub fn still_in_front(window: &tauri::Window) -> bool {
    #[cfg(windows)]
    {
        window.hwnd().is_ok_and(com::in_front)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        false
    }
}

/// The pages' browser process has ended (a crash, or it was killed): every page is gone with it. On the main
/// thread. Each page reports the same end, so only the first report finds pages to close.
fn browser_gone(app: &AppHandle) {
    let state = app.state::<BrowserState>();
    let labels = lock(&state.registry).clear();
    if labels.is_empty() {
        return;
    }
    // The tabs keep their viewports: the app sends no new report for a page that is loaded again in its place.
    lock(&state.emulated).clear();
    lock(&state.cookies).none_left();
    state.browser_pid.store(0, Ordering::SeqCst);
    for label in labels {
        if let Some(page) = app.get_webview(&label) {
            let _ = page.close();
        }
    }
    emit(app, "browser://gone", Gone {});
}

/// Closes every page before Marshell exits, and waits for their browser process to end: a sign-in survives only a
/// clean close (S12 finding 12). On the main thread; nothing to do, and no wait, with no page open.
pub fn close_all(app: &AppHandle) {
    let Some(state) = app.try_state::<BrowserState>() else {
        return;
    };
    let labels = lock(&state.registry).clear();
    lock(&state.viewports).clear();
    lock(&state.emulated).clear();
    lock(&state.cookies).none_left();
    if labels.is_empty() {
        return;
    }
    #[cfg(windows)]
    let browser = com::BrowserProcess::open(state.browser_pid.swap(0, Ordering::SeqCst));
    for label in labels {
        if let Some(page) = app.get_webview(&label) {
            let _ = page.close();
        }
    }
    #[cfg(windows)]
    if let Some(browser) = browser
        && !browser.wait(QUIT_WAIT)
    {
        eprintln!("Marshell: the browser pages were still closing at exit; a sign-in from the last minute may be lost");
    }
}

fn supported() -> Result<(), String> {
    if cfg!(windows) {
        Ok(())
    } else {
        Err(UNAVAILABLE.to_string())
    }
}

/// Creates the key's page under `label`, hidden, loading `url`. A `private` page keeps nothing on disk: its cookies
/// and storage live in memory, shared with the other private pages, and end with the last of them (S14, result 3).
fn create(app: &AppHandle, key: &str, label: &str, url: Url, private: bool) -> Result<(), String> {
    let state = app.state::<BrowserState>();
    let window = crate::main_window(app).ok_or("Marshell's window is gone.")?;
    // Whether a navigation is running. The builder's own handlers keep it, since they hear the first navigation
    // too, which starts before the COM handlers are in. A page is made on its way to its first address, so it
    // starts as loading: its first state, sent while it still says `about:blank`, must not read as a page that
    // has arrived there (the app would save that as the tab's address).
    let loading = Arc::new(AtomicBool::new(true));
    // What WebView2 says about each tab the page asks for: recorded by the COM handler, read by the builder's.
    let gate = Arc::new(Mutex::new(PopupGate::default()));
    let builder = WebviewBuilder::new(label, WebviewUrl::External(url))
        .focused(false)
        .data_directory(state.profile.clone())
        .additional_browser_args(BROWSER_ARGS)
        .incognito(private)
        // Every top-level navigation: a link, a redirect, a script, Back and Forward.
        .on_navigation({
            let (app, key, loading) = (app.clone(), key.to_string(), loading.clone());
            move |url| {
                let ok = allowed(&app, url);
                if ok {
                    loading.store(true, Ordering::SeqCst);
                } else {
                    refuse(&app, &key, url.as_str());
                }
                ok
            }
        })
        // A page never opens a window. What it asked for goes to the app, which opens it as a tab or doesn't.
        .on_new_window({
            let (app, key, gate) = (app.clone(), key.to_string(), gate.clone());
            move |url, _features| {
                // Queued either way, so the gate's answer for this address is taken and can't answer for the next.
                let (on_main, key, gate, ok) = (app.clone(), key.clone(), gate.clone(), allowed(&app, &url));
                post(&app, move || {
                    if ok {
                        popup(&on_main, &key, &url, &gate);
                    } else {
                        let now = on_main.state::<BrowserState>().now_ms();
                        lock(&gate).take(&url, now);
                        emit(&on_main, "browser://refused", refusal(&key, url.as_str(), None));
                    }
                });
                NewWindowResponse::Deny
            }
        })
        .on_page_load({
            let loading = loading.clone();
            move |_page, payload| {
                if matches!(payload.event(), PageLoadEvent::Finished) {
                    loading.store(false, Ordering::SeqCst);
                }
            }
        });
    // A release build has no DevTools unless they are asked for; a debug build has them unless they are refused.
    #[cfg(debug_assertions)]
    let builder = builder.devtools(false);
    // Created with no size and then hidden, so it never draws over the page that is shown.
    let page = window
        .add_child(builder, LogicalPosition::new(0.0, 0.0), LogicalSize::new(0.0, 0.0))
        .map_err(|e| e.to_string())?;
    let _ = page.hide();
    page.on_webview_event({
        let (app, key) = (app.clone(), key.to_string());
        move |event| on_drag(&app, &key, event)
    });
    #[cfg(windows)]
    com::install(app, &page, key, loading, gate, *lock(&state.dark));
    if lock(&state.registry).label(key) != Some(label) {
        // Closed while it was being created.
        let _ = page.close();
        return Ok(());
    }
    lock(&state.cookies).made(key, private);
    // Hidden, at the size and the viewport it will be shown at, so the site lays out once.
    let slot = lock(&state.placer).last();
    if let Some(slot) = slot {
        settle(app, key, &page, slot);
    }
    if lock(&state.wanted).as_deref() == Some(key) {
        let (on_main, key) = (app.clone(), key.to_string());
        app.run_on_main_thread(move || show(&on_main, Some(&key)))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Opens `url` in the key's page, creating the page when the key has none.
#[tauri::command]
pub async fn browser_open(app: AppHandle, key: String, url: String) -> Result<(), String> {
    supported()?;
    let Some(url) = checked(&app, &key, &url) else {
        return Err(REFUSED.to_string());
    };
    // No page is made before the app has told the cookie rule: a page made on a guess could keep what it shouldn't.
    let private = lock(&app.state::<BrowserState>().cookies).private()?;
    let label = lock(&app.state::<BrowserState>().registry).open(&key);
    let Some(label) = label else {
        let page = page(&app, &key).ok_or("The page is still opening.")?;
        return page.navigate(url).map_err(|e| e.to_string());
    };
    // Held until the page exists, so the pages' browser process doesn't end and start again in between.
    #[cfg(windows)]
    let _cleared_through = if private { None } else { start_clear(&app).await };
    let (on_runtime, made) = (app.clone(), key.clone());
    off_the_runtime(move || create(&on_runtime, &made, &label, url, private))
        .await
        .and_then(|created| created)
        .inspect_err(|_| {
            lock(&app.state::<BrowserState>().registry).close(&key);
        })
}

/// A blank page that closes when it is let go.
#[cfg(windows)]
struct Blank(Webview);

#[cfg(windows)]
impl Drop for Blank {
    fn drop(&mut self) {
        let _ = self.0.close();
    }
}

/// How long the first page of a run waits for what the last run left to be cleared.
#[cfg(windows)]
const START_CLEAR_WAIT: Duration = Duration::from_secs(5);

/// Before a page that keeps data is made: under "until quit", the first one in a run waits here while the cookies
/// and site data the last run left are cleared (a run that ended without its quit leaves them), and a page asked
/// for in the meantime waits for the same clear. The clear goes through a blank page, which is given back so the
/// caller keeps it until its own page exists. The page is made whether the clear answered or not.
#[cfg(windows)]
async fn start_clear(app: &AppHandle) -> Option<Blank> {
    let state = app.state::<BrowserState>();
    let _one_at_a_time = state.start_clear.lock().await;
    if !lock(&state.cookies).take_start_clear() {
        return None;
    }
    let on_runtime = app.clone();
    let blank = match off_the_runtime(move || blank_page(&on_runtime))
        .await
        .and_then(|made| made)
    {
        Ok(page) => Blank(page),
        Err(e) => {
            eprintln!("Marshell: the last run's cookies were not cleared: {e}");
            return None;
        }
    };
    if let Err(e) = com::clear_site_data(&blank.0, START_CLEAR_WAIT).await {
        eprintln!("Marshell: the last run's cookies were not cleared: {e}");
    }
    Some(blank)
}

/// Shows the key's page and hides every other; `None` hides them all.
#[tauri::command]
pub async fn browser_show(app: AppHandle, key: Option<String>) -> Result<(), String> {
    supported()?;
    let on_main = app.clone();
    app.run_on_main_thread(move || show(&on_main, key.as_deref()))
        .map_err(|e| e.to_string())
}

/// Where the shown page goes, as the app measured it, with the layout rule for the window's next resize.
#[tauri::command]
pub async fn browser_place(app: AppHandle, report: Report) -> Result<(), String> {
    supported()?;
    let on_main = app.clone();
    app.run_on_main_thread(move || place(&on_main, report))
        .map_err(|e| e.to_string())
}

/// `action`: "go" (with `url`), "back", "forward", "reload" or "stop".
#[tauri::command]
pub async fn browser_nav(app: AppHandle, key: String, action: String, url: Option<String>) -> Result<(), String> {
    supported()?;
    if action == "go" {
        let url = url.ok_or("go needs an address")?;
        let Some(url) = checked(&app, &key, &url) else {
            return Err(REFUSED.to_string());
        };
        return page(&app, &key)
            .ok_or(NO_PAGE)?
            .navigate(url)
            .map_err(|e| e.to_string());
    }
    let page = page(&app, &key).ok_or(NO_PAGE)?;
    #[cfg(windows)]
    {
        com::nav(&page, &action)
    }
    #[cfg(not(windows))]
    {
        let _ = page;
        Err(UNAVAILABLE.to_string())
    }
}

/// Closes the key's page and forgets the key. A key with no page is not an error.
#[tauri::command]
pub async fn browser_close(app: AppHandle, key: String) -> Result<(), String> {
    supported()?;
    let state = app.state::<BrowserState>();
    let label = lock(&state.registry).close(&key);
    {
        let mut wanted = lock(&state.wanted);
        if wanted.as_deref() == Some(key.as_str()) {
            *wanted = None;
        }
    }
    // The tab keeps its viewport (its page may be loaded again in its place); what the page was asked to emulate
    // goes with the page.
    if let Some(label) = &label {
        lock(&state.emulated).forget(label);
    }
    lock(&state.cookies).gone(&key);
    // A page still being created has no webview yet: `create` closes it when it sees the key is gone.
    match label.and_then(|label| app.get_webview(&label)) {
        Some(page) => page.close().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Every page there is, in the order the pages were opened.
fn pages(app: &AppHandle) -> Vec<Webview> {
    let labels = lock(&app.state::<BrowserState>().registry).labels();
    labels.iter().filter_map(|label| app.get_webview(label)).collect()
}

/// The chords the app's keymap claims, replacing the last list. Esc, Ctrl+L and F12 need no claim.
#[tauri::command]
pub async fn browser_keys(app: AppHandle, chords: Vec<Chord>) -> Result<(), String> {
    supported()?;
    *lock(&app.state::<BrowserState>().claimed) = chords.into_iter().collect();
    Ok(())
}

/// Moves the keyboard into the key's page. `reason`: "next" (to its first element), "previous" (to its last) or
/// "programmatic" (to where it was).
#[tauri::command]
pub async fn browser_focus(app: AppHandle, key: String, reason: String) -> Result<(), String> {
    supported()?;
    let page = page(&app, &key).ok_or(NO_PAGE)?;
    #[cfg(windows)]
    {
        com::focus(&page, &reason)
    }
    #[cfg(not(windows))]
    {
        let _ = (page, reason);
        Err(UNAVAILABLE.to_string())
    }
}

/// The key's page as it was last drawn, as a `data:image/png;base64,` URL. `Err("hidden")` at once for a page that
/// is not the shown one: WebView2 draws nothing for a hidden page, and the capture would wait for its limit.
#[tauri::command]
pub async fn browser_capture(app: AppHandle, key: String) -> Result<String, String> {
    supported()?;
    let shown = lock(&app.state::<BrowserState>().registry).shown() == Some(key.as_str());
    if !shown {
        return Err("hidden".to_string());
    }
    let page = page(&app, &key).ok_or(NO_PAGE)?;
    #[cfg(windows)]
    {
        let png = com::capture(&page).await?;
        Ok(format!("data:image/png;base64,{}", rules::base64(&png)))
    }
    #[cfg(not(windows))]
    {
        let _ = page;
        Err(UNAVAILABLE.to_string())
    }
}

/// The colour scheme pages see: the app's theme. Kept for the pages opened later.
#[tauri::command]
pub async fn browser_theme(app: AppHandle, dark: bool) -> Result<(), String> {
    supported()?;
    *lock(&app.state::<BrowserState>().dark) = Some(dark);
    #[cfg(windows)]
    for page in pages(&app) {
        com::theme(&page, dark);
    }
    Ok(())
}

/// Clears everything the pages' profile holds (sign-ins, storage, cache, history), then loads the pages that keep
/// data again.
#[tauri::command]
pub async fn browser_clear_data(app: AppHandle) -> Result<(), String> {
    supported()?;
    #[cfg(windows)]
    {
        // The profile on disk is reached through a page that keeps data. A private page's profile is the one in
        // memory, and clearing through it would leave the disk as it was; so with no page that keeps data open, one
        // is made for the moment, blank and hidden. The private pages' own data ends when the app loads them again.
        match kept_page(&app) {
            Some(page) => com::clear_data(&page).await?,
            None => com::clear_data(&Blank(blank_page(&app)?).0).await?,
        }
        // Only the pages that keep data are loaded again here; a private page is made again by the app.
        let kept = lock(&app.state::<BrowserState>().cookies).kept_keys();
        for page in kept.iter().filter_map(|key| page(&app, key)) {
            com::nav(&page, "reload")?;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err(UNAVAILABLE.to_string())
    }
}

/// A picture of a page for "Save screenshot": the PNG in base64, and whether the page was longer than one picture
/// holds.
#[derive(Serialize)]
pub struct Shot {
    png: String,
    cut: bool,
}

/// A picture of the key's page: the part on screen, or with `full` the whole page from its top, cut at 16,384
/// pixels a side. `Err("hidden")` at once for a page that is not the shown one, which never answers a capture.
/// Nothing but the key and the choice comes from the app: the page's size is measured here.
#[tauri::command]
pub async fn browser_screenshot(app: AppHandle, key: String, full: bool) -> Result<Shot, String> {
    supported()?;
    let label = lock(&app.state::<BrowserState>().registry)
        .shot_target(&key)
        .map(str::to_string)?;
    let page = app.get_webview(&label).ok_or(NO_PAGE)?;
    #[cfg(windows)]
    {
        let (clip, cut) = if full {
            let metrics = com::page_metrics(&page).await?;
            let scale = crate::main_window(&app)
                .and_then(|window| window.scale_factor().ok())
                .unwrap_or(1.0);
            let (clip, cut) = rules::content_size(&metrics)
                .and_then(|size| rules::shot_clip(size, scale))
                .ok_or("the page has no size")?;
            (Some(clip), cut)
        } else {
            (None, false)
        };
        let answer = com::screenshot(&page, clip).await?;
        let png = rules::shot_png(&answer).ok_or("the page gave no picture")?;
        Ok(Shot { png, cut })
    }
    #[cfg(not(windows))]
    {
        let _ = (page, full);
        Err(UNAVAILABLE.to_string())
    }
}

/// The cookie rule from the app's settings: "always", "until-quit" or "never". It decides the pages made from now
/// on; the app loads the open ones again when that changes what they keep.
#[tauri::command]
pub async fn browser_cookies(app: AppHandle, rule: String) -> Result<(), String> {
    let rule = CookieRule::parse(&rule).ok_or("no such cookie rule")?;
    lock(&app.state::<BrowserState>().cookies).tell(rule);
    Ok(())
}

/// A page that keeps data on disk, to clear the profile through.
#[cfg(windows)]
fn kept_page(app: &AppHandle) -> Option<Webview> {
    let key = lock(&app.state::<BrowserState>().cookies)
        .kept_page()
        .map(str::to_string)?;
    page(app, &key)
}

/// How long quitting waits for the cookies to be cleared.
#[cfg(windows)]
const QUIT_CLEAR_WAIT: Duration = Duration::from_secs(3);

/// Before Marshell quits: under "until quit" once a page that kept data was made in this run, or under "never" after
/// a page was made under "until quit" and "always" was not chosen since, the cookies and site
/// data are cleared, through such a page or a blank one made for it, and `then` runs on the main thread when the
/// clear has answered or `QUIT_CLEAR_WAIT` has passed, whichever is first; otherwise `then` runs at once. A second
/// call while the first is waiting does nothing. A run that ends another way leaves the clear to the next start.
pub fn before_quit(app: &AppHandle, then: impl FnOnce(&AppHandle) + Send + 'static) {
    let Some(state) = app.try_state::<BrowserState>() else {
        return then(app);
    };
    if state.quitting.swap(true, Ordering::SeqCst) {
        return;
    }
    if !cfg!(windows) || !lock(&state.cookies).clears_at_quit() {
        return then(app);
    }
    #[cfg(windows)]
    {
        // Whichever comes first, the clear's end or the limit, takes `then` and runs it; the other finds it gone.
        fn finish<F: FnOnce(&AppHandle) + Send + 'static>(app: &AppHandle, then: &Mutex<Option<F>>) {
            let Some(then) = lock(then).take() else {
                return;
            };
            let on_main = app.clone();
            if app.run_on_main_thread(move || then(&on_main)).is_err() {
                eprintln!("Marshell: quitting could not go on after the cookies were cleared");
                // `then` is gone with the call that failed, so the next quit asked for starts over.
                if let Some(state) = app.try_state::<BrowserState>() {
                    state.quitting.store(false, Ordering::SeqCst);
                }
            }
        }
        let then = Arc::new(Mutex::new(Some(then)));
        let (late_app, late_then) = (app.clone(), then.clone());
        std::thread::spawn(move || {
            std::thread::sleep(QUIT_CLEAR_WAIT);
            finish(&late_app, &late_then);
        });
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let cleared = match kept_page(&app) {
                Some(page) => com::clear_site_data(&page, QUIT_CLEAR_WAIT).await,
                None => match blank_page(&app) {
                    Ok(page) => com::clear_site_data(&Blank(page).0, QUIT_CLEAR_WAIT).await,
                    Err(e) => Err(e),
                },
            };
            if let Err(e) = cleared {
                eprintln!("Marshell: the cookies were not cleared at quit; the next start clears them: {e}");
            }
            finish(&app, &then);
        });
    }
}

/// A hidden page that stays on `about:blank`, on the pages' profile; the app never hears of it. It keeps data, as
/// the profile on disk is what it is made to reach.
#[cfg(windows)]
fn blank_page(app: &AppHandle) -> Result<Webview, String> {
    let state = app.state::<BrowserState>();
    let window = crate::main_window(app).ok_or("Marshell's window is gone.")?;
    let blank = Url::parse("about:blank").map_err(|e| e.to_string())?;
    // A label of its own each time: two clears can meet (a start's and the person's), and a label is one page's.
    let label = format!("blank-{}", state.blanks.fetch_add(1, Ordering::SeqCst) + 1);
    let builder = WebviewBuilder::new(label, WebviewUrl::External(blank))
        .focused(false)
        .data_directory(state.profile.clone())
        .additional_browser_args(BROWSER_ARGS)
        .on_navigation(|url| url.as_str() == "about:blank")
        .on_new_window(|_url, _features| NewWindowResponse::Deny);
    let page = window
        .add_child(builder, LogicalPosition::new(0.0, 0.0), LogicalSize::new(0.0, 0.0))
        .map_err(|e| e.to_string())?;
    let _ = page.hide();
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_own_origins_name_the_dev_server_and_the_api() {
        assert_eq!(
            own_origins(Some("http://localhost:1420/"), Some(49152)),
            [("localhost".to_string(), 1420), ("127.0.0.1".to_string(), 49152)]
        );
        assert_eq!(own_origins(None, None), []);
        // A dev URL on the scheme's own port still names a port.
        assert_eq!(
            own_origins(Some("http://devbox.lan/"), None),
            [("devbox.lan".to_string(), 80)]
        );
        assert_eq!(own_origins(Some("not a url"), None), []);
    }

    #[test]
    fn making_a_page_leaves_the_runtime_free() {
        // One thread runs everything on this runtime. The job waits for something only that thread can do, so a
        // job run on it never gets its answer.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let (tx, rx) = mpsc::channel::<()>();
        let answered = runtime.block_on(async move {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                let _ = tx.send(());
            });
            off_the_runtime(move || rx.recv_timeout(Duration::from_secs(3)).is_ok()).await
        });
        assert_eq!(answered, Ok(true));
    }

    #[test]
    fn a_refusal_carries_its_reason_and_a_short_address() {
        let json = |r: PageUrl| serde_json::to_value(r).unwrap();
        assert_eq!(
            json(refusal("s/t1", "javascript:alert(1)", None)),
            serde_json::json!({"key": "s/t1", "url": "javascript:alert(1)"})
        );
        assert_eq!(
            json(refusal("s/t1", "https://example.com/", Some("popup"))),
            serde_json::json!({"key": "s/t1", "url": "https://example.com/", "why": "popup"})
        );
        let long = format!("https://example.com/{}", "a".repeat(5000));
        assert_eq!(json(refusal("s/t1", &long, None))["url"].as_str().unwrap().len(), 4096);
    }
}
