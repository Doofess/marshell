//! Spike 6q-0: the pure rules the browser child follows. Kept apart so 6q-3 can lift them as they are.

use super::PaneRect;
use serde::Deserialize;
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

/// Predict mode: the pane's layout as Rust learns it from the front's reports, so the page can move in the same instant
/// the window resizes instead of a report's trip later (S12 finding 20). Each edge moves by a share of the window's change
/// in size: 1 for an edge that rides the window's right or bottom edge, 0 for one that stays put, the pane's share for an
/// edge set by an `fr` column. Two reports at different window sizes give the shares; until then the pane rides the right
/// and bottom edges and keeps its width, as anchor mode did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    last: Option<PaneRect>,
    /// How far the left, right, top and bottom edges move per pixel the window grows.
    share: [f64; 4],
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            last: None,
            share: [1.0, 1.0, 0.0, 1.0],
        }
    }
}

/// A window change smaller than this says nothing about the shares: the report's sizes are rounded.
const MIN_LEARN_STEP: f64 = 2.0;

impl Layout {
    /// Takes a report from the front: the placeholder's box and the size of the layout it was measured in.
    pub fn learn(&mut self, r: PaneRect) {
        if let Some(p) = self.last {
            let share = |now: f64, before: f64, step: f64| ((now - before) / step).clamp(0.0, 1.0);
            let dw = r.win_width - p.win_width;
            if dw.abs() >= MIN_LEARN_STEP {
                self.share[0] = share(r.x, p.x, dw);
                self.share[1] = share(r.x + r.width, p.x + p.width, dw);
            }
            let dh = r.win_height - p.win_height;
            if dh.abs() >= MIN_LEARN_STEP {
                self.share[2] = share(r.y, p.y, dh);
                self.share[3] = share(r.y + r.height, p.y + p.height, dh);
            }
        }
        self.last = Some(r);
    }

    /// Where the pane will be once the layout is `win_width`×`win_height`; None before the first report.
    pub fn place(&self, win_width: f64, win_height: f64) -> Option<LogicalRect> {
        let r = self.last?;
        let (dw, dh) = (win_width - r.win_width, win_height - r.win_height);
        let left = r.x + self.share[0] * dw;
        let right = r.x + r.width + self.share[1] * dw;
        let top = r.y + self.share[2] * dh;
        let bottom = r.y + r.height + self.share[3] * dh;
        Some(LogicalRect {
            x: left.max(0.0),
            y: top.max(0.0),
            width: (right - left).max(0.0),
            height: (bottom - top).max(0.0),
        })
    }
}

/// Whether a report was measured in a layout of the window's size now (its sizes are rounded, so within half a pixel).
pub fn same_size(win_width: f64, win_height: f64, r: PaneRect) -> bool {
    (win_width - r.win_width).abs() < 0.5 && (win_height - r.win_height).abs() < 0.5
}

/// One straight stretch of the front's layout: from window width `from` until the next piece starts, the pane's left edge
/// is `x` and its width `width`, each changing by its `_per_px` for every pixel the window is wider than `from`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Piece {
    pub from: f64,
    pub x: f64,
    pub x_per_px: f64,
    pub width: f64,
    pub width_per_px: f64,
}

/// Rule mode: the front's own layout, sent whenever it changes, as straight pieces over the window's width (S12 finding
/// 21). Unlike predict mode it knows the jumps (the sidebar folding to the rail, the page taking the whole main area), so
/// the page lands where the front will lay it out at every width. The pane's top stays put and its bottom keeps its
/// distance from the window's bottom.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub pieces: Vec<Piece>,
    pub top: f64,
    pub bottom: f64,
}

impl Rule {
    /// The same rule with its pieces in order of width, as `place` reads them.
    pub fn sorted(mut self) -> Self {
        self.pieces.sort_by(|a, b| a.from.total_cmp(&b.from));
        self
    }

