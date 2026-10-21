//! The tray icon. The shell draws the icon and the menu and handles Open and Quit itself, so they work even when
//! the webview is stuck. Every other choice goes to the webview as a `tray-action` event.

use serde::Deserialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::image::Image;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

const TRAY_ID: &str = "main";
/// Windows' own tray size at 100% scaling; the system scales it from there.
const SIZE: u32 = 32;
/// The accent dot that says a session is waiting.
const DOT: [u8; 4] = [0xD9, 0x4F, 0x2B, 0xFF];
/// Longest menu line, in characters, ellipsis included.
const MAX_LABEL: usize = 64;

/// What the webview sends: the menu it built from the needs-you list, the tooltip and how many are waiting.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayModel {
    pub items: Vec<TrayItemDto>,
    pub tooltip: String,
    pub waiting: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayKind {
    Item,
    Session,
    Separator,
}

/// One menu entry, as the webview's `TrayItem` writes it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayItemDto {
    pub id: String,
    pub label: String,
    pub kind: TrayKind,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub number: Option<u32>,
    #[serde(default)]
    pub disabled: bool,
}

/// What the icon on screen shows, so a menu update only sets the icon when it would look different.
#[derive(Default)]
pub struct TrayState {
    ready: AtomicBool,
    /// (a session is waiting, the taskbar is light) of the icon last set.
    shown: Mutex<Option<(bool, bool)>>,
}

/// True once `build` made the icon. Without it the close button must quit: nothing could bring the window back.
pub fn tray_ready(app: &tauri::AppHandle) -> bool {
    app.try_state::<TrayState>()
        .is_some_and(|s| s.ready.load(Ordering::SeqCst))
}

/// One plain line for a menu: the session number in front, `&` doubled so it is no mnemonic, control characters
/// as spaces, cut to 64 characters, and the shortcut after a tab (the Windows menu convention, display only).
pub fn menu_label(item: &TrayItemDto) -> String {
    let text = match (item.kind, item.number) {
        (TrayKind::Session, Some(n)) => format!("{n} · {}", item.label),
        _ => item.label.clone(),
    };
    let mut line = one_line(&text);
    if line.chars().count() > MAX_LABEL {
        line = line.chars().take(MAX_LABEL - 1).collect();
        line.push('…');
    }
    // The text is shown as written: the menu reads `&` as a mnemonic marker unless it is doubled.
    let mut label = line.replace('&', "&&");
    if let Some(shortcut) = &item.shortcut {
        label.push('\t');
        label.push_str(&one_line(shortcut));
    }
    label
}

/// `text` with every control character, and the Unicode line and paragraph separators, as a space.
fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// The two wands as strokes on the 1024-unit grid of `assets/icon/tray-mono.svg`, and the dot.
const WANDS: [((f32, f32), (f32, f32)); 2] = [((400.0, 884.0), (168.0, 150.0)), ((624.0, 884.0), (856.0, 150.0))];
const STROKE: f32 = 170.0;
const DOT_AT: (f32, f32) = (830.0, 190.0);
const DOT_RADIUS: f32 = 170.0;

/// The tray icon as RGBA: the two wands, white on a dark taskbar and black on a light one, and the accent dot
/// on top while something is waiting. Drawn like the badge, so the icon needs no asset or image decoder.
pub fn tray_rgba(waiting: bool, light_taskbar: bool) -> (Vec<u8>, u32, u32) {
    let scale = SIZE as f32 / 1024.0;
    let ink = if light_taskbar { [0, 0, 0] } else { [0xFF, 0xFF, 0xFF] };
    let mut px = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let at = (x as f32 + 0.5, y as f32 + 0.5);
            let mut covered = 0.0f32;
            for (from, to) in WANDS {
                let d = distance_to_segment(at, scaled(from, scale), scaled(to, scale));
                covered = covered.max(coverage(STROKE / 2.0 * scale - d));
            }
            let mut rgba = [ink[0], ink[1], ink[2], (covered * 255.0).round() as u8];
            if waiting {
                let d = distance(at, scaled(DOT_AT, scale));
                rgba = over(DOT, coverage(DOT_RADIUS * scale - d), rgba);
            }
            let i = ((y * SIZE + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&rgba);
        }
    }
    (px, SIZE, SIZE)
}

fn scaled((x, y): (f32, f32), scale: f32) -> (f32, f32) {
    (x * scale, y * scale)
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// How far `p` is from the nearest point of the segment `a`-`b`; a round-capped stroke is every point within half its width.
fn distance_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    distance(p, (a.0 + t * dx, a.1 + t * dy))
}

/// How much of a pixel lies inside a shape whose edge is `inside` pixels away (positive within it): a one-pixel ramp.
fn coverage(inside: f32) -> f32 {
    (inside + 0.5).clamp(0.0, 1.0)
}

