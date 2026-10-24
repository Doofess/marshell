//! `~/.marshell/settings.json`: how the app looks. A missing, broken or newer-version file
//! means the defaults, and a broken file is left alone until the next save moves it aside.

use crate::store::{self, Versioned};
use marshell_protocol::api::{Appearance, NotifySettings, QuietHours, Settings};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Serialize, Deserialize, Default)]
struct FileDoc {
    #[serde(default)]
    appearance: Appearance,
    #[serde(default)]
    pause_warned: bool,
    #[serde(default)]
    onboarded: bool,
    #[serde(default)]
    close_to_tray: Option<bool>,
    #[serde(default)]
    status_off: Vec<String>,
    /// Absent in a file written before notifications had settings. A hand-edited block that does not read
    /// falls back on its own, so it cannot take the rest of the file with it.
    #[serde(default, deserialize_with = "lenient_notifications")]
    notifications: Option<NotifySettings>,
    /// None (a file from before the switch) means on.
    #[serde(default)]
    history_import: Option<bool>,
    /// None is no size limit. A value off the list of choices reads as none.
    #[serde(default)]
    history_limit_gb: Option<u32>,
    /// None keeps recordings and images forever. A value off the list of choices reads as none.
    #[serde(default)]
    history_limit_days: Option<u32>,
    /// Read only, to migrate an older file; never written.
    #[serde(default, skip_serializing)]
    sound_off: bool,
}

/// The notifications block, or the defaults when it is not one (a wrong type or `null` where a value goes).
fn lenient_notifications<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<NotifySettings>, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_value(value).unwrap_or_else(|err| {
        tracing::warn!("settings file has an invalid notifications block, using the defaults: {err}");
        NotifySettings::default()
    })))
}

impl Versioned for FileDoc {
    const VERSION: u32 = 1;
}

/// The accent colours that have a name.
const ACCENT_PRESETS: [&str; 9] = [
    "sand", "amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate",
];

/// Terminal themes that are always there; any other value must be an imported theme's id.
const BUILT_IN_TERMINAL_THEMES: [&str; 3] = ["follow-app", "dark", "light"];

/// The size limits on recordings and images, in GB. Anything else is refused.
pub const LIMIT_GB_CHOICES: [u32; 6] = [1, 2, 5, 10, 20, 50];

/// The age limits on recordings and images, in days. Anything else is refused.
pub const LIMIT_DAY_CHOICES: [u32; 4] = [30, 90, 180, 365];

/// Checks a pair of retention limits against the choices the app offers.
pub fn check_limits(limit_gb: Option<u32>, limit_days: Option<u32>) -> Result<(), String> {
    if limit_gb.is_some_and(|gb| !LIMIT_GB_CHOICES.contains(&gb)) {
        return Err("The size limit must be 1, 2, 5, 10, 20 or 50 GB.".into());
    }
    if limit_days.is_some_and(|days| !LIMIT_DAY_CHOICES.contains(&days)) {
        return Err("The age limit must be 30, 90, 180 or 365 days.".into());
    }
    Ok(())
}

/// A missing, broken or newer-version file means the defaults. The file itself is not touched.
pub fn load(home: &Path) -> Settings {
    let doc = store::read::<FileDoc>(&crate::paths::settings_file(home)).unwrap_or_else(|err| {
        tracing::warn!("ignoring settings file, using defaults: {err:#}");
        None
    });
    let doc = doc.unwrap_or_default();
    let (mut appearance, pause_warned, onboarded) = (doc.appearance, doc.pause_warned, doc.onboarded);
    let status_off = clean_status_off(doc.status_off);
    // A hand-edited value that fails validation would make every later save fail, so it gives way to its default.
    let replaced = repair(&mut appearance);
    if !replaced.is_empty() {
        tracing::warn!(
            "settings file has invalid values, using the defaults for: {}",
            replaced.join(", ")
        );
    }
    let mut notifications = doc.notifications.unwrap_or_else(|| {
        let mut n = NotifySettings::default();
        n.needs_you.sound = !doc.sound_off;
        n
    });
    if parse_hhmm(&notifications.quiet_hours.start).is_none() || parse_hhmm(&notifications.quiet_hours.end).is_none() {
        tracing::warn!("settings file has invalid quiet hours, using the defaults");
        notifications.quiet_hours = QuietHours::default();
    }
    // A hand-edited limit off the list would make every later save fail, so it reads as no limit.
    let (history_limit_gb, history_limit_days) = (
        doc.history_limit_gb.filter(|gb| LIMIT_GB_CHOICES.contains(gb)),
        doc.history_limit_days.filter(|days| LIMIT_DAY_CHOICES.contains(days)),
    );
    Settings {
        appearance,
        pause_warned,
        onboarded,
        close_to_tray: doc.close_to_tray,
        status_off,
        notifications,
        history_import: doc.history_import.unwrap_or(true),
        history_limit_gb,
        history_limit_days,
    }
}

