//! `~/.marshell/layout.json`: the tabs that were open and how the window was laid out. A missing,
//! broken or newer-version file means the defaults, and `load` never writes.

use crate::github::actions::valid_node_id;
use crate::store::{self, Versioned};
use marshell_protocol::api::{
    ChangesDrafts, DraftComment, LauncherChoice, LayoutDoc, PaneTab, PaneView, Panels, PickedComment, RecentFile,
    SavedTab, SentComment, SessionMode, TreeState, TreeView, ViewLayout,
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

    if fields
        .get("panels")
        .is_some_and(|p| serde_json::from_value::<Panels>(p.clone()).is_err())
    {
        let kept = fields.get_mut("panels").is_some_and(lenient_panels);
        if !kept {
            fields.remove("panels");
        }
    }

    if let Some(tree) = fields.get_mut("tree")
        && serde_json::from_value::<TreeView>(tree.clone()).is_err()
    {
        // A width that can't be read is dropped alone; a tree that is still unreadable goes whole.
        if let Some(t) = tree.as_object_mut()
            && t.get("width")
                .is_some_and(|w| !w.as_u64().is_some_and(|n| u32::try_from(n).is_ok()))
        {
            t.insert("width".into(), Value::Null);
        }
        if serde_json::from_value::<TreeView>(tree.clone()).is_err() {
            fields.remove("tree");
        }
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

/// Drops from the saved stack the weights and the fill that can't be read, one by one. False when the stack itself is
/// unreadable, which is when its `shown` isn't a list of strings.
fn lenient_panels(panels: &mut serde_json::Value) -> bool {
    use serde_json::Value;

    let Some(fields) = panels.as_object_mut() else {
        return false;
    };
    let shown_ok = fields
        .get("shown")
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().all(Value::is_string));
    if !shown_ok {
        return false;
    }
    match fields.get_mut("weights") {
        Some(Value::Object(weights)) => {
            weights.retain(|_, w| w.as_u64().is_some_and(|n| u32::try_from(n).is_ok()));
        }
        Some(_) => {
            fields.remove("weights");
        }
        None => {}
    }
    if fields.get("filled").is_some_and(|f| !f.is_string()) {
        fields.remove("filled");
    }
    true
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
    for tab in tabs.iter_mut() {
        // A viewport that is not a known size is dropped alone; the tab stays.
        if let Some(t) = tab.as_object_mut() {
            if t.get("viewport").is_some_and(|v| !v.as_str().is_some_and(is_viewport)) {
                t.remove("viewport");
            }
            if t.get("preview").is_some_and(|p| !p.is_boolean()) {
                t.remove("preview");
            }
        }
    }
    tabs.retain(|tab| serde_json::from_value::<PaneTab>(tab.clone()).is_ok());
    if !fields.get("active").is_some_and(Value::is_string) {
        fields.insert("active".into(), Value::Null);
    }
    if fields.get("active_browser").is_some_and(|a| !a.is_string()) {
        fields.remove("active_browser");
    }
    if let Some(tree) = fields.get_mut("tree")
        && !lenient_tree_state(tree)
    {
        fields.remove("tree");
    }
    if let Some(changes) = fields.get_mut("changes")
        && !lenient_changes(changes)
    {
        fields.remove("changes");
    }
    true
}

/// Keeps the comments of a session's saved Changes drafts that parse, and the folds whose value is a boolean, so that
/// one wrongly typed draft never reaches the strict parse. False when the drafts can't be read: no `checkout` string.
fn lenient_changes(changes: &mut serde_json::Value) -> bool {
    use serde_json::{Map, Value};

    let Some(fields) = changes.as_object_mut() else {
        return false;
    };
    if !fields.get("checkout").is_some_and(Value::is_string) {
        return false;
    }
    if fields.get("base").is_some_and(|b| !b.is_string()) {
        fields.insert("base".into(), Value::Null);
    }
    if !fields.get("comments").is_some_and(Value::is_array) {
        fields.insert("comments".into(), Value::Array(Vec::new()));
    }
    if let Some(comments) = fields.get_mut("comments").and_then(Value::as_array_mut) {
        comments.retain(|c| serde_json::from_value::<DraftComment>(c.clone()).is_ok());
    }
    if !fields.get("picked").is_some_and(Value::is_array) {
        fields.insert("picked".into(), Value::Array(Vec::new()));
    }
    if let Some(picked) = fields.get_mut("picked").and_then(Value::as_array_mut) {
        picked.retain(|p| serde_json::from_value::<PickedComment>(p.clone()).is_ok());
    }
    if !fields.get("sent").is_some_and(Value::is_array) {
        fields.insert("sent".into(), Value::Array(Vec::new()));
    }
    if let Some(sent) = fields.get_mut("sent").and_then(Value::as_array_mut) {
        sent.retain(|s| serde_json::from_value::<SentComment>(s.clone()).is_ok());
    }
    if !fields.get("folds").is_some_and(Value::is_object) {
        fields.insert("folds".into(), Value::Object(Map::new()));
    }
    if let Some(folds) = fields.get_mut("folds").and_then(Value::as_object_mut) {
        folds.retain(|_, v| v.is_boolean());
    }
    true
}

/// Drops the non-string folders of a saved tree. False when the tree still can't be read.
fn lenient_tree_state(tree: &mut serde_json::Value) -> bool {
    use serde_json::Value;

    if let Some(Value::Array(expanded)) = tree.get_mut("expanded") {
        expanded.retain(Value::is_string);
    }
    serde_json::from_value::<TreeState>(tree.clone()).is_ok()
}

/// A folder of a saved tree: relative to the root, "/" separated, no `..`.
pub fn tree_folder_ok(f: &str) -> bool {
    !f.is_empty()
        && f.chars().count() <= MAX_PATH_CHARS
        && !f.contains(['\\', ':'])
        && !f.starts_with('/')
        && f.split('/').all(|c| !matches!(c, "" | "." | ".."))
}

impl Versioned for FileDoc {
    const VERSION: u32 = 1;
}

const SIDEBAR_MODES: [&str; 3] = ["expanded", "rail", "focus"];
const SIDEBAR_WIDTH: std::ops::RangeInclusive<u32> = 200..=400;
const PANE_WIDTH: std::ops::RangeInclusive<u32> = 320..=4096;
pub(crate) const MAX_PANE_TABS: usize = 32;
const MAX_PANE_HISTORY: usize = 50;
const MAX_RECENT_FILES: usize = 200;
const MAX_PATH_CHARS: usize = 4096;
const MAX_TREE_FOLDERS: usize = 500;
const MAX_BASE_CHARS: usize = 200;
const MAX_COMMENTS: usize = 50;
const MAX_PICKED: usize = 200;
const MAX_SENT: usize = 500;
const MAX_SENT_COUNT: u32 = 100_000;
const MAX_PICKED_NOTE: usize = 2000;

fn picked_note_ok(note: &str) -> bool {
    note.chars().count() <= MAX_PICKED_NOTE && !note.chars().any(|ch| ch.is_control() && !matches!(ch, '\n' | '\t'))
}
const MAX_COMMENT_ROWS: u32 = 100_000;
const MAX_COMMENT_HASH: usize = 16;
const MAX_COMMENT_QUOTE_LINES: usize = 3;
/// A quote line is a diff sign and up to 200 characters.
const MAX_COMMENT_QUOTE_CHARS: usize = 201;
const MAX_COMMENT_TEXT: usize = 2000;
const MAX_FOLDS: usize = 1000;
const TREE_WIDTH: std::ops::RangeInclusive<u32> = 160..=2048;
const PANE_VIEWS: [&str; 2] = ["rendered", "source"];
/// The side pane's panels, top to bottom.
const PANEL_IDS: [&str; 3] = ["browser", "files", "changes"];
const PANEL_WEIGHT: std::ops::RangeInclusive<u32> = 1..=100_000;

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

fn base_ok(base: &str) -> bool {
    base.chars().count() <= MAX_BASE_CHARS && !base.chars().any(char::is_control)
}

/// The field of a saved comment that breaks a bound, if one does. The caller checks that ids are unique.
fn comment_problem(c: &DraftComment) -> Option<&'static str> {
    let lower_hex = |s: &str| {
        (1..=MAX_COMMENT_HASH).contains(&s.len()) && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    };
    if !is_pane_id(&c.id) {
        Some("id")
    } else if !tree_folder_ok(&c.path) {
        Some("path")
    } else if c.start < 1 || c.end < c.start {
        Some("start")
    } else if !(1..=MAX_COMMENT_ROWS).contains(&c.rows) {
        Some("rows")
    } else if !lower_hex(&c.first) {
        Some("first")
    } else if !lower_hex(&c.hash) {
        Some("hash")
    } else if c.quote.len() > MAX_COMMENT_QUOTE_LINES
        || c.quote
            .iter()
            .any(|q| q.chars().count() > MAX_COMMENT_QUOTE_CHARS || q.chars().any(|ch| ch.is_control() && ch != '\t'))
    {
        Some("quote")
    } else if !(1..=MAX_COMMENT_TEXT).contains(&c.text.chars().count())
        || c.text.chars().any(|ch| ch.is_control() && !matches!(ch, '\n' | '\t'))
    {
        Some("text")
    } else {
        None
    }
}

