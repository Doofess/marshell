//! `~/.marshell/profiles.json`. Detected shells are never written unless the user edited them;
//! the file holds only edited detected profiles and custom ones.

use crate::store::{self, Versioned};
use marshell_protocol::api::{Profile, ProfilesDoc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
struct FileDoc {
    profiles: Vec<Profile>,
    default_id: Option<String>,
}

impl Versioned for FileDoc {
    const VERSION: u32 = 1;
}

fn file(home: &Path) -> PathBuf {
    home.join("profiles.json")
}

/// True when `p` is exactly what detection found (the `detected` flag itself is ignored).
fn is_unchanged(p: &Profile, detected: &[Profile]) -> bool {
    detected.iter().any(|d| {
        let mut p = p.clone();
        p.detected = d.detected;
        &p == d
    })
}

/// Detected profiles, with file entries replacing a detected one that has the same id and
/// custom ids appended. A missing, broken or newer-version file means detected profiles only.
pub fn load(home: &Path, detected: Vec<Profile>) -> ProfilesDoc {
    let from_file = store::read::<FileDoc>(&file(home)).unwrap_or_else(|err| {
        tracing::warn!("ignoring profiles file: {err:#}");
        None
    });
    let Some(from_file) = from_file else {
        let default_id = detected.first().map(|p| p.id.clone()).unwrap_or_default();
        return ProfilesDoc {
            profiles: detected,
            default_id,
        };
    };
    let mut profiles = detected;
    for mut edited in from_file.profiles {
        edited.detected = false;
        match profiles.iter().position(|p| p.id == edited.id) {
            Some(i) => profiles[i] = edited,
            None => profiles.push(edited),
        }
    }
    let default_id = from_file
        .default_id
        .filter(|id| profiles.iter().any(|p| &p.id == id))
        .or_else(|| profiles.first().map(|p| p.id.clone()))
        .unwrap_or_default();
    ProfilesDoc { profiles, default_id }
}

/// Sets each profile's `detected` flag: true only when it still matches detection.
pub fn normalize(doc: &ProfilesDoc, detected: &[Profile]) -> ProfilesDoc {
    let mut doc = doc.clone();
    for p in &mut doc.profiles {
        p.detected = is_unchanged(p, detected);
    }
    doc
}

/// Writes only the entries that differ from detection or are custom.
/// Refuses when the existing file cannot be read (broken, or written by a newer Marshell),
/// so a file the user may want to repair is never overwritten.
pub fn save(home: &Path, doc: &ProfilesDoc, detected: &[Profile]) -> anyhow::Result<()> {
    let path = file(home);
    store::read::<FileDoc>(&path)?;
    let profiles = normalize(doc, detected)
        .profiles
        .into_iter()
        .filter(|p| !p.detected)
        .collect();
    store::write(
        &path,
        &FileDoc {
            profiles,
            default_id: Some(doc.default_id.clone()),
        },
    )
}

pub fn validate(doc: &ProfilesDoc) -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for p in &doc.profiles {
        if p.id.trim().is_empty() {
            return Err("A profile needs an id.".into());
        }
        if p.name.trim().is_empty() {
            return Err("A profile needs a name.".into());
        }
        if p.command.is_empty() || p.command[0].trim().is_empty() {
            return Err(format!("Profile \"{}\" needs a command.", p.name));
        }
        if !ids.insert(p.id.as_str()) {
            return Err(format!("Two profiles share the id \"{}\".", p.id));
        }
        if !names.insert(p.name.as_str()) {
            return Err(format!("Two profiles share the name \"{}\".", p.name));
        }
    }
    if !doc.profiles.iter().any(|p| p.id == doc.default_id) {
        return Err("The default profile does not exist.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::ShellFamily;

    fn profile(id: &str, name: &str, detected: bool) -> Profile {
        Profile {
            id: id.into(),
            name: name.into(),
            command: vec![format!("{id}.exe")],
            family: ShellFamily::PowerShell,
            cwd: None,
            env: Vec::new(),
            detected,
        }
    }

    fn found() -> Vec<Profile> {
        vec![
            profile("pwsh", "PowerShell 7", true),
            profile("cmd", "Command Prompt", true),
        ]
    }

    fn file_json(home: &Path) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(file(home)).unwrap()).unwrap()
    }

    #[test]
    fn load_without_file_returns_detected() {
        let home = tempfile::tempdir().unwrap();
        let doc = load(home.path(), found());
        assert_eq!(doc.profiles, found());
        assert_eq!(doc.default_id, "pwsh");
    }

    #[test]
    fn edited_detected_profile_wins_by_id() {
        let home = tempfile::tempdir().unwrap();
        let mut edited = profile("cmd", "My cmd", false);
        edited.command = vec!["cmd.exe".into(), "/k".into()];
        let doc = ProfilesDoc {
            profiles: vec![profile("pwsh", "PowerShell 7", true), edited.clone()],
            default_id: "cmd".into(),
        };
        save(home.path(), &doc, &found()).unwrap();
        let loaded = load(home.path(), found());
        assert_eq!(loaded.profiles, vec![profile("pwsh", "PowerShell 7", true), edited]);
        assert_eq!(loaded.default_id, "cmd");
    }

    #[test]
    fn save_skips_unchanged_detected() {
        let home = tempfile::tempdir().unwrap();
        let mut profiles = found();
        profiles.push(profile("my-pwsh", "Mine", false));
        let doc = ProfilesDoc {
            profiles,
            default_id: "pwsh".into(),
        };
        save(home.path(), &doc, &found()).unwrap();
        let json = file_json(home.path());
        assert_eq!(json["version"], 1);
        let ids: Vec<_> = json["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].clone())
            .collect();
        assert_eq!(ids, ["my-pwsh"]);
        assert_eq!(json["default_id"], "pwsh");
    }

    #[test]
    fn corrupt_profiles_file_falls_back_to_detected() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(file(home.path()), "{nope").unwrap();
        let doc = load(home.path(), found());
        assert_eq!(doc.profiles, found());
        assert_eq!(std::fs::read(file(home.path())).unwrap(), b"{nope");
        // A save does not overwrite it either.
        assert!(save(home.path(), &doc, &found()).is_err());
        assert_eq!(std::fs::read(file(home.path())).unwrap(), b"{nope");
    }

    #[test]
    fn save_refuses_to_replace_a_newer_file() {
        let home = tempfile::tempdir().unwrap();
        let newer = br#"{"version":9,"profiles":[],"default_id":null}"#;
        std::fs::write(file(home.path()), newer).unwrap();
        let doc = load(home.path(), found());
        assert!(save(home.path(), &doc, &found()).is_err());
        assert_eq!(std::fs::read(file(home.path())).unwrap(), newer);
    }

    #[test]
    fn validate_rejects_duplicate_ids_and_empty_command() {
        let ok = ProfilesDoc {
            profiles: found(),
            default_id: "cmd".into(),
        };
        assert!(validate(&ok).is_ok());

        let mut dup_id = ok.clone();
        dup_id.profiles[1].id = "pwsh".into();
        assert!(validate(&dup_id).is_err());

        let mut dup_name = ok.clone();
        dup_name.profiles[1].name = "PowerShell 7".into();
        assert!(validate(&dup_name).is_err());

        let mut no_command = ok.clone();
        no_command.profiles[0].command.clear();
        assert!(validate(&no_command).is_err());

        let mut empty_name = ok.clone();
        empty_name.profiles[0].name = "  ".into();
        assert!(validate(&empty_name).is_err());

        let mut no_default = ok;
        no_default.default_id = "zsh".into();
        assert!(validate(&no_default).is_err());
    }
}
