//! SPIKE S13, never merged. Two blocks of fixed size against the window's right edge, to see whether a native child keeps up
//! with the edge where the webview's own buttons trail (`docs/spikes/S13-window-buttons-strip.md`).
//! A is a plain Win32 child at y = 40. B is a child webview at y = 84. Both are 138 x 40 logical pixels.
//! `MARSHELL_STRIP_SPIKE=subclass` moves A inside the window's own `WM_WINDOWPOSCHANGED`; the default moves it from Tauri's event.
#![cfg(windows)]

use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

use tauri::{LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, HWND_TOP, IDC_ARROW, LoadCursorW, MA_NOACTIVATE, RegisterClassW,
    SWP_NOACTIVATE, SetWindowPos, WM_MOUSEACTIVATE, WM_WINDOWPOSCHANGED, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS,
    WS_VISIBLE,
};

const WIDTH: f64 = 138.0;
const HEIGHT: f64 = 40.0;
const A_TOP: f64 = 40.0;
const B_TOP: f64 = 84.0;
const B_LABEL: &str = "strip-spike";

/// Block A's HWND, 0 until it exists.
static BLOCK_A: AtomicIsize = AtomicIsize::new(0);
/// The window's scale, times 1000.
static SCALE: AtomicU32 = AtomicU32::new(1000);
static SUBCLASS: AtomicBool = AtomicBool::new(false);

fn px(logical: f64) -> i32 {
    (logical * f64::from(SCALE.load(Ordering::Relaxed)) / 1000.0).round() as i32
}

/// Puts A against the right edge of a client area `width` physical pixels wide.
fn place_a(width: i32) {
    let a = BLOCK_A.load(Ordering::Relaxed) as HWND;
    if a.is_null() {
        return;
    }
    // SAFETY: `a` is a window this module made on this thread, and the call only moves it.
    unsafe {
        SetWindowPos(
            a,
            HWND_TOP,
            width - px(WIDTH),
            px(A_TOP),
            px(WIDTH),
            px(HEIGHT),
            SWP_NOACTIVATE,
        );
    }
}

unsafe extern "system" fn block_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as LRESULT;
    }
    // SAFETY: the default procedure, with the arguments this procedure was given.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe extern "system" fn parent_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_WINDOWPOSCHANGED {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: `hwnd` is the window this procedure was called for, and `rect` is a local.
        if unsafe { GetClientRect(hwnd, &mut rect) } != 0 {
            place_a(rect.right - rect.left);
        }
    }
    // SAFETY: the next procedure in the chain, with the arguments this one was given.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Makes both blocks. On the main thread, after the app's own webview exists.
pub fn install(app: &tauri::AppHandle) {
    let Some(window) = crate::main_window(app) else {
        return;
    };
    let Ok(parent) = window.hwnd() else {
        return;
    };
    let parent = parent.0 as HWND;
    let scale = window.scale_factor().unwrap_or(1.0);
    SCALE.store((scale * 1000.0).round() as u32, Ordering::Relaxed);
    let subclass = std::env::var("MARSHELL_STRIP_SPIKE").is_ok_and(|v| v == "subclass");
    SUBCLASS.store(subclass, Ordering::Relaxed);
    let width = window.inner_size().map(|s| s.width as i32).unwrap_or(0);

    let class: Vec<u16> = "MarshellStripSpike\0".encode_utf16().collect();
    // SAFETY: plain Win32 calls on the thread that owns the parent window. The class name outlives both calls that read it.
    unsafe {
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(block_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: 14usize as _, // COLOR_HIGHLIGHT + 1,
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let a = CreateWindowExW(
            0,
            class.as_ptr(),
            null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            width - px(WIDTH),
            px(A_TOP),
            px(WIDTH),
            px(HEIGHT),
            parent,
            null_mut(),
            instance,
            null(),
        );
        BLOCK_A.store(a as isize, Ordering::Relaxed);
        if subclass {
            SetWindowSubclass(parent, Some(parent_proc), 0x5713, 0);
        }
    }
    place_a(width);

    let Ok(blank) = "about:blank".parse() else {
        return;
    };
    let builder = WebviewBuilder::new(B_LABEL, WebviewUrl::External(blank)).focused(false);
    let x = f64::from(width) / scale - WIDTH;
    if let Err(e) = window.add_child(builder, LogicalPosition::new(x, B_TOP), LogicalSize::new(WIDTH, HEIGHT)) {
        eprintln!("Marshell spike: no block B: {e}");
    }
    eprintln!(
        "Marshell spike: blocks placed, A moved by {}",
        if subclass { "the subclass" } else { "the Resized event" }
    );
}

/// From Tauri's `Resized`: moves B always, and A unless the subclass moves it.
pub fn on_resized(window: &tauri::Window, size: tauri::PhysicalSize<u32>) {
    let scale = window.scale_factor().unwrap_or(1.0);
    SCALE.store((scale * 1000.0).round() as u32, Ordering::Relaxed);
    if !SUBCLASS.load(Ordering::Relaxed) {
        place_a(size.width as i32);
    }
    if let Some(b) = window.app_handle().get_webview(B_LABEL) {
        let x = f64::from(size.width) / scale - WIDTH;
        let _ = b.set_bounds(tauri::Rect {
            position: LogicalPosition::new(x.max(0.0), B_TOP).into(),
            size: LogicalSize::new(WIDTH, HEIGHT).into(),
        });
    }
}