/// `top` at `amount` of its alpha, drawn over `under`.
fn over(top: [u8; 4], amount: f32, under: [u8; 4]) -> [u8; 4] {
    let (a_top, a_under) = (top[3] as f32 / 255.0 * amount, under[3] as f32 / 255.0);
    let a = a_top + a_under * (1.0 - a_top);
    if a <= 0.0 {
        return [0; 4];
    }
    let mix = |t: u8, u: u8| ((t as f32 * a_top + u as f32 * a_under * (1.0 - a_top)) / a).round() as u8;
    [
        mix(top[0], under[0]),
        mix(top[1], under[1]),
        mix(top[2], under[2]),
        (a * 255.0).round() as u8,
    ]
}

/// Whether the Windows taskbar is light, so the icon can be black on it. A missing value means dark.
#[cfg(windows)]
fn taskbar_is_light() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide("SystemUsesLightTheme");
    let (mut data, mut len) = (0u32, std::mem::size_of::<u32>() as u32);
    // SAFETY: both names are NUL-terminated, and `data` is a writable u32 whose size `len` says.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut len,
        )
    };
    status == 0 && data != 0
}

/// The other systems draw the icon in their own colours (macOS as a template), so it is the dark-taskbar one.
#[cfg(not(windows))]
fn taskbar_is_light() -> bool {
    false
}

fn icon(waiting: bool, light_taskbar: bool) -> Image<'static> {
    let (rgba, w, h) = tray_rgba(waiting, light_taskbar);
    Image::new_owned(rgba, w, h)
}

/// Brings the window back: out of the minimised state and the tray, in front, and in view for the core's timers.
pub fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    if let Some(core) = app.try_state::<crate::CoreHandle>() {
        core.0.set_window_hidden(false);
    }
}