    /// Where the pane is when the window is `win_width`×`win_height`; None for a rule with no pieces. A width under the
    /// first piece's start takes the first piece.
    pub fn place(&self, win_width: f64, win_height: f64) -> Option<LogicalRect> {
        let p = self
            .pieces
            .iter()
            .rev()
            .find(|p| p.from <= win_width)
            .or_else(|| self.pieces.first())?;
        let d = win_width - p.from;
        Some(LogicalRect {
            x: (p.x + p.x_per_px * d).max(0.0),
            y: self.top,
            width: (p.width + p.width_per_px * d).max(0.0),
            height: (win_height - self.top - self.bottom).max(0.0),
        })
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

    /// The spike's layout: a 240 px sidebar, then the main area split half and half with a 1 px hairline, under a 40 px
    /// bar, with the pane flush with the window's right and bottom edges.
    fn pane_in(win_width: f64, win_height: f64) -> PaneRect {
        let main = win_width - 240.0 - 1.0;
        PaneRect {
            x: 240.0 + main / 2.0 + 1.0,
            y: 40.0,
            width: main / 2.0,
            height: win_height - 40.0,
            win_width,
            win_height,
        }
    }

    fn rect_of(p: PaneRect) -> LogicalRect {
        LogicalRect {
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
        }
    }

    #[test]
    fn before_any_report_there_is_nowhere_to_put_the_page() {
        assert_eq!(Layout::default().place(1200.0, 780.0), None);
    }

    #[test]
    fn one_report_rides_the_right_and_bottom_edges_and_keeps_its_width() {
        let mut l = Layout::default();
        l.learn(pane_in(1200.0, 780.0));
        let p = pane_in(1200.0, 780.0);
        assert_eq!(
            l.place(1300.0, 800.0),
            Some(LogicalRect {
                x: p.x + 100.0,
                y: 40.0,
                width: p.width,
                height: p.height + 20.0
            })
        );
    }

    #[test]
    fn two_reports_learn_the_shares_and_predict_the_layout_exactly() {
        let mut l = Layout::default();
        l.learn(pane_in(1200.0, 780.0));
        l.learn(pane_in(1260.0, 800.0));
        for (w, h) in [(1500.0, 900.0), (900.0, 600.0), (1260.0, 800.0)] {
            assert_eq!(l.place(w, h), Some(rect_of(pane_in(w, h))), "{w}x{h}");
        }
    }

    #[test]
    fn a_divider_move_at_the_same_size_shifts_the_pane_and_keeps_the_shares() {
        let mut l = Layout::default();
        l.learn(pane_in(1200.0, 780.0));
        l.learn(pane_in(1260.0, 780.0));
        // The divider moves 30 px left with the window unchanged: the pane's left edge follows, its right edge stays.
        let mut moved = pane_in(1260.0, 780.0);
        moved.x -= 30.0;
        moved.width += 30.0;
        l.learn(moved);
        let at = l.place(1360.0, 780.0).unwrap();
        assert_eq!(at.x, moved.x + 50.0);
        assert_eq!(at.x + at.width, 1360.0);
    }

    #[test]
    fn a_step_too_small_to_measure_teaches_nothing_and_shares_stay_between_0_and_1() {
        let mut l = Layout::default();
        l.learn(pane_in(1200.0, 780.0));
        // One pixel of window with the pane jumping 40 px (the divider moved too): no share comes from it.
        let mut odd = pane_in(1201.0, 780.0);
        odd.x += 40.0;
        l.learn(odd);
        assert_eq!(l.share, [1.0, 1.0, 0.0, 1.0]);
        // A real step with the divider moving at the same time would give a share past 1: it is held to 1.
        let mut jump = pane_in(1221.0, 780.0);
        jump.x += 200.0;
        l.learn(jump);
        assert_eq!(l.share[0], 1.0);
    }

    fn piece(from: f64, x: f64, x_per_px: f64, width: f64, width_per_px: f64) -> Piece {
        Piece {
            from,
            x,
            x_per_px,
            width,
            width_per_px,
        }
    }

    /// The live check's layout on 2026-11-09: a 251 px sidebar that folds to the 52 px rail below 960 px, a half-and-half
    /// split with a 1 px hairline, and the page taking the whole main area when that is under 720 px. Under its 101.5 px
    /// bars, flush with the bottom.
    fn live_rule() -> Rule {
        Rule {
            pieces: vec![
                piece(0.0, 52.0, 0.0, -52.0, 1.0),
                piece(772.0, 412.5, 0.5, 359.5, 0.5),
                piece(960.0, 251.0, 0.0, 709.0, 1.0),
                piece(971.0, 611.5, 0.5, 359.5, 0.5),
            ],
            top: 101.5,
            bottom: 0.0,
        }
    }

    #[test]
    fn a_report_is_for_the_window_now_only_within_half_a_pixel_both_ways() {
        let p = pane_in(1200.0, 780.0);
        assert!(same_size(1200.0, 780.0, p));
        assert!(same_size(1200.4, 779.6, p));
        assert!(!same_size(1201.0, 780.0, p));
        assert!(!same_size(1200.0, 781.0, p));
    }

    #[test]
    fn without_pieces_the_rule_places_nothing() {
        assert_eq!(Rule::default().place(1200.0, 780.0), None);
    }

    #[test]
    fn the_rule_lands_on_what_the_front_measured_on_each_side_of_every_jump() {
        let r = live_rule();
        // (window width, measured x, measured width) from the hand drag's reports.
        for (w, x, width) in [
            (704.0, 52.0, 652.0),
            (770.0, 52.0, 718.0),
            (772.0, 412.5, 359.5),
            (849.0, 451.0, 398.0),
            (959.0, 506.0, 453.0),
            (960.0, 251.0, 709.0),
            (970.0, 251.0, 719.0),
            (971.0, 611.5, 359.5),
            (1091.0, 671.5, 419.5),
        ] {
            assert_eq!(
                r.place(w, 780.0),
                Some(LogicalRect {
                    x,
                    y: 101.5,
                    width,
                    height: 678.5
                }),
                "{w}"
            );
        }
    }

    #[test]
    fn a_width_between_whole_pixels_stays_on_its_side_of_a_jump() {
        let r = live_rule();
        assert_eq!(r.place(959.5, 780.0).unwrap().x, 506.25);
        assert_eq!(r.place(960.0, 780.0).unwrap().x, 251.0);
    }

    #[test]
    fn pieces_sent_out_of_order_are_put_in_order() {
        let mut shuffled = live_rule();
        shuffled.pieces.reverse();
        let r = shuffled.sorted();
        assert_eq!(r, live_rule());
        assert_eq!(r.place(1091.0, 780.0).unwrap().x, 671.5);
    }

    #[test]
    fn the_height_follows_the_window_and_nothing_goes_negative() {
        let r = live_rule();
        assert_eq!(r.place(1091.0, 600.0).unwrap().height, 498.5);
        let tiny = r.place(10.0, 50.0).unwrap();
        assert_eq!((tiny.x, tiny.width, tiny.height), (52.0, 0.0, 0.0));
    }
}
