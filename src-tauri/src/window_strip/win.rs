//! The strip's native window, Windows only.
//!
//! A plain Win32 child of the main window sits over the page's Minimize, Maximize and Close. The page's own buttons trail
//! the window's right edge while it is dragged, because the webview repaints a frame behind it; a native child moved from
//! Tauri's `Resized` event keeps up (spike S13). The strip draws nothing of its own: it blits the pictures the page sent
//! and turns the pointer into the three window actions (PLAN §9, Window-buttons strip slice decisions).
//!
//! The middle column answers the hit test as the system's Maximize button, which is what makes Windows 11 show Snap
//! Layouts over it; its pointer then arrives as non-client messages, which feed the same pointer rules.
//!
//! The strip takes the pointer over its whole height, as the page's buttons under it do. The window's top edge resizes
//! on the one row above the client area, which no child window covers: under Tauri's `unstable` feature the app's
//! webview is a child webview, so Tauri makes no top-edge resize window of its own (live check, finding 1).
//!
//! Everything here runs on the main thread, the one that owns the window. The state sits behind one lock, and the lock
//! is never held across a call that can send a message to the strip's own procedure (`SetWindowPos`, `ShowWindow`,
//! `SetCapture`, `ReleaseCapture`, `SetForegroundWindow`, a `tauri::Window` method): every path takes the lock, copies out
//! what it needs, lets go, and only then calls. The procedure never panics, since a panic across the FFI boundary aborts
//! the app.
#![cfg(windows)]

use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex, MutexGuard};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, DIB_RGB_COLORS, EndPaint, InvalidateRect, PAINTSTRUCT,
    ScreenToClient, SetDIBitsToDevice,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, FindWindowExW, GWL_STYLE, GetClientRect, GetCursorPos, GetForegroundWindow,
    GetParent, GetWindowLongW, HTCLIENT, HTMAXBUTTON, HWND_TOP, IDC_ARROW, IsZoomed, LoadCursorW, MA_NOACTIVATE,
    PostMessageW, RegisterClassW, SW_HIDE, SW_SHOWNA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetForegroundWindow, SetWindowLongW, SetWindowPos, ShowWindow, WM_APP, WM_CAPTURECHANGED,
    WM_ERASEBKGND, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCHITTEST,
    WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSEMOVE, WM_PAINT, WNDCLASSW, WS_CHILD,
    WS_CLIPSIBLINGS, WindowFromPoint,
};

use super::Paint;
use super::pointer::{Button, Event, Outcome, Pointer, hit};

/// `WM_MOUSELEAVE` and its non-client twin, which windows-sys keeps behind the whole Controls feature.
const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_NCMOUSELEAVE: u32 = 0x02A2;

/// Where the strip's left edge is, in the parent's client area, when the strip ends at the right edge.
pub(super) fn left_edge(client_width: i32, widths: [u32; 3]) -> i32 {
    let sum: i64 = widths.iter().map(|w| i64::from(*w)).sum();
    (i64::from(client_width) - sum).clamp(0, i64::from(i32::MAX)) as i32
}

/// The picture a button is drawn with, and its width. The middle slot shows Restore while the window is maximized.
pub(super) fn slot_picture(paint: &Paint, button: Button, maximized: bool, lit: bool) -> (&[u8], u32) {
    let b = match button {
        Button::Minimize => &paint.minimize,
        Button::Maximize if maximized => &paint.restore,
        Button::Maximize => &paint.maximize,
        Button::Close => &paint.close,
    };
    (if lit { &b.hover } else { &b.rest }, b.width)
}

/// What a point `x` pixels from the strip's left edge is to the system. The middle column is the system's own Maximize
/// button, which is what makes Windows 11 show Snap Layouts over it; the other two are plain client area.
pub(super) fn hit_code(x: i32, widths: [u32; 3]) -> u32 {
    match hit(x, widths) {
        Some(Button::Maximize) => HTMAXBUTTON,
        _ => HTCLIENT,
    }
}

