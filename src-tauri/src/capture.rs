//! Capture a screen region (Windows): open the system's snipping tool, then wait for the image it puts on the clipboard.
//!
//! Ruling: the wait polls the clipboard's change counter every 150 ms for at most 60 s, rather than using
//! `AddClipboardFormatListener`, which needs a message-only window of its own. A poll is simpler and cannot leak a window.
//!
//! The wait ends on the first change that carries an image (a text copy during the wait is not the capture), when the frontend
//! cancels (Esc in the overlay shows up there as the window getting its focus back with no image), or after 60 s. Every end
//! releases the running flag, because the flag is held by a guard.
//!
//! The image is read here, not by the page: while the snipping overlay is up the page has no focus, and `navigator.clipboard.read()`
//! fails with "Document is not focused". `read_clipboard_image` opens the clipboard itself (no focus needed), retrying briefly while
//! another process holds it, and hands the raw PNG or .bmp bytes to the page, which converts them to PNG like any other image.

use std::sync::atomic::{AtomicBool, Ordering};

/// What the person sees when a capture can't run here.
#[cfg_attr(windows, allow(dead_code))]
const NOT_AVAILABLE: &str = "Capturing the screen isn't available on this system yet.";
#[cfg_attr(not(windows), allow(dead_code))]
const ALREADY_RUNNING: &str = "A capture is already running.";
#[cfg_attr(not(windows), allow(dead_code))]
const DID_NOT_OPEN: &str = "The snipping tool didn't open.";
#[cfg_attr(not(windows), allow(dead_code))]
const CLIPBOARD_BUSY: &str = "The clipboard is busy.";

/// A clipboard change counts as the capture only when it carries an image: a text copy changes the counter too.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn image_arrived(start_seq: u32, now_seq: u32, has_image: bool) -> bool {
    now_seq != start_seq && has_image
}

/// Wraps a CF_DIB payload (an info header of 40 bytes or more, optional masks and colour table, then the pixels) in a BITMAPFILEHEADER, so
/// it is a .bmp file the webview can decode. None when the header doesn't fit the payload.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    const FILE_HEADER: usize = 14;
    let u32_at = |at: usize| -> Option<u32> { Some(u32::from_le_bytes(dib.get(at..at + 4)?.try_into().ok()?)) };
    let u16_at = |at: usize| -> Option<u16> { Some(u16::from_le_bytes(dib.get(at..at + 2)?.try_into().ok()?)) };
    if dib.len() < 40 {
        return None;
    }
    let header = usize::try_from(u32_at(0)?).ok()?;
    if header < 40 || header > dib.len() {
        return None;
    }
    // A 40-byte header keeps its channel masks after it; a larger one carries them inside.
    let masks = match (header, u32_at(16)?) {
        (40, 3) => 12,
        (40, 6) => 16,
        _ => 0,
    };
    let bit_count = u16_at(14)?;
    let colours_used = usize::try_from(u32_at(32)?).ok()?;
    let table = if colours_used != 0 {
        colours_used.checked_mul(4)?
    } else if bit_count <= 8 {
        (1usize << bit_count) * 4
    } else {
        0
    };
    let before_pixels = header.checked_add(masks)?.checked_add(table)?;
    if before_pixels > dib.len() {
        return None;
    }
    let offset = u32::try_from(FILE_HEADER + before_pixels).ok()?;
    let size = u32::try_from(FILE_HEADER + dib.len()).ok()?;
    let mut bmp = Vec::with_capacity(FILE_HEADER + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&size.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
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
    use super::dib_to_bmp;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    const CF_DIB: u32 = 8;
    const CF_DIBV5: u32 = 17;

    /// Another process holds the clipboard open; the caller tries again.
    pub struct Busy;

    /// The clipboard, open for this thread; dropping it closes it, on every path out of a read.
    struct Open;

    impl Open {
        fn new() -> Result<Self, Busy> {
            // SAFETY: a null owner window ties the open clipboard to this thread; the guard closes it again.
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                Ok(Self)
            } else {
                Err(Busy)
            }
        }

        /// Copies the global memory the clipboard holds for `format`, or None when it holds none.
        fn copy(&self, format: u32) -> Option<Vec<u8>> {
            // SAFETY: `self` proves the clipboard is open, which GetClipboardData needs. The handle stays owned by the clipboard:
            // it is only locked here, read for exactly the size the system reports, and unlocked, before the clipboard closes.
            unsafe {
                let handle = GetClipboardData(format);
                if handle.is_null() {
                    return None;
                }
                let size = GlobalSize(handle);
                let ptr = GlobalLock(handle) as *const u8;
                if ptr.is_null() {
                    return None;
                }
                let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
                GlobalUnlock(handle);
                Some(bytes)
            }
        }
    }

    impl Drop for Open {
        fn drop(&mut self) {
            // SAFETY: only an open clipboard has a guard, and this thread opened it.
            unsafe { CloseClipboard() };
        }
    }

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

        /// The image on the clipboard: PNG bytes if it holds the registered `PNG` format, else the DIB as a .bmp file, else None.
        /// Err when the clipboard can't be opened right now.
        pub fn read_image(&self) -> Result<Option<Vec<u8>>, Busy> {
            let clipboard = Open::new()?;
            // SAFETY: plain query of the clipboard's formats.
            let has = |format: u32| format != 0 && unsafe { IsClipboardFormatAvailable(format) != 0 };
            if has(self.png) {
                return Ok(clipboard.copy(self.png));
            }
            if has(CF_DIB) {
                return Ok(clipboard.copy(CF_DIB).and_then(|dib| dib_to_bmp(&dib)));
            }
            Ok(None)
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

/// The image on the clipboard as raw bytes: PNG, or a .bmp file for a bitmap. Empty when the clipboard holds no image.
#[cfg(windows)]
#[tauri::command]
pub async fn read_clipboard_image() -> Result<tauri::ipc::Response, String> {
    use std::time::Duration;

    const ATTEMPTS: u32 = 10;
    const RETRY_AFTER: Duration = Duration::from_millis(50);

    let formats = clipboard::ImageFormats::new();
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(RETRY_AFTER).await;
        }
        // The clipboard is open only inside `read_image`, never across the sleep.
        if let Ok(image) = formats.read_image() {
            return Ok(tauri::ipc::Response::new(image.unwrap_or_default()));
        }
    }
    Err(CLIPBOARD_BUSY.to_string())
}

