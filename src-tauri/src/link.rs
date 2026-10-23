//! The `marshell://` link. It carries no authority: it brings the window forward and selects a tab, and nothing
//! else (docs/adr/0001-threat-model.md, "The `marshell://` link"). Two shapes parse, everything else is refused.
//!
//! Both click paths read it through `parse`: an in-process toast activation, and a second launch that the
//! single-instance plugin forwards (only a lone `argv[1]` counts, see `from_argv`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{Emitter, Manager};

/// What a link may ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Show,
    Focus(String),
}

/// The longest argument looked at. The longest valid link is `marshell://focus/` + 64 + `/`, well under this.
const MAX_LINK_BYTES: usize = 128;
const MAX_TAB_ID: usize = 64;
/// A repeat of the same link inside this window is one click that Windows reported twice.
const REPEAT_MS: u64 = 2_000;

fn valid_tab_id(id: &str) -> bool {
    (1..=MAX_TAB_ID).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Reads `marshell://show` or `marshell://focus/<tab>`. The scheme and the word are read in any case, the tab id
/// exactly as written, and one trailing `/` is accepted. A query, a fragment, a path, a space or anything else is
/// refused rather than trimmed.
pub fn parse(s: &str) -> Option<Link> {
    if s.len() > MAX_LINK_BYTES || !s.is_ascii() {
        return None;
    }
    let (scheme, rest) = s.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("marshell") {
        return None;
    }
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.eq_ignore_ascii_case("show") {
        return Some(Link::Show);
    }
    let (word, tab) = rest.split_once('/')?;
    (word.eq_ignore_ascii_case("focus") && valid_tab_id(tab)).then(|| Link::Focus(tab.to_string()))
}

/// The link a toast carries: the tab's, or `marshell://show` when there is no tab or its id would not parse.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn link_for(tab_id: Option<&str>) -> String {
    match tab_id {
        Some(id) if valid_tab_id(id) => format!("marshell://focus/{id}"),
        _ => "marshell://show".to_string(),
    }
}

/// A link from the command line: only a lone `argv[1]`. Flags, extra arguments and a link in another position
/// are not links, so nothing else on the command line is ever read.
pub fn from_argv(argv: &[String]) -> Option<Link> {
    match argv {
        [_, one] => parse(one),
        _ => None,
    }
}

/// The last link opened and when, so one click that Windows reports twice opens once.
#[derive(Debug, Default)]
pub struct Recent {
    last: Option<(Link, u64)>,
}

impl Recent {
    /// False for the same link within 2 000 ms of the last admitted one.
    pub fn admit(&mut self, link: &Link, now_ms: u64) -> bool {
        if let Some((last, at)) = &self.last
            && last == link
            && now_ms.saturating_sub(*at) < REPEAT_MS
        {
            return false;
        }
        self.last = Some((link.clone(), now_ms));
        true
    }
}

fn now_ms() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Opens a link: the window comes forward whatever the link was (a second launch without one still shows it),
/// and a `Focus` that was not just opened selects its tab. The webview hears first, so it can move keyboard focus
/// out of the terminal before the window takes the keyboard: a link from a web page must not land the person's
/// typing in an agent's prompt.
pub fn open(app: &tauri::AppHandle, link: Option<Link>) {
    let admitted = match (&link, app.try_state::<Mutex<Recent>>()) {
        (Some(focus @ Link::Focus(_)), Some(recent)) => {
            recent.lock().unwrap_or_else(|e| e.into_inner()).admit(focus, now_ms())
        }
        _ => true,
    };
    if let Some(tab_id) = focus_event(link.as_ref(), admitted) {
        let _ = app.emit("focus-session", HashMap::from([("tab_id", tab_id)]));
    }
    crate::tray::show_main(app);
}

/// The `focus-session` payload a link sends: its tab for a `Focus`, none for anything else, and nothing at all for a
/// repeated `Focus` (the first one already moved the selection and the focus).
pub fn focus_event(link: Option<&Link>, admitted: bool) -> Option<Option<String>> {
    match link {
        Some(Link::Focus(id)) => admitted.then(|| Some(id.clone())),
        _ => Some(None),
    }
}

/// The exact command the registry holds for this exe: `"<exe>" "%1"`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn command_for(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

/// Whether a registry command is exactly this exe's (ASCII case-insensitive, the way Windows compares paths).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn points_at(command: &str, exe: &Path) -> bool {
    command.eq_ignore_ascii_case(&command_for(exe))
}

