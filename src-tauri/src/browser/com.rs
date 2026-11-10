//! The WebView2 layer under a page (Windows). A handler runs inside WebView2's own callback, on the main thread: it
//! reads its event and queues what follows with `post`, and never calls back into a webview from inside the callback.

use super::rules::{KeyRoute, MAX_ICON_BYTES, focus_out_direction, icon_src, key_event, route_key};
use super::{
    BrowserState, FocusOut, Job, PageFocus, PageIcon, PageKey, PageKeyEvent, PageState, emit, focus_app, lock, post,
    refuse,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use tauri::{AppHandle, Manager, Url, Webview};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, COREWEBVIEW2_KEY_EVENT_KIND,
    COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_KEY_UP,
    COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_UP,
    COREWEBVIEW2_MOVE_FOCUS_REASON, COREWEBVIEW2_MOVE_FOCUS_REASON_NEXT, COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS,
    COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, COREWEBVIEW2_PHYSICAL_KEY_STATUS,
    COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK, COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT,
    COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED, ICoreWebView2, ICoreWebView2_13, ICoreWebView2_15,
    ICoreWebView2Controller, ICoreWebView2Profile2,
};
use webview2_com::{
    AcceleratorKeyPressedEventHandler, CapturePreviewCompletedHandler, ClearBrowsingDataCompletedHandler,
    DocumentTitleChangedEventHandler, FaviconChangedEventHandler, FocusChangedEventHandler, GetFaviconCompletedHandler,
    HistoryChangedEventHandler, MoveFocusRequestedEventHandler, NavigationCompletedEventHandler,
    NavigationStartingEventHandler, ProcessFailedEventHandler, SourceChangedEventHandler, take_pwstr,
};
use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
use windows::Win32::System::Com::{IStream, STREAM_SEEK_SET};
use windows_core::Interface;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

fn failed(what: &str, e: &windows_core::Error) {
    eprintln!("Marshell: a browser page's {what} failed: {e}");
}

/// For what an old WebView2 runtime doesn't have: said once, and the page goes on without it.
fn missing(what: &str, e: &windows_core::Error) {
    static SAID: AtomicBool = AtomicBool::new(false);
    if !SAID.swap(true, Ordering::SeqCst) {
        eprintln!("Marshell: this WebView2 runtime has no {what} for browser pages: {e}");
    }
}

fn down(vk: u16) -> bool {
    // SAFETY: reads this thread's key state; the handler runs on the thread that took the key message.
    unsafe { GetKeyState(i32::from(vk)) < 0 }
}

/// What a page's handlers carry: no pointer to the webview, so a handler can't keep its own webview alive.
#[derive(Clone)]
struct Page {
    app: AppHandle,
    key: String,
    loading: Arc<AtomicBool>,
}

impl Page {
    /// The page's address, title and history, read from the sender of an event inside that event.
    fn state(&self, w: &ICoreWebView2) -> windows_core::Result<PageState> {
        let (mut url, mut title) = (windows_core::PWSTR::null(), windows_core::PWSTR::null());
        let (mut back, mut forward) = (windows_core::BOOL::default(), windows_core::BOOL::default());
        // SAFETY: getters on the live webview, on its own thread, each writing to a local that outlives the call;
        // take_pwstr frees each string WebView2 allocated.
        unsafe {
            w.Source(&mut url)?;
            w.DocumentTitle(&mut title)?;
            w.CanGoBack(&mut back)?;
            w.CanGoForward(&mut forward)?;
        }
        Ok(PageState {
            key: self.key.clone(),
            url: take_pwstr(url),
            title: take_pwstr(title),
            can_back: back.as_bool(),
            can_forward: forward.as_bool(),
            loading: self.loading.load(Ordering::SeqCst),
        })
    }

    /// Queues the page's state for the app's webview.
    fn send_state(&self, sender: Option<ICoreWebView2>) {
        let Some(w) = sender else { return };
        match self.state(&w) {
            Ok(state) => {
                let app = self.app.clone();
                post(&self.app, move || emit(&app, "browser://state", state));
            }
            Err(e) => failed("state", &e),
        }
    }
}