/// The saved Changes drafts of one session, with the same rules as `repair_pane`, and errors naming the field.
fn validate_changes(session: &str, d: &ChangesDrafts) -> Result<(), String> {
    let at = format!("pane.sessions.{session}.changes");
    if !(1..=MAX_PATH_CHARS).contains(&d.checkout.chars().count()) {
        return Err(format!("{at}.checkout must be 1 to {MAX_PATH_CHARS} characters"));
    }
    if d.base.as_deref().is_some_and(|b| !base_ok(b)) {
        return Err(format!(
            "{at}.base must be at most {MAX_BASE_CHARS} characters, with no control character"
        ));
    }
    if d.comments.len() > MAX_COMMENTS {
        return Err(format!(
            "{at}.comments holds at most {MAX_COMMENTS}, not {}",
            d.comments.len()
        ));
    }
    let mut seen = BTreeSet::new();
    for c in &d.comments {
        if !seen.insert(&c.id) {
            return Err(format!("{at}.comments.{}.id is used twice", c.id));
        }
        if let Some(field) = comment_problem(c) {
            let rule = match field {
                "id" => "must be 1 to 64 of a-z, 0-9 and -",
                "path" => "isn't relative with / separators",
                "start" => "must be 1 or more, and the end not before it",
                "rows" => "must be 1 to 100000",
                "first" | "hash" => "must be 1 to 16 of 0-9 and a-f",
                "quote" => "holds at most 3 lines of 201 characters, with no control character but a tab",
                _ => "must be 1 to 2000 characters, with no control character but a newline and a tab",
            };
            return Err(format!("{at}.comments.{}.{field} {rule}", c.id));
        }
    }
    if d.picked.len() > MAX_PICKED {
        return Err(format!(
            "{at}.picked holds at most {MAX_PICKED}, not {}",
            d.picked.len()
        ));
    }
    let mut seen = BTreeSet::new();
    for p in &d.picked {
        if !seen.insert(&p.id) {
            return Err(format!("{at}.picked.{}.id is used twice", p.id));
        }
        if !valid_node_id(&p.id) {
            return Err(format!(
                "{at}.picked.{}.id must be 1 to 100 of A-Z, a-z, 0-9, _, = and -",
                p.id
            ));
        }
        if p.number < 1 {
            return Err(format!("{at}.picked.{}.number must be 1 or more", p.id));
        }
        if !picked_note_ok(&p.note) {
            return Err(format!(
                "{at}.picked.{}.note must be at most 2000 characters, with no control character but a newline and a tab",
                p.id
            ));
        }
    }
    if d.sent.len() > MAX_SENT {
        return Err(format!("{at}.sent holds at most {MAX_SENT}, not {}", d.sent.len()));
    }
    let mut seen = BTreeSet::new();
    for s in &d.sent {
        if !seen.insert(&s.id) {
            return Err(format!("{at}.sent.{}.id is used twice", s.id));
        }
        if !valid_node_id(&s.id) {
            return Err(format!(
                "{at}.sent.{}.id must be 1 to 100 of A-Z, a-z, 0-9, _, = and -",
                s.id
            ));
        }
        if s.number < 1 {
            return Err(format!("{at}.sent.{}.number must be 1 or more", s.id));
        }
        if !(1..=MAX_SENT_COUNT).contains(&s.count) {
            return Err(format!("{at}.sent.{}.count must be 1 to {MAX_SENT_COUNT}", s.id));
        }
    }
    if d.folds.len() > MAX_FOLDS {
        return Err(format!("{at}.folds holds at most {MAX_FOLDS} files"));
    }
    if d.folds.keys().any(|f| !tree_folder_ok(f)) {
        return Err(format!("{at}.folds holds a path that isn't relative"));
    }
    Ok(())
}

fn path_ok(path: &str) -> bool {
    !path.is_empty() && path.chars().count() <= MAX_PATH_CHARS
}

const MAX_TITLE_CHARS: usize = 200;
/// A place's `top`, in ten-thousandths of the page's height.
const MAX_PLACE_TOP: u32 = 10_000;

/// A page a browser tab may hold: a blank page or an http(s) address of at most `MAX_PATH_CHARS`.
fn is_page_address(s: &str) -> bool {
    s.chars().count() <= MAX_PATH_CHARS && (s == "about:blank" || s.starts_with("http://") || s.starts_with("https://"))
}

/// Pulls the stack of panels back into range: known panels in order, good weights, a fill that can be.
fn repair_panels(panels: &mut Option<Panels>) {
    let Some(p) = panels else {
        return;
    };
    let shown: Vec<String> = PANEL_IDS
        .iter()
        .filter(|id| p.shown.iter().any(|s| s == *id))
        .map(|id| id.to_string())
        .collect();
    if shown.is_empty() {
        *panels = None;
        return;
    }
    p.weights
        .retain(|k, v| PANEL_IDS.contains(&k.as_str()) && PANEL_WEIGHT.contains(v));
    if !(shown.len() >= 2 && p.filled.as_ref().is_some_and(|f| shown.contains(f))) {
        p.filled = None;
    }
    p.shown = shown;
}

