//! Spike 6q-0 Task 10 (never merged): oversize and clip. WebView2 shows the old frame, pinned top-left, until the page
//! draws at a new size (S12 finding 23), so the page's size changes only when the pane outgrows it. wry's container HWND
//! takes the pane's rect and crops the larger page; a shrinking pane keeps the old frame's right and bottom edges, and
//! its scrollbar, outside the crop. The page takes the pane's exact size once the placements stop.

use super::{LABEL, log};
use serde_json::json;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
use windows::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos};

/// How long the placements must stop before the page takes the pane's size.
const SETTLE: Duration = Duration::from_millis(200);

/// Bumped by every placement; a settle timer acts only if it is still the newest.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// The pane's last rect in physical pixels: x, y, width, height.
static LAST: Mutex<Option<[i32; 4]>> = Mutex::new(None);

fn physical(x: f64, y: f64, width: f64, height: f64, scale: f64) -> [i32; 4] {
    let p = |v: f64| (v * scale).round() as i32;
    [p(x), p(y), p(width.max(0.0)), p(height.max(0.0))]
}

/// The page's next size: unchanged while the pane fits inside it, else the pane's size plus `ahead`, per axis.
pub fn grown(page: (i32, i32), pane: (i32, i32), ahead: i32) -> (i32, i32) {
    let axis = |page: i32, pane: i32| if pane > page { pane + ahead } else { page };
    (axis(page.0, pane.0), axis(page.1, pane.1))
}

/// Still-edge anchoring, one axis, in physical pixels. `last` and `pane` are the pane's (near, far) edges before and
/// now; `page` is the page's (offset from the pane's near edge, length). While the far edge stays put and the page is
/// longer than the pane, the page keeps its length and hangs off the near edge, so its far side (the scrollbar) stays
/// in view. Anything else gives the page the pane's exact length.
pub fn anchored(last: Option<(i32, i32)>, pane: (i32, i32), page: (i32, i32)) -> (i32, i32) {
    let len = pane.1 - pane.0;
    match last {
        Some(l) if l == pane => page,
        Some((_, far)) if far == pane.1 && page.1 > len => (len - page.1, page.1),
        _ => (0, len),
    }
}

/// SAFETY for every COM call here: the controller is live, and `with_webview` runs us on the main thread.
unsafe fn sizes(controller: &ICoreWebView2Controller) -> windows_core::Result<(HWND, RECT)> {
    let mut parent = HWND::default();
    let mut bounds = RECT::default();
    unsafe {
        controller.ParentWindow(&mut parent)?;
        controller.Bounds(&mut bounds)?;
    }
    Ok((parent, bounds))
}

/// Places the pane at the window-relative rect (CSS pixels): the container moves to it at once, and the page grows
/// only when the pane outgrows it. Runs on the main thread, inline (`with_webview` from the main thread is inline).
/// With `still`, the page is held against the pane's still edge instead of its top-left corner (`anchored`), and
/// `ahead` is not used.
pub fn place(
    app: &AppHandle,
    page: &tauri::Webview,
    rect: [f64; 4],
    ahead: u32,
    still: bool,
    why: &str,
) -> Result<(), String> {
    let scale = page.window().scale_factor().unwrap_or(1.0);
    let v = if still {
        // Sizes come from the rounded edges, so an edge that stays put in CSS pixels stays put in physical ones.
        let p = |v: f64| (v * scale).round() as i32;
        let (left, top) = (p(rect[0]), p(rect[1]));
        let right = p(rect[0] + rect[2].max(0.0));
        let bottom = p(rect[1] + rect[3].max(0.0));
        [left, top, right - left, bottom - top]
    } else {
        physical(rect[0], rect[1], rect[2], rect[3], scale)
    };
    let last = LAST.lock().ok().and_then(|mut last| last.replace(v));
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app2 = app.clone();
    let why = why.to_string();
    page.with_webview(move |pv| {
        let controller = pv.controller();
        let started = std::time::Instant::now();
        // SAFETY: see `sizes`; SetWindowPos on the container wry made for this controller, on its own thread.
        let result = unsafe {
            sizes(&controller).and_then(|(parent, bounds)| {
                let now = (bounds.right - bounds.left, bounds.bottom - bounds.top);
                let (at, next) = if still {
                    let edges = |r: [i32; 4]| ((r[0], r[0] + r[2]), (r[1], r[1] + r[3]));
                    let (pane_x, pane_y) = edges(v);
                    let (last_x, last_y) = last.map(edges).unzip();
                    let (x, width) = anchored(last_x, pane_x, (bounds.left, now.0));
                    let (y, height) = anchored(last_y, pane_y, (bounds.top, now.1));
                    ((x, y), (width, height))
                } else {
                    ((0, 0), grown(now, (v[2], v[3]), ahead as i32))
                };
                if next != now || (bounds.left, bounds.top) != at {
                    controller.SetBounds(RECT {
                        left: at.0,
                        top: at.1,
                        right: at.0 + next.0,
                        bottom: at.1 + next.1,
                    })?;
                }
                SetWindowPos(
                    parent.0,
                    std::ptr::null_mut(),
                    v[0],
                    v[1],
                    v[2],
                    v[3],
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                Ok((now, next, at))
            })
        };
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        match result {
            Ok((now, next, at)) => log(
                &app2,
                "rust",
                "clip",
                json!({ "why": why, "pane": v, "page": [next.0, next.1], "at": [at.0, at.1], "still": still, "grew": next != now, "ms": ms }),
            ),
            Err(e) => log(&app2, "rust", "clip-failed", json!({ "error": e.to_string() })),
        }
    })
    .map_err(|e| e.to_string())?;
    let app3 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SETTLE);
        let app4 = app3.clone();
        let _ = app3.run_on_main_thread(move || settle(&app4, Some(generation)));
    });
    Ok(())
}