/// Adds the page's handlers and sends its first state. `loading` is the flag the builder's own handlers keep;
/// `dark` is the colour scheme the app asked for, if it has.
pub fn install(app: &AppHandle, page: &Webview, key: &str, loading: Arc<AtomicBool>, dark: Option<bool>) {
    let page_of = Page {
        app: app.clone(),
        key: key.to_string(),
        loading,
    };
    let result = page.with_webview(move |pv| {
        let controller = pv.controller();
        // SAFETY: a COM call on the live controller, on the webview's own thread (with_webview runs it there).
        let w = match unsafe { controller.CoreWebView2() } {
            Ok(w) => w,
            Err(e) => return failed("webview", &e),
        };
        if let Err(e) = state_handlers(&page_of, &w) {
            failed("state handlers", &e);
        }
        if let Err(e) = process_handler(&page_of, &w) {
            failed("process handler", &e);
        }
        // The key-ups of the keys taken from the page, by virtual key.
        let taken: Arc<Mutex<HashSet<u32>>> = Arc::default();
        if let Err(e) = focus_handlers(&page_of, &controller, taken.clone()) {
            failed("focus handlers", &e);
        }
        if let Err(e) = key_handler(&page_of, &controller, taken) {
            failed("key handler", &e);
        }
        if let Err(e) = focus_out_handler(&page_of, &controller) {
            failed("focus-out handler", &e);
        }
        if let Err(e) = icon_handler(&page_of, &w) {
            missing("site icons", &e);
        }
        if let Some(dark) = dark
            && let Err(e) = set_scheme(&w, dark)
        {
            missing("colour scheme setting", &e);
        }
        // F12 opens DevTools on the page only, in a release build too: the builder left them off.
        // SAFETY: COM calls on the live webview, on its own thread.
        let devtools = unsafe { w.Settings().and_then(|s| s.SetAreDevToolsEnabled(true)) };
        if let Err(e) = devtools {
            failed("DevTools setting", &e);
        }
        let mut pid = 0u32;
        // SAFETY: as above; the getter writes to a local that outlives the call.
        if unsafe { w.BrowserProcessId(&mut pid) }.is_ok()
            && let Some(state) = page_of.app.try_state::<BrowserState>()
        {
            state.browser_pid.store(pid, Ordering::SeqCst);
        }
    });
    if let Err(e) = result {
        eprintln!("Marshell: a browser page's handlers were not added: {e}");
    }
}

/// The bar follows the page: its address, its title, whether Back and Forward can go anywhere, and whether it is
/// loading. The first state is sent at once, since the first navigation started before these handlers were added.
fn state_handlers(page: &Page, w: &ICoreWebView2) -> windows_core::Result<()> {
    let mut token = 0i64;
    let (a, b, c, d, e) = (page.clone(), page.clone(), page.clone(), page.clone(), page.clone());
    // SAFETY: COM calls on the live webview, on its own thread; each handler is owned by the webview from here on,
    // and `token` outlives each call.
    unsafe {
        w.add_HistoryChanged(
            &HistoryChangedEventHandler::create(Box::new(move |sender, _| {
                a.send_state(sender);
                Ok(())
            })),
            &mut token,
        )?;
        w.add_SourceChanged(
            &SourceChangedEventHandler::create(Box::new(move |sender, _| {
                b.send_state(sender);
                Ok(())
            })),
            &mut token,
        )?;
        w.add_DocumentTitleChanged(
            &DocumentTitleChangedEventHandler::create(Box::new(move |sender, _| {
                c.send_state(sender);
                Ok(())
            })),
            &mut token,
        )?;
        w.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |sender, args| {
                // Tauri hands its navigation handler a parsed URL, and lets through an address that doesn't
                // parse. The allow-list must have seen every address, so that one is cancelled here.
                if let Some(args) = args {
                    let mut uri = windows_core::PWSTR::null();
                    // SAFETY: reading the event's own argument inside its callback; take_pwstr frees the string.
                    args.Uri(&mut uri)?;
                    let uri = take_pwstr(uri);
                    if uri.parse::<Url>().is_err() {
                        // SAFETY: as above.
                        args.SetCancel(true)?;
                        refuse(&d.app, &d.key, &uri);
                        return Ok(());
                    }
                }
                d.send_state(sender);
                Ok(())
            })),
            &mut token,
        )?;
        w.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(move |sender, _| {
                e.loading.store(false, Ordering::SeqCst);
                e.send_state(sender);
                Ok(())
            })),
            &mut token,
        )?;
    }
    page.send_state(Some(w.clone()));
    Ok(())
}