/// GDI wants blue first. A trailing partial pixel is left as it is.
pub(super) fn bgra(rgba: &[u8]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for pixel in out.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    out
}

/// The button a point in the strip's client area is over, for a strip `height` tall.
fn button_at(x: i32, y: i32, widths: [u32; 3], height: u32) -> Option<Button> {
    if y < 0 || i64::from(y) >= i64::from(height) {
        return None;
    }
    hit(x, widths)
}

/// The same set with every picture turned to BGRA, made once when the pictures arrive.
fn bgra_paint(paint: &Paint) -> Paint {
    let mut out = paint.clone();
    for b in [&mut out.minimize, &mut out.maximize, &mut out.restore, &mut out.close] {
        b.rest = bgra(&b.rest);
        b.hover = bgra(&b.hover);
    }
    out
}

fn code_of(button: Button) -> WPARAM {
    match button {
        Button::Minimize => 0,
        Button::Maximize => 1,
        Button::Close => 2,
    }
}

fn button_of(code: WPARAM) -> Option<Button> {
    match code {
        0 => Some(Button::Minimize),
        1 => Some(Button::Maximize),
        2 => Some(Button::Close),
        _ => None,
    }
}

/// The signed x and y packed in a mouse message's `lparam`.
fn point_of(lparam: LPARAM) -> (i32, i32) {
    (i32::from(lparam as u16 as i16), i32::from((lparam >> 16) as u16 as i16))
}

/// Everything the window remembers, behind one lock (see the module comment).
struct State {
    /// The strip's window, 0 until `install` made it.
    hwnd: isize,
    app: Option<tauri::AppHandle>,
    /// The pictures, already BGRA. Shared so a paint can keep them without the lock.
    pictures: Option<Arc<Paint>>,
    pointer: Pointer,
    /// Whether a `WM_MOUSELEAVE` is already asked for.
    tracking: bool,
    /// The same for `WM_NCMOUSELEAVE`, over the middle column.
    tracking_non_client: bool,
    /// Whether the parent was maximized when the strip last drew.
    zoomed: bool,
}

static STATE: Mutex<State> = Mutex::new(State {
    hwnd: 0,
    app: None,
    pictures: None,
    pointer: Pointer::new(),
    tracking: false,
    tracking_non_client: false,
    zoomed: false,
});

/// A poisoned lock still holds a state that is safe to read, and the procedure must not panic.
fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The strip's window, if `install` made one.
fn strip() -> Option<HWND> {
    let hwnd = state().hwnd;
    (hwnd != 0).then_some(hwnd as HWND)
}

fn parent_of(hwnd: HWND) -> HWND {
    // SAFETY: a query on a window handle; a stale handle makes it return null.
    unsafe { GetParent(hwnd) }
}

fn is_zoomed(parent: HWND) -> bool {
    // SAFETY: a query on a window handle; a stale handle makes it return 0.
    !parent.is_null() && unsafe { IsZoomed(parent) } != 0
}

fn invalidate(hwnd: HWND) {
    // SAFETY: marks the whole client area of the strip's window for repaint; it sends no message.
    unsafe { InvalidateRect(hwnd, null(), 0) };
}

/// Puts the strip against the right edge of a parent client area `client_width` pixels wide. A no-op without a window or
/// pictures.
fn place(client_width: i32) {
    let (hwnd, widths, height) = {
        let s = state();
        let Some(p) = s.pictures.as_ref() else { return };
        (s.hwnd, p.widths(), p.height)
    };
    if hwnd == 0 {
        return;
    }
    let sum: u32 = widths.iter().sum();
    // SAFETY: `hwnd` is the strip's own window on this thread; the lock is released, as the strip's procedure may run.
    unsafe {
        SetWindowPos(
            hwnd as HWND,
            HWND_TOP,
            left_edge(client_width, widths),
            0,
            sum as i32,
            height as i32,
            SWP_NOACTIVATE,
        );
    }
}

