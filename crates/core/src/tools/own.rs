//! The tools the person defined themselves, in `~/.marshell/tools.json` (`{"version":1,"tools":[...]}`).
//!
//! Reading never repairs or rewrites the file: one that can't be read gives no own tools and one problem, and a save is
//! refused until the person fixes it. One entry that does not fit is that entry's problem, listed at its place, and the
//! others load (the `agents.json` rules). Writing goes through `crate::store` (versioned, atomic) after a
//! changed-since-read check.

use crate::paths;
use crate::store::{self, Versioned};
use crate::tools::profile::{Tools, validate_own};
use marshell_protocol::tools::{OwnToolProblem, OwnTools, OwnToolsSave, ToolProfile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// The newest file version this Marshell reads.
const VERSION: u64 = 1;

#[derive(Serialize, Deserialize)]
struct FileDoc {
    /// Valid entries as serialized; an entry that was refused and sent back unchanged, as it was in the file.
    tools: Vec<Value>,
}

impl Versioned for FileDoc {
    const VERSION: u32 = VERSION as u32;
}

/// What `tools.json` held when it was read.
pub struct LoadedTools {
    /// The shipped profiles and the valid own ones.
    pub tools: Arc<Tools>,
    /// The valid own profiles, in file order.
    pub own: Vec<ToolProfile>,
    /// The entries that were refused, at their places in the file, or the one problem of a file that can't be read.
    pub problems: Vec<OwnToolProblem>,
    /// What `GET /v1/tools/own` says.
    pub listed: OwnTools,
    /// The file is missing or parsed. False for a broken or newer file, which a save must not replace.
    pub readable: bool,
    /// The entries that were refused: what the editor was shown for each, and the entry as the file held it.
    refused: Vec<(ToolProfile, Value)>,
}

/// SHA-256 (hex) of the file's bytes, or "" when there is no file.
fn version_of(bytes: Option<&[u8]>) -> String {
    bytes.map(|b| hex::encode(Sha256::digest(b))).unwrap_or_default()
}

/// One entry of the file's `tools` list.
struct Slot {
    /// The entry as parsed. For one that could not be, what could be read of it (see `placeholder`), so the list keeps
    /// its places and an index names the entry the editor shows.
    profile: ToolProfile,
    /// Why the entry could not be parsed, in words.
    broken: Option<String>,
    /// The entry as the file held it.
    raw: Value,
}

/// What can be read of an entry that does not parse, for the editor to show beside its problem.
fn placeholder(entry: &Value) -> ToolProfile {
    let text = |key: &str| entry.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let words = |key: &str| -> Vec<String> {
        entry
            .get(key)
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    };
    ToolProfile {
        id: text("id"),
        name: text("name"),
        commands: words("commands"),
        scripts: words("scripts"),
        launcher: false,
        needs_you: Vec::new(),
        ports: Vec::new(),
        quiet_ports: Vec::new(),
        presets: Vec::new(),
    }
}

/// The entries in the file's bytes, each on its own, or why the file as a whole can't be read.
fn parse(bytes: &[u8]) -> Result<Vec<Slot>, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| format!("it isn't valid JSON (line {})", e.line()))?;
    if value.get("version").and_then(Value::as_u64).unwrap_or(0) > VERSION {
        return Err("it was written by a newer Marshell".into());
    }
    let Some(tools) = value.get("tools") else {
        return Err("it has an unexpected shape (missing field `tools`)".into());
    };
    let Some(list) = tools.as_array() else {
        return Err("it has an unexpected shape (`tools` isn't a list)".into());
    };
    Ok(list
        .iter()
        .map(|entry| match serde_json::from_value::<ToolProfile>(entry.clone()) {
            Ok(profile) => Slot {
                profile,
                broken: None,
                raw: entry.clone(),
            },
            Err(e) => Slot {
                profile: placeholder(entry),
                broken: Some(format!("It has an unexpected shape ({e}).")),
                raw: entry.clone(),
            },
        })
        .collect())
}

