//! `~/.marshell/layout.json`: the tabs that were open and how the window was laid out. A missing,
//! broken or newer-version file means the defaults, and `load` never writes.

use crate::store::{self, Versioned};
use marshell_protocol::api::{LauncherChoice, LayoutDoc, SessionMode, ViewLayout};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The file holds the `LayoutDoc` fields directly, next to `"version"`.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct FileDoc(LayoutDoc);

impl Versioned for FileDoc {
    const VERSION: u32 = 1;
}

/// The names the frontend uses for pane arrangements (`ARRANGEMENTS` in `splitLayout.ts`).
const ARRANGEMENTS: [&str; 6] = ["single", "columns", "rows", "grid", "main-left", "main-top"];
const SIDEBAR_MODES: [&str; 3] = ["expanded", "rail", "focus"];
const MAX_PANES: usize = 4;
const SIDEBAR_WIDTH: std::ops::RangeInclusive<u32> = 200..=400;
const RATIO: std::ops::RangeInclusive<f32> = 0.1..=0.9;
/// Ratios are the column split and the row split, so there are never more than two.
const MAX_RATIOS: usize = 2;

/// A missing, broken or newer-version file means the defaults. The file itself is not touched.
pub fn load(home: &Path) -> LayoutDoc {
    match store::read::<FileDoc>(&crate::paths::layout_file(home)) {
        Ok(Some(FileDoc(doc))) => repair(doc),
        Ok(None) => LayoutDoc::default(),
        Err(err) => {
            tracing::warn!("ignoring layout file, using defaults: {err:#}");
            LayoutDoc::default()
        }
    }
}

pub fn save(home: &Path, doc: &LayoutDoc) -> anyhow::Result<()> {
    store::write(&crate::paths::layout_file(home), &FileDoc(doc.clone()))
}

/// How many folders the launcher remembers a choice for.
const MAX_LAUNCHER_FOLDERS: usize = 32;

/// Notes the profile, agent and mode just used in `folder`. Full auto and Bypass are never remembered.
/// The choice gets a number one higher than any other, so the order of use survives a restart;
/// past 32 folders the one used longest ago is dropped.
pub fn remember(
    doc: &mut LayoutDoc,
    folder: &str,
    profile_id: String,
    agent: Option<String>,
    mode: Option<SessionMode>,
) {
    let mode = mode.filter(|m| !matches!(m, SessionMode::FullAuto | SessionMode::Bypass));
    let newest = doc.launcher.values().map(|c| c.seq).max().unwrap_or(0);
    let seq = newest.saturating_add(1);
    doc.launcher.insert(
        folder.to_string(),
        LauncherChoice {
            profile_id,
            agent,
            seq,
            mode,
        },
    );
    while doc.launcher.len() > MAX_LAUNCHER_FOLDERS {
        let oldest = doc.launcher.iter().min_by_key(|(_, c)| c.seq).map(|(k, _)| k.clone());
        match oldest {
            Some(key) => doc.launcher.remove(&key),
            None => break,
        };
    }
}

/// Pulls a hand-edited or stale document back into range, so the frontend never gets a layout it cannot draw.
pub fn repair(mut doc: LayoutDoc) -> LayoutDoc {
    let view = &mut doc.view;
    view.panes.truncate(MAX_PANES);
    if view.panes.is_empty() {
        view.panes.push(None);
    }
    // A pane whose tab is gone is shown empty.
    for pane in &mut view.panes {
        if pane
            .as_ref()
            .is_some_and(|id| !doc.tabs.iter().any(|t| &t.tab_id == id))
        {
            *pane = None;
        }
    }
    let count = view.panes.len() as u32;
    if view.active >= count {
        view.active = 0;
    }
    if view.zoomed.is_some_and(|z| z >= count) {
        view.zoomed = None;
    }
    view.sidebar_width = view.sidebar_width.clamp(*SIDEBAR_WIDTH.start(), *SIDEBAR_WIDTH.end());
    if !SIDEBAR_MODES.contains(&view.sidebar.as_str()) {
        view.sidebar = "expanded".into();
    }
    if view.ratios.len() > MAX_RATIOS || view.ratios.iter().any(|r| !RATIO.contains(r)) {
        view.ratios.clear();
    }
    if view.arrangement.as_deref().is_some_and(|a| !ARRANGEMENTS.contains(&a)) {
        view.arrangement = None;
    }
    doc
}

