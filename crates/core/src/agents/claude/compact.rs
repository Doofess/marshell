//! Claude Code's auto-compact window, read from settings without changing them.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Files are highest precedence first; a model setting wins over that file's top-level setting.
/// `auto` stops the search and leaves the status line's percentage in charge.
/// `CLAUDE_CODE_AUTO_COMPACT_WINDOW` is deliberately not read: this resolver follows settings
/// files only. Marshell strips inherited Claude session markers, not all `CLAUDE_CODE_*` settings.
pub fn compact_window(model_id: Option<&str>, files: &[Value]) -> Option<u64> {
    for file in files {
        let model = model_id.and_then(|id| file.get("modelSettings")?.get(id)?.get("autoCompactWindow"));
        for value in [model, file.get("autoCompactWindow")].into_iter().flatten() {
            if let Some(window) = value.as_u64() {
                return Some(window.clamp(100_000, 1_000_000));
            }
            if value.as_str().is_some_and(|s| s.eq_ignore_ascii_case("auto")) {
                return None;
            }
        }
    }
    None
}

/// Reads managed, project-local, project and user settings in that order. Missing or invalid JSON
/// is skipped. The user file always lives under `user_home`, regardless of `MARSHELL_CLAUDE_SETTINGS`.
pub fn read_settings(user_home: &Path, project_dir: Option<&Path>, managed: &[PathBuf]) -> Vec<Value> {
    let mut paths = managed.to_vec();
    if let Some(project) = project_dir {
        paths.push(project.join(".claude/settings.local.json"));
        paths.push(project.join(".claude/settings.json"));
    }
    paths.push(user_home.join(".claude/settings.json"));
    paths
        .iter()
        // Check links too: a local .claude directory must not send these reads to a network share.
        .filter_map(|path| crate::server::local_file(path.to_str()?))
        .filter_map(|path| serde_json::from_slice(&std::fs::read(path).ok()?).ok())
        .collect()
}

/// Reuses the config guard's organization roots and managed filename.
pub fn managed_settings_paths() -> Vec<PathBuf> {
    let roots = crate::config::guard::denied_for_os();
    #[cfg(windows)]
    let dir = roots[0].join("ClaudeCode"); // Program Files
    #[cfg(target_os = "macos")]
    let dir = roots[0].join("Application Support/ClaudeCode"); // /Library
    #[cfg(not(any(windows, target_os = "macos")))]
    let dir = roots[0].join("claude-code"); // /etc
    vec![dir.join(crate::config::guard::MANAGED_NAMES[0])]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn local_beats_user() {
        let files = [
            json!({"autoCompactWindow": 300000}),
            json!({"autoCompactWindow": 500000}),
        ];
        assert_eq!(compact_window(None, &files), Some(300_000));
    }

    #[test]
    fn model_beats_top_level_in_the_same_file() {
        let files = [json!({
            "modelSettings": {"claude-opus-5-5": {"autoCompactWindow": 200000}},
            "autoCompactWindow": 300000
        })];
        assert_eq!(compact_window(Some("claude-opus-5-5"), &files), Some(200_000));
        assert_eq!(compact_window(Some("other-model"), &files), Some(300_000));
        assert_eq!(compact_window(None, &files), Some(300_000));
    }

    #[test]
    fn auto_stops_the_search() {
        for auto in ["auto", "AUTO", "Auto"] {
            let files = [json!({"autoCompactWindow": auto}), json!({"autoCompactWindow": 300000})];
            assert_eq!(compact_window(None, &files), None);
            let files = [json!({
                "modelSettings": {"opus": {"autoCompactWindow": auto}},
                "autoCompactWindow": 300000
            })];
            assert_eq!(compact_window(Some("opus"), &files), None);
        }
    }

    #[test]
    fn integers_are_clamped() {
        for (given, want) in [
            (0, 100_000),
            (99_999, 100_000),
            (300_000, 300_000),
            (1_000_001, 1_000_000),
            (u64::MAX, 1_000_000),
        ] {
            assert_eq!(compact_window(None, &[json!({"autoCompactWindow": given})]), Some(want));
        }
    }

    #[test]
    fn invalid_values_are_skipped() {
        for value in [
            json!(-1),
            json!(300000.0),
            json!("300000"),
            json!("unknown"),
            json!(null),
            json!(true),
            json!([]),
            json!({}),
        ] {
            let files = [
                json!({"autoCompactWindow": value}),
                json!({"autoCompactWindow": 300000}),
            ];
            assert_eq!(compact_window(None, &files), Some(300_000), "{value}");
            let files = [json!({"modelSettings": {"opus": {"autoCompactWindow": value}}, "autoCompactWindow": 200000})];
            assert_eq!(compact_window(Some("opus"), &files), Some(200_000), "{value}");
        }
        assert_eq!(compact_window(None, &[json!({})]), None);
    }

    #[test]
    fn reads_managed_local_project_and_user_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let project = dir.path().join("project");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(project.join(".claude")).unwrap();
        let managed = [
            dir.path().join("managed-first.json"),
            dir.path().join("managed-second.json"),
        ];
        let paths = [
            managed[0].clone(),
            managed[1].clone(),
            project.join(".claude/settings.local.json"),
            project.join(".claude/settings.json"),
            home.join(".claude/settings.json"),
        ];
        let values = [100_000, 200_000, 300_000, 400_000, 500_000];
        for (path, value) in paths.iter().zip(values) {
            fs::write(path, json!({"autoCompactWindow": value}).to_string()).unwrap();
        }
        for (path, want) in paths.iter().zip(values) {
            let files = read_settings(&home, Some(&project), &managed);
            assert_eq!(compact_window(None, &files), Some(want));
            fs::remove_file(path).unwrap();
        }
        assert!(read_settings(&home, Some(&project), &managed).is_empty());
    }

    #[test]
    fn skips_missing_and_invalid_json_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".claude")).unwrap();
        let user = json!({"autoCompactWindow": 300000});
        fs::write(dir.path().join(".claude/settings.json"), user.to_string()).unwrap();
        let bad = dir.path().join("bad.json");
        for text in [
            "{",
            "{\"autoCompactWindow\": 100000,}",
            "{/* comment */\"autoCompactWindow\":100000}",
        ] {
            fs::write(&bad, text).unwrap();
            assert_eq!(
                read_settings(dir.path(), None, &[dir.path().join("missing.json"), bad.clone()]),
                vec![user.clone()]
            );
        }
    }
}
