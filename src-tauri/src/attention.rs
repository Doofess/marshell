//! The needs-you count on the taskbar, the flash and the sound.

use tauri::{Manager, UserAttentionType};

const SIZE: u32 = 16;
/// The disc's colour (RGBA) and the digits' colour.
const DISC: [u8; 4] = [0xD9, 0x4F, 0x2B, 0xFF];
const INK: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// 3x5 glyphs, one row per entry, the low three bits left to right. `+` is the last one.
const GLYPHS: [[u8; 5]; 11] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
    [0b000, 0b010, 0b111, 0b010, 0b000],
];

/// The overlay icon for a count: a filled disc with the count in white, "9+" above nine. Returns RGBA, width, height.
pub fn badge_rgba(count: u32) -> (Vec<u8>, u32, u32) {
    let mut px = vec![0u8; (SIZE * SIZE * 4) as usize];
    let centre = (SIZE as f32 - 1.0) / 2.0;
    let radius = SIZE as f32 / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f32 - centre, y as f32 - centre);
            if dx * dx + dy * dy <= radius * radius {
                put(&mut px, x, y, DISC);
            }
        }
    }
    let (glyphs, scale): (Vec<usize>, u32) = if count > 9 {
        (vec![9, 10], 1)
    } else {
        (vec![count as usize], 2)
    };
    let width = glyphs.len() as u32 * 3 * scale + (glyphs.len() as u32 - 1) * scale;
    let (left, top) = ((SIZE - width) / 2, (SIZE - 5 * scale) / 2);
    for (i, g) in glyphs.iter().enumerate() {
        let gx = left + i as u32 * 4 * scale;
        for (row, bits) in GLYPHS[*g].iter().enumerate() {
            for col in 0..3u32 {
                if bits >> (2 - col) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            put(&mut px, gx + col * scale + sx, top + row as u32 * scale + sy, INK);
                        }
                    }
                }
            }
        }
    }
    (px, SIZE, SIZE)
}

fn put(px: &mut [u8], x: u32, y: u32, rgba: [u8; 4]) {
    let at = ((y * SIZE + x) * 4) as usize;
    px[at..at + 4].copy_from_slice(&rgba);
}

/// Shows the count on the taskbar button (Windows overlay icon); 0 clears it.
#[tauri::command]
pub fn set_badge(app: tauri::AppHandle, count: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let window = app.get_webview_window("main").ok_or("no main window")?;
        let icon = if count == 0 {
            None
        } else {
            let (rgba, w, h) = badge_rgba(count);
            Some(tauri::image::Image::new_owned(rgba, w, h))
        };
        window.set_overlay_icon(icon).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (app, count);
        Ok(())
    }
}

/// Flashes the taskbar button and, when `sound`, plays the needs-you sound. The window decides when to call it (a new request while unfocused).
#[tauri::command]
pub fn alert_needs_you(app: tauri::AppHandle, sound: bool) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("no main window")?;
    window
        .request_user_attention(Some(UserAttentionType::Informational))
        .map_err(|e| e.to_string())?;
    if sound {
        play_sound();
    }
    Ok(())
}

/// Windows' own notification sound, so it follows the user's sound scheme and volume (3d decision 9).
const SOUND_ALIAS: &str = "Notification.Default";

/// `s` as a NUL-terminated UTF-16 string for the Win32 API.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Plays the sound without waiting for it. No fallback beep: a scheme with the sound set to (None) stays silent.
#[cfg(windows)]
fn play_sound() {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
    // Kept for the life of the process: with SND_ASYNC the call returns before the sound ends.
    static ALIAS: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    let alias = ALIAS.get_or_init(|| wide(SOUND_ALIAS));
    // SAFETY: `alias` is a NUL-terminated UTF-16 string that is never freed.
    let ok = unsafe {
        PlaySoundW(
            alias.as_ptr(),
            std::ptr::null_mut(),
            SND_ALIAS | SND_ASYNC | SND_NODEFAULT,
        )
    };
    if ok == 0 {
        eprintln!("Marshell: no sound for needs-you ({SOUND_ALIAS})");
    }
}

/// The system sounds of macOS and Linux come when those platforms are built.
#[cfg(not(windows))]
fn play_sound() {}
#[cfg(test)]
mod tests {
    use super::{SOUND_ALIAS, badge_rgba, wide};

    #[test]
    fn the_sound_is_windows_own_notification_alias_as_a_c_wide_string() {
        assert_eq!(SOUND_ALIAS, "Notification.Default");
        let w = wide(SOUND_ALIAS);
        assert_eq!(w.last(), Some(&0));
        assert_eq!(String::from_utf16(&w[..w.len() - 1]).unwrap(), SOUND_ALIAS);
    }

    #[test]
    fn badge_rgba_sizes_and_nine_plus() {
        let (three, w, h) = badge_rgba(3);
        assert_eq!((w, h), (16, 16));
        assert_eq!(three.len(), 16 * 16 * 4);
        let (many, _, _) = badge_rgba(12);
        assert_eq!(many.len(), 16 * 16 * 4);
        assert_ne!(three, many);
        assert_eq!(badge_rgba(10).0, badge_rgba(99).0);
        assert_ne!(badge_rgba(9).0, badge_rgba(10).0);
    }
}
