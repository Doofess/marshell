//! `~/.marshell/layout.json`: the tabs that were open and how the window was laid out. A missing,
//! broken or newer-version file means the defaults, and `load` never writes.

use crate::store::{self, Versioned};
use marshell_protocol::api::{
    LauncherChoice, LayoutDoc, PaneTab, PaneView, RecentFile, SavedTab, SessionMode, ViewLayout,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

/// The file holds the `LayoutDoc` fields directly, next to `"version"`.
struct FileDoc(LayoutDoc);

impl Serialize for FileDoc {
    /// Writes the document and, in `view`, the three split view fields a build from before slice 6q-2 can't read the
    /// file without: `arrangement` and `zoomed` as null, `ratios` empty. Only the file has them. This build ignores
    /// them when it reads, and an older build ignores `view.pane`.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde_json::Value;

        let mut json = serde_json::to_value(&self.0).map_err(serde::ser::Error::custom)?;
        if let Some(view) = json.get_mut("view").and_then(Value::as_object_mut) {
            view.insert("arrangement".into(), Value::Null);
            view.insert("ratios".into(), Value::Array(Vec::new()));
            view.insert("zoomed".into(), Value::Null);
        }
        json.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FileDoc {
    /// Reads the file's JSON, drops what the side pane can't read (see `lenient_pane`), then parses the rest strictly.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut json = serde_json::Value::deserialize(deserializer)?;
        lenient_pane(&mut json);
        serde_json::from_value::<LayoutDoc>(json)
            .map(FileDoc)
            .map_err(serde::de::Error::custom)
    }
}

/// Drops the parts of `view.pane` that this build can't read, so that a pane written by a later build or edited by
/// hand never costs the rest of the file. Everything outside the pane is left for the strict parse.
fn lenient_pane(doc: &mut serde_json::Value) {
    use serde_json::{Map, Value};

    let Some(view) = doc.get_mut("view").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(pane) = view.get_mut("pane") else {
        return;
    };
    let Some(fields) = pane.as_object_mut() else {
        view.remove("pane");
        return;
    };
    if !fields.get("open").is_some_and(Value::is_boolean) {
        fields.insert("open".into(), Value::Bool(false));
    }
    let width_fits = fields
        .get("width")
        .is_some_and(|w| w.as_u64().is_some_and(|n| u32::try_from(n).is_ok()));
    if !width_fits {
        fields.insert("width".into(), Value::Null);
    }

    if !fields.get("sessions").is_some_and(Value::is_object) {
        fields.insert("sessions".into(), Value::Object(Map::new()));
    }
    if let Some(sessions) = fields.get_mut("sessions").and_then(Value::as_object_mut) {
        sessions.retain(|_, session| lenient_session(session));
    }

    if !fields.get("recent").is_some_and(Value::is_array) {
        fields.insert("recent".into(), Value::Array(Vec::new()));
    }
    if let Some(recent) = fields.get_mut("recent").and_then(Value::as_array_mut) {
        recent.retain(|file| serde_json::from_value::<RecentFile>(file.clone()).is_ok());
    }
}

/// Keeps the tabs of one session that parse. False when the session itself is unreadable.
fn lenient_session(session: &mut serde_json::Value) -> bool {
    use serde_json::Value;

    let Some(fields) = session.as_object_mut() else {
        return false;
    };
    let Some(tabs) = fields.get_mut("tabs").and_then(Value::as_array_mut) else {
        return false;
    };
    tabs.retain(|tab| serde_json::from_value::<PaneTab>(tab.clone()).is_ok());
    if !fields.get("active").is_some_and(Value::is_string) {
        fields.insert("active".into(), Value::Null);
    }
    true
}

impl Versioned for FileDoc {
    const VERSION: u32 = 1;
}

const SIDEBAR_MODES: [&str; 3] = ["expanded", "rail", "focus"];
const SIDEBAR_WIDTH: std::ops::RangeInclusive<u32> = 200..=400;
const PANE_WIDTH: std::ops::RangeInclusive<u32> = 320..=4096;
const MAX_PANE_TABS: usize = 32;
const MAX_PANE_HISTORY: usize = 50;
const MAX_RECENT_FILES: usize = 200;
const MAX_PATH_CHARS: usize = 4096;
const PANE_VIEWS: [&str; 2] = ["rendered", "source"];

/// A missing, broken or newer-version file means the defaults. The file itself is not touched.
/// The side pane's parts that can't be read are dropped and the rest of the file is kept.
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

/// Notes the profile, agent, mode and preset just used in `folder`. Full auto and Bypass are never remembered.
/// The choice gets a number one higher than any other, so the order of use survives a restart;
/// past 32 folders the one used longest ago is dropped.
pub fn remember(
    doc: &mut LayoutDoc,
    folder: &str,
    profile_id: String,
    agent: Option<String>,
    mode: Option<SessionMode>,
    preset: Option<String>,
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
            preset,
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
    // A pane whose tab is gone is shown empty.
    for pane in &mut view.panes {
        if pane
            .as_ref()
            .is_some_and(|id| !doc.tabs.iter().any(|t| &t.tab_id == id))
        {
            *pane = None;
        }
    }
    fold_to_one_pane(view);
    view.sidebar_width = view.sidebar_width.clamp(*SIDEBAR_WIDTH.start(), *SIDEBAR_WIDTH.end());
    if !SIDEBAR_MODES.contains(&view.sidebar.as_str()) {
        view.sidebar = "expanded".into();
    }
    let mut kept: Vec<String> = Vec::new();
    for id in std::mem::take(&mut view.recent_commands) {
        if is_command_id(&id) && !kept.contains(&id) && kept.len() < MAX_RECENT {
            kept.push(id);
        }
    }
    view.recent_commands = kept;
    repair_pane(&mut view.pane, &doc.tabs);
    doc
}

fn is_pane_id(id: &str) -> bool {
    is_command_id(id)
}

fn path_ok(path: &str) -> bool {
    !path.is_empty() && path.chars().count() <= MAX_PATH_CHARS
}

/// Pulls the pane back into range: drops what cannot be drawn and clamps the rest, so one bad part loses nothing else.
fn repair_pane(pane: &mut PaneView, tabs: &[SavedTab]) {
    if pane.width.is_some_and(|w| !PANE_WIDTH.contains(&w)) {
        pane.width = None;
    }
    pane.sessions.retain(|id, _| tabs.iter().any(|t| &t.tab_id == id));
    for st in pane.sessions.values_mut() {
        let mut seen = BTreeSet::new();
        let mut kept: Vec<PaneTab> = Vec::new();
        for mut tab in std::mem::take(&mut st.tabs) {
            if tab.kind != "file" || !is_pane_id(&tab.id) || seen.contains(&tab.id) {
                continue;
            }
            tab.history.retain(|e| path_ok(&e.path));
            if tab.history.is_empty() {
                continue;
            }
            for e in &mut tab.history {
                if !PANE_VIEWS.contains(&e.view.as_str()) {
                    e.view = "rendered".into();
                }
            }
            let dropped = tab.history.len().saturating_sub(MAX_PANE_HISTORY);
            tab.history.drain(..dropped);
            tab.index = (tab.index.saturating_sub(dropped as u32)).min(tab.history.len() as u32 - 1);
            seen.insert(tab.id.clone());
            kept.push(tab);
            if kept.len() == MAX_PANE_TABS {
                break;
            }
        }
        st.tabs = kept;
        if !st.active.as_ref().is_some_and(|a| st.tabs.iter().any(|t| &t.id == a)) {
            st.active = st.tabs.first().map(|t| t.id.clone());
        }
    }
    pane.sessions.retain(|_, st| !st.tabs.is_empty());
    pane.recent.retain(|r| path_ok(&r.path));
    pane.recent.truncate(MAX_RECENT_FILES);
}

/// Split view is gone: a saved split keeps the active pane's tab if it has one, else the first open one,
/// and the view holds that one pane.
fn fold_to_one_pane(view: &mut ViewLayout) {
    let at = |i: u32| view.panes.get(i as usize).and_then(|p| p.clone());
    let kept = at(view.active).or_else(|| view.panes.iter().flatten().next().cloned());
    view.panes = vec![kept];
    view.active = 0;
}

const MAX_RECENT: usize = 5;

fn is_command_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

/// The same rules as `repair`, but a bad value is an error naming the field. Whether a pane's tab
/// exists is not checked here; `repair` empties such a pane when the file is next loaded.
pub fn validate_view(v: &ViewLayout) -> Result<(), String> {
    if v.panes.len() != 1 {
        return Err(format!("panes must hold exactly 1 entry, not {}", v.panes.len()));
    }
    if v.active != 0 {
        return Err(format!("active {} is outside the 1 pane", v.active));
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
    if v.recent_commands.len() > MAX_RECENT || v.recent_commands.iter().any(|id| !is_command_id(id)) {
        return Err("recent_commands must hold at most 5 command ids of a-z, 0-9 and -".into());
    }
    validate_pane(&v.pane)
}

/// The same rules as `repair_pane`, but a bad value is an error naming the field. Whether a session exists is not checked here.
pub fn validate_pane(p: &PaneView) -> Result<(), String> {
    if let Some(w) = p.width.filter(|w| !PANE_WIDTH.contains(w)) {
        return Err(format!(
            "pane.width must be {} to {}, not {w}",
            PANE_WIDTH.start(),
            PANE_WIDTH.end()
        ));
    }
    for (session, st) in &p.sessions {
        if st.tabs.len() > MAX_PANE_TABS {
            return Err(format!(
                "pane.sessions.{session}.tabs holds at most {MAX_PANE_TABS}, not {}",
                st.tabs.len()
            ));
        }
        let mut seen = BTreeSet::new();
        for tab in &st.tabs {
            let at = format!("pane.sessions.{session}.tabs.{}", tab.id);
            if !is_pane_id(&tab.id) || !seen.insert(&tab.id) {
                return Err(format!(
                    "{at}: id must be unique, 1 to 64 of a-z, 0-9 and -, not \"{}\"",
                    tab.id
                ));
            }
            if tab.kind != "file" {
                return Err(format!("{at}: kind must be \"file\", not \"{}\"", tab.kind));
            }
            if tab.history.is_empty() || tab.history.len() > MAX_PANE_HISTORY {
                return Err(format!(
                    "{at}: history must hold 1 to {MAX_PANE_HISTORY} entries, not {}",
                    tab.history.len()
                ));
            }
            if tab.index as usize >= tab.history.len() {
                return Err(format!(
                    "{at}: index {} is outside the {} history entries",
                    tab.index,
                    tab.history.len()
                ));
            }
            for e in &tab.history {
                if !path_ok(&e.path) {
                    return Err(format!("{at}: path must be 1 to {MAX_PATH_CHARS} characters"));
                }
                if !PANE_VIEWS.contains(&e.view.as_str()) {
                    return Err(format!("{at}: view must be one of {PANE_VIEWS:?}, not \"{}\"", e.view));
                }
            }
        }
        let active_ok = match &st.active {
            Some(a) => st.tabs.iter().any(|t| &t.id == a),
            None => st.tabs.is_empty(),
        };
        if !active_ok {
            return Err(format!(
                "pane.sessions.{session}.active must name one of its tabs (or be null with no tabs)"
            ));
        }
    }
    if p.recent.len() > MAX_RECENT_FILES || p.recent.iter().any(|r| !path_ok(&r.path)) {
        return Err(format!(
            "pane.recent must hold at most {MAX_RECENT_FILES} files with a path of 1 to {MAX_PATH_CHARS} characters"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::{PaneEntry, PaneTab, PaneView, RecentFile, SavedTab, SessionTabs};

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
            elevated: false,
            preset: None,
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

    /// The view as a build from slice 6q-1 reads it: every field here is one it can't do without.
    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct ViewBeforeThePane {
        panes: Vec<Option<String>>,
        arrangement: Option<String>,
        ratios: Vec<f32>,
        active: u32,
        zoomed: Option<u32>,
        sidebar: String,
        sidebar_width: u32,
        shell_hidden: bool,
        grouped: bool,
    }

    #[test]
    fn the_file_keeps_the_split_fields_an_older_build_reads() {
        let home = tempfile::tempdir().unwrap();
        let mut doc = doc_with("a");
        doc.view.pane.open = true;
        save(home.path(), &doc).unwrap();
        let on_disk: serde_json::Value =
            serde_json::from_slice(&std::fs::read(home.path().join("layout.json")).unwrap()).unwrap();
        let view = on_disk["view"].as_object().unwrap();
        assert_eq!(view.get("arrangement"), Some(&serde_json::Value::Null));
        assert_eq!(view.get("ratios"), Some(&serde_json::json!([])));
        assert_eq!(view.get("zoomed"), Some(&serde_json::Value::Null));
        let old: ViewBeforeThePane = serde_json::from_value(on_disk["view"].clone()).unwrap();
        assert_eq!(old.panes, vec![Some("a".to_string())]);
        assert!(old.ratios.is_empty());
        // Only the file carries them: the document the API sends has no such fields, and reading the file back gives the same document.
        let sent = serde_json::to_value(&doc).unwrap();
        assert!(sent["view"].get("ratios").is_none() && sent["view"].get("arrangement").is_none());
        assert_eq!(load(home.path()), doc);
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

    fn doc_with_tabs(ids: &[&str], view: ViewLayout) -> LayoutDoc {
        LayoutDoc {
            tabs: ids.iter().map(|i| tab(i)).collect(),
            view,
            ..LayoutDoc::default()
        }
    }

    fn some(id: &str) -> Option<String> {
        Some(id.to_string())
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
            active: 5,
            sidebar: "huge".into(),
            sidebar_width: 10,
            ..ViewLayout::default()
        };
        let v = repair(doc).view;
        assert_eq!(v.active, 0);
        assert_eq!((v.sidebar.as_str(), v.sidebar_width), ("expanded", 200));
        assert_eq!(v.panes, vec![Some("a".to_string())]);

        let mut doc = doc_with("a");
        doc.view.panes = Vec::new();
        assert_eq!(repair(doc.clone()).view.panes, vec![None::<String>]);
        doc.view.panes = vec![Some("a".into()); 6];
        assert_eq!(repair(doc).view.panes, vec![Some("a".to_string())]);
    }

    #[test]
    fn repair_folds_a_split_to_the_active_session() {
        let doc = doc_with_tabs(
            &["a", "b", "c"],
            ViewLayout {
                panes: vec![some("a"), some("b"), some("c")],
                active: 1,
                ..ViewLayout::default()
            },
        );
        assert_eq!(repair(doc).view.panes, vec![some("b")]);
    }

    #[test]
    fn repair_folds_past_an_empty_or_closed_focused_pane() {
        let doc = doc_with_tabs(
            &["b", "c"],
            ViewLayout {
                panes: vec![some("a"), None, some("c")],
                active: 0,
                ..ViewLayout::default()
            },
        );
        assert_eq!(repair(doc).view.panes, vec![some("c")]);
    }

    #[test]
    fn repair_folds_a_split_with_no_open_tab_to_one_empty_pane() {
        let doc = doc_with_tabs(
            &[],
            ViewLayout {
                panes: vec![some("x"), some("y")],
                ..ViewLayout::default()
            },
        );
        let v = repair(doc).view;
        assert_eq!(v.panes, vec![None::<String>]);
        assert_eq!(v.active, 0);
    }

    #[test]
    fn repair_trims_recent_commands() {
        let mut doc = LayoutDoc::default();
        doc.view.recent_commands = [
            "close",
            "close",
            "Bad Id",
            "",
            "find",
            "rename",
            "history",
            "focus-mode",
            "split",
            "preview",
        ]
        .map(String::from)
        .to_vec();
        let fixed = repair(doc);
        assert_eq!(
            fixed.view.recent_commands,
            ["close", "find", "rename", "history", "focus-mode"]
        );
    }

    #[test]
    fn validate_refuses_a_bad_recent_command() {
        let bad = |ids: Vec<String>| {
            let v = ViewLayout {
                recent_commands: ids,
                ..ViewLayout::default()
            };
            validate_view(&v).unwrap_err()
        };
        assert!(bad(vec!["a b".into()]).contains("recent_commands"));
        assert!(bad(vec!["x".into(); 6]).contains("recent_commands"));
    }

    #[test]
    fn validate_view_names_the_field() {
        assert_eq!(validate_view(&ViewLayout::default()), Ok(()));
        let bad = |change: fn(&mut ViewLayout)| {
            let mut v = ViewLayout::default();
            change(&mut v);
            validate_view(&v).unwrap_err()
        };
        assert!(bad(|v| v.panes = Vec::new()).contains("panes"));
        assert!(bad(|v| v.active = 1).contains("active"));
        assert!(bad(|v| v.sidebar_width = 100).contains("sidebar_width"));
        assert!(bad(|v| v.sidebar = "x".into()).contains("sidebar"));
    }

    #[test]
    fn validate_view_refuses_a_split() {
        let bad = |change: fn(&mut ViewLayout)| {
            let mut v = ViewLayout::default();
            change(&mut v);
            validate_view(&v).unwrap_err()
        };
        assert_eq!(validate_view(&ViewLayout::default()), Ok(()));
        assert!(bad(|v| v.panes = vec![None, None]).contains("panes"));
    }

    fn file_tab(id: &str, paths: &[&str]) -> PaneTab {
        PaneTab {
            id: id.into(),
            kind: "file".into(),
            history: paths
                .iter()
                .map(|p| PaneEntry {
                    path: (*p).into(),
                    line: None,
                    col: None,
                    view: "rendered".into(),
                })
                .collect(),
            index: 0,
        }
    }

    fn session(tabs: Vec<PaneTab>) -> SessionTabs {
        let active = tabs.first().map(|t| t.id.clone());
        SessionTabs { tabs, active }
    }

    fn pane_with(id: &str, st: SessionTabs) -> PaneView {
        let mut pane = PaneView::default();
        pane.sessions.insert(id.into(), st);
        pane
    }

    fn repaired(id: &str, st: SessionTabs) -> PaneView {
        let mut doc = doc_with(id);
        doc.view.pane = pane_with(id, st);
        repair(doc).view.pane
    }

    #[test]
    fn repair_drops_a_session_that_has_no_saved_tab() {
        let mut doc = doc_with("a");
        doc.view.pane = pane_with("a", session(vec![file_tab("t1", &["C:/a.md"])]));
        doc.view
            .pane
            .sessions
            .insert("gone".into(), session(vec![file_tab("t1", &["C:/a.md"])]));
        let pane = repair(doc).view.pane;
        assert_eq!(pane.sessions.keys().collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn repair_drops_a_tab_of_an_unknown_kind() {
        let mut t2 = file_tab("t2", &["C:/b.md"]);
        t2.kind = "browser".into();
        let pane = repaired("a", session(vec![file_tab("t1", &["C:/a.md"]), t2]));
        let ids: Vec<_> = pane.sessions["a"].tabs.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["t1"]);
    }

    #[test]
    fn repair_drops_duplicate_ids_and_empty_histories() {
        let first = file_tab("t1", &["C:/first.md"]);
        let tabs = vec![first.clone(), file_tab("t1", &["C:/second.md"]), file_tab("t2", &[])];
        let pane = repaired("a", session(tabs));
        assert_eq!(pane.sessions["a"].tabs, vec![first]);
    }

    #[test]
    fn repair_clamps_the_index_and_the_history() {
        let paths: Vec<String> = (0..60).map(|i| format!("C:/f{i}.md")).collect();
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        let mut long = file_tab("t1", &refs);
        long.index = 59;
        let mut short = file_tab("t2", &["C:/a", "C:/b", "C:/c"]);
        short.index = 99;
        let pane = repaired("a", session(vec![long, short]));
        let tabs = &pane.sessions["a"].tabs;
        assert_eq!(tabs[0].history.len(), 50);
        assert_eq!(tabs[0].history[0].path, "C:/f10.md");
        assert_eq!(tabs[0].index, 49);
        assert_eq!(tabs[1].index, 2);
    }

    #[test]
    fn repair_keeps_at_most_32_tabs() {
        let tabs: Vec<PaneTab> = (0..40).map(|i| file_tab(&format!("t{i}"), &["C:/a"])).collect();
        let pane = repaired("a", session(tabs));
        let kept = &pane.sessions["a"].tabs;
        assert_eq!(kept.len(), 32);
        assert_eq!(kept[31].id, "t31");
    }

    #[test]
    fn repair_points_active_at_a_tab_that_exists() {
        let mut st = session(vec![file_tab("t1", &["C:/a"]), file_tab("t2", &["C:/b"])]);
        st.active = Some("nope".into());
        let pane = repaired("a", st);
        assert_eq!(pane.sessions["a"].active.as_deref(), Some("t1"));

        let mut empty = session(vec![]);
        empty.active = Some("x".into());
        let pane = repaired("a", empty);
        assert!(pane.sessions.is_empty());
    }

    #[test]
    fn repair_resets_a_width_out_of_range() {
        for (given, kept) in [
            (Some(100), None),
            (Some(9000), None),
            (Some(480), Some(480)),
            (None, None),
        ] {
            let mut doc = doc_with("a");
            doc.view.pane.width = given;
            assert_eq!(repair(doc).view.pane.width, kept);
        }
    }

    #[test]
    fn repair_fixes_an_unknown_view() {
        let mut t = file_tab("t1", &["C:/a"]);
        t.history[0].view = "tree".into();
        let pane = repaired("a", session(vec![t]));
        assert_eq!(pane.sessions["a"].tabs[0].history[0].view, "rendered");
    }

    #[test]
    fn repair_trims_recent_files() {
        let mut doc = doc_with("a");
        doc.view.pane.recent = (0..250)
            .map(|i| RecentFile {
                project: "C:/p".into(),
                path: if i == 3 { String::new() } else { format!("C:/p/{i}.md") },
            })
            .collect();
        let recent = repair(doc).view.pane.recent;
        assert!(recent.len() <= 200);
        assert!(recent.iter().all(|r| !r.path.is_empty()));
    }

    #[test]
    fn validate_view_names_the_pane_field() {
        let with = |change: fn(&mut PaneView)| {
            let mut v = ViewLayout::default();
            change(&mut v.pane);
            validate_view(&v).unwrap_err()
        };
        let one = |change: fn(&mut PaneTab)| {
            let mut v = ViewLayout::default();
            let mut t = file_tab("t1", &["C:/a", "C:/b"]);
            change(&mut t);
            v.pane = pane_with("a", session(vec![t]));
            validate_view(&v).unwrap_err()
        };
        assert!(with(|p| p.width = Some(100)).contains("width"));
        assert!(
            with(|p| {
                let tabs = (0..33).map(|i| file_tab(&format!("t{i}"), &["C:/a"])).collect();
                p.sessions.insert("a".into(), session(tabs));
            })
            .contains("tabs")
        );
        assert!(one(|t| t.id = "A b".into()).contains("id"));
        assert!(one(|t| t.kind = "browser".into()).contains("kind"));
        assert!(one(|t| t.history.clear()).contains("history"));
        assert!(one(|t| t.index = 3).contains("index"));
        assert!(one(|t| t.history[0].view = "tree".into()).contains("view"));
        assert!(one(|t| t.history[0].path = String::new()).contains("path"));
        assert!(one(|t| t.history[0].path = "x".repeat(4097)).contains("path"));
        assert!(
            with(|p| {
                let mut st = session(vec![file_tab("t1", &["C:/a"])]);
                st.active = Some("nope".into());
                p.sessions.insert("a".into(), st);
            })
            .contains("active")
        );
        assert!(
            with(|p| {
                let mut st = session(vec![file_tab("t1", &["C:/a"])]);
                st.active = None;
                p.sessions.insert("a".into(), st);
            })
            .contains("active")
        );
        assert!(
            with(|p| {
                p.recent = (0..201)
                    .map(|i| RecentFile {
                        project: "C:/p".into(),
                        path: format!("C:/p/{i}"),
                    })
                    .collect();
            })
            .contains("recent")
        );

        assert_eq!(validate_view(&ViewLayout::default()), Ok(()));
        let v = ViewLayout {
            pane: pane_with("a", session(vec![file_tab("t1", &["C:/a"])])),
            ..ViewLayout::default()
        };
        assert_eq!(validate_view(&v), Ok(()));
    }

    #[test]
    fn remember_numbers_choices_and_keeps_the_newest_32() {
        let mut doc = LayoutDoc::default();
        for i in 0..40 {
            remember(&mut doc, &format!("C:/work/{i}"), "cmd".into(), None, None, None);
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
        remember(&mut doc, "C:/a", "cmd".into(), None, None, None);
        remember(&mut doc, "C:/b", "cmd".into(), None, None, None);
        remember(&mut doc, "C:/a", "pwsh".into(), Some("claude".into()), None, None);
        assert_eq!(doc.launcher.len(), 2);
        let a = &doc.launcher["C:/a"];
        assert_eq!(
            (a.profile_id.as_str(), a.agent.as_deref(), a.seq),
            ("pwsh", Some("claude"), 3)
        );
    }

    #[test]
    fn remember_keeps_the_preset_and_an_old_layout_reads_without_one() {
        let mut doc = LayoutDoc::default();
        remember(
            &mut doc,
            "C:/a",
            "pwsh".into(),
            Some("claude".into()),
            None,
            Some("deepseek".into()),
        );
        assert_eq!(doc.launcher["C:/a"].preset.as_deref(), Some("deepseek"));
        // A layout.json from before the field existed has no `preset` on a tab or on a launcher choice.
        let old = r#"{"tabs":[{"tab_id":"a","number":1,"name":"A","agent":"claude","profile_id":"cmd","cwd":"C:/w"}],
            "view":{"panes":[null],"arrangement":null,"ratios":[],"active":0,"zoomed":null,"sidebar":"expanded","sidebar_width":280,"shell_hidden":false,"grouped":false},
            "launcher":{"C:/w":{"profile_id":"cmd","agent":"claude","seq":1}}}"#;
        let doc: LayoutDoc = serde_json::from_str(old).unwrap();
        assert_eq!(doc.tabs[0].preset, None);
        assert_eq!(doc.launcher["C:/w"].preset, None);
        // And a tab without a preset writes none.
        assert!(!serde_json::to_string(&doc).unwrap().contains("preset"));
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
            remember(&mut doc, "C:/a", "pwsh".into(), Some("claude".into()), Some(mode), None);
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

    /// Writes a layout file with one saved tab `ta`, the rail sidebar at 304 and the given pane
    /// and sidebar width, loads it from a temp home and returns the document.
    fn load_with_pane(pane: serde_json::Value, sidebar_width: serde_json::Value) -> LayoutDoc {
        let home = tempfile::tempdir().unwrap();
        let file = serde_json::json!({
            "version": 1,
            "tabs": [{"tab_id": "ta", "number": 1, "name": "A", "agent": null, "profile_id": "cmd", "cwd": "C:/w"}],
            "view": {
                "panes": ["ta"], "active": 0, "sidebar": "rail", "sidebar_width": sidebar_width,
                "shell_hidden": false, "grouped": false, "pane": pane
            },
            "launcher": {}
        });
        std::fs::write(home.path().join("layout.json"), file.to_string()).unwrap();
        load(home.path())
    }

    fn good_file_tab(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id, "kind": "file", "index": 0,
            "history": [{"path": "C:\\a.md", "line": null, "col": null, "view": "rendered"}]
        })
    }

    fn assert_tabs_and_sidebar_kept(doc: &LayoutDoc) {
        assert_eq!(doc.tabs.len(), 1);
        assert_eq!(doc.tabs[0].tab_id, "ta");
        assert_eq!(doc.view.sidebar, "rail");
        assert_eq!(doc.view.sidebar_width, 304);
    }

    #[test]
    fn a_tab_of_another_builds_shape_is_dropped_and_the_file_still_loads() {
        let pane = serde_json::json!({
            "open": true, "width": 480, "recent": [],
            "sessions": {"ta": {
                "tabs": [good_file_tab("t1"), {"id": "t2", "kind": "browser", "url": "https://example.com/"}],
                "active": "t2"
            }}
        });
        let doc = load_with_pane(pane, 304.into());
        assert_tabs_and_sidebar_kept(&doc);
        let session = &doc.view.pane.sessions["ta"];
        assert_eq!(session.tabs.len(), 1);
        assert_eq!(session.tabs[0].id, "t1");
        assert_eq!(session.active, some("t1"));
    }

    #[test]
    fn a_pane_with_values_of_the_wrong_type_loses_only_those_values() {
        let pane = serde_json::json!({
            "open": "yes", "width": 480.5,
            "recent": [{"project": "p", "path": "C:\\a.md"}, 7, {"path": 3}],
            "sessions": {"ta": {
                "tabs": [good_file_tab("t1"), {"id": "t2", "kind": "file", "index": -1, "history": []}],
                "active": 5
            }}
        });
        let doc = load_with_pane(pane, 304.into());
        assert_tabs_and_sidebar_kept(&doc);
        let pane = &doc.view.pane;
        assert!(!pane.open);
        assert_eq!(pane.width, None);
        assert_eq!(pane.recent.len(), 1);
        assert_eq!(pane.recent[0].project, "p");
        let session = &pane.sessions["ta"];
        assert_eq!(session.tabs.len(), 1);
        assert_eq!(session.tabs[0].id, "t1");
        assert_eq!(session.active, some("t1"));
    }

    #[test]
    fn a_pane_that_is_not_an_object_reads_as_an_empty_pane() {
        let doc = load_with_pane(serde_json::json!([]), 304.into());
        assert_tabs_and_sidebar_kept(&doc);
        assert_eq!(doc.view.pane, PaneView::default());

        let pane = serde_json::json!({"open": true, "sessions": {"ta": {"tabs": "x", "active": null}}});
        let doc = load_with_pane(pane, 304.into());
        assert_tabs_and_sidebar_kept(&doc);
        assert!(doc.view.pane.open);
        assert!(doc.view.pane.sessions.is_empty());
    }

    #[test]
    fn a_broken_part_outside_the_pane_still_means_the_defaults() {
        let doc = load_with_pane(serde_json::json!({"open": true}), "wide".into());
        assert_eq!(doc, LayoutDoc::default());
    }
}
