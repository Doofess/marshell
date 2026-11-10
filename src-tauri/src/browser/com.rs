//! The WebView2 layer under a page (Windows). A handler runs inside WebView2's own callback, on the main thread: it
//! reads its event and queues what follows with `post`, and never calls back into a webview from inside the callback.

use super::{BrowserState, PageKey, PageState, emit, post, refuse};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, Url, Webview};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED, ICoreWebView2,
};
use webview2_com::{
    DocumentTitleChangedEventHandler, HistoryChangedEventHandler, NavigationCompletedEventHandler,
    NavigationStartingEventHandler, ProcessFailedEventHandler, SourceChangedEventHandler, take_pwstr,
};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

fn failed(what: &str, e: &windows_core::Error) {
    eprintln!("Marshell: a browser page's {what} failed: {e}");
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

/// Adds the page's handlers and sends its first state. `loading` is the flag the builder's own handlers keep.
pub fn install(app: &AppHandle, page: &Webview, key: &str, loading: Arc<AtomicBool>) {
    let page_of = Page {
        app: app.clone(),
        key: key.to_string(),
        loading,
    };
    let result = page.with_webview(move |pv| {
        // SAFETY: a COM call on the live controller, on the webview's own thread (with_webview runs it there).
        let w = match unsafe { pv.controller().CoreWebView2() } {
            Ok(w) => w,
            Err(e) => return failed("webview", &e),
        };
        if let Err(e) = state_handlers(&page_of, &w) {
            failed("state handlers", &e);
        }
        if let Err(e) = process_handler(&page_of, &w) {
            failed("process handler", &e);
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