/// A page whose renderer ended says so in the app; the browser process ending takes every page with it.
fn process_handler(page: &Page, w: &ICoreWebView2) -> windows_core::Result<()> {
    let mut token = 0i64;
    let page = page.clone();
    let handler = ProcessFailedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
        // SAFETY: reading the event's own argument inside its callback, into a local that outlives the call.
        unsafe { args.ProcessFailedKind(&mut kind)? };
        let app = page.app.clone();
        if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED {
            post(&page.app, move || super::browser_gone(&app));
        } else if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED {
            page.loading.store(false, Ordering::SeqCst);
            let payload = PageKey { key: page.key.clone() };
            post(&page.app, move || emit(&app, "browser://failed", payload));
        }
        Ok(())
    }));
    // SAFETY: a COM call on the live webview, on its own thread; `token` outlives the call.
    unsafe { w.add_ProcessFailed(&handler, &mut token) }
}

/// The app knows which page holds the keyboard. A page that takes it starts with no key held back: a key-up that
/// went elsewhere (Alt+Tab with a taken key still down) must not swallow that key's next release.
fn focus_handlers(
    page: &Page,
    controller: &ICoreWebView2Controller,
    taken: Arc<Mutex<HashSet<u32>>>,
) -> windows_core::Result<()> {
    let mut token = 0i64;
    let send = |page: &Page, focused: bool| {
        let app = page.app.clone();
        let payload = PageFocus {
            key: page.key.clone(),
            focused,
        };
        post(&page.app, move || emit(&app, "browser://focus", payload));
    };
    let (got, lost) = (page.clone(), page.clone());
    // SAFETY: COM calls on the live controller, on its own thread; `token` outlives each call.
    unsafe {
        controller.add_GotFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                lock(&taken).clear();
                send(&got, true);
                Ok(())
            })),
            &mut token,
        )?;
        controller.add_LostFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                send(&lost, false);
                Ok(())
            })),
            &mut token,
        )
    }
}

/// Marshell's keys and F12 are taken from the page (6q-0 decision 9). The key-up of a taken key is taken too, by
/// its virtual key, so a modifier let go first can't leak the release into the page. Nothing moves focus from
/// inside this callback: what follows a taken key is posted.
fn key_handler(
    page: &Page,
    controller: &ICoreWebView2Controller,
    taken: Arc<Mutex<HashSet<u32>>>,
) -> windows_core::Result<()> {
    let page = page.clone();
    let mut token = 0i64;
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
        let mut vk = 0u32;
        let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
        // SAFETY: reading the event's own arguments inside its callback, into locals that outlive the calls.
        unsafe {
            args.KeyEventKind(&mut kind)?;
            args.VirtualKey(&mut vk)?;
            args.PhysicalKeyStatus(&mut status)?;
        }
        let is_down =
            kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN || kind == COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN;
        let is_up = kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_UP || kind == COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_UP;
        if is_up {
            let was_taken = lock(&taken).remove(&vk);
            if was_taken {
                // SAFETY: setting the event's own answer inside its callback.
                unsafe { args.SetHandled(true)? };
            }
            return Ok(());
        }
        if !is_down {
            return Ok(());
        }
        let (ctrl, shift, alt) = (down(VK_CONTROL), down(VK_SHIFT), down(VK_MENU));
        let route = match page.app.try_state::<BrowserState>() {
            Some(state) => route_key(vk, ctrl, shift, alt, &lock(&state.claimed)),
            None => KeyRoute::Page,
        };
        let (app, key) = (page.app.clone(), page.key.clone());
        let then: Job = match route {
            KeyRoute::Page => return Ok(()),
            KeyRoute::DevTools => Box::new(move || open_devtools(&app, &key)),
            KeyRoute::App => {
                let Some(event) = key_event(vk, ctrl, shift, alt, status.WasKeyDown.as_bool()) else {
                    return Ok(());
                };
                Box::new(move || {
                    focus_app(&app);
                    emit(&app, "browser://key", PageKeyEvent { key, event });
                })
            }
        };
        lock(&taken).insert(vk);
        // SAFETY: setting the event's own answer inside its callback.
        unsafe { args.SetHandled(true)? };
        post(&page.app, then);
        Ok(())
    }));
    // SAFETY: a COM call on the live controller, on its own thread; `token` outlives the call.
    unsafe { controller.add_AcceleratorKeyPressed(&handler, &mut token) }
}

