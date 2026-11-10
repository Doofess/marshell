//! The browser tab's pages (slice 6q-3a): each one a WebView2 child webview of the main window, on one profile
//! under the Marshell home, apart from the person's own browser.
//!
//! A page gets no command, no plugin and no event of the app's: the capability names the app's own webview only
//! (main.rs's tests), and everything here is sent to that webview alone. Where a page may go is `rules`'s
//! allow-list, asked on every way in.

#[cfg(windows)]
mod com;
pub mod rules;

use rules::{Chord, Own, Placer, Rect, Registry, Report, Size};
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

fn refuse(app: &AppHandle, key: &str, url: &str) {
    let on_main = app.clone();
    let payload = PageUrl {
        key: key.to_string(),
        url: url.to_string(),
    };
    post(app, move || emit(&on_main, "browser://refused", payload));
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

fn shown_page(app: &AppHandle) -> Option<Webview> {
    let state = app.state::<BrowserState>();
    let label = {
        let registry = lock(&state.registry);
        registry.shown().and_then(|key| registry.label(key))?.to_string()
    };
    app.get_webview(&label)
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
    if let (Some(to), Some(page)) = (to, shown_page(app)) {
        set_bounds(&page, to);
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
    let to = lock(&state.placer).on_report(report, win, scale, state.now_ms());
    if let Some(page) = shown_page(app) {
        set_bounds(&page, to);
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
    let Some(page) = show.and_then(|label| app.get_webview(&label)) else {
        return;
    };
    if let Some(to) = lock(&state.placer).last() {
        set_bounds(&page, to);
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

/// Creates the key's page under `label`, hidden, loading `url`.
fn create(app: &AppHandle, key: &str, label: &str, url: Url) -> Result<(), String> {
    let state = app.state::<BrowserState>();
    let window = crate::main_window(app).ok_or("Marshell's window is gone.")?;
    // Whether a navigation is running. The builder's own handlers keep it, since they hear the first navigation
    // too, which starts before the COM handlers are in.
    let loading = Arc::new(AtomicBool::new(false));
    let builder = WebviewBuilder::new(label, WebviewUrl::External(url))
        .focused(false)
        .data_directory(state.profile.clone())
        .additional_browser_args(BROWSER_ARGS)
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
            let (app, key) = (app.clone(), key.to_string());
            move |url, _features| {
                if allowed(&app, &url) {
                    let on_main = app.clone();
                    let payload = PageUrl {
                        key: key.clone(),
                        url: url.to_string(),
                    };
                    post(&app, move || emit(&on_main, "browser://popup", payload));
                } else {
                    refuse(&app, &key, url.as_str());
                }
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
    com::install(app, &page, key, loading, *lock(&state.dark));
    if lock(&state.registry).label(key) != Some(label) {
        // Closed while it was being created.
        let _ = page.close();
        return Ok(());
    }
    // Hidden, at the size it will be shown at, so the site lays out once.
    if let Some(to) = lock(&state.placer).last() {
        set_bounds(&page, to);
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
    let label = lock(&app.state::<BrowserState>().registry).open(&key);
    let Some(label) = label else {
        let page = page(&app, &key).ok_or("The page is still opening.")?;
        return page.navigate(url).map_err(|e| e.to_string());
    };
    create(&app, &key, &label, url).inspect_err(|_| {
        lock(&app.state::<BrowserState>().registry).close(&key);
    })
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

/// Clears everything the pages' profile holds (sign-ins, storage, cache, history), then loads every page again.
#[tauri::command]
pub async fn browser_clear_data(app: AppHandle) -> Result<(), String> {
    supported()?;
    #[cfg(windows)]
    {
        let open = pages(&app);
        if let Some(first) = open.first() {
            com::clear_data(first).await?;
            for page in &open {
                com::nav(page, "reload")?;
            }
            return Ok(());
        }
        // The profile is reached through a page, so with none open one is made for the moment, blank and hidden.
        let page = blank_page(&app)?;
        let cleared = com::clear_data(&page).await;
        let _ = page.close();
        cleared
    }
    #[cfg(not(windows))]
    {
        Err(UNAVAILABLE.to_string())
    }
}

/// A hidden page that stays on `about:blank`, on the pages' profile; the app never hears of it.
#[cfg(windows)]
fn blank_page(app: &AppHandle) -> Result<Webview, String> {
    let state = app.state::<BrowserState>();
    let window = crate::main_window(app).ok_or("Marshell's window is gone.")?;
    let blank = Url::parse("about:blank").map_err(|e| e.to_string())?;
    let builder = WebviewBuilder::new("page-blank", WebviewUrl::External(blank))
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
}
