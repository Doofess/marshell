//! The tools the person defined themselves, in `~/.marshell/tools.json` (`{"version":1,"tools":[...]}`).
//!
//! The file is only read, never written or repaired. One that can't be read gives no own tools and one problem. One entry
//! that does not fit is that entry's problem, listed at its place, and the others load (the `agents.json` rules).

use crate::paths;
use crate::tools::profile::{Tools, validate_own};
use marshell_protocol::tools::{OwnToolProblem, ToolProfile};
use serde_json::Value;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// The newest file version this Marshell reads.
const VERSION: u64 = 1;

/// What `tools.json` held when it was read.
pub struct LoadedTools {
    /// The shipped profiles and the valid own ones.
    pub tools: Arc<Tools>,
    /// The valid own profiles, in file order.
    pub own: Vec<ToolProfile>,
    /// The entries that were refused, at their places in the file, or the one problem of a file that can't be read.
    pub problems: Vec<OwnToolProblem>,
}

/// Reads `tools.json`. The file is never written here.
pub fn load_full(home: &Path) -> LoadedTools {
    let bytes = match std::fs::read(paths::tools_file(home)) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return unreadable(e.to_string()),
    };
    let entries = match bytes.as_deref().map(parse).transpose() {
        Ok(entries) => entries.unwrap_or_default(),
        Err(why) => return unreadable(why),
    };
    // Only the entries that parsed are checked, and a problem is moved back to the entry's place in the file.
    let mut places = Vec::new();
    let mut parsed = Vec::new();
    let mut problems = Vec::new();
    for (index, entry) in entries.into_iter().enumerate() {
        match entry {
            Ok(profile) => {
                places.push(index);
                parsed.push(profile);
            }
            Err(problem) => problems.push(OwnToolProblem {
                index: index as u32,
                ..problem
            }),
        }
    }
    let (own, refused) = validate_own(&parsed, &Tools::shipped());
    problems.extend(refused.into_iter().map(|p| OwnToolProblem {
        index: places[p.index as usize] as u32,
        ..p
    }));
    problems.sort_by_key(|p| p.index);
    LoadedTools {
        tools: Arc::new(Tools::with_own(own.clone())),
        own,
        problems,
    }
}

/// The entries in the file's bytes, each on its own, or why the file as a whole can't be read.
fn parse(bytes: &[u8]) -> Result<Vec<Result<ToolProfile, OwnToolProblem>>, String> {
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
        .map(|entry| {
            serde_json::from_value::<ToolProfile>(entry.clone()).map_err(|e| OwnToolProblem {
                index: 0,
                id: entry
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .map(str::to_string),
                reason: format!("It has an unexpected shape ({e})."),
            })
        })
        .collect())
}

fn unreadable(why: String) -> LoadedTools {
    LoadedTools {
        tools: Arc::new(Tools::shipped()),
        own: Vec::new(),
        problems: vec![OwnToolProblem {
            index: 0,
            id: None,
            reason: format!("tools.json can't be read: {why}."),
        }],
    }
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
}