/// Tab past the page's last element, or Shift+Tab before its first, hands focus back to the app, which moves it to
/// the element on that side of the page. Unhandled, WebView2 would cycle focus inside the page.
fn focus_out_handler(page: &Page, controller: &ICoreWebView2Controller) -> windows_core::Result<()> {
    let page = page.clone();
    let mut token = 0i64;
    let handler = MoveFocusRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut reason = COREWEBVIEW2_MOVE_FOCUS_REASON::default();
        // SAFETY: reading the event's own argument inside its callback, into a local that outlives the call.
        unsafe { args.Reason(&mut reason)? };
        let Some(direction) = focus_out_direction(reason.0) else {
            return Ok(());
        };
        // SAFETY: setting the event's own answer inside its callback.
        unsafe { args.SetHandled(true)? };
        let app = page.app.clone();
        let payload = FocusOut {
            key: page.key.clone(),
            direction,
        };
        post(&page.app, move || {
            focus_app(&app);
            emit(&app, "browser://focus-out", payload);
        });
        Ok(())
    }));
    // SAFETY: a COM call on the live controller, on its own thread; `token` outlives the call.
    unsafe { controller.add_MoveFocusRequested(&handler, &mut token) }
}

fn open_devtools(app: &AppHandle, key: &str) {
    let Some(page) = super::page(app, key) else { return };
    let opened = page.with_webview(|pv| {
        // SAFETY: COM calls on the live controller, on the webview's own thread.
        if let Err(e) = unsafe { pv.controller().CoreWebView2().and_then(|w| w.OpenDevToolsWindow()) } {
            failed("DevTools", &e);
        }
    });
    if let Err(e) = opened {
        eprintln!("Marshell: a browser page's DevTools did not open: {e}");
    }
}

/// Moves the keyboard into the page: at its first element ("next"), its last ("previous"), or where it was.
pub fn focus(page: &Webview, reason: &str) -> Result<(), String> {
    let reason = match reason {
        "next" => COREWEBVIEW2_MOVE_FOCUS_REASON_NEXT,
        "previous" => COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS,
        "programmatic" => COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC,
        other => return Err(format!("no such reason: {other}")),
    };
    page.with_webview(move |pv| {
        // SAFETY: a COM call on the live controller, on the webview's own thread.
        if let Err(e) = unsafe { pv.controller().MoveFocus(reason) } {
            failed("focus", &e);
        }
    })
    .map_err(|e| e.to_string())
}

/// The site's icon changed: the app is told once the icon has been read. The read is asked for outside the event.
fn icon_handler(page: &Page, w: &ICoreWebView2) -> windows_core::Result<()> {
    let w = w.cast::<ICoreWebView2_15>()?;
    let mut token = 0i64;
    let (app, key) = (page.app.clone(), page.key.clone());
    let handler = FaviconChangedEventHandler::create(Box::new(move |_, _| {
        let (on_main, key) = (app.clone(), key.clone());
        post(&app, move || send_icon(&on_main, &key));
        Ok(())
    }));
    // SAFETY: a COM call on the live webview, on its own thread; `token` outlives the call.
    unsafe { w.add_FaviconChanged(&handler, &mut token) }
}

