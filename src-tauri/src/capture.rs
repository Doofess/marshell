//! Capture a screen region (Windows): open the system's snipping tool, then wait for the image it puts on the clipboard.
//!
//! Ruling: the wait polls the clipboard's change counter every 150 ms for at most 60 s, rather than using
//! `AddClipboardFormatListener`, which needs a message-only window of its own. A poll is simpler and cannot leak a window.
//!
//! The wait ends on the first change that carries an image (a text copy during the wait is not the capture), when the frontend
//! cancels (Esc in the overlay shows up there as the window getting its focus back with no image), or after 60 s. Every end
//! releases the running flag, because the flag is held by a guard.

use std::sync::atomic::{AtomicBool, Ordering};

/// What the person sees when a capture can't run here.
#[cfg_attr(windows, allow(dead_code))]
const NOT_AVAILABLE: &str = "Capturing the screen isn't available on this system yet.";
#[cfg_attr(not(windows), allow(dead_code))]
const ALREADY_RUNNING: &str = "A capture is already running.";
#[cfg_attr(not(windows), allow(dead_code))]
const DID_NOT_OPEN: &str = "The snipping tool didn't open.";

/// A clipboard change counts as the capture only when it carries an image: a text copy changes the counter too.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn image_arrived(start_seq: u32, now_seq: u32, has_image: bool) -> bool {
    now_seq != start_seq && has_image
}

/// Whether a capture is running, and whether the frontend asked it to stop.
#[derive(Default)]
pub struct CaptureState {
    running: AtomicBool,
    cancelled: AtomicBool,
}

/// Holds the running flag; dropping it, on any path out of the capture, clears the flag.
#[cfg_attr(not(windows), allow(dead_code))]
struct Running<'a>(&'a CaptureState);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::SeqCst);
    }
}

impl CaptureState {
    /// Claims the capture, or None when one is already running.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn begin(&self) -> Option<Running<'_>> {
        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()?;
        self.cancelled.store(false, Ordering::SeqCst);
        Some(Running(self))
    }

    /// Asks the running capture to stop. With nothing running it does nothing, so a late cancel can't end the next capture.
    fn cancel(&self) {
        if self.running.load(Ordering::SeqCst) {
            self.cancelled.store(true, Ordering::SeqCst);
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[cfg(windows)]
mod clipboard {
    use windows_sys::Win32::System::DataExchange::{
        GetClipboardSequenceNumber, IsClipboardFormatAvailable, RegisterClipboardFormatW,
    };

    const CF_DIB: u32 = 8;
    const CF_DIBV5: u32 = 17;

    /// The formats an image can arrive in: the registered `PNG`, and the two bitmap formats the snipping tool and most apps use.
    pub struct ImageFormats {
        png: u32,
    }

    impl ImageFormats {
        pub fn new() -> Self {
            let name: Vec<u16> = "PNG".encode_utf16().chain(Some(0)).collect();
            // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call.
            Self {
                png: unsafe { RegisterClipboardFormatW(name.as_ptr()) },
            }
        }

        pub fn on_clipboard(&self) -> bool {
            [self.png, CF_DIB, CF_DIBV5].into_iter().any(|format| {
                format != 0 && {
                    // SAFETY: plain query of the clipboard's formats; it needs no open clipboard.
                    unsafe { IsClipboardFormatAvailable(format) != 0 }
                }
            })
        }
    }

    pub fn sequence() -> u32 {
        // SAFETY: no arguments, no state.
        unsafe { GetClipboardSequenceNumber() }
    }
}

/// Opens the snipping tool and waits for the image it leaves on the clipboard. True: an image arrived. False: cancelled or timed out.
#[cfg(windows)]
#[tauri::command]
pub async fn capture_screen_region(
    app: tauri::AppHandle,
    state: tauri::State<'_, CaptureState>,
) -> Result<bool, String> {
    use std::time::Duration;
    use tauri_plugin_opener::OpenerExt;
    use tokio::time::{Instant, MissedTickBehavior, interval};

    const POLL: Duration = Duration::from_millis(150);
    const LIMIT: Duration = Duration::from_secs(60);

    let _running = state.begin().ok_or_else(|| ALREADY_RUNNING.to_string())?;
    let formats = clipboard::ImageFormats::new();
    let start_seq = clipboard::sequence();
    app.opener()
        .open_url("ms-screenclip:", None::<&str>)
        .map_err(|_| DID_NOT_OPEN.to_string())?;
    let deadline = Instant::now() + LIMIT;
    let mut tick = interval(POLL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        // The image is checked first, so one that lands as the cancel arrives is still taken.
        if image_arrived(start_seq, clipboard::sequence(), formats.on_clipboard()) {
            return Ok(true);
        }
        if state.is_cancelled() || Instant::now() >= deadline {
            return Ok(false);
        }
    }
}

#[cfg(not(windows))]
#[tauri::command]
pub async fn capture_screen_region(_state: tauri::State<'_, CaptureState>) -> Result<bool, String> {
    Err(NOT_AVAILABLE.to_string())
}

/// Ends the running capture early (Esc in the snipping overlay). With none running it does nothing.
#[tauri::command]
pub fn cancel_capture(state: tauri::State<'_, CaptureState>) {
    state.cancel();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unchanged_clipboard_is_not_an_arrival() {
        assert!(!image_arrived(5, 5, true));
    }

    #[test]
    fn a_text_copy_is_not_an_arrival() {
        assert!(!image_arrived(5, 6, false));
    }

    #[test]
    fn a_changed_clipboard_with_an_image_is_an_arrival() {
        assert!(image_arrived(5, 6, true));
    }

    #[test]
    fn a_second_capture_is_refused_until_the_first_is_dropped() {
        let state = CaptureState::default();
        let first = state.begin();
        assert!(first.is_some());
        assert!(state.begin().is_none());
        drop(first);
        assert!(state.begin().is_some());
    }

    #[test]
    fn a_cancel_with_nothing_running_is_forgotten() {
        let state = CaptureState::default();
        state.cancel();
        let _running = state.begin().expect("idle");
        assert!(!state.is_cancelled());
    }

    #[test]
    fn a_cancel_during_a_capture_is_seen_and_does_not_outlive_it() {
        let state = CaptureState::default();
        let running = state.begin().expect("idle");
        state.cancel();
        assert!(state.is_cancelled());
        drop(running);
        let _next = state.begin().expect("idle");
        assert!(!state.is_cancelled());
    }
}