#[cfg(windows)]
mod registry {
    use super::{command_for, points_at};
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey,
        RegCreateKeyExW, RegDeleteTreeW, RegGetValueW, RegSetValueExW,
    };

    const KEY: &str = r"Software\Classes\marshell";
    const COMMAND_KEY: &str = r"Software\Classes\marshell\shell\open\command";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// A REG_SZ under HKCU. `name` None is the key's default value.
    fn read(key: &str, name: Option<&str>) -> Option<String> {
        let key = wide(key);
        let name = name.map(wide);
        let name_ptr = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
        let mut bytes = 0u32;
        // SAFETY: the names are NUL-terminated; a null data pointer asks only for the size in `bytes`.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name_ptr,
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut bytes,
            )
        };
        if status != 0 {
            return None;
        }
        let mut buf = vec![0u16; (bytes as usize).div_ceil(2) + 1];
        let mut len = (buf.len() * 2) as u32;
        // SAFETY: `buf` is writable for `len` bytes.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name_ptr,
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        if status != 0 {
            return None;
        }
        let units = (len as usize / 2).min(buf.len());
        let end = buf[..units].iter().position(|&c| c == 0).unwrap_or(units);
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    fn write(key: &str, name: Option<&str>, value: &str) -> Result<(), String> {
        let key_w = wide(key);
        let mut handle: HKEY = std::ptr::null_mut();
        // SAFETY: the key name is NUL-terminated and `handle` is written on success.
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                key_w.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut handle,
                std::ptr::null_mut(),
            )
        };
        if status != 0 {
            return Err(format!("could not open the registry key (error {status})"));
        }
        let name_w = name.map(wide);
        let name_ptr = name_w.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
        let data = wide(value);
        // SAFETY: `handle` is open, the name is NUL-terminated or null, and `data` is `len` bytes of UTF-16.
        let status = unsafe {
            RegSetValueExW(
                handle,
                name_ptr,
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        // SAFETY: `handle` came from RegCreateKeyExW above.
        unsafe { RegCloseKey(handle) };
        if status != 0 {
            return Err(format!("could not write the registry value (error {status})"));
        }
        Ok(())
    }

    /// The link is registered and its command is this exe's.
    pub fn registered_here() -> bool {
        let Ok(exe) = std::env::current_exe() else { return false };
        read(KEY, Some("URL Protocol")).is_some() && read(COMMAND_KEY, None).is_some_and(|c| points_at(&c, &exe))
    }

    /// Writes the three values; an existing registration, another Marshell's included, now points here.
    pub fn register() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| format!("could not find this program's path: {e}"))?;
        write(KEY, None, "URL:Marshell")?;
        write(KEY, Some("URL Protocol"), "")?;
        write(COMMAND_KEY, None, &command_for(&exe))
    }

    /// Deletes the key, but only when it points at this exe: another Marshell's registration stays. True when
    /// it deleted the key.
    pub fn unregister() -> Result<bool, String> {
        if !registered_here() {
            return Ok(false);
        }
        let key = wide(KEY);
        // SAFETY: the key name is NUL-terminated.
        let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
        if status != 0 {
            return Err(format!("could not remove the registry key (error {status})"));
        }
        Ok(true)
    }
}

/// Whether the link is registered for this exe. A link another build or install registered reads false.
#[cfg(windows)]
pub fn registered_here() -> bool {
    registry::registered_here()
}

/// A second launch hands its foreground right to the running Marshell before it forwards, so the window may
/// come to the front.
#[cfg(windows)]
pub fn hand_over_foreground() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
    // SAFETY: no pointers; ASFW_ANY lets any process take the foreground.
    unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

/// Removes Marshell's banners from the notification centre: with the link gone, their protocol links would
/// open Windows' "find an app" prompt. A failure is logged and ignored; the switch has already changed.
#[cfg(windows)]
fn clear_banners(app: &tauri::AppHandle) {
    use windows::UI::Notifications::ToastNotificationManager;
    let aumid = windows::core::HSTRING::from(crate::toast::app_aumid(app));
    if let Err(e) = ToastNotificationManager::History().and_then(|history| history.ClearWithId(&aumid)) {
        eprintln!("Marshell: could not clear the notification centre: {e}");
    }
}

