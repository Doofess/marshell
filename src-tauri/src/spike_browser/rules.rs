//! Spike 6q-0: the pure rules the browser child follows. Kept apart so 6q-3 can lift them as they are.

use super::PaneRect;
use tauri::Url;

/// A rect in CSS (logical) pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Anchor mode: where the pane goes when the window becomes `win_width`×`win_height`, before the front has measured
/// again. It keeps its width and its right and bottom insets, so it rides the right edge, and its height stretches.
pub fn anchored(rect: PaneRect, win_width: f64, win_height: f64) -> LogicalRect {
    let right = rect.win_width - (rect.x + rect.width);
    let bottom = rect.win_height - (rect.y + rect.height);
    LogicalRect {
        x: (win_width - right - rect.width).max(0.0),
        y: rect.y,
        width: rect.width,
        height: (win_height - bottom - rect.y).max(0.0),
    }
}

/// Where the page may go: web pages and a blank page. The app's own `*.localhost` hosts (its assets, its IPC) are never a
/// page, and `localhost` itself is, for port chips. Every way in (typed, a link, a redirect, a drop) passes through here.
pub fn allowed_navigation(url: &Url) -> bool {
    match url.scheme() {
        "http" | "https" => !url
            .host_str()
            .is_some_and(|h| h.to_ascii_lowercase().ends_with(".localhost")),
        "about" => url.as_str() == "about:blank",
        _ => false,
    }
}

// Windows virtual-key codes the rules name.
pub const VK_TAB: u32 = 0x09;
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_L: u32 = 0x4C;
pub const VK_F12: u32 = 0x7B;

/// Who a key pressed in the page belongs to (6q decision 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRoute {
    /// Marshell's: every Ctrl+Shift chord, Ctrl+L (the address bar) and Esc.
    App,
    /// F12 opens the page's own DevTools.
    DevTools,
    /// Everything else, the browser's own keys included (F5, Ctrl+R, Ctrl+F, Ctrl+P).
    Page,
}

/// AltGr arrives as Ctrl+Alt, so any chord with Alt stays in the page: it types a character on many layouts.
pub fn route_key(vk: u32, ctrl: bool, shift: bool, alt: bool) -> KeyRoute {
    match (vk, ctrl, shift, alt) {
        (VK_F12, false, false, false) => KeyRoute::DevTools,
        (VK_ESCAPE, false, false, false) => KeyRoute::App,
        (VK_L, true, false, false) => KeyRoute::App,
        (_, true, true, false) => KeyRoute::App,
        _ => KeyRoute::Page,
    }
}

/// The DOM `KeyboardEvent.code` for a virtual key, for the keys the app's keymap reads.
pub fn vk_to_code(vk: u32) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL", "KeyM", "KeyN",
        "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV", "KeyW", "KeyX", "KeyY", "KeyZ",
    ];
    const DIGITS: [&str; 10] = [
        "Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9",
    ];
    const FKEYS: [&str; 12] = [
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    ];
    Some(match vk {
        0x41..=0x5A => LETTERS[(vk - 0x41) as usize],
        0x30..=0x39 => DIGITS[(vk - 0x30) as usize],
        0x70..=0x7B => FKEYS[(vk - 0x70) as usize],
        VK_TAB => "Tab",
        VK_ESCAPE => "Escape",
        0x0D => "Enter",
        0x08 => "Backspace",
        0x20 => "Space",
        0x25 => "ArrowLeft",
        0x26 => "ArrowUp",
        0x27 => "ArrowRight",
        0x28 => "ArrowDown",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x24 => "Home",
        0x23 => "End",
        0x2E => "Delete",
        0xBA => "Semicolon",
        0xBB => "Equal",
        0xBC => "Comma",
        0xBD => "Minus",
        0xBE => "Period",
        0xBF => "Slash",
        0xC0 => "Backquote",
        0xDB => "BracketLeft",
        0xDC => "Backslash",
        0xDD => "BracketRight",
        0xDE => "Quote",
        _ => return None,
    })
}

/// Which way focus leaves the page when Tab or Shift+Tab runs off its end, from WebView2's `MoveFocusRequested` reason
/// (0 programmatic, 1 next, 2 previous). A programmatic request is not the user leaving the page.
pub fn focus_out_direction(reason: i32) -> Option<&'static str> {
    match reason {
        1 => Some("next"),
        2 => Some("previous"),
        _ => None,
    }
}

/// What the app is sent for a forwarded key.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct KeyEvent {
    pub code: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// From WebView2's `WasKeyDown`: the key was already down, so this is the keyboard's repeat.
    pub repeat: bool,
}

pub fn key_event(vk: u32, ctrl: bool, shift: bool, alt: bool, was_key_down: bool) -> KeyEvent {
    KeyEvent {
        code: vk_to_code(vk).map_or_else(|| format!("VK{vk:#04X}"), str::to_string),
        ctrl,
        shift,
        alt,
        repeat: was_key_down,
    }
}