/// Reads `tools.json`. The file is never written here.
pub fn load_full(home: &Path) -> LoadedTools {
    let bytes = match std::fs::read(paths::tools_file(home)) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return unreadable(String::new(), e.to_string()),
    };
    let version = version_of(bytes.as_deref());
    let slots = match bytes.as_deref().map(parse).transpose() {
        Ok(slots) => slots.unwrap_or_default(),
        Err(why) => return unreadable(version, why),
    };
    // Only the entries that parsed are checked, and a problem is moved back to the entry's place in the file.
    let places: Vec<usize> = (0..slots.len()).filter(|&i| slots[i].broken.is_none()).collect();
    let parsed: Vec<ToolProfile> = places.iter().map(|&i| slots[i].profile.clone()).collect();
    let (own, mut problems) = validate_own(&parsed, &Tools::shipped());
    for problem in &mut problems {
        problem.index = places[problem.index as usize] as u32;
    }
    for (index, slot) in slots.iter().enumerate() {
        if let Some(reason) = &slot.broken {
            problems.push(OwnToolProblem {
                index: index as u32,
                id: Some(slot.profile.id.clone()).filter(|id| !id.is_empty()),
                reason: reason.clone(),
            });
        }
    }
    problems.sort_by_key(|p| p.index);
    let refused: Vec<(ToolProfile, Value)> = problems
        .iter()
        .map(|p| {
            (
                slots[p.index as usize].profile.clone(),
                slots[p.index as usize].raw.clone(),
            )
        })
        .collect();
    let list: Vec<ToolProfile> = slots.into_iter().map(|s| s.profile).collect();
    LoadedTools {
        tools: Arc::new(Tools::with_own(own.clone())),
        own,
        listed: OwnTools {
            tools: list,
            problems: problems.clone(),
            version,
        },
        problems,
        readable: true,
        refused,
    }
}

fn unreadable(version: String, why: String) -> LoadedTools {
    let problems = vec![OwnToolProblem {
        index: 0,
        id: None,
        reason: format!("tools.json can't be read: {why}."),
    }];
    LoadedTools {
        tools: Arc::new(Tools::shipped()),
        own: Vec::new(),
        listed: OwnTools {
            tools: Vec::new(),
            problems: problems.clone(),
            version,
        },
        problems,
        readable: false,
        refused: Vec::new(),
    }
}

/// `tools.json` as `GET /v1/tools/own` shows it.
pub fn load(home: &Path) -> OwnTools {
    load_full(home).listed
}

/// Why a save was refused.
#[derive(Debug)]
pub enum SaveError {
    /// The file changed since the editor read it.
    Stale,
    /// The file can't be read, so it is left for the person to repair.
    Unreadable(Vec<OwnToolProblem>),
    /// Entries failed validation. Nothing was written.
    Invalid(Vec<OwnToolProblem>),
    Io(anyhow::Error),
}

/// Replaces the own tools with the validated entries. The caller serialises saves.
pub fn save(home: &Path, req: OwnToolsSave) -> Result<OwnTools, SaveError> {
    let current = load_full(home);
    if current.listed.version != req.version {
        return Err(SaveError::Stale);
    }
    if !current.readable {
        return Err(SaveError::Unreadable(current.problems));
    }
    // An entry sent back exactly as a refused one was shown goes back to the file as it was there, with whatever field or
    // fault made it fail: saving another tool must not edit it, nor be refused for it. One the person changed is checked
    // and written as sent, and one left out is gone.
    let mut untouched: Vec<Option<&(ToolProfile, Value)>> = current.refused.iter().map(Some).collect();
    let originals: Vec<Option<&Value>> = req
        .tools
        .iter()
        .map(|sent| {
            untouched
                .iter_mut()
                .find(|slot| slot.is_some_and(|(shown, _)| shown == sent))
                .and_then(Option::take)
                .map(|(_, raw)| raw)
        })
        .collect();
    let checked: Vec<usize> = (0..req.tools.len()).filter(|&i| originals[i].is_none()).collect();
    let to_check: Vec<ToolProfile> = checked.iter().map(|&i| req.tools[i].clone()).collect();
    let (_, mut problems) = validate_own(&to_check, &Tools::shipped());
    if !problems.is_empty() {
        for problem in &mut problems {
            problem.index = checked[problem.index as usize] as u32;
        }
        return Err(SaveError::Invalid(problems));
    }
    let mut tools = Vec::with_capacity(req.tools.len());
    for (sent, original) in req.tools.iter().zip(originals) {
        tools.push(match original {
            Some(raw) => raw.clone(),
            None => serde_json::to_value(sent).map_err(|e| SaveError::Io(e.into()))?,
        });
    }
    store::write(&paths::tools_file(home), &FileDoc { tools }).map_err(SaveError::Io)?;
    Ok(load(home))
}