/// The switch's state, read from the registry (never stored). `None` where the link doesn't exist.
#[tauri::command]
pub fn notification_link() -> Option<bool> {
    #[cfg(windows)]
    {
        Some(registered_here())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Turns the link on (pointing it at this exe) or off (only when it points here). Returns the state after.
#[tauri::command]
pub fn set_notification_link(app: tauri::AppHandle, on: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        if on {
            registry::register()?;
        } else if registry::unregister()? {
            clear_banners(&app);
        }
        Ok(registered_here())
    }
    #[cfg(not(windows))]
    {
        let _ = (app, on);
        Err("the notification centre link is only available on Windows".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn focus(id: &str) -> Option<Link> {
        Some(Link::Focus(id.to_string()))
    }

    #[test]
    fn every_opened_link_tells_the_webview_except_a_repeat() {
        let t1 = Link::Focus("t1".into());
        assert_eq!(focus_event(Some(&t1), true), Some(Some("t1".to_string())));
        assert_eq!(focus_event(Some(&t1), false), None);
        // `show`, and a second launch with no link, still take keyboard focus out of the terminal.
        assert_eq!(focus_event(Some(&Link::Show), true), Some(None));
        assert_eq!(focus_event(None, true), Some(None));
    }

    #[test]
    fn parses_the_two_shapes() {
        assert_eq!(parse("marshell://show"), Some(Link::Show));
        assert_eq!(parse("marshell://focus/t3f9a0c41b2de"), focus("t3f9a0c41b2de"));
        assert_eq!(parse("MARSHELL://focus/t1/"), focus("t1"));
        assert_eq!(parse("MARSHELL://FOCUS/T1"), focus("T1"));
        assert_eq!(parse("marshell://show/"), Some(Link::Show));
    }

    #[test]
    fn refuses_everything_else() {
        let long_tab = format!("marshell://focus/{}", "a".repeat(65));
        let huge = "a".repeat(10_000);
        let huge_link = format!("marshell://focus/{huge}");
        for bad in [
            "marshell://focus/",
            "marshell://focus",
            "marshell://focus/../x",
            "marshell://focus/t1?x=1",
            "marshell://focus/t1#x",
            "marshell://focus/t1 extra",
            "marshell://focus/t1//",
            "marshell://focus/t1/x",
            "marshell://show//",
            "marshell://show/x",
            "marshell://show?x=1",
            "marshell://answer/t1",
            "marshell:focus/t1",
            "marshell:///focus/t1",
            "https://focus/t1",
            "xmarshell://show",
            " marshell://show",
            "marshell://show ",
            "marshell://focus/t\u{e9}",
            "marshell://focus/t1\n",
            long_tab.as_str(),
            huge.as_str(),
            huge_link.as_str(),
            "",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_tab_id_of_64_characters_parses() {
        let id = "a".repeat(64);
        assert_eq!(parse(&format!("marshell://focus/{id}")), focus(&id));
        assert_eq!(parse(&format!("marshell://focus/{id}/")), focus(&id));
    }

    #[test]
    fn link_for_round_trips() {
        assert_eq!(parse(&link_for(Some("t1"))), focus("t1"));
        assert_eq!(link_for(None), "marshell://show");
        assert_eq!(link_for(Some("bad id")), "marshell://show");
        assert_eq!(link_for(Some("")), "marshell://show");
    }

    #[test]
    fn recent_admits_a_repeat_only_after_2_s() {
        let mut recent = Recent::default();
        let t1 = Link::Focus("t1".into());
        let t2 = Link::Focus("t2".into());
        assert!(recent.admit(&t1, 0));
        assert!(!recent.admit(&t1, 1_500));
        assert!(recent.admit(&t2, 1_600));
        assert!(recent.admit(&t1, 4_000));
    }

    #[test]
    fn argv_takes_only_a_lone_link() {
        let argv = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(from_argv(&argv(&["m.exe", "marshell://focus/t1"])), focus("t1"));
        assert_eq!(from_argv(&argv(&["m.exe"])), None);
        assert_eq!(from_argv(&argv(&[])), None);
        assert_eq!(from_argv(&argv(&["m.exe", "--flag", "marshell://focus/t1"])), None);
        assert_eq!(from_argv(&argv(&["m.exe", "marshell://focus/t1", "x"])), None);
        assert_eq!(from_argv(&argv(&["m.exe", "--flag"])), None);
    }

    #[test]
    fn points_at_compares_the_whole_command() {
        let exe = Path::new(r"C:\Program Files\Marshell\marshell.exe");
        let command = command_for(exe);
        assert_eq!(command, r#""C:\Program Files\Marshell\marshell.exe" "%1""#);
        assert!(points_at(&command, exe));
        assert!(points_at(&command.to_ascii_uppercase(), exe));
        assert!(!points_at(
            &command_for(Path::new(r"C:\dev\target\debug\marshell.exe")),
            exe
        ));
        assert!(!points_at(r#""C:\Program Files\Marshell\marshell.exe""#, exe));
        assert!(!points_at(&format!("{command} --x"), exe));
        assert!(!points_at("", exe));
    }
}