/// Standard base64 with padding, for the frozen picture's `data:` URL. The spike adds no crate for it.
pub fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const VK_A: u32 = 0x41;
    const VK_C: u32 = 0x43;
    const VK_F: u32 = 0x46;
    const VK_I: u32 = 0x49;
    const VK_K: u32 = 0x4B;
    const VK_P: u32 = 0x50;
    const VK_Q: u32 = 0x51;
    const VK_R: u32 = 0x52;
    const VK_V: u32 = 0x56;
    const VK_Z: u32 = 0x5A;
    const VK_F5: u32 = 0x74;

    #[test]
    fn web_pages_and_blank_are_allowed() {
        for u in [
            "https://example.com/",
            "http://127.0.0.1:47620/",
            "http://localhost:3000/",
            "about:blank",
        ] {
            assert!(allowed_navigation(&Url::parse(u).unwrap()), "{u}");
        }
    }

    #[test]
    fn files_and_the_apps_own_hosts_are_refused() {
        for u in [
            "file:///C:/Windows/win.ini",
            "http://tauri.localhost/",
            "https://tauri.localhost/",
            "http://ipc.localhost/x",
            "http://asset.localhost/x",
            "tauri://localhost/",
            "data:text/html,hi",
            "javascript:alert(1)",
        ] {
            assert!(!allowed_navigation(&Url::parse(u).unwrap()), "{u}");
        }
    }

    #[test]
    fn marshells_chords_go_to_the_app() {
        assert_eq!(route_key(VK_K, true, true, false), KeyRoute::App); // palette
        assert_eq!(route_key(VK_Q, true, true, false), KeyRoute::App); // the pane
        assert_eq!(route_key(VK_I, true, true, false), KeyRoute::App); // focus mode, not DevTools
        assert_eq!(route_key(VK_TAB, true, true, false), KeyRoute::App); // select the terminal
        assert_eq!(route_key(VK_L, true, false, false), KeyRoute::App); // address bar
        assert_eq!(route_key(VK_ESCAPE, false, false, false), KeyRoute::App);
    }

    #[test]
    fn f12_opens_the_pages_devtools() {
        assert_eq!(route_key(VK_F12, false, false, false), KeyRoute::DevTools);
    }

    #[test]
    fn the_page_keeps_the_browsers_keys() {
        assert_eq!(route_key(VK_F5, false, false, false), KeyRoute::Page);
        for vk in [VK_R, VK_F, VK_P, VK_C, VK_V, VK_A, VK_Z] {
            assert_eq!(route_key(vk, true, false, false), KeyRoute::Page);
        }
    }

    #[test]
    fn altgr_combos_stay_in_the_page() {
        // AltGr arrives as Ctrl+Alt: AltGr+Q is @ on a German layout, AltGr+Shift+K must not open the palette.
        assert_eq!(route_key(VK_Q, true, false, true), KeyRoute::Page);
        assert_eq!(route_key(VK_K, true, true, true), KeyRoute::Page);
    }

    #[test]
    fn virtual_keys_map_to_dom_codes() {
        assert_eq!(vk_to_code(VK_K), Some("KeyK"));
        assert_eq!(vk_to_code(0x31), Some("Digit1"));
        assert_eq!(vk_to_code(VK_TAB), Some("Tab"));
        assert_eq!(vk_to_code(VK_ESCAPE), Some("Escape"));
        assert_eq!(vk_to_code(VK_F12), Some("F12"));
    }

    #[test]
    fn a_held_chord_is_sent_as_a_repeat() {
        assert!(key_event(VK_K, true, true, false, true).repeat);
        assert!(!key_event(VK_K, true, true, false, false).repeat);
        assert_eq!(key_event(VK_K, true, true, false, false).code, "KeyK");
    }

    #[test]
    fn tabbing_off_either_end_of_the_page_names_the_direction() {
        // COREWEBVIEW2_MOVE_FOCUS_REASON: 0 programmatic, 1 next, 2 previous.
        assert_eq!(focus_out_direction(1), Some("next"));
        assert_eq!(focus_out_direction(2), Some("previous"));
        assert_eq!(focus_out_direction(0), None);
    }

    #[test]
    fn redirects_and_drops_are_checked_like_typed_urls() {
        // One rule for every way in: on_navigation sees redirects and drops as plain URLs.
        assert!(!allowed_navigation(&Url::parse("file://server/share/a.pdf").unwrap()));
        assert!(!allowed_navigation(&Url::parse("http://sub.tauri.localhost/").unwrap()));
    }

    #[test]
    fn the_pane_rides_the_right_edge() {
        let r = PaneRect {
            x: 700.0,
            y: 40.0,
            width: 480.0,
            height: 700.0,
            win_width: 1200.0,
            win_height: 780.0,
        };
        assert_eq!(
            anchored(r, 1200.0, 780.0),
            LogicalRect {
                x: 700.0,
                y: 40.0,
                width: 480.0,
                height: 700.0
            }
        );
        assert_eq!(
            anchored(r, 1300.0, 830.0),
            LogicalRect {
                x: 800.0,
                y: 40.0,
                width: 480.0,
                height: 750.0
            }
        );
    }

    #[test]
    fn a_tiny_window_never_gives_negative_bounds() {
        let r = PaneRect {
            x: 700.0,
            y: 40.0,
            width: 480.0,
            height: 700.0,
            win_width: 1200.0,
            win_height: 780.0,
        };
        let a = anchored(r, 300.0, 60.0);
        assert!(a.x >= 0.0 && a.height >= 0.0);
    }

    #[test]
    fn the_frozen_picture_is_sent_as_standard_base64() {
        // RFC 4648 §10's vectors: every padding case.
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (raw, encoded) in cases {
            assert_eq!(base64(raw.as_bytes()), encoded, "{raw:?}");
        }
        assert_eq!(base64(&[0xfb, 0xff, 0xbf]), "+/+/");
    }
}