/// When the file was last read: its modified time and length, or `None` for no file.
type Stamp = Option<(SystemTime, u64)>;

/// The parsed `tools.json`, read again only when the file's modified time or length changes. There is no watcher.
#[derive(Default)]
pub struct ToolsCache(Mutex<Option<(Stamp, Arc<LoadedTools>)>>);

impl ToolsCache {
    /// One `metadata` call when nothing changed.
    pub fn get(&self, home: &Path) -> Arc<LoadedTools> {
        // None: the stamp can't be had, so nothing is cached for this read.
        let stamp: Option<Stamp> = match std::fs::metadata(paths::tools_file(home)) {
            Ok(m) => m.modified().ok().map(|t| Some((t, m.len()))),
            Err(e) if e.kind() == ErrorKind::NotFound => Some(None),
            Err(_) => None,
        };
        if let Some(stamp) = &stamp
            && let Some((seen, loaded)) = &*self.0.lock().unwrap_or_else(|e| e.into_inner())
            && seen == stamp
        {
            return loaded.clone();
        }
        // The lock is not held while the file is read.
        let loaded = Arc::new(load_full(home));
        if let Some(stamp) = stamp {
            *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some((stamp, loaded.clone()));
        }
        loaded
    }

    /// Drops what was read, after a save.
    pub fn forget(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths;
    use serde_json::json;

    fn own_json(id: &str) -> serde_json::Value {
        json!({"id": id, "name": "Mine", "commands": [format!("{id}-marshell-test")]})
    }

    fn write(home: &Path, doc: &serde_json::Value) {
        std::fs::write(paths::tools_file(home), doc.to_string()).unwrap();
    }

    fn ids(loaded: &LoadedTools) -> Vec<String> {
        loaded.tools.profiles().map(|p| p.def.id.clone()).collect()
    }

    #[test]
    fn a_missing_tools_file_gives_the_shipped_set() {
        let home = tempfile::tempdir().unwrap();
        let loaded = load_full(home.path());
        assert_eq!(ids(&loaded), ids_of(&Tools::shipped()));
        assert!(loaded.own.is_empty());
        assert!(loaded.problems.is_empty());
        assert!(
            !paths::tools_file(home.path()).exists(),
            "reading never creates the file"
        );
    }

    fn ids_of(tools: &Tools) -> Vec<String> {
        tools.profiles().map(|p| p.def.id.clone()).collect()
    }

    #[test]
    fn an_edited_file_is_read_again() {
        let home = tempfile::tempdir().unwrap();
        let cache = ToolsCache::default();
        assert!(cache.get(home.path()).own.is_empty());
        write(home.path(), &json!({"version": 1, "tools": [own_json("a")]}));
        let first = cache.get(home.path());
        assert_eq!(first.own.len(), 1);
        assert!(
            Arc::ptr_eq(&first, &cache.get(home.path())),
            "an unchanged file is not read again"
        );
        // Another length, so the stamp moves even when the clock is coarse.
        write(
            home.path(),
            &json!({"version": 1, "tools": [own_json("a"), own_json("a-longer-id")]}),
        );
        let again = cache.get(home.path());
        assert_eq!(again.own.len(), 2);
        assert!(again.tools.profile("a-longer-id").is_some());
    }

    #[test]
    fn a_broken_entry_is_listed_at_its_place_and_the_others_load() {
        let home = tempfile::tempdir().unwrap();
        write(
            home.path(),
            &json!({"version": 1, "tools": [
                own_json("first"),
                {"id": "typo", "name": "Typo", "comands": ["x"]},
                own_json("vite"),
                own_json("last"),
            ]}),
        );
        let loaded = load_full(home.path());
        let own: Vec<&str> = loaded.own.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(own, ["first", "last"]);
        let got: Vec<(u32, Option<&str>)> = loaded.problems.iter().map(|p| (p.index, p.id.as_deref())).collect();
        assert_eq!(got, [(1, Some("typo")), (2, Some("vite"))]);
        assert!(loaded.problems[0].reason.starts_with("It has an unexpected shape ("));
        assert_eq!(loaded.problems[1].reason, "The id \"vite\" is taken by a shipped tool.");
        assert!(loaded.tools.profile("first").is_some() && loaded.tools.profile("last").is_some());
    }

    #[test]
    fn a_newer_file_gives_one_problem_and_no_own_tools() {
        let home = tempfile::tempdir().unwrap();
        write(home.path(), &json!({"version": 2, "tools": [own_json("a")]}));
        let loaded = load_full(home.path());
        assert!(loaded.own.is_empty());
        assert_eq!(ids(&loaded), ids_of(&Tools::shipped()));
        assert_eq!(loaded.problems.len(), 1);
        assert_eq!(loaded.problems[0].index, 0);
        assert_eq!(loaded.problems[0].id, None);
        assert_eq!(
            loaded.problems[0].reason,
            "tools.json can't be read: it was written by a newer Marshell."
        );
    }

    #[test]
    fn bad_json_and_a_wrong_shape_give_one_problem_each() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(paths::tools_file(home.path()), "{\n not json").unwrap();
        let loaded = load_full(home.path());
        assert!(loaded.own.is_empty());
        assert_eq!(
            loaded.problems[0].reason,
            "tools.json can't be read: it isn't valid JSON (line 2)."
        );
        write(home.path(), &json!({"version": 1}));
        assert_eq!(
            load_full(home.path()).problems[0].reason,
            "tools.json can't be read: it has an unexpected shape (missing field `tools`)."
        );
        write(home.path(), &json!({"version": 1, "tools": {}}));
        assert_eq!(
            load_full(home.path()).problems[0].reason,
            "tools.json can't be read: it has an unexpected shape (`tools` isn't a list)."
        );
    }