/// Gives the page the pane's exact size: after a quiet `SETTLE` (`generation` still the newest), or at once (`None`)
/// when oversize is turned off.
pub fn settle(app: &AppHandle, generation: Option<u64>) {
    if generation.is_some_and(|g| g != GENERATION.load(Ordering::SeqCst)) {
        return;
    }
    let Some(v) = LAST.lock().ok().and_then(|l| *l) else {
        return;
    };
    let Some(page) = app.get_webview(LABEL) else {
        return;
    };
    let app2 = app.clone();
    let _ = page.with_webview(move |pv| {
        let controller = pv.controller();
        // SAFETY: see `sizes`.
        let result = unsafe {
            sizes(&controller).and_then(|(_, bounds)| {
                let now = (bounds.right - bounds.left, bounds.bottom - bounds.top);
                if now != (v[2], v[3]) {
                    controller.SetBounds(RECT {
                        left: 0,
                        top: 0,
                        right: v[2],
                        bottom: v[3],
                    })?;
                }
                Ok(now)
            })
        };
        match result {
            Ok(was) => log(
                &app2,
                "rust",
                "clip-settle",
                json!({ "pane": v, "was": [was.0, was.1] }),
            ),
            Err(e) => log(&app2, "rust", "clip-failed", json!({ "error": e.to_string() })),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{anchored, grown};

    #[test]
    fn a_shrinking_pane_keeps_the_page_size() {
        assert_eq!(grown((800, 600), (700, 550), 64), (800, 600));
    }

    #[test]
    fn a_pane_outgrowing_the_page_grows_it_with_room_ahead() {
        assert_eq!(grown((800, 600), (810, 600), 64), (874, 600));
        assert_eq!(grown((800, 600), (700, 640), 64), (800, 704));
    }

    #[test]
    fn a_pane_inside_the_room_ahead_changes_nothing() {
        assert_eq!(grown((874, 600), (850, 600), 64), (874, 600));
    }

    // Still-edge anchoring, one axis: the pane's (near, far) edges before and now, the page's (offset, length).

    #[test]
    fn a_pane_shrinking_from_its_near_edge_keeps_the_page_against_the_still_far_edge() {
        // The divider moved right by 100: the page keeps its 500 and hangs 100 off the pane's near edge.
        assert_eq!(anchored(Some((700, 1200)), (800, 1200), (0, 500)), (-100, 500));
        // And 30 more: still the same page, 130 off.
        assert_eq!(anchored(Some((800, 1200)), (830, 1200), (-100, 500)), (-130, 500));
    }

    #[test]
    fn a_pane_growing_back_inside_the_kept_page_uncovers_it() {
        assert_eq!(anchored(Some((830, 1200)), (760, 1200), (-130, 500)), (-60, 500));
    }

    #[test]
    fn a_pane_outgrowing_the_page_gets_the_exact_size() {
        assert_eq!(anchored(Some((700, 1200)), (650, 1200), (0, 500)), (0, 550));
        assert_eq!(anchored(Some((760, 1200)), (650, 1200), (-60, 500)), (0, 550));
    }

    #[test]
    fn a_moving_far_edge_gets_the_exact_size() {
        // A window's right-edge drag: the far edge moves, so nothing is kept.
        assert_eq!(anchored(Some((700, 1200)), (700, 1100), (0, 500)), (0, 400));
        assert_eq!(anchored(Some((700, 1200)), (690, 1100), (0, 500)), (0, 410));
    }

    #[test]
    fn an_axis_that_did_not_move_leaves_the_page_alone() {
        assert_eq!(anchored(Some((800, 1200)), (800, 1200), (-100, 500)), (-100, 500));
    }

    #[test]
    fn the_first_placement_is_exact() {
        assert_eq!(anchored(None, (700, 1200), (0, 0)), (0, 500));
    }
}