/// Draws the three slots side by side.
fn paint_window(hwnd: HWND) {
    let mut ps = PAINTSTRUCT {
        hdc: null_mut(),
        fErase: 0,
        rcPaint: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        fRestore: 0,
        fIncUpdate: 0,
        rgbReserved: [0; 32],
    };
    // SAFETY: `hwnd` is the window being painted and `ps` is a local; `EndPaint` below closes it. `BeginPaint` may send
    // `WM_ERASEBKGND`, which takes no lock.
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    let (paint, lit) = {
        let s = state();
        (s.pictures.clone(), s.pointer.lit())
    };
    if let (false, Some(paint)) = (hdc.is_null(), paint) {
        let maximized = is_zoomed(parent_of(hwnd));
        let height = paint.height;
        let mut x = 0i32;
        for button in [Button::Minimize, Button::Maximize, Button::Close] {
            let (picture, width) = slot_picture(&paint, button, maximized, lit == Some(button));
            // `Paint::check` guarantees the length; a shorter picture is skipped rather than read past.
            if picture.len() >= width as usize * height as usize * 4 {
                let info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width as i32,
                        biHeight: -(height as i32), // negative: rows top-down
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB,
                        biSizeImage: 0,
                        biXPelsPerMeter: 0,
                        biYPelsPerMeter: 0,
                        biClrUsed: 0,
                        biClrImportant: 0,
                    },
                    bmiColors: Default::default(),
                };
                // SAFETY: `picture` holds at least `width * height` 32-bit pixels, the size `info` describes, and
                // outlives the call; `hdc` is the one `BeginPaint` returned.
                unsafe {
                    SetDIBitsToDevice(
                        hdc,
                        x,
                        0,
                        width,
                        height,
                        0,
                        0,
                        0,
                        height,
                        picture.as_ptr().cast(),
                        &info,
                        DIB_RGB_COLORS,
                    );
                }
            }
            x += width as i32;
        }
    }
    // SAFETY: closes the paint `BeginPaint` opened, with the same window and struct.
    unsafe { EndPaint(hwnd, &ps) };
}

/// Feeds one event to the pointer. Returns what it decided, with the lock already released.
fn feed(event: Event) -> Outcome {
    state().pointer.on(event)
}

/// Carries out a button's action on the main window, with the calls the page's own buttons make.
fn act(button: Button) {
    let app = state().app.clone();
    let Some(window) = app.as_ref().and_then(crate::main_window) else {
        return;
    };
    let result = match button {
        Button::Minimize => window.minimize(),
        Button::Maximize => match window.is_maximized() {
            Ok(true) => window.unmaximize(),
            Ok(false) => window.maximize(),
            Err(e) => Err(e),
        },
        Button::Close => window.close(),
    };
    if let Err(e) = result {
        eprintln!("Marshell: the window strip's {button:?} button failed: {e}");
    }
}

/// What the mouse did, whichever message said so.
#[derive(Clone, Copy, PartialEq)]
enum Input {
    Move,
    Down,
    Up,
}

/// The strip's size as the hit test needs it, or `None` before the first pictures.
fn measures() -> Option<([u32; 3], u32)> {
    let s = state();
    s.pictures.as_ref().map(|p| (p.widths(), p.height))
}

/// A mouse move, press or release over `over`. `non_client` says the message came from the middle column, which the
/// hit test hands to the system as its Maximize button; the two kinds of message each have their own leave tracking.
fn on_pointer(hwnd: HWND, input: Input, over: Option<Button>, non_client: bool) {
    let outcome = match input {
        Input::Move => {
            let outcome = feed(Event::Move(over));
            track(hwnd, non_client);
            outcome
        }
        Input::Down => {
            let Some(button) = over else { return };
            feed(Event::Down(button))
        }
        // The press is settled before the capture goes, as releasing it sends `WM_CAPTURECHANGED`.
        Input::Up => feed(Event::Up(over)),
    };
    match input {
        Input::Down => {
            let parent = parent_of(hwnd);
            // SAFETY: a query, then bringing the parent window forward; the lock is released. The strip itself never
            // takes the keyboard (`MA_NOACTIVATE`). This comes before the capture, so a window coming forward can't
            // take a capture the press depends on.
            unsafe {
                if !parent.is_null() && GetForegroundWindow() != parent {
                    SetForegroundWindow(parent);
                }
            }
            // SAFETY: the strip's own window takes the mouse; the lock is released.
            unsafe { SetCapture(hwnd) };
        }
        Input::Up => {
            // SAFETY: gives the mouse back; the `WM_CAPTURECHANGED` it sends finds the lock free.
            unsafe { ReleaseCapture() };
        }
        Input::Move => {}
    }
    if outcome.redraw {
        invalidate(hwnd);
    }
    if let Some(button) = outcome.act {
        // SAFETY: posts to the strip's own window; the action runs when the message arrives, not inside this one.
        unsafe { PostMessageW(hwnd, WM_APP, code_of(button), 0) };
    }
}