/// Pulls the pane back into range: drops what cannot be drawn and clamps the rest, so one bad part loses nothing else.
fn repair_pane(pane: &mut PaneView, tabs: &[SavedTab]) {
    if pane.width.is_some_and(|w| !PANE_WIDTH.contains(&w)) {
        pane.width = None;
    }
    repair_panels(&mut pane.panels);
    pane.sessions.retain(|id, _| tabs.iter().any(|t| &t.tab_id == id));
    for st in pane.sessions.values_mut() {
        let mut seen = BTreeSet::new();
        let mut kept: Vec<PaneTab> = Vec::new();
        for mut tab in std::mem::take(&mut st.tabs) {
            if !matches!(tab.kind.as_str(), "file" | "browser") || !is_pane_id(&tab.id) || seen.contains(&tab.id) {
                continue;
            }
            if !tab.opened_by.as_deref().is_some_and(is_opener) {
                tab.opened_by = None;
                tab.unseen = false;
            }
            if tab.viewport.as_deref().is_some_and(|v| !is_viewport(v)) {
                tab.viewport = None;
            }
            if tab.kind == "browser" {
                // A browser tab holds one address: the entry it was showing.
                let at = (tab.index as usize).min(tab.history.len().saturating_sub(1));
                let Some(mut e) = (at < tab.history.len()).then(|| tab.history.swap_remove(at)) else {
                    continue;
                };
                if !is_page_address(&e.path) {
                    continue;
                }
                e.line = None;
                e.col = None;
                e.view = "rendered".into();
                tab.history = vec![e];
                tab.index = 0;
                tab.title = tab.title.take().map(|t| t.chars().take(MAX_TITLE_CHARS).collect());
                seen.insert(tab.id.clone());
                kept.push(tab);
                if kept.len() == MAX_PANE_TABS {
                    break;
                }
                continue;
            }
            tab.title = None;
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
        if !st
            .active_browser
            .as_ref()
            .is_some_and(|a| st.tabs.iter().any(|t| &t.id == a))
        {
            st.active_browser = None;
        }
        if !st.active.as_ref().is_some_and(|a| st.tabs.iter().any(|t| &t.id == a)) {
            st.active = st.tabs.first().map(|t| t.id.clone());
        }
    }
    for st in pane.sessions.values_mut() {
        if let Some(tree) = &mut st.tree {
            if tree.root.chars().count() > MAX_PATH_CHARS {
                // The expanded folders were relative to it.
                tree.root.clear();
                tree.expanded.clear();
            }
            tree.expanded.retain(|f| tree_folder_ok(f));
            tree.expanded.truncate(MAX_TREE_FOLDERS);
        }
    }
    for st in pane.sessions.values_mut() {
        if let Some(drafts) = &mut st.changes {
            if !(1..=MAX_PATH_CHARS).contains(&drafts.checkout.chars().count()) {
                // The comments were made in that checkout.
                st.changes = None;
                continue;
            }
            if drafts.base.as_deref().is_some_and(|b| !base_ok(b)) {
                drafts.base = None;
            }
            let mut seen = BTreeSet::new();
            drafts
                .comments
                .retain(|c| comment_problem(c).is_none() && seen.insert(c.id.clone()));
            drafts.comments.truncate(MAX_COMMENTS);
            let mut seen = BTreeSet::new();
            drafts.picked.retain(|p| {
                valid_node_id(&p.id) && p.number >= 1 && picked_note_ok(&p.note) && seen.insert(p.id.clone())
            });
            drafts.picked.truncate(MAX_PICKED);
            let mut seen = BTreeSet::new();
            drafts.sent.retain(|s| {
                valid_node_id(&s.id)
                    && s.number >= 1
                    && (1..=MAX_SENT_COUNT).contains(&s.count)
                    && seen.insert(s.id.clone())
            });
            drafts.sent.truncate(MAX_SENT);
            drafts.folds.retain(|f, _| tree_folder_ok(f));
            if drafts.folds.len() > MAX_FOLDS {
                let keep: BTreeSet<String> = drafts.folds.keys().take(MAX_FOLDS).cloned().collect();
                drafts.folds.retain(|f, _| keep.contains(f));
            }
        }
    }
    pane.sessions
        .retain(|_, st| !st.tabs.is_empty() || st.tree.is_some() || st.changes.is_some());
    if let Some(tree) = &mut pane.tree
        && tree.width.is_some_and(|w| !TREE_WIDTH.contains(&w))
    {
        tree.width = None;
    }
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

/// An opener: 1 to 32 characters of a-z, 0-9 and "-".
pub(crate) fn is_opener(id: &str) -> bool {
    (1..=32).contains(&id.len()) && id.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

/// A browser tab's viewport size: phone, tablet, laptop or desktop.
pub(crate) fn is_viewport(id: &str) -> bool {
    matches!(id, "phone" | "tablet" | "laptop" | "desktop")
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

/// The pane's stack of panels: the shown ones in the fixed order, their weights, and the filled one. A bad value is an error naming the field.
fn validate_panels(p: &Panels) -> Result<(), String> {
    let ordered: Vec<&str> = PANEL_IDS
        .iter()
        .copied()
        .filter(|id| p.shown.iter().any(|s| s == id))
        .collect();
    if p.shown.is_empty() || p.shown.iter().map(String::as_str).ne(ordered.iter().copied()) {
        return Err(format!(
            "pane.panels.shown must hold \"browser\", \"files\" and \"changes\" at most once each, in that order, and at least one, not {:?}",
            p.shown
        ));
    }
    for (key, weight) in &p.weights {
        if !PANEL_IDS.contains(&key.as_str()) {
            return Err(format!("pane.panels.weights.{key} is not a panel"));
        }
        if !PANEL_WEIGHT.contains(weight) {
            return Err(format!(
                "pane.panels.weights.{key} must be {} to {}, not {weight}",
                PANEL_WEIGHT.start(),
                PANEL_WEIGHT.end()
            ));
        }
    }
    if let Some(f) = &p.filled
        && (p.shown.len() < 2 || !p.shown.contains(f))
    {
        return Err(format!(
            "pane.panels.filled must name a shown panel, and only while two or more are shown, not {f:?}"
        ));
    }
    Ok(())
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
    if let Some(panels) = &p.panels {
        validate_panels(panels)?;
    }
    if let Some(w) = p
        .tree
        .as_ref()
        .and_then(|t| t.width)
        .filter(|w| !TREE_WIDTH.contains(w))
    {
        return Err(format!(
            "pane.tree.width must be {} to {}, not {w}",
            TREE_WIDTH.start(),
            TREE_WIDTH.end()
        ));
    }
    for (session, st) in &p.sessions {
        if let Some(tree) = &st.tree {
            if tree.root.chars().count() > MAX_PATH_CHARS {
                return Err(format!(
                    "pane.sessions.{session}.tree.root is longer than {MAX_PATH_CHARS} characters"
                ));
            }
            if tree.expanded.len() > MAX_TREE_FOLDERS {
                return Err(format!(
                    "pane.sessions.{session}.tree.expanded holds at most {MAX_TREE_FOLDERS} folders"
                ));
            }
            if tree.expanded.iter().any(|f| !tree_folder_ok(f)) {
                return Err(format!(
                    "pane.sessions.{session}.tree.expanded holds a folder that isn't relative"
                ));
            }
        }
        if let Some(drafts) = &st.changes {
            validate_changes(session, drafts)?;
        }
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
            if !matches!(tab.kind.as_str(), "file" | "browser") {
                return Err(format!(
                    "{at}: kind must be \"file\" or \"browser\", not \"{}\"",
                    tab.kind
                ));
            }
            if tab.opened_by.as_deref().is_some_and(|o| !is_opener(o)) {
                return Err(format!(
                    "{at}: opened_by must be 1 to 32 of a-z, 0-9 and -, not \"{}\"",
                    tab.opened_by.as_deref().unwrap_or_default()
                ));
            }
            if tab.unseen && tab.opened_by.is_none() {
                return Err(format!("{at}: unseen needs opened_by"));
            }
            if tab.viewport.as_deref().is_some_and(|v| !is_viewport(v)) {
                return Err(format!("{at}: viewport must be phone, tablet, laptop or desktop"));
            }
            let browser = tab.kind == "browser";
            if !browser && tab.title.is_some() {
                return Err(format!("{at}: only a browser tab has a title"));
            }
            if tab.title.as_ref().is_some_and(|t| t.chars().count() > MAX_TITLE_CHARS) {
                return Err(format!("{at}: title must be at most {MAX_TITLE_CHARS} characters"));
            }
            if browser && tab.history.len() != 1 {
                return Err(format!(
                    "{at}: a browser tab's history must hold exactly 1 entry, not {}",
                    tab.history.len()
                ));
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
                if browser {
                    if !is_page_address(&e.path) {
                        return Err(format!("{at}: a browser tab's path must be a web address"));
                    }
                    if e.view != "rendered" {
                        return Err(format!("{at}: a browser tab's view must be \"rendered\""));
                    }
                    if e.line.is_some() || e.col.is_some() {
                        return Err(format!("{at}: a browser tab has no line or col"));
                    }
                    if e.page.is_some() || e.top.is_some() {
                        return Err(format!("{at}: a browser tab has no page or top"));
                    }
                }
                if e.page == Some(0) {
                    return Err(format!("{at}: page must be 1 or more, not 0"));
                }
                if let Some(top) = e.top {
                    if e.page.is_none() {
                        return Err(format!("{at}: top needs page"));
                    }
                    if top > MAX_PLACE_TOP {
                        return Err(format!("{at}: top must be at most {MAX_PLACE_TOP}, not {top}"));
                    }
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
        if st
            .active_browser
            .as_ref()
            .is_some_and(|a| !st.tabs.iter().any(|t| &t.id == a))
        {
            return Err(format!(
                "pane.sessions.{session}.active_browser must name one of its tabs (or be null)"
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
    use marshell_protocol::api::{CommentSide, PaneEntry, PaneTab, PaneView, RecentFile, SavedTab, SessionTabs};
    use std::collections::BTreeMap;

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
                    page: None,
                    top: None,
                    view: "rendered".into(),
                })
                .collect(),
            index: 0,
            title: None,
            opened_by: None,
            unseen: false,
            viewport: None,
            preview: None,
        }
    }

    fn pane_with_place(page: Option<u32>, top: Option<u32>) -> PaneView {
        let mut t = file_tab("a", &["C:/work/a.pdf"]);
        t.history[0].page = page;
        t.history[0].top = top;
        pane_with("s", session(vec![t]))
    }

    #[test]
    fn a_file_entry_keeps_its_place() {
        let pane = pane_with_place(Some(3), Some(2500));
        assert_eq!(validate_pane(&pane), Ok(()));
        let json = serde_json::to_string(&pane).unwrap();
        let back: PaneView = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pane);
        let e = &back.sessions["s"].tabs[0].history[0];
        assert_eq!((e.page, e.top), (Some(3), Some(2500)));
        let bare = serde_json::to_string(&pane_with_place(None, None)).unwrap();
        assert!(!bare.contains("\"page\"") && !bare.contains("\"top\""));
    }

    #[test]
    fn an_entry_from_an_older_file_has_no_place() {
        let e: PaneEntry =
            serde_json::from_str(r#"{"path":"C:/a.pdf","line":null,"col":null,"view":"rendered"}"#).unwrap();
        assert_eq!((e.page, e.top), (None, None));
        let e: PaneEntry = serde_json::from_str(r#"{"path":"C:/a.pdf","view":"rendered"}"#).unwrap();
        assert_eq!((e.page, e.top), (None, None));
    }

    #[test]
    fn a_place_out_of_bounds_is_refused() {
        assert!(
            validate_pane(&pane_with_place(Some(0), None))
                .unwrap_err()
                .contains("page")
        );
        assert!(
            validate_pane(&pane_with_place(Some(1), Some(10_001)))
                .unwrap_err()
                .contains("top")
        );
        assert!(
            validate_pane(&pane_with_place(None, Some(5)))
                .unwrap_err()
                .contains("top")
        );
        assert_eq!(validate_pane(&pane_with_place(Some(1), Some(10_000))), Ok(()));
        assert_eq!(validate_pane(&pane_with_place(Some(1), Some(0))), Ok(()));
    }

    #[test]
    fn a_browser_tab_has_no_place() {
        for (page, top) in [(Some(1), None), (Some(1), Some(0))] {
            let mut t = file_tab("b", &["https://example.com/"]);
            t.kind = "browser".into();
            t.history[0].page = page;
            t.history[0].top = top;
            let err = validate_pane(&pane_with("s", session(vec![t]))).unwrap_err();
            assert!(err.contains("page"), "{err}");
        }
    }

    fn session(tabs: Vec<PaneTab>) -> SessionTabs {
        let active = tabs.first().map(|t| t.id.clone());
        SessionTabs {
            tabs,
            active,
            active_browser: None,
            tree: None,
            changes: None,
        }
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
        t2.kind = "editor".into();
        let pane = repaired("a", session(vec![file_tab("t1", &["C:/a.md"]), t2]));
        let ids: Vec<_> = pane.sessions["a"].tabs.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["t1"]);
    }

    fn browser_tab(id: &str, url: &str) -> PaneTab {
        let mut t = file_tab(id, &[url]);
        t.kind = "browser".into();
        t
    }

    fn kept_ids(tabs: Vec<PaneTab>) -> Vec<String> {
        repaired("a", session(tabs))
            .sessions
            .get("a")
            .map_or_else(Vec::new, |s| s.tabs.iter().map(|t| t.id.clone()).collect())
    }

    #[test]
    fn repair_keeps_a_browser_tab() {
        let tabs = vec![file_tab("t1", &["C:/a.md"]), browser_tab("t2", "https://example.com/")];
        assert_eq!(kept_ids(tabs), ["t1", "t2"]);
    }

    #[test]
    fn repair_drops_a_browser_tab_with_a_bad_address() {
        for bad in ["file:///C:/x", "javascript:alert(1)", "", "ftp://x/"] {
            let tabs = vec![file_tab("t1", &["C:/a.md"]), browser_tab("t2", bad)];
            assert_eq!(kept_ids(tabs), ["t1"], "{bad:?}");
        }
        for good in ["about:blank", "http://localhost:3000/"] {
            let tabs = vec![file_tab("t1", &["C:/a.md"]), browser_tab("t2", good)];
            assert_eq!(kept_ids(tabs), ["t1", "t2"], "{good:?}");
        }
    }

    #[test]
    fn repair_folds_a_browser_tab_to_one_entry() {
        let mut t = file_tab("t1", &["https://a.test/", "https://b.test/", "https://c.test/"]);
        t.kind = "browser".into();
        t.index = 2;
        let pane = repaired("a", session(vec![t]));
        let kept = &pane.sessions["a"].tabs[0];
        assert_eq!(kept.history.len(), 1);
        assert_eq!(kept.history[0].path, "https://c.test/");
        assert_eq!(kept.index, 0);
    }

    #[test]
    fn repair_trims_a_long_title() {
        let mut b = browser_tab("t1", "https://example.com/");
        b.title = Some("é".repeat(300));
        let mut f = file_tab("t2", &["C:/a.md"]);
        f.title = Some("stray".into());
        let pane = repaired("a", session(vec![b, f]));
        let tabs = &pane.sessions["a"].tabs;
        assert_eq!(tabs[0].title.as_deref(), Some("é".repeat(200).as_str()));
        assert_eq!(tabs[1].title, None);
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
        assert!(one(|t| t.kind = "editor".into()).contains("kind"));
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
    fn validate_view_names_the_browser_field() {
        let one = |change: fn(&mut PaneTab)| {
            let mut v = ViewLayout::default();
            let mut t = browser_tab("t1", "https://example.com/");
            change(&mut t);
            v.pane = pane_with("a", session(vec![t]));
            validate_view(&v).unwrap_err()
        };
        let ok = ViewLayout {
            pane: pane_with("a", session(vec![browser_tab("t1", "https://example.com/")])),
            ..ViewLayout::default()
        };
        assert_eq!(validate_view(&ok), Ok(()));
        assert!(one(|t| t.history.push(t.history[0].clone())).contains("history"));
        assert!(one(|t| t.index = 1).contains("index"));
        assert!(one(|t| t.history[0].path = "file:///x".into()).contains("path"));
        assert!(one(|t| t.history[0].view = "source".into()).contains("view"));
        assert!(one(|t| t.history[0].line = Some(3)).contains("line"));
        assert!(one(|t| t.title = Some("x".repeat(201))).contains("title"));
        let mut v = ViewLayout::default();
        let mut f = file_tab("t1", &["C:/a"]);
        f.title = Some("x".into());
        v.pane = pane_with("a", session(vec![f]));
        assert!(validate_view(&v).unwrap_err().contains("title"));
    }

    fn opened(mut t: PaneTab, by: Option<&str>, unseen: bool) -> PaneTab {
        t.opened_by = by.map(str::to_string);
        t.unseen = unseen;
        t
    }

    #[test]
    fn repair_keeps_who_opened_a_tab() {
        let t = opened(file_tab("t1", &["C:/a.md"]), Some("claude"), true);
        let pane = repaired("a", session(vec![t.clone()]));
        assert_eq!(pane.sessions["a"].tabs, vec![t]);
    }

    #[test]
    fn repair_drops_a_bad_opener() {
        for bad in ["", "Claude Code", &"a".repeat(33)] {
            let t = opened(file_tab("t1", &["C:/a.md"]), Some(bad), true);
            let pane = repaired("a", session(vec![t]));
            let kept = &pane.sessions["a"].tabs[0];
            assert_eq!(kept.opened_by, None, "{bad:?}");
            assert!(!kept.unseen, "{bad:?}");
        }
        let mut b = browser_tab("t2", "https://example.com/");
        b.opened_by = Some("A".into());
        b.unseen = true;
        let pane = repaired("a", session(vec![b]));
        assert_eq!(pane.sessions["a"].tabs[0].opened_by, None);
        assert!(!pane.sessions["a"].tabs[0].unseen);
    }

    #[test]
    fn repair_clears_unseen_without_an_opener() {
        let t = opened(file_tab("t1", &["C:/a.md"]), None, true);
        let pane = repaired("a", session(vec![t]));
        assert!(!pane.sessions["a"].tabs[0].unseen);
    }

    #[test]
    fn validate_view_names_the_opener_field() {
        let one = |t: PaneTab| {
            let v = ViewLayout {
                pane: pane_with("a", session(vec![t])),
                ..ViewLayout::default()
            };
            validate_view(&v)
        };
        for bad in ["", "Claude Code", &"a".repeat(33)] {
            let err = one(opened(file_tab("t1", &["C:/a"]), Some(bad), false)).unwrap_err();
            assert!(err.contains("opened_by"), "{bad:?}: {err}");
        }
        let err = one(opened(file_tab("t1", &["C:/a"]), None, true)).unwrap_err();
        assert!(err.contains("unseen"), "{err}");
        assert_eq!(one(opened(file_tab("t1", &["C:/a"]), Some("shell"), true)), Ok(()));
    }

    #[test]
    fn an_older_layout_file_loads_without_the_new_fields() {
        let json = r#"{"id":"t1","kind":"file","history":[{"path":"C:/a","view":"rendered"}],"index":0}"#;
        let t: PaneTab = serde_json::from_str(json).unwrap();
        assert_eq!(t.opened_by, None);
        assert!(!t.unseen);
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
    fn a_tab_with_an_unreadable_viewport_is_kept_without_it() {
        for bad in [
            serde_json::json!(390),
            serde_json::json!("wide"),
            serde_json::json!(null),
        ] {
            let pane = serde_json::json!({
                "open": true,
                "sessions": {"ta": {"tabs": [{
                    "id": "a", "kind": "browser",
                    "history": [{"path": "https://example.com/", "view": "rendered"}],
                    "index": 0, "viewport": bad
                }], "active": "a"}}
            });
            let doc = load_with_pane(pane, 304.into());
            let tabs = &doc.view.pane.sessions["ta"].tabs;
            assert_eq!(tabs.len(), 1, "{bad:?}");
            assert_eq!(tabs[0].viewport, None, "{bad:?}");
        }
    }

    #[test]
    fn a_tab_keeps_a_viewport_it_can_have() {
        let mut t = browser_tab("a", "https://example.com/");
        t.viewport = Some("phone".into());
        let mut doc = doc_with("ta");
        doc.view.pane = pane_with("ta", session(vec![t]));
        assert_eq!(validate_view(&doc.view), Ok(()));
        let home = tempfile::tempdir().unwrap();
        save(home.path(), &doc).unwrap();
        let back = load(home.path());
        assert_eq!(back.view.pane.sessions["ta"].tabs[0].viewport.as_deref(), Some("phone"));
    }

    #[test]
    fn validate_pane_refuses_a_viewport_that_is_not_a_size() {
        let mut t = browser_tab("a", "https://example.com/");
        t.viewport = Some("wide".into());
        let err = validate_pane(&pane_with("ta", session(vec![t]))).unwrap_err();
        assert!(
            err.contains("viewport must be phone, tablet, laptop or desktop"),
            "{err}"
        );
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

    fn stack(shown: &[&str], weights: &[(&str, u32)], filled: Option<&str>) -> marshell_protocol::api::Panels {
        marshell_protocol::api::Panels {
            shown: shown.iter().map(|s| s.to_string()).collect(),
            weights: weights.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            filled: filled.map(String::from),
        }
    }

    fn repaired_stack(panels: marshell_protocol::api::Panels) -> Option<marshell_protocol::api::Panels> {
        let mut doc = doc_with("a");
        doc.view.pane.panels = Some(panels);
        repair(doc).view.pane.panels
    }

    #[test]
    fn an_old_pane_reads_with_no_panels() {
        let pane = serde_json::json!({
            "open": true, "width": 480, "recent": [],
            "sessions": {"ta": {"tabs": [good_file_tab("t1")], "active": "t1"}}
        });
        let doc = load_with_pane(pane, 304.into());
        assert_eq!(doc.view.pane.panels, None);
        assert_eq!(doc.view.pane.sessions["ta"].active_browser, None);
        assert_eq!(doc.view.pane.sessions["ta"].tabs.len(), 1);
    }

    #[test]
    fn a_stack_of_the_wrong_type_loses_only_itself() {
        for bad in [serde_json::json!("files"), serde_json::json!({"shown": "files"})] {
            let pane = serde_json::json!({
                "open": true, "width": 480, "recent": [], "panels": bad,
                "sessions": {"ta": {"tabs": [good_file_tab("t1")], "active": "t1", "active_browser": 7}}
            });
            let doc = load_with_pane(pane, 304.into());
            assert_tabs_and_sidebar_kept(&doc);
            let pane = &doc.view.pane;
            assert!(pane.open);
            assert_eq!(pane.width, Some(480));
            assert_eq!(pane.panels, None);
            assert_eq!(pane.sessions["ta"].tabs.len(), 1);
            assert_eq!(pane.sessions["ta"].active, some("t1"));
            assert_eq!(pane.sessions["ta"].active_browser, None);
        }
    }

    #[test]
    fn a_bad_weight_or_fill_loses_only_itself() {
        for (bad, weights) in [
            (
                serde_json::json!({"shown": ["files"], "weights": {"files": 1.5}}),
                vec![],
            ),
            (serde_json::json!({"shown": ["files"], "filled": 3}), vec![]),
            (
                serde_json::json!({"shown": ["browser", "files"], "weights": {"files": -5, "browser": 3}, "filled": 7}),
                vec![("browser", 3)],
            ),
        ] {
            let shown = bad["shown"].clone();
            let pane = serde_json::json!({"open": true, "width": 480, "recent": [], "panels": bad, "sessions": {}});
            let doc = load_with_pane(pane, 304.into());
            assert_tabs_and_sidebar_kept(&doc);
            let panels = doc.view.pane.panels.expect("the stack is kept");
            assert_eq!(serde_json::json!(panels.shown), shown);
            let want: std::collections::BTreeMap<String, u32> =
                weights.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
            assert_eq!(panels.weights, want);
            assert_eq!(panels.filled, None);
        }
    }

    #[test]
    fn a_stack_whose_shown_cannot_be_read_is_dropped() {
        let pane = serde_json::json!({"open": true, "width": 480, "recent": [], "sessions": {},
            "panels": {"shown": "browser", "weights": {"browser": 3}}});
        let doc = load_with_pane(pane, 304.into());
        assert_eq!(doc.view.pane.panels, None);
    }

    #[test]
    fn repair_keeps_a_good_stack() {
        let good = stack(
            &["browser", "files"],
            &[("browser", 6000), ("files", 14000)],
            Some("files"),
        );
        assert_eq!(repaired_stack(good.clone()), Some(good));
    }

    #[test]
    fn repair_orders_and_dedupes_shown() {
        let got = repaired_stack(stack(&["files", "browser", "files"], &[], None)).unwrap();
        assert_eq!(got.shown, vec!["browser", "files"]);
    }

    #[test]
    fn repair_drops_an_unknown_panel() {
        let got = repaired_stack(stack(&["tree", "files"], &[], None)).unwrap();
        assert_eq!(got.shown, vec!["files"]);
        assert_eq!(repaired_stack(stack(&["tree"], &[], None)), None);
    }

    #[test]
    fn repair_drops_a_bad_weight() {
        let got = repaired_stack(stack(
            &["browser", "files"],
            &[("tree", 5), ("browser", 0), ("files", 100_001)],
            None,
        ))
        .unwrap();
        assert!(got.weights.is_empty());
        let got = repaired_stack(stack(
            &["browser", "files"],
            &[("tree", 5), ("browser", 0), ("files", 7)],
            None,
        ))
        .unwrap();
        assert_eq!(got.weights.len(), 1);
        assert_eq!(got.weights["files"], 7);
    }

    #[test]
    fn repair_clears_a_fill_that_cannot_be() {
        assert_eq!(
            repaired_stack(stack(&["files"], &[], Some("browser"))).unwrap().filled,
            None
        );
        assert_eq!(
            repaired_stack(stack(&["files"], &[], Some("files"))).unwrap().filled,
            None
        );
    }

    #[test]
    fn changes_is_a_panel_and_fill_holds_with_three() {
        let three = stack(&["browser", "files", "changes"], &[("changes", 9)], Some("changes"));
        let got = repaired_stack(three.clone()).unwrap();
        assert_eq!(got.shown, ["browser", "files", "changes"]);
        assert_eq!(got.weights["changes"], 9);
        assert_eq!(got.filled.as_deref(), Some("changes"));
        let pane = PaneView {
            panels: Some(three),
            ..PaneView::default()
        };
        assert!(validate_pane(&pane).is_ok());
        let out_of_order = stack(&["changes", "browser", "files"], &[], None);
        assert_eq!(
            repaired_stack(out_of_order).unwrap().shown,
            ["browser", "files", "changes"]
        );
        assert_eq!(
            repaired_stack(stack(&["changes"], &[], Some("changes")))
                .unwrap()
                .filled,
            None
        );
    }

    #[test]
    fn repair_clears_an_active_browser_that_names_no_tab() {
        let mut st = session(vec![browser_tab("t1", "https://example.com/")]);
        st.active_browser = Some("gone".into());
        assert_eq!(repaired("ta", st).sessions["ta"].active_browser, None);
        let mut st = session(vec![browser_tab("t1", "https://example.com/")]);
        st.active_browser = Some("t1".into());
        assert_eq!(repaired("ta", st).sessions["ta"].active_browser, some("t1"));
    }

    #[test]
    fn validate_names_the_stack_field() {
        let check = |panels: marshell_protocol::api::Panels, starts: &str| {
            let pane = PaneView {
                panels: Some(panels),
                ..PaneView::default()
            };
            let err = validate_pane(&pane).unwrap_err();
            assert!(err.starts_with(starts), "{err}");
        };
        check(stack(&["files", "browser"], &[], None), "pane.panels.shown");
        check(stack(&["files", "files"], &[], None), "pane.panels.shown");
        check(stack(&[], &[], None), "pane.panels.shown");
        check(stack(&["tree"], &[], None), "pane.panels.shown");
        check(stack(&["files"], &[("tree", 5)], None), "pane.panels.weights.tree");
        check(stack(&["files"], &[("files", 0)], None), "pane.panels.weights.files");
        check(
            stack(&["files"], &[("files", 100_001)], None),
            "pane.panels.weights.files",
        );
        check(stack(&["files"], &[], Some("files")), "pane.panels.filled");
        check(stack(&["browser", "files"], &[], Some("tree")), "pane.panels.filled");
        let mut st = session(vec![browser_tab("t1", "https://example.com/")]);
        st.active_browser = Some("gone".into());
        let err = validate_pane(&pane_with("a", st)).unwrap_err();
        assert!(err.starts_with("pane.sessions.a.active_browser"), "{err}");
        let good = stack(&["browser", "files"], &[("browser", 1)], Some("browser"));
        assert!(
            validate_pane(&PaneView {
                panels: Some(good),
                ..PaneView::default()
            })
            .is_ok()
        );
    }

    fn pane_json_with_session(session: serde_json::Value) -> serde_json::Value {
        serde_json::json!({"open": true, "width": 480, "recent": [], "sessions": {"ta": session}})
    }

    #[test]
    fn a_tab_keeps_preview_and_drops_an_unreadable_one() {
        let mut keep = good_file_tab("t1");
        keep["preview"] = serde_json::json!(true);
        let mut bad = good_file_tab("t2");
        bad["preview"] = serde_json::json!("yes");
        let doc = load_with_pane(
            pane_json_with_session(serde_json::json!({"tabs": [keep, bad], "active": "t1"})),
            serde_json::json!(304),
        );
        let tabs = &doc.view.pane.sessions["ta"].tabs;
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].preview, Some(true));
        assert_eq!(tabs[1].preview, None);
    }

    #[test]
    fn a_session_keeps_its_tree_and_drops_bad_folders() {
        let doc = load_with_pane(
            pane_json_with_session(serde_json::json!({
                "tabs": [good_file_tab("t1")], "active": "t1",
                "tree": {"root": "C:\\p", "expanded": ["src", "src/a", "", "/abs", "a/../b", "a\\b", 7]}
            })),
            serde_json::json!(304),
        );
        let tree = doc.view.pane.sessions["ta"].tree.clone().unwrap();
        assert_eq!(tree.root, "C:\\p");
        assert_eq!(tree.expanded, vec!["src".to_string(), "src/a".to_string()]);
    }

    #[test]
    fn a_session_tree_keeps_at_most_500_folders() {
        let folders: Vec<String> = (0..600).map(|i| format!("d{i}")).collect();
        let doc = load_with_pane(
            pane_json_with_session(serde_json::json!({
                "tabs": [good_file_tab("t1")], "active": "t1",
                "tree": {"root": "C:\\p", "expanded": folders}
            })),
            serde_json::json!(304),
        );
        let expanded = &doc.view.pane.sessions["ta"].tree.as_ref().unwrap().expanded;
        assert_eq!(expanded.len(), 500);
        assert_eq!(expanded[0], "d0");
        assert_eq!(expanded[499], "d499");
    }

    #[test]
    fn an_unreadable_session_tree_is_dropped_and_the_tabs_kept() {
        for tree in [serde_json::json!(5), serde_json::json!({"root": 3})] {
            let doc = load_with_pane(
                pane_json_with_session(serde_json::json!({
                    "tabs": [good_file_tab("t1")], "active": "t1", "tree": tree
                })),
                serde_json::json!(304),
            );
            let st = &doc.view.pane.sessions["ta"];
            assert_eq!(st.tabs.len(), 1);
            assert_eq!(st.tree, None);
        }
    }

    #[test]
    fn the_window_tree_width_outside_160_to_2048_is_dropped() {
        for width in [100, 5000] {
            let mut pane = pane_json_with_session(serde_json::json!({"tabs": [good_file_tab("t1")], "active": "t1"}));
            pane["tree"] = serde_json::json!({"hidden": true, "width": width});
            let doc = load_with_pane(pane, serde_json::json!(304));
            assert_eq!(
                doc.view.pane.tree,
                Some(TreeView {
                    hidden: true,
                    width: None
                })
            );
        }
    }

    #[test]
    fn a_session_with_a_tree_and_no_tabs_is_kept() {
        let tree = serde_json::json!({"root": "C:\\p", "expanded": []});
        let doc = load_with_pane(
            pane_json_with_session(serde_json::json!({"tabs": [], "active": null, "tree": tree})),
            serde_json::json!(304),
        );
        assert!(doc.view.pane.sessions["ta"].tree.is_some());
        let doc = load_with_pane(
            pane_json_with_session(serde_json::json!({"tabs": [], "active": null})),
            serde_json::json!(304),
        );
        assert!(doc.view.pane.sessions.is_empty());
    }

    #[test]
    fn validate_pane_refuses_a_bad_tree() {
        let with_tree = |expanded: Vec<String>| {
            let mut st = session(vec![]);
            st.tree = Some(TreeState {
                root: "C:\\p".into(),
                expanded,
            });
            pane_with("s", st)
        };
        let many: Vec<String> = (0..501).map(|i| format!("d{i}")).collect();
        assert_eq!(
            validate_pane(&with_tree(many)).unwrap_err(),
            "pane.sessions.s.tree.expanded holds at most 500 folders"
        );
        assert_eq!(
            validate_pane(&with_tree(vec!["a/../b".into()])).unwrap_err(),
            "pane.sessions.s.tree.expanded holds a folder that isn't relative"
        );
        assert!(validate_pane(&with_tree(vec!["src".into()])).is_ok());
        let pane = PaneView {
            tree: Some(TreeView {
                hidden: false,
                width: Some(100),
            }),
            ..PaneView::default()
        };
        assert_eq!(
            validate_pane(&pane).unwrap_err(),
            "pane.tree.width must be 160 to 2048, not 100"
        );
    }

    #[test]
    fn a_saved_tree_root_over_4096_characters_is_refused_and_repaired() {
        let with_root = |root: String| {
            let mut st = session(vec![]);
            st.tree = Some(TreeState {
                root,
                expanded: vec!["src".into()],
            });
            pane_with("s", st)
        };
        assert!(validate_pane(&with_root("a".repeat(4096))).is_ok());
        assert!(validate_pane(&with_root(String::new())).is_ok());
        let mut pane = with_root("a".repeat(4097));
        assert_eq!(
            validate_pane(&pane).unwrap_err(),
            "pane.sessions.s.tree.root is longer than 4096 characters"
        );
        repair_pane(&mut pane, &[tab("s")]);
        let tree = pane.sessions["s"].tree.as_ref().unwrap();
        assert_eq!(tree.root, "");
        assert!(tree.expanded.is_empty());
    }

    fn comment(id: &str) -> DraftComment {
        DraftComment {
            id: id.into(),
            path: "src/a.ts".into(),
            side: CommentSide::New,
            start: 3,
            end: 4,
            rows: 20,
            first: "1a2b".into(),
            hash: "00ff".into(),
            quote: vec!["+let a = 1;".into(), "\tlet b = 2;".into()],
            text: "Why not a const?\nIt never changes.".into(),
        }
    }

    fn drafts(comments: Vec<DraftComment>) -> ChangesDrafts {
        ChangesDrafts {
            checkout: "C:/work".into(),
            base: Some("origin/main".into()),
            comments,
            folds: BTreeMap::from([("src/a.ts".to_string(), true)]),
            picked: Vec::new(),
            sent: Vec::new(),
        }
    }

    fn picked(id: &str) -> PickedComment {
        PickedComment {
            id: id.into(),
            number: 7,
            note: "Fix this\nplease".into(),
        }
    }

    fn sent(id: &str) -> SentComment {
        SentComment {
            id: id.into(),
            number: 7,
            count: 2,
        }
    }

    fn pane_of(d: ChangesDrafts) -> PaneView {
        pane_with("s1", session_with_drafts(d))
    }

    #[test]
    fn picked_and_sent_round_trip_through_layout_json() {
        let home = tempfile::tempdir().unwrap();
        let mut d = drafts(vec![]);
        d.picked = vec![picked("PRRT_a"), picked("PRRC_b")];
        d.sent = vec![sent("PRRT_c"), sent("PRRT_d")];
        let mut doc = doc_with("a");
        doc.view.pane = pane_with("a", session_with_drafts(d));
        save(home.path(), &doc).unwrap();
        assert_eq!(load(home.path()).view.pane, doc.view.pane);
    }

    #[test]
    fn old_drafts_without_picked_or_sent_load_as_empty() {
        let doc = load_with_pane(
            serde_json::json!({
                "open": true, "width": 480, "recent": [],
                "sessions": {
                    "ta": {
                        "tabs": [], "active": null,
                        "changes": {"checkout": "C:/work", "base": null, "comments": [], "folds": {"a": true}}
                    }
                }
            }),
            serde_json::json!(304),
        );
        let changes = doc.view.pane.sessions["ta"].changes.as_ref().unwrap();
        assert!(changes.picked.is_empty() && changes.sent.is_empty());
        assert_eq!(changes.folds.len(), 1);
    }

    #[test]
    fn validate_refuses_a_bad_picked_or_sent() {
        let bad_picked = |f: &dyn Fn(&mut PickedComment)| {
            let mut p = picked("PRRT_a");
            f(&mut p);
            let mut d = drafts(vec![]);
            d.picked = vec![p];
            validate_pane(&pane_of(d))
        };
        for err in [
            bad_picked(&|p| p.id = "a b".into()),
            bad_picked(&|p| p.note = "a".repeat(2001)),
            bad_picked(&|p| p.note = "a\u{7}b".into()),
            bad_picked(&|p| p.number = 0),
        ] {
            assert!(err.unwrap_err().contains("changes.picked"));
        }
        assert!(bad_picked(&|p| p.note = String::new()).is_ok());
        assert!(bad_picked(&|p| p.note = "a".repeat(2000)).is_ok());
        let sent_with = |v: Vec<SentComment>| {
            let mut d = drafts(vec![]);
            d.sent = v;
            validate_pane(&pane_of(d))
        };
        let mut s = sent("PRRT_a");
        s.id = "a b".into();
        assert!(sent_with(vec![s]).unwrap_err().contains("changes.sent"));
        let mut s = sent("PRRT_a");
        s.count = 0;
        assert!(sent_with(vec![s]).unwrap_err().contains("changes.sent"));
        let mut s = sent("PRRT_a");
        s.count = 100_001;
        assert!(sent_with(vec![s]).is_err());
        let mut s = sent("PRRT_a");
        s.number = 0;
        assert!(sent_with(vec![s]).unwrap_err().contains("changes.sent"));
        assert!(
            sent_with(vec![sent("PRRT_a"), sent("PRRT_a")])
                .unwrap_err()
                .contains("changes.sent")
        );
        let many: Vec<SentComment> = (0..501).map(|i| sent(&format!("S{i}"))).collect();
        assert!(sent_with(many).unwrap_err().contains("changes.sent"));
        let mut d = drafts(vec![]);
        d.picked = vec![picked("PRRT_a"), picked("PRRT_a")];
        assert!(validate_pane(&pane_of(d)).unwrap_err().contains("changes.picked"));
        let mut d = drafts(vec![]);
        d.picked = (0..201).map(|i| picked(&format!("P{i}"))).collect();
        assert!(validate_pane(&pane_of(d)).unwrap_err().contains("changes.picked"));
    }

    #[test]
    fn repair_keeps_the_good_picked_and_sent() {
        let mut d = drafts(vec![]);
        let mut dup = picked("P1");
        dup.note = "second".into();
        let mut bad = picked("bad id");
        bad.number = 0;
        d.picked = vec![picked("P1"), bad, dup, picked("P2")];
        d.picked.extend((0..250).map(|i| picked(&format!("Q{i}"))));
        d.sent = (0..600).map(|i| sent(&format!("S{i}"))).collect();
        let mut zero = sent("Z");
        zero.count = 0;
        d.sent.insert(0, zero);
        let pane = repaired("s", session_with_drafts(d));
        let changes = pane.sessions["s"].changes.as_ref().unwrap();
        assert_eq!(changes.picked.len(), 200);
        assert_eq!(changes.picked[0].note, "Fix this\nplease");
        assert_eq!(changes.picked[1].id, "P2");
        assert_eq!(changes.sent.len(), 500);
        assert_eq!(changes.sent[0].id, "S0");

        let good = |id: &str| serde_json::to_value(picked(id)).unwrap();
        let doc = load_with_pane(
            serde_json::json!({
                "open": true, "width": 480, "recent": [],
                "sessions": {
                    "ta": {
                        "tabs": [], "active": null,
                        "changes": {
                            "checkout": "C:/work", "base": null, "comments": [], "folds": {},
                            "picked": [good("P1"), {"id": 3}, good("P2")],
                            "sent": "nope"
                        }
                    }
                }
            }),
            serde_json::json!(304),
        );
        let changes = doc.view.pane.sessions["ta"].changes.as_ref().unwrap();
        let ids: Vec<&str> = changes.picked.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["P1", "P2"]);
        assert!(changes.sent.is_empty());
    }

    fn session_with_drafts(d: ChangesDrafts) -> SessionTabs {
        SessionTabs {
            changes: Some(d),
            ..session(vec![])
        }
    }

    #[test]
    fn drafts_round_trip_through_layout_json() {
        let home = tempfile::tempdir().unwrap();
        let mut doc = doc_with("a");
        doc.view.pane = pane_with("a", session_with_drafts(drafts(vec![comment("c1"), comment("c2")])));
        save(home.path(), &doc).unwrap();
        let loaded = load(home.path());
        assert_eq!(loaded.view.pane, doc.view.pane);
        let changes = loaded.view.pane.sessions["a"].changes.as_ref().unwrap();
        assert_eq!(changes.comments.len(), 2);
        assert_eq!(changes.folds.len(), 1);
    }

    #[test]
    fn drafts_keep_a_session_with_no_tabs() {
        let pane = repaired("s", session_with_drafts(drafts(vec![comment("c1")])));
        assert_eq!(pane.sessions["s"].tabs.len(), 0);
        assert!(pane.sessions["s"].tree.is_none());
        assert!(pane.sessions["s"].changes.is_some());
    }

    #[test]
    fn validate_refuses_a_51st_comment() {
        let many: Vec<DraftComment> = (0..51).map(|i| comment(&format!("c{i}"))).collect();
        let err = validate_pane(&pane_with("s1", session_with_drafts(drafts(many)))).unwrap_err();
        assert!(
            err.contains("pane.sessions.s1.changes.comments holds at most 50"),
            "{err}"
        );
        let fifty: Vec<DraftComment> = (0..50).map(|i| comment(&format!("c{i}"))).collect();
        assert!(validate_pane(&pane_with("s1", session_with_drafts(drafts(fifty)))).is_ok());
    }

    #[test]
    fn validate_refuses_a_comment_path_with_dotdot() {
        let mut c = comment("c1");
        c.path = "../x.ts".into();
        let err = validate_pane(&pane_with("s1", session_with_drafts(drafts(vec![c])))).unwrap_err();
        assert!(err.contains("isn't relative"), "{err}");
    }

    #[test]
    fn validate_refuses_text_with_an_escape() {
        let mut c = comment("c1");
        c.text = "a\u{1b}[201~b".into();
        let err = validate_pane(&pane_with("s1", session_with_drafts(drafts(vec![c])))).unwrap_err();
        assert!(err.contains("pane.sessions.s1.changes.comments.c1.text"), "{err}");
        let mut c = comment("c1");
        c.quote = vec!["+a\u{1b}b".into()];
        assert!(validate_pane(&pane_with("s1", session_with_drafts(drafts(vec![c])))).is_err());
    }

    #[test]
    fn validate_checks_the_other_draft_bounds() {
        let bad = |f: &dyn Fn(&mut DraftComment)| {
            let mut c = comment("c1");
            f(&mut c);
            validate_pane(&pane_with("s1", session_with_drafts(drafts(vec![c])))).is_err()
        };
        assert!(bad(&|c| c.start = 0));
        assert!(bad(&|c| c.end = 2));
        assert!(bad(&|c| c.rows = 0));
        assert!(bad(&|c| c.rows = 100_001));
        assert!(bad(&|c| c.first = String::new()));
        assert!(bad(&|c| c.hash = "XYZ".into()));
        assert!(bad(&|c| c.hash = "0".repeat(17)));
        assert!(bad(&|c| c.quote = vec!["+a".into(); 4]));
        assert!(bad(&|c| c.quote = vec!["a".repeat(202)]));
        assert!(bad(&|c| c.text = String::new()));
        assert!(bad(&|c| c.text = "a".repeat(2001)));
        assert!(bad(&|c| c.id = "Bad Id".into()));
        assert!(!bad(&|c| c.text = "a".repeat(2000)));
        assert!(!bad(&|c| c.quote = vec!["a".repeat(201); 3]));
        let mut d = drafts(vec![comment("c1"), comment("c1")]);
        assert!(validate_pane(&pane_with("s1", session_with_drafts(d.clone()))).is_err());
        d.comments.pop();
        d.checkout = String::new();
        assert!(validate_pane(&pane_with("s1", session_with_drafts(d.clone()))).is_err());
        d.checkout = "C:/w".into();
        d.base = Some("a".repeat(201));
        assert!(validate_pane(&pane_with("s1", session_with_drafts(d.clone()))).is_err());
        d.base = Some("ba\nd".into());
        assert!(validate_pane(&pane_with("s1", session_with_drafts(d.clone()))).is_err());
        d.base = None;
        d.folds.insert("../x".into(), true);
        assert!(validate_pane(&pane_with("s1", session_with_drafts(d))).is_err());
    }

    #[test]
    fn repair_drops_only_the_bad_comment() {
        let mut bad = comment("c2");
        bad.start = 0;
        let mut d = drafts(vec![comment("c1"), bad, comment("c3")]);
        d.folds.insert("../x".into(), false);
        let pane = repaired("s", session_with_drafts(d));
        let changes = pane.sessions["s"].changes.as_ref().unwrap();
        let ids: Vec<&str> = changes.comments.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["c1", "c3"]);
        assert_eq!(changes.folds.keys().collect::<Vec<_>>(), ["src/a.ts"]);
    }

    #[test]
    fn a_wrongly_typed_draft_costs_only_that_draft() {
        let good = |id: &str| serde_json::to_value(comment(id)).unwrap();
        let mut bad = good("c2");
        bad["start"] = serde_json::json!("x");
        let doc = load_with_pane(
            serde_json::json!({
                "open": true, "width": 480, "recent": [],
                "sessions": {
                    "ta": {
                        "tabs": [good_file_tab("t1")], "active": "t1",
                        "changes": {
                            "checkout": "C:/work", "base": null,
                            "comments": [good("c1"), bad, 5],
                            "folds": {"a": true, "b": 1}
                        }
                    },
                    "tb": {
                        "tabs": [], "active": null,
                        "changes": {"checkout": 7, "base": null, "comments": [], "folds": {}}
                    }
                }
            }),
            serde_json::json!(304),
        );
        let sessions = &doc.view.pane.sessions;
        assert_eq!(sessions["ta"].tabs.len(), 1);
        let changes = sessions["ta"].changes.as_ref().unwrap();
        assert_eq!(changes.comments.len(), 1);
        assert_eq!(changes.comments[0].id, "c1");
        assert_eq!(changes.folds, BTreeMap::from([("a".to_string(), true)]));
        // A drafts block with no readable checkout goes whole.
        assert_eq!(doc.tabs.len(), 1);
        assert!(sessions.get("tb").is_none_or(|s| s.changes.is_none()));
    }
}