/// "HH:MM" on a 24-hour clock as minutes since midnight. None for anything else.
pub fn parse_hhmm(s: &str) -> Option<u16> {
    let b = s.as_bytes();
    if b.len() != 5 || b[2] != b':' || ![0, 1, 3, 4].iter().all(|&i| b[i].is_ascii_digit()) {
        return None;
    }
    let hours = u16::from(b[0] - b'0') * 10 + u16::from(b[1] - b'0');
    let minutes = u16::from(b[3] - b'0') * 10 + u16::from(b[4] - b'0');
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

/// Agent ids, without empty entries or repeats, in the order they were written.
fn clean_status_off(ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    ids.into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect()
}

/// Writes the settings. A file that is not valid JSON is first copied to `settings.json.broken`,
/// because `store::write` only keeps a `.bak` of a file that parses.
pub fn save(home: &Path, s: &Settings) -> anyhow::Result<()> {
    let path = crate::paths::settings_file(home);
    if let Ok(old) = std::fs::read(&path)
        && serde_json::from_slice::<serde_json::Value>(&old).is_err()
    {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".broken");
        std::fs::copy(&path, path.with_file_name(name))?;
    }
    store::write(
        &path,
        &FileDoc {
            appearance: s.appearance.clone(),
            pause_warned: s.pause_warned,
            onboarded: s.onboarded,
            close_to_tray: s.close_to_tray,
            status_off: clean_status_off(s.status_off.clone()),
            notifications: Some(s.notifications.clone()),
            history_import: Some(s.history_import),
            history_limit_gb: s.history_limit_gb,
            history_limit_days: s.history_limit_days,
            sound_off: false,
        },
    )
}