/// Creates the tray icon with Open and Quit only; the webview fills in the rest through `set_tray`.
pub fn build(app: &tauri::AppHandle) -> tauri::Result<()> {
    app.manage(TrayState::default());
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", "Open Marshell", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit Marshell", true, None::<&str>)?,
        ],
    )?;
    let light = taskbar_is_light();
    let builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Marshell")
        .icon(icon(false, light))
        .menu(&menu)
        // Left click shows the window; the menu is the right click's.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu_choice(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    #[cfg(target_os = "macos")]
    let builder = builder.icon_as_template(true);
    builder.build(app)?;
    if let Some(state) = app.try_state::<TrayState>() {
        *state.shown.lock().unwrap_or_else(|e| e.into_inner()) = Some((false, light));
        state.ready.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// Open and Quit are the shell's own. Pause changes nothing on screen, so it leaves the window be; every other
/// choice shows the window first, and the webview does the rest.
fn on_menu_choice(app: &tauri::AppHandle, id: &str) {
    match id {
        "open" => show_main(app),
        "quit" => app.exit(0),
        other => {
            if other != "pause" {
                show_main(app);
            }
            let sent = app.emit("tray-action", std::collections::HashMap::from([("id", other)]));
            if let Err(e) = sent {
                eprintln!("Marshell: could not tell the window about the tray choice: {e}");
            }
        }
    }
}

fn menu_of(app: &tauri::AppHandle, items: &[TrayItemDto]) -> tauri::Result<Menu<tauri::Wry>> {
    let mut built: Vec<Box<dyn IsMenuItem<tauri::Wry>>> = Vec::with_capacity(items.len());
    for item in items {
        built.push(match item.kind {
            TrayKind::Separator => Box::new(PredefinedMenuItem::separator(app)?),
            _ => Box::new(MenuItem::with_id(
                app,
                item.id.as_str(),
                menu_label(item),
                !item.disabled,
                None::<&str>,
            )?),
        });
    }
    let refs: Vec<&dyn IsMenuItem<tauri::Wry>> = built.iter().map(|b| b.as_ref()).collect();
    Menu::with_items(app, &refs)
}

/// Sets the icon when what it shows changed: a session started or stopped waiting, or the taskbar changed colour.
fn show_icon(app: &tauri::AppHandle, waiting: bool, light: bool) -> Result<(), String> {
    let (Some(tray), Some(state)) = (app.tray_by_id(TRAY_ID), app.try_state::<TrayState>()) else {
        return Err("the tray is not running".into());
    };
    let mut shown = state.shown.lock().unwrap_or_else(|e| e.into_inner());
    if *shown != Some((waiting, light)) {
        tray.set_icon(Some(icon(waiting, light))).map_err(|e| e.to_string())?;
        *shown = Some((waiting, light));
    }
    Ok(())
}

/// The system changed its theme: the taskbar may have turned light or dark, so the wands may need the other colour.
pub fn theme_changed(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    let shown = *state.shown.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((waiting, _)) = shown
        && let Err(e) = show_icon(app, waiting, taskbar_is_light())
    {
        eprintln!("Marshell: the tray icon keeps its colour: {e}");
    }
}

/// Rebuilds the menu, tooltip and icon from what the webview says needs the person.
#[tauri::command]
pub fn set_tray(app: tauri::AppHandle, model: TrayModel) -> Result<(), String> {
    let tray = app.tray_by_id(TRAY_ID).ok_or("the tray is not running")?;
    let menu = menu_of(&app, &model.items).map_err(|e| e.to_string())?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    tray.set_tooltip(Some(model.tooltip.as_str()))
        .map_err(|e| e.to_string())?;
    show_icon(&app, model.waiting > 0, taskbar_is_light())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str) -> TrayItemDto {
        TrayItemDto {
            id: "x".into(),
            label: label.into(),
            kind: TrayKind::Item,
            shortcut: None,
            number: None,
            disabled: false,
        }
    }

    #[test]
    fn a_session_item_reads_number_name_and_phrase() {
        let session = TrayItemDto {
            kind: TrayKind::Session,
            number: Some(3),
            ..item("api-server: Wants to run `npm test`")
        };
        assert_eq!(menu_label(&session), "3 · api-server: Wants to run `npm test`");
    }

    #[test]
    fn an_ampersand_is_doubled_so_it_is_not_a_mnemonic() {
        assert_eq!(menu_label(&item("R&D: Save & quit")), "R&&D: Save && quit");
    }

    #[test]
    fn control_characters_become_spaces() {
        assert_eq!(menu_label(&item("a\tb\nc\r\nd\u{7}e")), "a b c  d e");
        assert_eq!(menu_label(&item("a\u{2028}b")), "a b");
    }

    #[test]
    fn a_long_label_is_cut_to_64_characters_with_an_ellipsis() {
        let cut = menu_label(&item(&"x".repeat(300)));
        assert_eq!(cut.chars().count(), 64);
        assert!(cut.ends_with('…'));
        // Multi-byte text is cut on a character, never inside one.
        let cut = menu_label(&item(&"é".repeat(100)));
        assert_eq!(cut.chars().count(), 64);
        assert!(cut.ends_with("é…"));
        // Exactly 64 stays as it is.
        assert_eq!(menu_label(&item(&"y".repeat(64))), "y".repeat(64));
    }

    #[test]
    fn the_shortcut_follows_a_tab() {
        let next = TrayItemDto {
            shortcut: Some("Ctrl+Shift+N".into()),
            ..item("Go to the oldest waiting session")
        };
        assert_eq!(menu_label(&next), "Go to the oldest waiting session\tCtrl+Shift+N");
        // After the cut, so a long label never loses it.
        let long = TrayItemDto {
            shortcut: Some("Ctrl+Shift+N".into()),
            ..item(&"x".repeat(300))
        };
        assert!(menu_label(&long).ends_with("…\tCtrl+Shift+N"));
    }

    fn pixel(px: &[u8], x: u32, y: u32) -> [u8; 4] {
        let at = ((y * SIZE + x) * 4) as usize;
        px[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn tray_rgba_is_32_square_and_the_dot_shows_only_when_waiting() {
        let (idle, w, h) = tray_rgba(false, false);
        assert_eq!((w, h, idle.len()), (32, 32, 32 * 32 * 4));
        // Right of the right wand's tip, inside the dot.
        assert_eq!(pixel(&idle, 30, 5)[3], 0);
        let (waiting, ..) = tray_rgba(true, false);
        assert_eq!(pixel(&waiting, 30, 5), [0xD9, 0x4F, 0x2B, 0xFF]);
        // The dot sits over the wand's tip.
        assert_eq!(pixel(&waiting, 26, 6), [0xD9, 0x4F, 0x2B, 0xFF]);
    }

    #[test]
    fn the_wands_are_white_on_a_dark_taskbar_and_black_on_a_light_one() {
        // On the left wand, halfway up.
        let (dark, ..) = tray_rgba(false, false);
        assert_eq!(&pixel(&dark, 8, 16)[..3], [0xFF, 0xFF, 0xFF]);
        assert_eq!(pixel(&dark, 8, 16)[3], 0xFF);
        let (light, ..) = tray_rgba(false, true);
        assert_eq!(&pixel(&light, 8, 16)[..3], [0, 0, 0]);
        assert_eq!(pixel(&light, 8, 16)[3], 0xFF);
        // Between the wands at the same height is empty.
        assert_eq!(pixel(&dark, 16, 16)[3], 0);
    }

    #[test]
    fn a_model_reads_from_the_webviews_json() {
        let json = r#"{
            "items": [
                { "id": "open", "label": "Open Marshell", "kind": "item" },
                { "id": "sep-1", "label": "", "kind": "separator" },
                { "id": "session-3", "label": "api: Wants to run", "kind": "session", "number": 3 },
                { "id": "next-waiting", "label": "Go to the oldest", "kind": "item",
                  "shortcut": "Ctrl+Shift+N", "disabled": true }
            ],
            "tooltip": "Marshell · 1 needs you",
            "waiting": 1
        }"#;
        let model: TrayModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.waiting, 1);
        assert_eq!(model.tooltip, "Marshell · 1 needs you");
        assert_eq!(model.items.len(), 4);
        assert_eq!(model.items[1].kind, TrayKind::Separator);
        assert_eq!(model.items[2].number, Some(3));
        assert!(!model.items[0].disabled);
        assert!(model.items[3].disabled);
        assert_eq!(model.items[3].shortcut.as_deref(), Some("Ctrl+Shift+N"));
    }
}
