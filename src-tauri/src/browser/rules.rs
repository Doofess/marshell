//! The rules a browser page follows, as pure functions: where it may go, whose key it is, where it sits, and which
//! pages exist. Nothing here touches a webview, so all of it is tested (slice 6q-3a).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tauri::Url;

/// An origin a page may never open: the dev server's, the local API's. A host and a port.
pub type Own = (String, u16);

/// Where a page may go: `http` and `https` pages and `about:blank`. Every way in passes through here: typed text, a
/// link, a redirect, a popup. Refused: an address with a user name or password; a host ending `.localhost` on the
/// scheme's default port, which is where Tauri serves the app and its IPC (`tauri.localhost`, `ipc.localhost`); and
/// every origin in `own`, where every name for this computer counts as one host.
pub fn allowed_navigation(url: &Url, own: &[Own]) -> bool {
    match url.scheme() {
        "http" | "https" => {
            if !url.username().is_empty() || url.password().is_some() {
                return false;
            }
            let Some(host) = url.host_str().map(HostName::parse) else {
                return false;
            };
            if host.ends_in_localhost && url.port().is_none() {
                return false;
            }
            let port = url.port_or_known_default();
            !own.iter().any(|(own_host, own_port)| {
                Some(*own_port) == port && {
                    let own_host = HostName::parse(own_host);
                    (host.this_computer && own_host.this_computer) || host.name == own_host.name
                }
            })
        }
        "about" => url.as_str() == "about:blank",
        _ => false,
    }
}

/// A host as the allow-list compares it: lower case, without a trailing dot.
struct HostName {
    name: String,
    /// `x.localhost`, though not `localhost` itself: the shape of Tauri's own hosts.
    ends_in_localhost: bool,
    /// Any name for this computer: `localhost`, a name under it (WebView2 resolves those to loopback too), a
    /// loopback address or the unspecified one.
    this_computer: bool,
}

impl HostName {
    /// From a URL's host or one of the app's own: a name, an IPv4 address, or an IPv6 address in brackets. A URL
    /// has already brought every spelling of an IPv4 address (`2130706433`, `0x7f.1`) to the dotted one.
    fn parse(host: &str) -> Self {
        use std::net::{Ipv4Addr, Ipv6Addr};
        let name = host.trim_end_matches('.').to_ascii_lowercase();
        let v6 = name.strip_prefix('[').and_then(|h| h.strip_suffix(']'));
        let (ends_in_localhost, this_computer) = if let Ok(ip) = name.parse::<Ipv4Addr>() {
            (false, ip.is_loopback() || ip.is_unspecified())
        } else if let Some(Ok(ip)) = v6.map(str::parse::<Ipv6Addr>) {
            let mapped = ip
                .to_ipv4_mapped()
                .is_some_and(|v4| v4.is_loopback() || v4.is_unspecified());
            (false, ip.is_loopback() || ip.is_unspecified() || mapped)
        } else {
            let under = name.ends_with(".localhost");
            (under, under || name == "localhost")
        };
        Self {
            name,
            ends_in_localhost,
            this_computer,
        }
    }
}

/// A key with its modifiers, as the app's keymap names it (`KeyboardEvent.code`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct Chord {
    pub code: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

// Windows virtual-key codes the rules name.
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_L: u32 = 0x4C;
pub const VK_F12: u32 = 0x7B;

/// Who a key pressed in a page belongs to (6q-0 decision 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRoute {
    /// Marshell's: Esc, Ctrl+L (the address box) and every chord the app's keymap claims.
    App,
    /// F12 opens the page's own DevTools.
    DevTools,
    /// Everything else, the browser's own keys included (F5, Ctrl+R, Ctrl+F, Ctrl+P).
    Page,
}