/// The same rules as `repair`, but a bad value is an error naming the field. Whether a pane's tab
/// exists is not checked here; `repair` empties such a pane when the file is next loaded.
pub fn validate_view(v: &ViewLayout) -> Result<(), String> {
    if !(1..=MAX_PANES).contains(&v.panes.len()) {
        return Err(format!(
            "panes must hold 1 to {MAX_PANES} entries, not {}",
            v.panes.len()
        ));
    }
    let count = v.panes.len() as u32;
    if v.active >= count {
        return Err(format!("active {} is outside the {count} panes", v.active));
    }
    if let Some(z) = v.zoomed
        && z >= count
    {
        return Err(format!("zoomed {z} is outside the {count} panes"));
    }
    if !SIDEBAR_WIDTH.contains(&v.sidebar_width) {
        return Err(format!(
            "sidebar_width must be {} to {}, not {}",
            SIDEBAR_WIDTH.start(),
            SIDEBAR_WIDTH.end(),
            v.sidebar_width
        ));
    }
    if !SIDEBAR_MODES.contains(&v.sidebar.as_str()) {
        return Err(format!(
            "sidebar must be one of {SIDEBAR_MODES:?}, not \"{}\"",
            v.sidebar
        ));
    }
    if v.ratios.len() > MAX_RATIOS || v.ratios.iter().any(|r| !RATIO.contains(r)) {
        return Err(format!(
            "ratios must be at most {MAX_RATIOS} values between {} and {}",
            RATIO.start(),
            RATIO.end()
        ));
    }
    if let Some(a) = &v.arrangement
        && !ARRANGEMENTS.contains(&a.as_str())
    {
        return Err(format!("arrangement must be one of {ARRANGEMENTS:?}, not \"{a}\""));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::SavedTab;

    fn tab(id: &str) -> SavedTab {
        SavedTab {
            tab_id: id.into(),
            number: 1,
            name: id.into(),
            agent: None,
            profile_id: "cmd".into(),
            cwd: "C:/work".into(),
            mode: None,
            muted: false,
        }
    }

    fn doc_with(name: &str) -> LayoutDoc {
        LayoutDoc {
            tabs: vec![tab(name)],
            view: ViewLayout {
                panes: vec![Some(name.into())],
                ..ViewLayout::default()
            },
            ..LayoutDoc::default()
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let home = tempfile::tempdir().unwrap();
        let doc = doc_with("a");
        save(home.path(), &doc).unwrap();
        assert_eq!(load(home.path()), doc);
        let on_disk: serde_json::Value =
            serde_json::from_slice(&std::fs::read(home.path().join("layout.json")).unwrap()).unwrap();
        assert_eq!(on_disk["version"], 1);
        assert_eq!(on_disk["tabs"][0]["tab_id"], "a");
    }

    #[test]
    fn keeps_previous_as_bak() {
        let home = tempfile::tempdir().unwrap();
        save(home.path(), &doc_with("a")).unwrap();
        save(home.path(), &doc_with("b")).unwrap();
        assert_eq!(load(home.path()), doc_with("b"));
        let bak = store::bak_path(&home.path().join("layout.json"));
        let old: serde_json::Value = serde_json::from_slice(&std::fs::read(bak).unwrap()).unwrap();
        assert_eq!(old["tabs"][0]["tab_id"], "a");
    }

    #[test]
    fn corrupt_file_falls_back_to_bak_then_defaults() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("layout.json");
        save(home.path(), &doc_with("a")).unwrap();
        save(home.path(), &doc_with("b")).unwrap();
        std::fs::write(&path, "{garbage").unwrap();
        assert_eq!(load(home.path()), doc_with("a"));
        // Load only reads: the broken file stays as it was.
        assert_eq!(std::fs::read(&path).unwrap(), b"{garbage");

        std::fs::write(store::bak_path(&path), "{also garbage").unwrap();
        assert_eq!(load(home.path()), LayoutDoc::default());
        assert_eq!(std::fs::read(&path).unwrap(), b"{garbage");
        assert_eq!(std::fs::read(store::bak_path(&path)).unwrap(), b"{also garbage");
    }

    #[test]
    fn missing_file_is_the_defaults_and_stays_missing() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(load(home.path()), LayoutDoc::default());
        assert!(!home.path().join("layout.json").exists());
    }

    #[test]
    fn repair_clamps_sidebar_width_and_drops_unknown_panes() {
        let mut doc = doc_with("a");
        doc.view.sidebar_width = 9000;
        doc.view.panes = vec![Some("gone".into())];
        let fixed = repair(doc);
        assert_eq!(fixed.view.sidebar_width, 400);
        assert_eq!(fixed.view.panes, vec![None::<String>]);
    }

    #[test]
    fn repair_fixes_the_rest_of_the_view() {
        let mut doc = doc_with("a");
        doc.view = ViewLayout {
            panes: vec![Some("a".into()), None],
            arrangement: Some("diagonal".into()),
            ratios: vec![0.5, 0.95],
            active: 5,
            zoomed: Some(2),
            sidebar: "huge".into(),
            sidebar_width: 10,
            ..ViewLayout::default()
        };
        let v = repair(doc).view;
        assert_eq!((v.arrangement, v.ratios, v.active, v.zoomed), (None, vec![], 0, None));
        assert_eq!((v.sidebar.as_str(), v.sidebar_width), ("expanded", 200));
        assert_eq!(v.panes, vec![Some("a".to_string()), None]);

        let mut doc = doc_with("a");
        doc.view.panes = Vec::new();
        assert_eq!(repair(doc.clone()).view.panes, vec![None::<String>]);
        doc.view.panes = vec![Some("a".into()); 6];
        assert_eq!(repair(doc).view.panes.len(), 4);
    }

    #[test]
    fn validate_view_names_the_field() {
        assert_eq!(validate_view(&ViewLayout::default()), Ok(()));
        let bad = |change: fn(&mut ViewLayout)| {
            let mut v = ViewLayout::default();
            change(&mut v);
            validate_view(&v).unwrap_err()
        };
        assert!(bad(|v| v.panes = vec![None; 5]).contains("panes"));
        assert!(bad(|v| v.active = 1).contains("active"));
        assert!(bad(|v| v.zoomed = Some(1)).contains("zoomed"));
        assert!(bad(|v| v.sidebar_width = 100).contains("sidebar_width"));
        assert!(bad(|v| v.sidebar = "x".into()).contains("sidebar"));
        assert!(bad(|v| v.ratios = vec![0.05]).contains("ratios"));
        assert!(bad(|v| v.arrangement = Some("x".into())).contains("arrangement"));
    }

    #[test]
    fn remember_numbers_choices_and_keeps_the_newest_32() {
        let mut doc = LayoutDoc::default();
        for i in 0..40 {
            remember(&mut doc, &format!("C:/work/{i}"), "cmd".into(), None, None);
        }
        assert_eq!(doc.launcher.len(), 32);
        // The eight oldest folders are gone; the newest is numbered 40.
        assert!(!doc.launcher.contains_key("C:/work/7"));
        assert!(doc.launcher.contains_key("C:/work/8"));
        assert_eq!(doc.launcher["C:/work/39"].seq, 40);
    }

    #[test]
    fn remember_replaces_a_folders_choice_and_makes_it_the_newest() {
        let mut doc = LayoutDoc::default();
        remember(&mut doc, "C:/a", "cmd".into(), None, None);
        remember(&mut doc, "C:/b", "cmd".into(), None, None);
        remember(&mut doc, "C:/a", "pwsh".into(), Some("claude".into()), None);
        assert_eq!(doc.launcher.len(), 2);
        let a = &doc.launcher["C:/a"];
        assert_eq!(
            (a.profile_id.as_str(), a.agent.as_deref(), a.seq),
            ("pwsh", Some("claude"), 3)
        );
    }

    #[test]
    fn remember_keeps_safe_modes_only() {
        let mut doc = LayoutDoc::default();
        for (mode, kept) in [
            (SessionMode::Bypass, None),
            (SessionMode::FullAuto, None),
            (SessionMode::Plan, Some(SessionMode::Plan)),
            (SessionMode::AutoEdit, Some(SessionMode::AutoEdit)),
            (SessionMode::Manual, Some(SessionMode::Manual)),
        ] {
            remember(&mut doc, "C:/a", "pwsh".into(), Some("claude".into()), Some(mode));
            assert_eq!(doc.launcher["C:/a"].mode, kept);
        }
    }

    #[test]
    fn a_layout_without_mode_keys_still_parses() {
        let doc: LayoutDoc = serde_json::from_str(
            r#"{"tabs":[{"tab_id":"a","number":1,"name":"A","agent":"claude","profile_id":"cmd","cwd":"C:/w"}],
                "view":{"panes":[null],"arrangement":null,"ratios":[],"active":0,"zoomed":null,"sidebar":"expanded","sidebar_width":280,"shell_hidden":false,"grouped":false},
                "launcher":{"C:/w":{"profile_id":"cmd","agent":null,"seq":1}}}"#,
        )
        .unwrap();
        assert_eq!(doc.tabs[0].mode, None);
        assert_eq!(doc.launcher["C:/w"].mode, None);
    }

    #[test]
    fn a_layout_file_from_before_seq_still_loads() {
        let home = tempfile::tempdir().unwrap();
        let old = r#"{"version":1,"tabs":[],"view":{"panes":[null],"arrangement":null,"ratios":[],"active":0,
            "zoomed":null,"sidebar":"expanded","sidebar_width":288,"shell_hidden":false,"grouped":false},
            "launcher":{"C:/a":{"profile_id":"cmd","agent":null}}}"#;
        std::fs::write(home.path().join("layout.json"), old).unwrap();
        let doc = load(home.path());
        assert_eq!(doc.launcher["C:/a"].seq, 0);
        assert_eq!(doc.launcher["C:/a"].profile_id, "cmd");
    }
}
