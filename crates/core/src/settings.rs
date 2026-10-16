//! `~/.marshell/settings.json`: how the app looks. A missing, broken or newer-version file
//! means the defaults, and a broken file is left alone until the next save moves it aside.

use crate::store::{self, Versioned};
use marshell_protocol::api::{Appearance, Settings};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Serialize, Deserialize, Default)]
struct FileDoc {
    #[serde(default)]
    appearance: Appearance,
}

impl Versioned for FileDoc {
    const VERSION: u32 = 1;
}

/// The accent colours that have a name.
const ACCENT_PRESETS: [&str; 8] = ["amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"];

/// Terminal themes that are always there; any other value must be an imported theme's id.
const BUILT_IN_TERMINAL_THEMES: [&str; 3] = ["follow-app", "dark", "light"];

/// A missing, broken or newer-version file means the defaults. The file itself is not touched.
pub fn load(home: &Path) -> Settings {
    let doc = store::read::<FileDoc>(&crate::paths::settings_file(home)).unwrap_or_else(|err| {
        tracing::warn!("ignoring settings file, using defaults: {err:#}");
        None
    });
    let mut appearance = doc.unwrap_or_default().appearance;
    // A hand-edited value that fails validation would make every later save fail, so it gives way to its default.
    let replaced = repair(&mut appearance);
    if !replaced.is_empty() {
        tracing::warn!(
            "settings file has invalid values, using the defaults for: {}",
            replaced.join(", ")
        );
    }
    Settings { appearance }
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
    check_terminal_theme(a)
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
        assert_eq!(s.appearance.accent, "amber");
        assert_eq!(s.appearance.terminal_font.size, 13);
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
        assert_eq!(a.accent, "amber");
        assert_eq!(a.terminal_font.size, 13);
        assert_eq!(a.theme, marshell_protocol::api::AppTheme::Dark);
        assert!(!a.contrast_protection);
        assert_eq!(a.terminal_font.family.as_deref(), Some("Fira Code"));
        // What load returns can be saved again: it no longer fails validation.
        assert!(validate(&Settings { appearance: a }).is_ok());
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