/// `claimed` is the app's own list, so a page and a terminal give up the same keys. AltGr arrives as Ctrl+Alt and
/// stays in the page unless the app claims that very chord.
pub fn route_key(vk: u32, ctrl: bool, shift: bool, alt: bool, claimed: &HashSet<Chord>) -> KeyRoute {
    match (vk, ctrl, shift, alt) {
        (VK_F12, false, false, false) => return KeyRoute::DevTools,
        (VK_ESCAPE, false, false, false) | (VK_L, true, false, false) => return KeyRoute::App,
        _ => {}
    }
    let Some(code) = vk_to_code(vk) else {
        return KeyRoute::Page;
    };
    let chord = Chord {
        code: code.to_string(),
        ctrl,
        shift,
        alt,
    };
    if claimed.contains(&chord) {
        KeyRoute::App
    } else {
        KeyRoute::Page
    }
}

/// The DOM `KeyboardEvent.code` for a virtual key. `codes.json` lists every answer, and the app builds its list of
/// claimed chords from that file, so a key the app can claim is a key this function can name.
pub fn vk_to_code(vk: u32) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL", "KeyM", "KeyN",
        "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV", "KeyW", "KeyX", "KeyY", "KeyZ",
    ];
    const DIGITS: [&str; 10] = [
        "Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9",
    ];
    const FKEYS: [&str; 24] = [
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15", "F16", "F17",
        "F18", "F19", "F20", "F21", "F22", "F23", "F24",
    ];
    Some(match vk {
        0x41..=0x5A => LETTERS[(vk - 0x41) as usize],
        0x30..=0x39 => DIGITS[(vk - 0x30) as usize],
        0x70..=0x87 => FKEYS[(vk - 0x70) as usize],
        0x09 => "Tab",
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
        0x2D => "Insert",
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
/// (0 programmatic, 1 next, 2 previous). A programmatic request is not the person leaving the page.
pub fn focus_out_direction(reason: i32) -> Option<&'static str> {
    match reason {
        1 => Some("next"),
        2 => Some("previous"),
        _ => None,
    }
}

/// A key the page gave up, as the app's webview hears it: the fields of a DOM `KeyboardEvent` the keymap reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct KeyEvent {
    pub code: &'static str,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub repeat: bool,
}

/// The event for a key pressed in a page; `was_down` is WebView2's "the key was already down", a held key. None for
/// a key with no code: the app's keymap couldn't name it either.
pub fn key_event(vk: u32, ctrl: bool, shift: bool, alt: bool, was_down: bool) -> Option<KeyEvent> {
    Some(KeyEvent {
        code: vk_to_code(vk)?,
        ctrl,
        shift,
        alt,
        repeat: was_down,
    })
}

/// The largest site icon passed to the app's webview.
pub const MAX_ICON_BYTES: usize = 64 * 1024;

/// A site's icon (PNG bytes) as a `data:` URL for the tab strip; None when there is none or it is over 64 KB.
pub fn icon_src(png: &[u8]) -> Option<String> {
    if png.is_empty() || png.len() > MAX_ICON_BYTES {
        return None;
    }
    Some(format!("data:image/png;base64,{}", base64(png)))
}

/// A box in CSS (logical) pixels, in the window's coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A size in CSS (logical) pixels.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// One straight stretch of the app's layout: from window width `from` until the next piece starts, the page's left
/// edge is `x` and its width `width`, each changing by its `_per_px` for every pixel the window is wider than `from`.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Piece {
    pub from: f64,
    pub x: f64,
    pub x_per_px: f64,
    pub width: f64,
    pub width_per_px: f64,
}

/// The app's own layout as straight pieces over the window's width (S12 finding 21), so the page lands where the app
/// will lay it out at every width, the jumps included. The page's top stays put and its bottom keeps its distance
/// from the window's bottom.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Rule {
    pub pieces: Vec<Piece>,
    pub top: f64,
    pub bottom: f64,
}

impl Rule {
    /// Which piece a window `width` wide is on; None for a rule with no pieces. A width under the first piece's start
    /// takes the first piece. The pieces must be in order of `from`.
    pub fn piece_at(&self, width: f64) -> Option<usize> {
        if self.pieces.is_empty() {
            return None;
        }
        Some(self.pieces.iter().rposition(|p| p.from <= width).unwrap_or(0))
    }