/// Which leave to ask for when a leave message finds the pointer still on the strip, over `over`: the non-client one
/// (`true`) or the client one. `None` when the pointer is in the kind of area the message says it left, so a leave never
/// asks for its own kind again and two leaves can't feed each other.
fn crossing(left_non_client: bool, over: Option<Button>) -> Option<bool> {
    let non_client = over == Some(Button::Maximize);
    (over.is_some() && non_client != left_non_client).then_some(non_client)
}

/// Asks the system for the leave message of one kind of area, unless it is already asked for.
fn track(hwnd: HWND, non_client: bool) {
    let already = {
        let mut s = state();
        if non_client {
            std::mem::replace(&mut s.tracking_non_client, true)
        } else {
            std::mem::replace(&mut s.tracking, true)
        }
    };
    if already {
        return;
    }
    let mut track = TRACKMOUSEEVENT {
        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: if non_client {
            TME_LEAVE | TME_NONCLIENT
        } else {
            TME_LEAVE
        },
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    // SAFETY: `track` is a local describing the strip's own window; the lock is released.
    unsafe { TrackMouseEvent(&mut track) };
}

/// After the strip moved: a pointer it moved away from lights nothing and holds no press. A click on Maximize or Restore
/// moves the strip from under a pointer that stays where it was, and no leave message is promised for that (live check,
/// finding 9). The next move over the strip asks for its leave again.
fn settle(hwnd: HWND) {
    // Read under the lock and let go of it: `under_pointer` asks the strip's own procedure, which takes the lock.
    let idle = state().pointer.idle();
    if idle || under_pointer(hwnd).is_some() {
        return;
    }
    let redraw = {
        let mut s = state();
        s.tracking = false;
        s.tracking_non_client = false;
        s.pointer.on(Event::Leave).redraw
    };
    if redraw {
        invalidate(hwnd);
    }
}

/// Gives every webview window of `parent` the `WS_CLIPSIBLINGS` style. wry makes its window without it, and a window
/// without it paints over the siblings above it: the page was drawn over the strip, which was there, took the pointer and
/// never showed (live check, finding 8).
fn clip_webviews(parent: HWND) {
    let class: Vec<u16> = "WRY_WEBVIEW\0".encode_utf16().collect();
    let mut child: HWND = null_mut();
    loop {
        // SAFETY: walks the parent's children of one class; the class buffer outlives the call, and a stale handle ends
        // the walk with null.
        child = unsafe { FindWindowExW(parent, child, class.as_ptr(), null()) };
        if child.is_null() {
            return;
        }
        // SAFETY: reads and sets a style bit on a child of the app's own window, on the thread that owns it, then has the
        // system take the new style in without moving, sizing, reordering or activating anything. The strip's lock is not
        // held.
        unsafe {
            let style = GetWindowLongW(child, GWL_STYLE) as u32;
            if style & WS_CLIPSIBLINGS == 0 {
                SetWindowLongW(child, GWL_STYLE, (style | WS_CLIPSIBLINGS) as i32);
                SetWindowPos(
                    child,
                    null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
    }
}

/// The button under the pointer right now, or `None` when the pointer is not on the strip at all.
fn under_pointer(hwnd: HWND) -> Option<Option<Button>> {
    let mut at = POINT { x: 0, y: 0 };
    // SAFETY: reads the pointer's place into a local, asks which window is there, and turns the point into the strip's
    // own coordinates. `WindowFromPoint` sends the strip a `WM_NCHITTEST`, whose handler takes the lock: this must be
    // called with the lock free, as it is by its one caller.
    unsafe {
        if GetCursorPos(&mut at) == 0 || WindowFromPoint(at) != hwnd {
            return None;
        }
        ScreenToClient(hwnd, &mut at);
    }
    let (widths, height) = measures()?;
    Some(button_at(at.x, at.y, widths, height))
}

unsafe extern "system" fn strip_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
        WM_PAINT => {
            paint_window(hwnd);
            return 0;
        }
        WM_NCHITTEST => {
            let (x, y) = point_of(lparam);
            let mut at = POINT { x, y };
            // SAFETY: turns a screen point into this window's client point; `at` is a local.
            unsafe { ScreenToClient(hwnd, &mut at) };
            let code = measures().map_or(HTCLIENT, |(widths, _)| hit_code(at.x, widths));
            return code as LRESULT;
        }
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_LBUTTONUP => {
            // Client coordinates. The middle column sends these only while the strip holds the mouse.
            let Some((widths, height)) = measures() else { return 0 };
            let (x, y) = point_of(lparam);
            let input = match msg {
                WM_MOUSEMOVE => Input::Move,
                WM_LBUTTONUP => Input::Up,
                _ => Input::Down,
            };
            on_pointer(hwnd, input, button_at(x, y, widths, height), false);
            return 0;
        }
        // The middle column's pointer, which the hit test made the system's Maximize button. None of these reaches the
        // default procedure, which would draw and track a caption button of its own.
        WM_NCMOUSEMOVE | WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK | WM_NCLBUTTONUP if wparam == HTMAXBUTTON as WPARAM => {
            let input = match msg {
                WM_NCMOUSEMOVE => Input::Move,
                WM_NCLBUTTONUP => Input::Up,
                _ => Input::Down,
            };
            on_pointer(hwnd, input, Some(Button::Maximize), true);
            return 0;
        }
        WM_MOUSELEAVE | WM_NCMOUSELEAVE => {
            {
                let mut s = state();
                if msg == WM_MOUSELEAVE {
                    s.tracking = false;
                } else {
                    s.tracking_non_client = false;
                }
            }
            // Crossing between the middle column and its neighbours leaves one kind of area for the other, with the
            // pointer still on the strip: then this is a move, not a leave, and the area the pointer is in now is the
            // one to hear from next. A release over the middle column ends here too, with no move after it.
            let event = match under_pointer(hwnd) {
                Some(over) => {
                    if let Some(non_client) = crossing(msg == WM_NCMOUSELEAVE, over) {
                        track(hwnd, non_client);
                    }
                    Event::Move(over)
                }
                None => Event::Leave,
            };
            if feed(event).redraw {
                invalidate(hwnd);
            }
            return 0;
        }
        WM_CAPTURECHANGED => {
            if feed(Event::Leave).redraw {
                invalidate(hwnd);
            }
            return 0;
        }
        WM_APP => {
            if let Some(button) = button_of(wparam) {
                act(button);
            }
            return 0;
        }
        _ => {}
    }
    // SAFETY: the default procedure, with the arguments this procedure was given.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Makes the strip's window, hidden. On the main thread, after the app's own webview exists. If anything fails the page's
/// own buttons stay what the person sees.
pub fn install(app: &tauri::AppHandle) {
    let Some(window) = crate::main_window(app) else {
        eprintln!("Marshell: no window strip this run: no main window");
        return;
    };
    let parent = match window.hwnd() {
        Ok(hwnd) => hwnd.0 as HWND,
        Err(e) => {
            eprintln!("Marshell: no window strip this run: {e}");
            return;
        }
    };
    let class: Vec<u16> = "MarshellWindowStrip\0".encode_utf16().collect();
    let title: Vec<u16> = "Window buttons\0".encode_utf16().collect();
    // SAFETY: plain Win32 calls on the thread that owns the parent window. The class and title buffers outlive the calls
    // that read them, and the strip's state is not locked, as creating the window sends it messages.
    let hwnd = unsafe {
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(strip_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        // A second registration of the same class fails and is harmless.
        RegisterClassW(&wc);
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            parent,
            null_mut(),
            instance,
            null(),
        )
    };
    if hwnd.is_null() {
        eprintln!("Marshell: no window strip this run: the window was not made");
        return;
    }
    clip_webviews(parent);
    let mut s = state();
    s.hwnd = hwnd as isize;
    s.app = Some(app.clone());
}

/// From Tauri's `Resized` event: follows the right edge, and redraws when maximize or restore changed the middle button.
pub fn on_resized(_window: &tauri::Window, size: tauri::PhysicalSize<u32>) {
    let Some(hwnd) = strip() else { return };
    place(i32::try_from(size.width).unwrap_or(i32::MAX));
    settle(hwnd);
    let zoomed = is_zoomed(parent_of(hwnd));
    let changed = {
        let mut s = state();
        std::mem::replace(&mut s.zoomed, zoomed) != zoomed
    };
    if changed {
        invalidate(hwnd);
    }
}

/// Called by the command with `Wanted::apply`'s result: `None` hides the strip, pictures place it and show it without
/// taking the keyboard. Quiet when there is no window, since the page's buttons are then what the person sees.
pub(super) fn show(paint: Option<&Paint>) {
    let Some(hwnd) = strip() else { return };
    let Some(paint) = paint else {
        {
            let mut s = state();
            s.pointer = Pointer::new();
            s.tracking = false;
            s.tracking_non_client = false;
        }
        // SAFETY: hides the strip's own window; the lock is released, as the procedure may run.
        unsafe { ShowWindow(hwnd, SW_HIDE) };
        return;
    };
    let converted = Arc::new(bgra_paint(paint));
    let parent = parent_of(hwnd);
    let zoomed = is_zoomed(parent);
    {
        let mut s = state();
        s.pictures = Some(converted);
        s.zoomed = zoomed;
    }
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: a query on the parent's window; `client` is a local.
    if !parent.is_null() && unsafe { GetClientRect(parent, &mut client) } != 0 {
        // Again here: a webview window made after `install` is brought in line before the strip shows.
        clip_webviews(parent);
        place(client.right - client.left);
    }
    // SAFETY: shows the strip's own window without activating it; the lock is released.
    unsafe { ShowWindow(hwnd, SW_SHOWNA) };
    invalidate(hwnd);
}

#[cfg(test)]
mod tests {
    use super::super::ButtonPaint;
    use super::*;

    #[test]
    fn the_strip_ends_at_the_right_edge() {
        assert_eq!(left_edge(1280, [46, 46, 46]), 1142);
        assert_eq!(left_edge(100, [46, 46, 46]), 0);
    }

    /// A set whose pictures each start with a byte of their own.
    fn marked() -> Paint {
        let button = |width: u32, rest: u8, hover: u8| ButtonPaint {
            width,
            rest: vec![rest; (width * 2 * 4) as usize],
            hover: vec![hover; (width * 2 * 4) as usize],
        };
        Paint {
            height: 2,
            minimize: button(3, 1, 2),
            maximize: button(4, 3, 4),
            restore: button(4, 5, 6),
            close: button(5, 7, 8),
        }
    }

    #[test]
    fn the_middle_slot_shows_restore_while_maximized() {
        let p = marked();
        assert_eq!(
            slot_picture(&p, Button::Maximize, false, false),
            (&p.maximize.rest[..], 4)
        );
        assert_eq!(
            slot_picture(&p, Button::Maximize, true, false),
            (&p.restore.rest[..], 4)
        );
        assert_eq!(
            slot_picture(&p, Button::Maximize, true, true),
            (&p.restore.hover[..], 4)
        );
        assert_eq!(
            slot_picture(&p, Button::Minimize, true, false),
            (&p.minimize.rest[..], 3)
        );
        assert_eq!(slot_picture(&p, Button::Close, true, true), (&p.close.hover[..], 5));
    }

    #[test]
    fn the_maximize_column_is_the_systems_maximize_button() {
        let widths = [46, 46, 46];
        assert_eq!(hit_code(0, widths), HTCLIENT);
        assert_eq!(hit_code(45, widths), HTCLIENT);
        assert_eq!(hit_code(46, widths), HTMAXBUTTON);
        assert_eq!(hit_code(91, widths), HTMAXBUTTON);
        assert_eq!(hit_code(92, widths), HTCLIENT);
        assert_eq!(hit_code(137, widths), HTCLIENT);
        // Past either end the default answer stands.
        assert_eq!(hit_code(-1, widths), HTCLIENT);
        assert_eq!(hit_code(138, widths), HTCLIENT);
    }

    #[test]
    fn a_leave_that_is_a_crossing_asks_for_the_other_areas_leave() {
        // Out of the client area and onto the middle column: the next leave to wait for is the non-client one.
        assert_eq!(crossing(false, Some(Button::Maximize)), Some(true));
        assert_eq!(crossing(true, Some(Button::Minimize)), Some(false));
        assert_eq!(crossing(true, Some(Button::Close)), Some(false));
        // A leave that finds the pointer in the area it says it left asks for nothing, so two leaves can't feed each other.
        assert_eq!(crossing(true, Some(Button::Maximize)), None);
        assert_eq!(crossing(false, Some(Button::Close)), None);
        assert_eq!(crossing(false, None), None);
        assert_eq!(crossing(true, None), None);
    }

    /// A hidden window of a system class, or of `class`.
    fn window(class: &str, style: u32, parent: HWND) -> HWND {
        let class: Vec<u16> = format!("{class}\0").encode_utf16().collect();
        // SAFETY: makes a hidden window on the test's own thread; the class buffer outlives the call.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null(),
                style,
                0,
                0,
                10,
                10,
                parent,
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            )
        };
        assert!(!hwnd.is_null(), "the test window was not made");
        hwnd
    }

    fn style_of(hwnd: HWND) -> u32 {
        // SAFETY: a query on a window the test made.
        unsafe { GetWindowLongW(hwnd, GWL_STYLE) as u32 }
    }

    #[test]
    fn the_webviews_window_stops_painting_over_its_siblings() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyWindow, WS_CLIPCHILDREN, WS_POPUP};
        let class: Vec<u16> = "WRY_WEBVIEW\0".encode_utf16().collect();
        // SAFETY: registers the class name wry gives its webview windows, with the default procedure, in the test's process.
        unsafe {
            RegisterClassW(&WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(DefWindowProcW),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: GetModuleHandleW(null()),
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class.as_ptr(),
            });
        }
        let parent = window("STATIC", WS_POPUP, null_mut());
        // The styles wry makes its window with: no `WS_CLIPSIBLINGS`.
        let webview = window("WRY_WEBVIEW", WS_CHILD | WS_CLIPCHILDREN, parent);
        let page = window("WRY_WEBVIEW", WS_CHILD | WS_CLIPCHILDREN, parent);
        let other = window("STATIC", WS_CHILD, parent);
        assert_eq!(style_of(webview) & WS_CLIPSIBLINGS, 0);

        clip_webviews(parent);

        assert_eq!(style_of(webview) & WS_CLIPSIBLINGS, WS_CLIPSIBLINGS);
        assert_eq!(style_of(page) & WS_CLIPSIBLINGS, WS_CLIPSIBLINGS);
        assert_eq!(style_of(webview) & WS_CLIPCHILDREN, WS_CLIPCHILDREN);
        assert_eq!(style_of(other) & WS_CLIPSIBLINGS, 0);
        // SAFETY: destroys the test's own windows; the children go with the parent.
        unsafe { DestroyWindow(parent) };
    }

    #[test]
    fn bgra_swaps_red_and_blue() {
        assert_eq!(bgra(&[1, 2, 3, 4, 5, 6, 7, 8]), vec![3, 2, 1, 4, 7, 6, 5, 8]);
    }
}