    fn profile(id: &str) -> ToolProfile {
        serde_json::from_value(own_json(id)).unwrap()
    }

    fn req(tools: Vec<ToolProfile>, version: &str) -> OwnToolsSave {
        OwnToolsSave {
            tools,
            version: version.into(),
        }
    }

    fn raw_tools(home: &Path) -> Vec<Value> {
        let doc: Value = serde_json::from_slice(&std::fs::read(paths::tools_file(home)).unwrap()).unwrap();
        doc["tools"].as_array().unwrap().clone()
    }

    fn sha(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    #[test]
    fn save_writes_valid_tools_and_returns_the_new_version() {
        let home = tempfile::tempdir().unwrap();
        let saved = save(home.path(), req(vec![profile("one"), profile("two")], "")).unwrap();
        let bytes = std::fs::read(paths::tools_file(home.path())).unwrap();
        assert_eq!(saved.version, sha(&bytes));
        assert!(saved.problems.is_empty());
        let ids: Vec<&str> = saved.tools.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["one", "two"]);
        let doc: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(doc["version"], 1);
        assert_eq!(doc["tools"][1]["id"], "two");
        assert_eq!(load(home.path()), saved);
        // Saving on the new version works, and saving on the old one is refused.
        assert!(save(home.path(), req(vec![profile("one")], &saved.version)).is_ok());
        assert!(matches!(
            save(home.path(), req(vec![profile("x")], "")),
            Err(SaveError::Stale)
        ));
    }