    /// Where the page is in a window of size `win`; None for a rule with no pieces.
    pub fn place(&self, win: Size) -> Option<Rect> {
        let p = self.pieces[self.piece_at(win.width)?];
        let d = win.width - p.from;
        Some(whole(Rect {
            x: p.x + p.x_per_px * d,
            y: self.top,
            width: p.width + p.width_per_px * d,
            height: win.height - self.top - self.bottom,
        }))
    }
}

/// A box a webview can take: no negative or missing number.
fn whole(r: Rect) -> Rect {
    let at_least_0 = |n: f64| if n.is_finite() { n.max(0.0) } else { 0.0 };
    Rect {
        x: at_least_0(r.x),
        y: at_least_0(r.y),
        width: at_least_0(r.width),
        height: at_least_0(r.height),
    }
}

/// Whether two layout sizes are one window size: the app's are CSS pixels, rounded, so within one physical pixel.
fn same_size(a: Size, b: Size, scale: f64) -> bool {
    let near = |a: f64, b: f64| ((a * scale).round() - (b * scale).round()).abs() <= 1.0;
    near(a.width, b.width) && near(a.height, b.height)
}

/// What the app reports: the page's slot as measured, the size of the layout it was measured in, the rule, and
/// whether the app's own layout motion is running.
#[derive(Clone, Debug, Deserialize)]
pub struct Report {
    pub rect: Rect,
    pub win: Size,
    pub rule: Option<Rule>,
    pub moving: bool,
}

/// How long the placer leaves the page to the app's reports after a motion's last sign.
pub const HOLD_MS: u64 = 250;

/// Decides the page's box. Inside a window resize the rule answers at once, so the page moves in the same instant as
/// the window. While the app's own motion runs, the rule would jump to the motion's end, so the placer holds and the
/// app's frame-by-frame reports place the page instead.
#[derive(Default)]
pub struct Placer {
    rule: Option<Rule>,
    last: Option<Rect>,
    piece: Option<usize>,
    hold_until: Option<u64>,
}

impl Placer {
    /// The window resized to `win`. None while a motion is held, or when the width crossed into another piece of the
    /// rule: that starts a motion in the app, and a hold here. None without a rule too. Otherwise the rule's box.
    pub fn on_resize(&mut self, win: Size, now_ms: u64) -> Option<Rect> {
        if self.hold_until.is_some_and(|until| now_ms < until) {
            return None;
        }
        self.hold_until = None;
        let rule = self.rule.as_ref()?;
        let piece = rule.piece_at(win.width);
        if self.piece.is_some() && piece != self.piece {
            self.piece = piece;
            self.hold_until = Some(now_ms + HOLD_MS);
            return None;
        }
        self.piece = piece;
        self.last = rule.place(win);
        self.last
    }

    /// A report from the app, with the window's size and scale now. A `moving` report holds for `HOLD_MS` more and is
    /// taken as measured. A still one ends the hold; it is taken as measured when its layout was the window's size
    /// now (within one physical pixel), else the rule answers for the size now, else the report is all there is.
    pub fn on_report(&mut self, report: Report, win_now: Size, scale: f64, now_ms: u64) -> Rect {
        if let Some(mut rule) = report.rule {
            rule.pieces.sort_by(|a, b| a.from.total_cmp(&b.from));
            self.rule = Some(rule);
        }
        let by_rule = || self.rule.as_ref().and_then(|rule| rule.place(win_now));
        let rect = if report.moving {
            self.hold_until = Some(now_ms + HOLD_MS);
            whole(report.rect)
        } else {
            self.hold_until = None;
            if same_size(report.win, win_now, scale) {
                whole(report.rect)
            } else {
                by_rule().unwrap_or_else(|| whole(report.rect))
            }
        };
        // Under the rule as it is now: a piece number of an older rule says nothing about this one.
        self.piece = self.rule.as_ref().and_then(|rule| rule.piece_at(win_now.width));
        self.last = Some(rect);
        rect
    }

    /// The box last decided.
    pub fn last(&self) -> Option<Rect> {
        self.last
    }
}