#[cfg(not(windows))]
#[tauri::command]
pub async fn read_clipboard_image() -> Result<tauri::ipc::Response, String> {
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

    /// A DIB of `len` bytes whose info header says: `header` bytes long, `bits` per pixel, `compression`, `colours_used` colours.
    fn dib(header: u32, bits: u16, compression: u32, colours_used: u32, len: usize) -> Vec<u8> {
        let mut dib = vec![0u8; len];
        dib[0..4].copy_from_slice(&header.to_le_bytes());
        dib[14..16].copy_from_slice(&bits.to_le_bytes());
        dib[16..20].copy_from_slice(&compression.to_le_bytes());
        dib[32..36].copy_from_slice(&colours_used.to_le_bytes());
        dib
    }

    fn offset_of(bmp: &[u8]) -> u32 {
        u32::from_le_bytes(bmp[10..14].try_into().unwrap())
    }

    #[test]
    fn a_24_bit_dib_becomes_a_bmp_with_the_pixels_right_after_the_header() {
        let dib = dib(40, 24, 0, 0, 44);
        let bmp = dib_to_bmp(&dib).expect("valid");
        assert_eq!(bmp.len(), 14 + dib.len());
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bmp[2..6].try_into().unwrap()), bmp.len() as u32);
        assert_eq!(&bmp[6..10], &[0; 4]);
        assert_eq!(offset_of(&bmp), 54);
        assert_eq!(&bmp[14..], &dib[..]);
    }

    #[test]
    fn a_bitfields_dib_skips_its_masks_before_the_pixels() {
        assert_eq!(offset_of(&dib_to_bmp(&dib(40, 32, 3, 0, 56)).unwrap()), 66);
        assert_eq!(offset_of(&dib_to_bmp(&dib(40, 32, 6, 0, 60)).unwrap()), 70);
    }

    #[test]
    fn a_palette_dib_skips_its_colour_table() {
        assert_eq!(
            offset_of(&dib_to_bmp(&dib(40, 8, 0, 0, 40 + 1024 + 4)).unwrap()),
            54 + 1024
        );
        assert_eq!(offset_of(&dib_to_bmp(&dib(40, 1, 0, 2, 40 + 8 + 4)).unwrap()), 54 + 8);
    }

    #[test]
    fn a_v5_header_carries_its_masks_inside() {
        assert_eq!(offset_of(&dib_to_bmp(&dib(124, 32, 3, 0, 128)).unwrap()), 14 + 124);
    }

    #[test]
    fn a_dib_whose_header_does_not_fit_is_refused() {
        assert!(dib_to_bmp(&[0; 39]).is_none());
        assert!(dib_to_bmp(&dib(39, 24, 0, 0, 44)).is_none());
        assert!(dib_to_bmp(&dib(45, 24, 0, 0, 44)).is_none());
        // The masks or the colour table would run past the end.
        assert!(dib_to_bmp(&dib(40, 32, 3, 0, 50)).is_none());
        assert!(dib_to_bmp(&dib(40, 8, 0, 0, 100)).is_none());
    }

    #[test]
    fn a_hostile_colour_count_is_refused_without_a_panic() {
        assert!(dib_to_bmp(&dib(40, 8, 0, u32::MAX, 44)).is_none());
        assert!(dib_to_bmp(&dib(40, 24, 0, u32::MAX, 44)).is_none());
    }

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