    #[test]
    fn save_refuses_a_stale_version() {
        let home = tempfile::tempdir().unwrap();
        write(home.path(), &json!({"version": 1, "tools": [own_json("a")]}));
        let seen = load(home.path());
        write(
            home.path(),
            &json!({"version": 1, "tools": [own_json("a"), own_json("b")]}),
        );
        let before = std::fs::read(paths::tools_file(home.path())).unwrap();
        let got = save(home.path(), req(vec![profile("c")], &seen.version));
        assert!(matches!(got, Err(SaveError::Stale)), "{got:?}");
        assert_eq!(std::fs::read(paths::tools_file(home.path())).unwrap(), before);
    }

    #[test]
    fn save_refuses_an_unreadable_file_and_leaves_it() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(paths::tools_file(home.path()), "{\n not json").unwrap();
        let seen = load(home.path());
        let got = save(home.path(), req(vec![profile("a")], &seen.version));
        let Err(SaveError::Unreadable(problems)) = got else {
            panic!("{got:?}")
        };
        assert_eq!(problems, seen.problems);
        assert_eq!(
            std::fs::read_to_string(paths::tools_file(home.path())).unwrap(),
            "{\n not json"
        );
        assert!(!crate::store::bak_path(&paths::tools_file(home.path())).exists());
    }

    #[test]
    fn save_refuses_an_invalid_regex_with_its_index() {
        let home = tempfile::tempdir().unwrap();
        let mut bad = profile("bad");
        bad.ports = vec![marshell_protocol::tools::PortPattern { pattern: "(".into() }];
        let got = save(home.path(), req(vec![profile("ok"), bad], ""));
        let Err(SaveError::Invalid(problems)) = got else {
            panic!("{got:?}")
        };
        assert_eq!(problems.len(), 1);
        assert_eq!((problems[0].index, problems[0].id.as_deref()), (1, Some("bad")));
        assert!(!paths::tools_file(home.path()).exists());
    }

    #[test]
    fn save_refuses_a_shipped_id() {
        let home = tempfile::tempdir().unwrap();
        let got = save(home.path(), req(vec![profile("vite")], ""));
        let Err(SaveError::Invalid(problems)) = got else {
            panic!("{got:?}")
        };
        assert_eq!(problems.len(), 1);
        // The validator's existing wording, which `GET /v1/tools` and the tools list already show for the same fault.
        assert_eq!(problems[0].reason, "The id \"vite\" is taken by a shipped tool.");
        assert!(!paths::tools_file(home.path()).exists());
    }

    #[test]
    fn an_entry_that_does_not_parse_is_listed_as_a_placeholder_and_kept_on_save() {
        let home = tempfile::tempdir().unwrap();
        let typo = json!({"id": "typo", "name": "Typo", "commands": ["typo-marshell-test"], "colour": "red",
                          "presets": [{"id": "p", "name": "P", "program": "cargo"}]});
        let string_commands = json!({"id": "str", "name": "Str", "commands": "str-marshell-test"});
        write(
            home.path(),
            &json!({"version": 1, "tools": [own_json("good"), typo.clone(), string_commands.clone()]}),
        );
        let seen = load(home.path());
        let listed: Vec<&str> = seen.tools.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(listed, ["good", "typo", "str"]);
        assert_eq!(seen.problems.len(), 2);
        for (problem, id) in seen.problems.iter().zip(["typo", "str"]) {
            assert_eq!(problem.id.as_deref(), Some(id));
            assert!(
                problem.reason.starts_with("It has an unexpected shape ("),
                "{}",
                problem.reason
            );
        }
        assert_eq!((seen.problems[0].index, seen.problems[1].index), (1, 2));
        assert_eq!(seen.tools[1].name, "Typo");

        // Another entry is saved, and the placeholders go back as they are.
        let mut sent = seen.tools.clone();
        sent.push(profile("added"));
        let saved = save(home.path(), req(sent, &seen.version)).unwrap();
        let written = raw_tools(home.path());
        assert_eq!(written.len(), 4);
        assert_eq!(written[1], typo, "the entry with an unknown field was rewritten");
        assert_eq!(written[2], string_commands, "the entry with a wrong type was rewritten");
        assert_eq!(written[3]["id"], "added");
        assert_eq!(saved.problems.len(), 2);

        // Left out, a placeholder is gone.
        let sent = vec![saved.tools[0].clone(), saved.tools[2].clone(), saved.tools[3].clone()];
        save(home.path(), req(sent, &saved.version)).unwrap();
        let written = raw_tools(home.path());
        let ids: Vec<&str> = written.iter().map(|v| v["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["good", "str", "added"]);
        assert_eq!(written[1], string_commands);
    }

    #[test]
    fn an_invalid_entry_sent_back_unchanged_keeps_its_value() {
        let home = tempfile::tempdir().unwrap();
        // A bad regex, and a field written the long way that a re-serialisation would drop or reorder.
        let bad = json!({"id": "bad", "name": "Bad", "commands": ["bad-marshell-test"],
                         "ports": [{"pattern": "("}], "launcher": false});
        write(
            home.path(),
            &json!({"version": 1, "tools": [bad.clone(), own_json("good")]}),
        );
        let seen = load(home.path());
        assert_eq!(seen.problems.len(), 1);
        assert_eq!(
            (seen.problems[0].index, seen.problems[0].id.as_deref()),
            (0, Some("bad"))
        );

        // The person adds an entry: the bad one is sent back as it was shown, and the save is not refused for it.
        let mut sent = seen.tools.clone();
        sent.push(profile("added"));
        save(home.path(), req(sent, &seen.version)).unwrap();
        let written = raw_tools(home.path());
        assert_eq!(written[0], bad);
        assert_eq!(written.len(), 3);

        // Changed, it is validated again and refused.
        let seen = load(home.path());
        let mut sent = seen.tools.clone();
        sent[0].name = "Still bad".into();
        let got = save(home.path(), req(sent, &seen.version));
        assert!(
            matches!(got, Err(SaveError::Invalid(ref p)) if p[0].index == 0),
            "{got:?}"
        );
        assert_eq!(raw_tools(home.path())[0], bad);
    }

    #[test]
    fn get_own_lists_broken_entries_with_their_problems() {
        let home = tempfile::tempdir().unwrap();
        let none = load(home.path());
        assert_eq!(
            (none.tools.len(), none.problems.len(), none.version.as_str()),
            (0, 0, "")
        );
        let text = json!({"version": 1, "tools": [
            own_json("first"),
            {"id": "typo", "name": "Typo", "comands": ["x"]},
            own_json("vite"),
        ]})
        .to_string();
        std::fs::write(paths::tools_file(home.path()), &text).unwrap();
        let got = load(home.path());
        let ids: Vec<&str> = got.tools.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["first", "typo", "vite"]);
        let at: Vec<(u32, Option<&str>)> = got.problems.iter().map(|p| (p.index, p.id.as_deref())).collect();
        assert_eq!(at, [(1, Some("typo")), (2, Some("vite"))]);
        assert_eq!(got.version, sha(text.as_bytes()));
    }

    #[test]
    fn forget_makes_the_cache_read_the_file_again() {
        let home = tempfile::tempdir().unwrap();
        let cache = ToolsCache::default();
        let first = cache.get(home.path());
        cache.forget();
        assert!(!Arc::ptr_eq(&first, &cache.get(home.path())));
    }
}