/// Which pages exist, by the app's key for each: the label of each page's webview, and which page is shown.
#[derive(Default)]
pub struct Registry {
    pages: HashMap<String, String>,
    next: u32,
    shown: Option<String>,
}

impl Registry {
    /// The label for a new page (`page-1`, `page-2`, …), or None when the key already has one.
    pub fn open(&mut self, key: &str) -> Option<String> {
        if self.pages.contains_key(key) {
            return None;
        }
        // Never a label again: a webview that is still closing keeps its label for a moment.
        self.next += 1;
        let label = format!("page-{}", self.next);
        self.pages.insert(key.to_string(), label.clone());
        Some(label)
    }

    pub fn label(&self, key: &str) -> Option<&str> {
        self.pages.get(key).map(String::as_str)
    }

    /// Forgets the key; its label, if it had one.
    pub fn close(&mut self, key: &str) -> Option<String> {
        if self.shown.as_deref() == Some(key) {
            self.shown = None;
        }
        self.pages.remove(key)
    }

    /// Forgets every page; their labels, in the order the pages were opened. The label counter goes on.
    pub fn clear(&mut self) -> Vec<String> {
        let labels = self.labels();
        self.pages.clear();
        self.shown = None;
        labels
    }

    /// Sets the shown page: the labels to hide, and the one to show. A key with no page shows nothing.
    pub fn show(&mut self, key: Option<&str>) -> (Vec<String>, Option<String>) {
        self.shown = key.filter(|k| self.pages.contains_key(*k)).map(str::to_string);
        let show = self.shown.as_deref().and_then(|k| self.pages.get(k)).cloned();
        let hide = self
            .labels()
            .into_iter()
            .filter(|label| Some(label) != show.as_ref())
            .collect();
        (hide, show)
    }

    pub fn shown(&self) -> Option<&str> {
        self.shown.as_deref()
    }

    /// Every page's label, in the order the pages were opened.
    pub fn labels(&self) -> Vec<String> {
        let number = |label: &String| label.trim_start_matches("page-").parse::<u32>().unwrap_or(0);
        let mut labels: Vec<String> = self.pages.values().cloned().collect();
        labels.sort_by_key(number);
        labels
    }
}

