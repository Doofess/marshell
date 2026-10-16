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
    Settings {
        appearance: doc.unwrap_or_default().appearance,
    }
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

pub fn validate(s: &Settings) -> Result<(), String> {
    let a = &s.appearance;
    if !ACCENT_PRESETS.contains(&a.accent.as_str()) && !is_hex_colour(&a.accent) {
        return Err(format!(
            "The accent \"{}\" is not a preset or a #rrggbb colour.",
            a.accent
        ));
    }
    if !(8..=32).contains(&a.terminal_font.size) {
        return Err("The terminal font size must be between 8 and 32.".into());
    }
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
    let terminal_theme = a.terminal_theme.as_str();
    if !BUILT_IN_TERMINAL_THEMES.contains(&terminal_theme) && !ids.contains(terminal_theme) {
        return Err(format!(
            "The terminal theme \"{terminal_theme}\" is not built in or imported."
        ));
    }
    Ok(())
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