/// Reads the page's icon as PNG and sends it to the app's webview: a `data:` URL, or none. On the main thread.
fn send_icon(app: &AppHandle, key: &str) {
    let Some(page) = super::page(app, key) else { return };
    let (app, key) = (app.clone(), key.to_string());
    let asked = page.with_webview(move |pv| {
        let send = move |src: Option<String>| {
            let on_main = app.clone();
            let payload = PageIcon { key, src };
            post(&app, move || emit(&on_main, "browser://icon", payload));
        };
        // SAFETY: COM calls on the live controller, on the webview's own thread.
        let w = match unsafe { pv.controller().CoreWebView2() }.and_then(|w| w.cast::<ICoreWebView2_15>()) {
            Ok(w) => w,
            Err(e) => return missing("site icons", &e),
        };
        let mut uri = windows_core::PWSTR::null();
        // SAFETY: as above; take_pwstr frees the string WebView2 allocated.
        let has_icon = unsafe { w.FaviconUri(&mut uri) }.is_ok() && !take_pwstr(uri).is_empty();
        if !has_icon {
            return send(None);
        }
        let handler = GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
            let png = result
                .ok()
                .and(stream)
                .and_then(|stream| read(&stream, MAX_ICON_BYTES + 1).ok());
            send(png.and_then(|png| icon_src(&png)));
            Ok(())
        }));
        // SAFETY: as above; the handler is owned by the webview until it has run.
        if let Err(e) = unsafe { w.GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &handler) } {
            failed("icon", &e);
        }
    });
    if let Err(e) = asked {
        eprintln!("Marshell: a browser page's icon was not read: {e}");
    }
}

fn set_scheme(w: &ICoreWebView2, dark: bool) -> windows_core::Result<()> {
    let scheme = if dark {
        COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK
    } else {
        COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT
    };
    // SAFETY: COM calls on the live webview, on its own thread.
    unsafe { w.cast::<ICoreWebView2_13>()?.Profile()?.SetPreferredColorScheme(scheme) }
}

/// The colour scheme pages see (`prefers-color-scheme`): the app's, not Windows'. It is the profile's setting.
pub fn theme(page: &Webview, dark: bool) {
    let set = page.with_webview(move |pv| {
        // SAFETY: a COM call on the live controller, on the webview's own thread.
        if let Err(e) = unsafe { pv.controller().CoreWebView2() }.and_then(|w| set_scheme(&w, dark)) {
            missing("colour scheme setting", &e);
        }
    });
    if let Err(e) = set {
        eprintln!("Marshell: a browser page's colour scheme was not set: {e}");
    }
}

/// "0x80070005 Access is denied."
fn hr_text(e: &windows_core::Error) -> String {
    format!("{:#010x} {}", e.code().0 as u32, e.message())
}

/// The first `limit` bytes of a stream, from its start.
fn read(stream: &IStream, limit: usize) -> windows_core::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 64 * 1024];
    // SAFETY: a stream WebView2 or this module made, read on the thread that owns it into a buffer of the length
    // given; `got` outlives each call.
    unsafe {
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        while out.len() < limit {
            let mut got = 0u32;
            stream
                .Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut got))
                .ok()?;
            if got == 0 {
                break;
            }
            out.extend_from_slice(&buf[..got as usize]);
        }
    }
    out.truncate(limit);
    Ok(out)
}

/// Waits off the async runtime's threads for an answer a WebView2 callback sends from the main thread.
async fn answer<T: Send + 'static>(
    rx: mpsc::Receiver<Result<T, String>>,
    limit: Duration,
    late: &'static str,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(limit).unwrap_or_else(|_| Err(late.to_string())))
        .await
        .map_err(|e| e.to_string())?
}