/// `#rrggbb`, lower case.
fn is_hex_colour(text: &str) -> bool {
    text.len() == 7
        && text.starts_with('#')
        && text[1..]
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn check_accent(a: &Appearance) -> Result<(), String> {
    if !ACCENT_PRESETS.contains(&a.accent.as_str()) && !is_hex_colour(&a.accent) {
        return Err(format!(
            "The accent \"{}\" is not a preset or a #rrggbb colour.",
            a.accent
        ));
    }
    Ok(())
}

fn check_font_size(a: &Appearance) -> Result<(), String> {
    if !(8..=32).contains(&a.terminal_font.size) {
        return Err("The terminal font size must be between 8 and 32.".into());
    }
    Ok(())
}

fn check_imported_themes(a: &Appearance) -> Result<(), String> {
    let mut ids = HashSet::new();
    for theme in &a.imported_themes {
        if !ids.insert(theme.id.as_str()) {
            return Err(format!("Two imported themes share the id \"{}\".", theme.id));
        }
        let c = &theme.colors;
        if c.ansi.len() != 16 {
            return Err(format!(
                "Theme \"{}\" needs 16 ANSI colours, but has {}.",
                theme.name,
                c.ansi.len()
            ));
        }
        let named = [
            ("foreground", &c.foreground),
            ("background", &c.background),
            ("cursor", &c.cursor),
            ("selection", &c.selection),
        ];
        let ansi = c.ansi.iter().map(|v| ("ansi", v));
        for (which, colour) in named.into_iter().chain(ansi) {
            if !is_hex_colour(colour) {
                return Err(format!(
                    "Theme \"{}\" has a {which} colour \"{colour}\" that is not #rrggbb.",
                    theme.name
                ));
            }
        }
    }
    Ok(())
}

/// Assumes the imported themes are valid, so their ids are unique.
fn check_terminal_theme(a: &Appearance) -> Result<(), String> {
    let terminal_theme = a.terminal_theme.as_str();
    let imported = a.imported_themes.iter().any(|t| t.id == terminal_theme);
    if !BUILT_IN_TERMINAL_THEMES.contains(&terminal_theme) && !imported {
        return Err(format!(
            "The terminal theme \"{terminal_theme}\" is not built in or imported."
        ));
    }
    Ok(())
}

pub fn validate(s: &Settings) -> Result<(), String> {
    let a = &s.appearance;
    check_accent(a)?;
    check_font_size(a)?;
    check_imported_themes(a)?;
    check_terminal_theme(a)?;
    check_limits(s.history_limit_gb, s.history_limit_days)
}

/// Puts the default in place of every field that fails its check, and returns the names of those
/// fields. The themes are checked before the terminal theme, which may point at one of them.
fn repair(a: &mut Appearance) -> Vec<&'static str> {
    let defaults = Appearance::default();
    let mut replaced = Vec::new();
    if check_accent(a).is_err() {
        a.accent = defaults.accent;
        replaced.push("accent");
    }
    if check_font_size(a).is_err() {
        a.terminal_font.size = defaults.terminal_font.size;
        replaced.push("terminal font size");
    }
    if check_imported_themes(a).is_err() {
        a.imported_themes = defaults.imported_themes;
        replaced.push("imported themes");
    }
    if check_terminal_theme(a).is_err() {
        a.terminal_theme = defaults.terminal_theme;
        replaced.push("terminal theme");
    }
    replaced
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::{ImportedTheme, TerminalColors};

    fn colors() -> TerminalColors {
        TerminalColors {
            foreground: "#cccccc".into(),
            background: "#1e1e1e".into(),
            cursor: "#ffffff".into(),
            selection: "#264f78".into(),
            ansi: (0..16)
                .map(|i| format!("#{:02x}{:02x}{:02x}", i * 16, i * 8, i * 4))
                .collect(),
        }
    }

    fn theme(id: &str) -> ImportedTheme {
        ImportedTheme {
            id: id.into(),
            name: "Campbell".into(),
            source: "windows-terminal".into(),
            colors: colors(),
        }
    }

    #[test]
    fn missing_file_gives_defaults() {
        let home = tempfile::tempdir().unwrap();
        let s = load(home.path());
        assert_eq!(s, Settings::default());
        assert_eq!(s.appearance.accent, "sand");
        assert_eq!(s.appearance.terminal_font.size, 13);
    }

    #[test]
    fn a_settings_file_without_close_to_tray_reads_none() {
        let home = tempfile::tempdir().unwrap();
        let file = crate::paths::settings_file(home.path());
        for json in [r#"{"version":1}"#, r#"{"version":1,"close_to_tray":null}"#] {
            std::fs::write(&file, json).unwrap();
            assert_eq!(load(home.path()).close_to_tray, None);
        }
    }

    #[test]
    fn history_import_defaults_to_on() {
        let home = tempfile::tempdir().unwrap();
        assert!(Settings::default().history_import);
        assert!(load(home.path()).history_import);
        let file = crate::paths::settings_file(home.path());
        std::fs::write(&file, r#"{"version":1}"#).unwrap();
        assert!(load(home.path()).history_import);
        for on in [false, true] {
            let s = Settings {
                history_import: on,
                ..Settings::default()
            };
            save(home.path(), &s).unwrap();
            assert_eq!(load(home.path()).history_import, on);
        }
    }

    #[test]
    fn retention_limits_accept_only_the_listed_values_and_round_trip() {
        for (gb, days) in [
            (Some(3), None),
            (None, Some(7)),
            (Some(3), Some(7)),
            (Some(0), None),
            (None, Some(0)),
        ] {
            let s = Settings {
                history_limit_gb: gb,
                history_limit_days: days,
                ..Settings::default()
            };
            assert!(validate(&s).is_err(), "{gb:?} {days:?}");
        }
        let home = tempfile::tempdir().unwrap();
        let s = Settings {
            history_limit_gb: Some(5),
            history_limit_days: Some(90),
            ..Settings::default()
        };
        assert!(validate(&s).is_ok());
        save(home.path(), &s).unwrap();
        let back = load(home.path());
        assert_eq!((back.history_limit_gb, back.history_limit_days), (Some(5), Some(90)));
        // An old file has no limits; a hand-edited value off the list reads as none.
        let file = crate::paths::settings_file(home.path());
        std::fs::write(&file, r#"{"version":1}"#).unwrap();
        assert_eq!(load(home.path()).history_limit_gb, None);
        std::fs::write(&file, r#"{"version":1,"history_limit_gb":3,"history_limit_days":90}"#).unwrap();
        let back = load(home.path());
        assert_eq!((back.history_limit_gb, back.history_limit_days), (None, Some(90)));
    }

    #[test]
    fn close_to_tray_round_trips_through_save_and_load() {
        let home = tempfile::tempdir().unwrap();
        for close_to_tray in [Some(true), Some(false), None] {
            let s = Settings {
                close_to_tray,
                ..Settings::default()
            };
            save(home.path(), &s).unwrap();
            assert_eq!(load(home.path()).close_to_tray, close_to_tray);
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let home = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        s.appearance.accent = "#3366cc".into();
        s.appearance.imported_themes.push(theme("campbell"));
        save(home.path(), &s).unwrap();
        assert_eq!(load(home.path()), s);
    }

    #[test]
    fn load_broken_file_gives_defaults() {
        let home = tempfile::tempdir().unwrap();
        let path = crate::paths::settings_file(home.path());
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load(home.path()), Settings::default());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{not json");
    }

    #[test]
    fn save_keeps_a_broken_file_aside() {
        let home = tempfile::tempdir().unwrap();
        let path = crate::paths::settings_file(home.path());
        std::fs::write(&path, "{not json").unwrap();
        save(home.path(), &Settings::default()).unwrap();
        let aside = home.path().join("settings.json.broken");
        assert_eq!(std::fs::read_to_string(aside).unwrap(), "{not json");
        assert_eq!(load(home.path()), Settings::default());
    }

    #[test]
    fn missing_fields_take_defaults() {
        let home = tempfile::tempdir().unwrap();
        let path = crate::paths::settings_file(home.path());
        std::fs::write(&path, r#"{"version":1,"appearance":{"accent":"teal"}}"#).unwrap();
        let s = load(home.path());
        assert_eq!(s.appearance.accent, "teal");
        assert_eq!(s.appearance.terminal_theme, "follow-app");
    }

    #[test]
    fn load_replaces_invalid_values_with_defaults_and_keeps_the_rest() {
        let home = tempfile::tempdir().unwrap();
        let path = crate::paths::settings_file(home.path());
        std::fs::write(
            &path,
            r#"{"version":1,"appearance":{"theme":"dark","accent":"pink","contrast_protection":false,"terminal_font":{"family":"Fira Code","size":40}}}"#,
        )
        .unwrap();
        let a = load(home.path()).appearance;
        assert_eq!(a.accent, "sand");
        assert_eq!(a.terminal_font.size, 13);
        assert_eq!(a.theme, marshell_protocol::api::AppTheme::Dark);
        assert!(!a.contrast_protection);
        assert_eq!(a.terminal_font.family.as_deref(), Some("Fira Code"));
        // What load returns can be saved again: it no longer fails validation.
        assert!(
            validate(&Settings {
                appearance: a,
                ..Settings::default()
            })
            .is_ok()
        );
    }

    #[test]
    fn pause_warned_round_trips_and_an_old_file_loads_false() {
        let home = tempfile::tempdir().unwrap();
        let s = Settings {
            pause_warned: true,
            ..Settings::default()
        };
        save(home.path(), &s).unwrap();
        assert!(load(home.path()).pause_warned);

        let file = crate::paths::settings_file(home.path());
        std::fs::write(&file, r#"{"version":1,"appearance":{}}"#).unwrap();
        assert!(!load(home.path()).pause_warned);
    }

    #[test]
    fn onboarded_round_trips_and_defaults_false() {
        let home = tempfile::tempdir().unwrap();
        let s = Settings {
            onboarded: true,
            ..Settings::default()
        };
        save(home.path(), &s).unwrap();
        assert!(load(home.path()).onboarded);

        let file = crate::paths::settings_file(home.path());
        std::fs::write(&file, r#"{"version":1,"appearance":{}}"#).unwrap();
        assert!(!load(home.path()).onboarded);
    }

    #[test]
    fn status_off_round_trips_without_repeats_and_defaults_empty() {
        let home = tempfile::tempdir().unwrap();
        let s = Settings {
            status_off: vec!["claude".into(), " ".into(), "claude".into(), "codex".into()],
            ..Settings::default()
        };
        save(home.path(), &s).unwrap();
        assert_eq!(load(home.path()).status_off, vec!["claude", "codex"]);

        let file = crate::paths::settings_file(home.path());
        std::fs::write(
            &file,
            r#"{"version":1,"appearance":{},"status_off":["claude","","claude"]}"#,
        )
        .unwrap();
        assert_eq!(load(home.path()).status_off, vec!["claude"]);
        std::fs::write(&file, r#"{"version":1,"appearance":{}}"#).unwrap();
        assert!(load(home.path()).status_off.is_empty());
    }

    #[test]
    fn settings_default_notifications() {
        let home = tempfile::tempdir().unwrap();
        let file = crate::paths::settings_file(home.path());
        std::fs::write(&file, r#"{"version":1,"appearance":{}}"#).unwrap();
        assert_eq!(load(home.path()).notifications, NotifySettings::default());
        let mut s = Settings::default();
        s.notifications.done.banner = false;
        s.notifications.subagent_sound = true;
        save(home.path(), &s).unwrap();
        assert_eq!(load(home.path()), s);
    }

    #[test]
    fn notify_settings_without_terminal_alerts_reads_true() {
        let home = tempfile::tempdir().unwrap();
        let file = crate::paths::settings_file(home.path());
        std::fs::write(
            &file,
            r#"{"version":1,"appearance":{},"notifications":{"done":{"banner":false,"sound":true},"subagent_sound":true}}"#,
        )
        .unwrap();
        let s = load(home.path());
        assert!(s.notifications.terminal_alerts);
        assert!(!s.notifications.done.banner);
    }

    #[test]
    fn legacy_sound_off_maps_to_needs_you_sound() {
        let home = tempfile::tempdir().unwrap();
        let file = crate::paths::settings_file(home.path());
        std::fs::write(&file, r#"{"version":1,"appearance":{},"sound_off":true}"#).unwrap();
        let s = load(home.path());
        let mut expected = NotifySettings::default();
        expected.needs_you.sound = false;
        assert_eq!(s.notifications, expected);
        save(home.path(), &s).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("\"notifications\""), "{text}");
        assert!(!text.contains("sound_off"), "{text}");
        assert_eq!(load(home.path()), s);
    }

    #[test]
    fn bad_quiet_hours_fall_back() {
        let home = tempfile::tempdir().unwrap();
        let file = crate::paths::settings_file(home.path());
        std::fs::write(
            &file,
            r#"{"version":1,"appearance":{},"onboarded":true,"notifications":{"done":{"banner":false,"sound":true},"quiet_hours":{"enabled":true,"start":"25:00","end":"08:00"}}}"#,
        )
        .unwrap();
        let s = load(home.path());
        assert_eq!(s.notifications.quiet_hours, QuietHours::default());
        assert!(!s.notifications.done.banner);
        assert!(s.onboarded);
    }

    #[test]
    fn a_notifications_block_missing_a_field_keeps_the_rest() {
        let home = tempfile::tempdir().unwrap();
        let file = crate::paths::settings_file(home.path());
        std::fs::write(
            &file,
            r#"{"version":1,"appearance":{},"onboarded":true,"notifications":{"done":{"banner":false},"quiet_hours":{"enabled":true,"start":"23:00"}}}"#,
        )
        .unwrap();
        let s = load(home.path());
        assert!(s.onboarded);
        assert!(!s.notifications.done.banner);
        assert!(s.notifications.done.sound);
        let q = &s.notifications.quiet_hours;
        assert_eq!((q.enabled, q.start.as_str(), q.end.as_str()), (true, "23:00", "08:00"));
    }

    #[test]
    fn a_notifications_block_of_the_wrong_type_falls_back_alone() {
        let home = tempfile::tempdir().unwrap();
        let file = crate::paths::settings_file(home.path());
        std::fs::write(
            &file,
            r#"{"version":1,"appearance":{"accent":"teal"},"onboarded":true,"notifications":{"done":{"banner":false,"sound":true},"quiet_hours":{"enabled":true,"start":2200,"end":null}}}"#,
        )
        .unwrap();
        let s = load(home.path());
        assert!(s.onboarded);
        assert_eq!(s.appearance.accent, "teal");
        assert_eq!(s.notifications, NotifySettings::default());
    }

    #[test]
    fn parse_hhmm_reads_24_hour_times() {
        assert_eq!(parse_hhmm("00:00"), Some(0));
        assert_eq!(parse_hhmm("08:00"), Some(480));
        assert_eq!(parse_hhmm("23:59"), Some(1439));
        for bad in ["24:00", "8:00", "08:60", "", "08:0", "ab:cd", "08-00", "+8:00"] {
            assert_eq!(parse_hhmm(bad), None, "{bad}");
        }
    }

    #[test]
    fn load_drops_a_terminal_theme_whose_import_is_invalid() {
        let home = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        let mut bad = theme("campbell");
        bad.colors.cursor = "red".into();
        s.appearance.imported_themes.push(bad);
        s.appearance.terminal_theme = "campbell".into();
        save(home.path(), &s).unwrap();
        let a = load(home.path()).appearance;
        assert!(a.imported_themes.is_empty());
        assert_eq!(a.terminal_theme, "follow-app");
    }

    #[test]
    fn validate_accepts_the_defaults_and_a_custom_accent() {
        assert!(validate(&Settings::default()).is_ok());
        let mut s = Settings::default();
        s.appearance.accent = "#3366cc".into();
        s.appearance.imported_themes.push(theme("campbell"));
        s.appearance.terminal_theme = "campbell".into();
        assert!(validate(&s).is_ok());
    }

    /// Applies `change` to the default settings and returns the error `validate` gives.
    fn error_after(change: impl FnOnce(&mut Settings)) -> String {
        let mut s = Settings::default();
        change(&mut s);
        validate(&s).unwrap_err()
    }

    #[test]
    fn validate_rejects_bad_values() {
        assert!(error_after(|s| s.appearance.accent = "pink".into()).contains("accent"));
        assert!(error_after(|s| s.appearance.accent = "#12345".into()).contains("accent"));
        assert!(error_after(|s| s.appearance.terminal_font.size = 7).contains("font size"));
        assert!(error_after(|s| s.appearance.terminal_font.size = 33).contains("font size"));
        assert!(error_after(|s| s.appearance.terminal_theme = "nope".into()).contains("terminal theme"));

        let mut short = theme("a");
        short.colors.ansi.pop();
        assert!(error_after(|s| s.appearance.imported_themes.push(short)).contains("16"));

        let mut red = theme("a");
        red.colors.cursor = "red".into();
        assert!(error_after(|s| s.appearance.imported_themes.push(red)).contains("#rrggbb"));

        let twice = error_after(|s| {
            s.appearance.imported_themes.push(theme("a"));
            s.appearance.imported_themes.push(theme("a"));
        });
        assert!(twice.contains("id"));
    }
}
