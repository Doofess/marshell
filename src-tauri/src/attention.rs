//! The needs-you count on the taskbar, the flash and the sounds.

use marshell_core::notify::SoundKind;
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

/// Flashes the taskbar button. The core decides when (a needs-you wait while the window is in the background).
pub fn flash(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main")
        && let Err(e) = window.request_user_attention(Some(UserAttentionType::Informational))
    {
        eprintln!("Marshell: could not flash the taskbar button: {e}");
    }
}

/// Windows' own sound for each kind, so each follows the user's sound scheme and volume (3d decision 9) and
/// they differ: in the default Windows 11 scheme these are five different files.
pub fn sound_alias(kind: SoundKind) -> &'static str {
    match kind {
        SoundKind::NeedsYou => "Notification.Default",
        SoundKind::Escalation => "Notification.Reminder",
        SoundKind::Done => "Notification.Mail",
        SoundKind::SubagentDone => "Notification.IM",
        SoundKind::Error => "SystemHand",
    }
}

/// Whether a `QUERY_USER_NOTIFICATION_STATE` asks apps to keep quiet: anything but accepting notifications
/// (5) or an app running in its own quiet mode (7). Windows 11 do-not-disturb still reads as 5 (spike S3).
pub fn quiet_for_os(state: i32) -> bool {
    !matches!(state, 5 | 7)
}

/// Whether Windows asks apps to keep quiet now (a full-screen app, presentation mode). False when it can't say.
#[cfg(windows)]
pub fn os_is_quiet() -> bool {
    use windows_sys::Win32::UI::Shell::SHQueryUserNotificationState;
    let mut state = 0;
    // SAFETY: `state` is a valid, writable i32 for the call to fill.
    let hr = unsafe { SHQueryUserNotificationState(&mut state) };
    hr >= 0 && quiet_for_os(state)
}

#[cfg(not(windows))]
pub fn os_is_quiet() -> bool {
    false
}

/// `s` as a NUL-terminated UTF-16 string for the Win32 API.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Plays the kind's sound without waiting for it. No fallback beep: a scheme with the sound set to (None) stays silent.
#[cfg(windows)]
pub fn play_sound(kind: SoundKind) {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
    // Kept for the life of the process: with SND_ASYNC the call returns before the sound ends. One per kind.
    static ALIASES: [std::sync::OnceLock<Vec<u16>>; 5] = [const { std::sync::OnceLock::new() }; 5];
    let name = sound_alias(kind);
    let alias = ALIASES[kind as usize].get_or_init(|| wide(name));
    // SAFETY: `alias` is a NUL-terminated UTF-16 string that is never freed.
    let ok = unsafe {
        PlaySoundW(
            alias.as_ptr(),
            std::ptr::null_mut(),
            SND_ALIAS | SND_ASYNC | SND_NODEFAULT,
        )
    };
    if ok == 0 {
        eprintln!("Marshell: no sound for {kind:?} ({name})");
    }
}

/// The system sounds of macOS and Linux come when those platforms are built.
#[cfg(not(windows))]
pub fn play_sound(_kind: SoundKind) {}

#[cfg(test)]
mod tests {
    use super::{SoundKind, badge_rgba, quiet_for_os, sound_alias, wide};

    #[test]
    fn each_kind_has_its_own_windows_sound() {
        assert_eq!(sound_alias(SoundKind::NeedsYou), "Notification.Default");
        assert_eq!(sound_alias(SoundKind::Escalation), "Notification.Reminder");
        assert_eq!(sound_alias(SoundKind::Done), "Notification.Mail");
        assert_eq!(sound_alias(SoundKind::SubagentDone), "Notification.IM");
        assert_eq!(sound_alias(SoundKind::Error), "SystemHand");
    }

    #[test]
    fn only_accepting_notifications_and_an_apps_own_quiet_mode_allow_sound() {
        // QUNS_NOT_PRESENT, BUSY, RUNNING_D3D_FULL_SCREEN, PRESENTATION_MODE, ACCEPTS_NOTIFICATIONS, QUIET_TIME, APP.
        let quiet: Vec<bool> = (1..=7).map(quiet_for_os).collect();
        assert_eq!(quiet, [true, true, true, true, false, true, false]);
    }

    #[test]
    fn an_alias_is_a_c_wide_string() {
        let w = wide("Notification.Mail");
        assert_eq!(w.last(), Some(&0));
        assert_eq!(String::from_utf16(&w[..w.len() - 1]).unwrap(), "Notification.Mail");
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