/// The page as WebView2 last drew it, as PNG bytes: `CapturePreview` into a stream in memory. The call is made on
/// the webview's thread and completes there; the command's thread only waits for the bytes.
pub async fn capture(page: &Webview) -> Result<Vec<u8>, String> {
    let (tx, rx) = mpsc::channel::<Result<Vec<u8>, String>>();
    page.with_webview(move |pv| {
        let done = tx.clone();
        // SAFETY: COM calls on the live controller, on the webview's own thread; the stream is made here and read
        // in the handler, on this same thread.
        let started = unsafe {
            CreateStreamOnHGlobal(HGLOBAL::default(), true).and_then(|stream| {
                let filled = stream.clone();
                let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
                    let png = result.and_then(|()| read(&filled, usize::MAX));
                    let _ = done.send(png.map_err(|e| hr_text(&e)));
                    Ok(())
                }));
                pv.controller().CoreWebView2()?.CapturePreview(
                    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                    &stream,
                    &handler,
                )
            })
        };
        if let Err(e) = started {
            let _ = tx.send(Err(hr_text(&e)));
        }
    })
    .map_err(|e| e.to_string())?;
    answer(rx, Duration::from_secs(5), "the page's picture was not ready in 5 s").await
}

/// Clears everything the profile holds: cookies and sign-ins, storage, the cache, history, saved permissions.
/// Asked of one page, done for the profile every page shares.
pub async fn clear_data(page: &Webview) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    page.with_webview(move |pv| {
        let done = tx.clone();
        // SAFETY: COM calls on the live controller, on the webview's own thread; the handler is owned by the
        // profile until it has run.
        let started = unsafe {
            pv.controller()
                .CoreWebView2()
                .and_then(|w| w.cast::<ICoreWebView2_13>())
                .and_then(|w| w.Profile())
                .and_then(|profile| profile.cast::<ICoreWebView2Profile2>())
                .and_then(|profile| {
                    let handler = ClearBrowsingDataCompletedHandler::create(Box::new(move |result| {
                        let _ = done.send(result.map_err(|e| hr_text(&e)));
                        Ok(())
                    }));
                    profile.ClearBrowsingDataAll(&handler)
                })
        };
        if let Err(e) = started {
            let _ = tx.send(Err(hr_text(&e)));
        }
    })
    .map_err(|e| e.to_string())?;
    answer(rx, Duration::from_secs(30), "the pages' data was not cleared in 30 s").await
}

/// Back, Forward, Reload and Stop, on the page's own history. Back and Forward pass the allow-list like any other
/// navigation.
pub fn nav(page: &Webview, action: &str) -> Result<(), String> {
    // SAFETY (each): a COM call on the live webview, made on its own thread inside with_webview below.
    let go: fn(&ICoreWebView2) -> windows_core::Result<()> = match action {
        "back" => |w| unsafe { w.GoBack() },
        "forward" => |w| unsafe { w.GoForward() },
        "reload" => |w| unsafe { w.Reload() },
        "stop" => |w| unsafe { w.Stop() },
        other => return Err(format!("no such action: {other}")),
    };
    page.with_webview(move |pv| {
        // SAFETY: a COM call on the live controller, on the webview's own thread.
        let done = unsafe { pv.controller().CoreWebView2() }.and_then(|w| go(&w));
        if let Err(e) = done {
            failed("navigation", &e);
        }
    })
    .map_err(|e| e.to_string())
}

/// Whether `hwnd` is the window in front.
pub fn in_front(hwnd: windows::Win32::Foundation::HWND) -> bool {
    // SAFETY: takes no argument and returns a handle that is only compared, never used.
    unsafe { GetForegroundWindow() == hwnd.0 }
}

/// The pages' browser process, held open so its end can be waited for.
pub struct BrowserProcess(HANDLE);

impl BrowserProcess {
    /// None for no process (`pid` 0) or one that has already ended and been forgotten by Windows.
    pub fn open(pid: u32) -> Option<Self> {
        if pid == 0 {
            return None;
        }
        // SAFETY: takes no pointer; the right asked for is only to wait. A null handle is the failure.
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        (!handle.is_null()).then_some(Self(handle))
    }

    /// Waits for the process to end; false when it is still running after `limit`.
    pub fn wait(&self, limit: Duration) -> bool {
        let ms = u32::try_from(limit.as_millis()).unwrap_or(u32::MAX - 1);
        // SAFETY: the handle is open for as long as `self` lives.
        unsafe { WaitForSingleObject(self.0, ms) == WAIT_OBJECT_0 }
    }
}

impl Drop for BrowserProcess {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenProcess and is closed once, here.
        unsafe { CloseHandle(self.0) };
    }
}