/// Standard base64 with padding, for a `data:` URL. No crate is added for it.
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

    fn own() -> Vec<Own> {
        vec![("localhost".into(), 1420), ("127.0.0.1".into(), 49152)]
    }

    #[test]
    fn web_pages_and_a_blank_page_are_allowed() {
        for u in [
            "https://example.com/",
            "http://localhost:3000/",
            "http://app.localhost:3000/",
            "http://127.0.0.1:8080/",
            "http://[::1]:5173/",
            "about:blank",
        ] {
            assert!(allowed_navigation(&Url::parse(u).unwrap(), &own()), "{u}");
        }
    }

    #[test]
    fn files_other_schemes_and_the_apps_own_origins_are_refused() {
        for u in [
            "file:///C:/Windows/win.ini",
            "file://server/share/a.pdf",
            "tauri://localhost/",
            "http://tauri.localhost/",
            "https://tauri.localhost/",
            "http://ipc.localhost/",
            "http://asset.localhost/x",
            "http://TAURI.LOCALHOST/",
            "http://tauri.localhost./",
            "http://sub.tauri.localhost/",
            "http://tauri.localhost:80/",
            "http://localhost:1420/",
            "http://127.0.0.1:1420/x",
            "http://[::1]:1420/",
            "http://127.0.0.2:1420/",
            "http://0.0.0.0:1420/",
            "http://2130706433:1420/",
            "http://[::ffff:127.0.0.1]:1420/",
            "http://anything.localhost:1420/",
            "http://localhost.:1420/",
            "http://localhost:49152/v1/health",
            "http://user:pw@example.com/",
            "http://user@example.com/",
            "about:config",
            "about:srcdoc",
            "javascript:alert(1)",
            "mailto:a@b.c",
            "vscode://x",
            "data:text/html,x",
            "blob:https://a/1",
            "ws://a/",
        ] {
            assert!(!allowed_navigation(&Url::parse(u).unwrap(), &own()), "{u}");
        }
    }

    #[test]
    fn an_own_origin_on_another_computer_is_only_that_host() {
        let own = vec![("devbox.lan".to_string(), 1420)];
        assert!(!allowed_navigation(
            &Url::parse("http://devbox.lan:1420/").unwrap(),
            &own
        ));
        assert!(allowed_navigation(&Url::parse("http://localhost:1420/").unwrap(), &own));
        assert!(allowed_navigation(
            &Url::parse("http://devbox.lan:3000/").unwrap(),
            &own
        ));
    }

    const VK_F: u32 = 0x46;
    const VK_K: u32 = 0x4B;
    const VK_P: u32 = 0x50;
    const VK_Q: u32 = 0x51;
    const VK_R: u32 = 0x52;
    const VK_T: u32 = 0x54;
    const VK_V: u32 = 0x56;
    const VK_Z: u32 = 0x5A;
    const VK_TAB: u32 = 0x09;
    const VK_F5: u32 = 0x74;

    fn chord(code: &str, ctrl: bool, shift: bool, alt: bool) -> Chord {
        Chord {
            code: code.into(),
            ctrl,
            shift,
            alt,
        }
    }

    fn claimed() -> HashSet<Chord> {
        HashSet::from([
            chord("KeyK", true, true, false),
            chord("Tab", true, false, false),
            chord("KeyT", true, true, true),
        ])
    }

    #[test]
    fn a_chord_the_app_claims_goes_to_the_app() {
        let c = claimed();
        assert_eq!(route_key(VK_K, true, true, false, &c), KeyRoute::App);
        assert_eq!(route_key(VK_TAB, true, false, false, &c), KeyRoute::App);
        assert_eq!(route_key(VK_T, true, true, true, &c), KeyRoute::App);
    }

    #[test]
    fn esc_and_ctrl_l_go_to_the_app_whatever_it_claims() {
        let none = HashSet::new();
        assert_eq!(route_key(VK_ESCAPE, false, false, false, &none), KeyRoute::App);
        assert_eq!(route_key(VK_L, true, false, false, &none), KeyRoute::App);
        assert_eq!(route_key(VK_ESCAPE, false, true, false, &none), KeyRoute::Page);
        assert_eq!(route_key(VK_L, true, true, false, &none), KeyRoute::Page);
    }

    #[test]
    fn f12_alone_opens_the_pages_devtools() {
        let c = claimed();
        assert_eq!(route_key(VK_F12, false, false, false, &c), KeyRoute::DevTools);
        assert_eq!(route_key(VK_F12, true, false, false, &c), KeyRoute::Page);
    }

    #[test]
    fn the_page_keeps_every_other_key() {
        let c = claimed();
        assert_eq!(route_key(VK_F5, false, false, false, &c), KeyRoute::Page);
        for vk in [VK_R, VK_F, VK_P] {
            assert_eq!(route_key(vk, true, false, false, &c), KeyRoute::Page);
        }
        // Paste and redo, which the app's keymap leaves alone.
        assert_eq!(route_key(VK_V, true, true, false, &c), KeyRoute::Page);
        assert_eq!(route_key(VK_Z, true, true, false, &c), KeyRoute::Page);
        // AltGr arrives as Ctrl+Alt: AltGr+Q is @ on a German layout.
        assert_eq!(route_key(VK_Q, true, false, true, &c), KeyRoute::Page);
        // A key with no code can't be in the app's list.
        assert_eq!(route_key(0x07, true, true, false, &c), KeyRoute::Page);
    }

    #[test]
    fn virtual_keys_map_to_dom_codes() {
        assert_eq!(vk_to_code(0x41), Some("KeyA"));
        assert_eq!(vk_to_code(0x30), Some("Digit0"));
        assert_eq!(vk_to_code(VK_F5), Some("F5"));
        assert_eq!(vk_to_code(0x87), Some("F24"));
        assert_eq!(vk_to_code(VK_TAB), Some("Tab"));
        assert_eq!(vk_to_code(0x25), Some("ArrowLeft"));
        assert_eq!(vk_to_code(0x2D), Some("Insert"));
        assert_eq!(vk_to_code(0xBF), Some("Slash"));
        assert_eq!(vk_to_code(0x07), None);
    }

    #[test]
    fn the_codes_file_lists_every_code() {
        let listed: Vec<String> = serde_json::from_str(include_str!("codes.json")).unwrap();
        let mut mapped: Vec<String> = (0..=0xFF).filter_map(vk_to_code).map(str::to_string).collect();
        mapped.sort();
        assert_eq!(listed, mapped);
    }

    #[test]
    fn tabbing_off_either_end_of_the_page_names_the_direction() {
        assert_eq!(focus_out_direction(1), Some("next"));
        assert_eq!(focus_out_direction(2), Some("previous"));
        assert_eq!(focus_out_direction(0), None);
    }

    /// Under 960 px a 48 px rail, from 960 px a 280 px sidebar; the main area split in two halves, the page in the
    /// second. Under 80 px of bars and above a 24 px status bar.
    fn two_pieces() -> Rule {
        Rule {
            pieces: vec![
                Piece {
                    from: 0.0,
                    x: 24.0,
                    x_per_px: 0.5,
                    width: -24.0,
                    width_per_px: 0.5,
                },
                Piece {
                    from: 960.0,
                    x: 620.0,
                    x_per_px: 0.5,
                    width: 340.0,
                    width_per_px: 0.5,
                },
            ],
            top: 80.0,
            bottom: 24.0,
        }
    }

    fn size(width: f64, height: f64) -> Size {
        Size { width, height }
    }

    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect { x, y, width, height }
    }

    fn report(rect: Rect, win: Size, rule: Option<Rule>, moving: bool) -> Report {
        Report {
            rect,
            win,
            rule,
            moving,
        }
    }

    /// A placer that has had one still report, measured in a window `width` wide and 700 high.
    fn placed_at(width: f64) -> Placer {
        let mut p = Placer::default();
        let win = size(width, 700.0);
        let measured = two_pieces().place(win).unwrap();
        p.on_report(report(measured, win, Some(two_pieces()), false), win, 1.0, 0);
        p
    }

    #[test]
    fn a_resize_inside_a_piece_follows_the_rule() {
        let mut p = placed_at(900.0);
        assert_eq!(
            p.on_resize(size(880.0, 600.0), 10),
            Some(rect(464.0, 80.0, 416.0, 496.0))
        );
        assert_eq!(p.last(), Some(rect(464.0, 80.0, 416.0, 496.0)));
    }

    #[test]
    fn a_resize_across_a_piece_holds() {
        let mut p = placed_at(960.0);
        let before = p.last();
        assert_eq!(p.on_resize(size(950.0, 700.0), 1000), None);
        assert_eq!(p.on_resize(size(940.0, 700.0), 1100), None);
        assert_eq!(p.last(), before);
    }

    #[test]
    fn the_hold_ends_after_250_ms() {
        let mut p = placed_at(960.0);
        assert_eq!(p.on_resize(size(950.0, 700.0), 1000), None);
        assert_eq!(p.on_resize(size(940.0, 700.0), 1249), None);
        assert_eq!(
            p.on_resize(size(940.0, 700.0), 1251),
            Some(rect(494.0, 80.0, 446.0, 596.0))
        );
    }

    #[test]
    fn a_moving_report_is_taken_as_measured_and_holds() {
        let mut p = placed_at(1200.0);
        let mid_motion = rect(700.0, 80.0, 500.0, 596.0);
        let win = size(1200.0, 700.0);
        assert_eq!(
            p.on_report(report(mid_motion, win, None, true), win, 1.0, 2000),
            mid_motion
        );
        assert_eq!(p.on_resize(size(1190.0, 700.0), 2100), None);
        // Each moving report holds again.
        p.on_report(report(mid_motion, win, None, true), win, 1.0, 2200);
        assert_eq!(p.on_resize(size(1190.0, 700.0), 2400), None);
        assert!(p.on_resize(size(1190.0, 700.0), 2451).is_some());
    }

    #[test]
    fn a_still_report_ends_the_hold() {
        let mut p = placed_at(1200.0);
        let win = size(1200.0, 700.0);
        let at_rest = two_pieces().place(win).unwrap();
        p.on_report(report(at_rest, win, None, true), win, 1.0, 2000);
        p.on_report(report(at_rest, win, None, false), win, 1.0, 2050);
        assert!(p.on_resize(size(1190.0, 700.0), 2060).is_some());
    }

    #[test]
    fn a_report_at_the_windows_size_is_taken_as_measured() {
        let mut p = placed_at(1200.0);
        // The divider moved: the slot is not where the old rule says, and the report carries no new rule.
        let measured = rect(800.0, 80.0, 400.0, 596.0);
        let win = size(1200.0, 700.0);
        assert_eq!(p.on_report(report(measured, win, None, false), win, 1.0, 0), measured);
        // 0.8 CSS px apart at 125% is one physical pixel: the same layout, rounded differently.
        let now = size(1200.8, 700.0);
        assert_eq!(p.on_report(report(measured, win, None, false), now, 1.25, 0), measured);
    }

    #[test]
    fn a_report_at_another_size_gets_the_rule() {
        let mut p = placed_at(1200.0);
        let stale = two_pieces().place(size(1200.0, 700.0)).unwrap();
        let now = size(1100.0, 640.0);
        assert_eq!(
            p.on_report(report(stale, size(1200.0, 700.0), None, false), now, 1.0, 0),
            two_pieces().place(now).unwrap()
        );
    }

    #[test]
    fn without_a_rule_the_report_is_followed() {
        let mut p = Placer::default();
        let measured = rect(600.0, 80.0, 400.0, 500.0);
        assert_eq!(
            p.on_report(
                report(measured, size(1000.0, 600.0), None, false),
                size(1100.0, 640.0),
                1.0,
                0
            ),
            measured
        );
        assert_eq!(p.on_resize(size(1200.0, 640.0), 10), None);
    }

    #[test]
    fn a_tiny_window_never_gives_a_negative_size() {
        let mut p = placed_at(900.0);
        let tiny = p.on_resize(size(20.0, 50.0), 10).unwrap();
        assert_eq!((tiny.width, tiny.height), (0.0, 0.0));
        assert!(tiny.x >= 0.0);
        let odd = rect(10.0, 10.0, -5.0, f64::NAN);
        let win = size(900.0, 700.0);
        let placed = p.on_report(report(odd, win, None, false), win, 1.0, 20);
        assert_eq!((placed.width, placed.height), (0.0, 0.0));
    }

    #[test]
    fn the_first_resize_after_a_new_rule_never_holds() {
        let mut p = placed_at(900.0);
        // The sidebar's width changed: a new rule whose second piece holds 900. Piece numbers of the old rule and
        // the new one say nothing about each other.
        let mut wider = two_pieces();
        wider.pieces.insert(
            1,
            Piece {
                from: 800.0,
                x: 500.0,
                x_per_px: 0.5,
                width: 300.0,
                width_per_px: 0.5,
            },
        );
        let win = size(900.0, 700.0);
        let measured = wider.place(win).unwrap();
        p.on_report(report(measured, win, Some(wider.clone()), false), win, 1.0, 0);
        assert_eq!(p.on_resize(size(890.0, 700.0), 10), wider.place(size(890.0, 700.0)));
    }

    #[test]
    fn pieces_sent_out_of_order_are_put_in_order() {
        let mut shuffled = two_pieces();
        shuffled.pieces.reverse();
        let mut p = Placer::default();
        let win = size(1200.0, 700.0);
        p.on_report(
            report(rect(0.0, 0.0, 0.0, 0.0), size(1.0, 1.0), Some(shuffled), false),
            win,
            1.0,
            0,
        );
        assert_eq!(p.last(), two_pieces().place(win));
    }

    #[test]
    fn registry_open_twice_makes_one_page() {
        let mut r = Registry::default();
        assert_eq!(r.open("s1/t1").as_deref(), Some("page-1"));
        assert_eq!(r.open("s1/t1"), None);
        assert_eq!(r.labels(), ["page-1"]);
        assert_eq!(r.label("s1/t1"), Some("page-1"));
    }

    #[test]
    fn registry_close_before_created_leaves_none() {
        let mut r = Registry::default();
        let label = r.open("s1/t1");
        assert_eq!(r.close("s1/t1"), label);
        assert!(r.labels().is_empty());
        assert_eq!(r.label("s1/t1"), None);
        assert_eq!(r.show(Some("s1/t1")), (vec![], None));
        assert_eq!(r.close("s1/t1"), None);
    }

    #[test]
    fn registry_show_hides_the_others() {
        let mut r = Registry::default();
        for key in ["a", "b", "c"] {
            r.open(key);
        }
        assert_eq!(
            r.show(Some("b")),
            (
                vec!["page-1".to_string(), "page-3".to_string()],
                Some("page-2".to_string())
            )
        );
        assert_eq!(r.shown(), Some("b"));
        let (hidden, shown) = r.show(None);
        assert_eq!((hidden.len(), shown), (3, None));
        assert_eq!(r.shown(), None);
    }

    #[test]
    fn registry_closing_the_shown_page_shows_nothing() {
        let mut r = Registry::default();
        r.open("a");
        r.show(Some("a"));
        r.close("a");
        assert_eq!(r.shown(), None);
    }

    #[test]
    fn registry_clear_forgets_every_page() {
        let mut r = Registry::default();
        r.open("a");
        r.open("b");
        r.show(Some("b"));
        assert_eq!(r.clear(), ["page-1", "page-2"]);
        assert!(r.labels().is_empty());
        assert_eq!(r.shown(), None);
        assert!(r.clear().is_empty());
        // Labels go on from where they were.
        assert_eq!(r.open("a").as_deref(), Some("page-3"));
    }

    #[test]
    fn registry_labels_are_never_reused() {
        let mut r = Registry::default();
        r.open("a");
        r.close("a");
        assert_eq!(r.open("a").as_deref(), Some("page-2"));
    }

    #[test]
    fn a_held_key_is_a_repeat() {
        let pressed = key_event(VK_L, true, false, false, false).unwrap();
        assert_eq!(
            pressed,
            KeyEvent {
                code: "KeyL",
                ctrl: true,
                shift: false,
                alt: false,
                repeat: false
            }
        );
        assert!(key_event(VK_L, true, false, false, true).unwrap().repeat);
        let with_shift_alt = key_event(0x09, false, true, true, false).unwrap();
        assert_eq!(
            (with_shift_alt.code, with_shift_alt.shift, with_shift_alt.alt),
            ("Tab", true, true)
        );
    }

    #[test]
    fn a_key_with_no_code_is_not_forwarded() {
        // VK_VOLUME_MUTE: no entry in codes.json, so the app's keymap can't name it.
        assert_eq!(key_event(0xAD, false, false, false, false), None);
    }

    #[test]
    fn a_key_event_is_camel_case_json() {
        let json = serde_json::to_value(key_event(VK_ESCAPE, false, false, false, false).unwrap()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "code": "Escape", "ctrl": false, "shift": false, "alt": false, "repeat": false })
        );
    }

    #[test]
    fn an_icon_over_64_kb_is_dropped() {
        assert_eq!(icon_src(&vec![0u8; 64 * 1024 + 1]), None);
        assert!(icon_src(&vec![0u8; 64 * 1024]).is_some());
    }

    #[test]
    fn an_empty_icon_is_none() {
        assert_eq!(icon_src(&[]), None);
        assert_eq!(icon_src(b"foo").as_deref(), Some("data:image/png;base64,Zm9v"));
    }

    #[test]
    fn base64_is_standard_with_padding() {
        // RFC 4648 §10's vectors: every padding case.
        for (raw, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(raw.as_bytes()), encoded, "{raw:?}");
        }
        assert_eq!(base64(&[0xfb, 0xff, 0xbf]), "+/+/");
    }
}
