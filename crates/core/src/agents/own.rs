//! The agents the person defined themselves, in `~/.marshell/agents.json` (`{"version":1,"agents":[...]}`).
//!
//! Reading never repairs or rewrites the file: one that can't be read gives no agents and one problem, and a save is refused
//! until the person fixes it. One entry that does not fit is that entry's problem, listed at its place, and the others load. Writing goes through `crate::store` (versioned, atomic) after a changed-since-read check.

use crate::agents::manifest::{AgentManifest, Agents, OwnAgentProblem, Reserved, validate_own};
use crate::paths;
use crate::store::{self, Versioned};
use marshell_protocol::manifest::{OwnAgents, OwnAgentsSave, PromptStyle};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[derive(Serialize, Deserialize)]
struct FileDoc {
    /// Valid entries as serialized; an entry that did not parse, as it was in the file.
    agents: Vec<Value>,
}

impl Versioned for FileDoc {
    const VERSION: u32 = 1;
}

/// What `agents.json` held when it was read.
pub struct Loaded {
    /// What `GET /v1/agents/own` says.
    pub own: OwnAgents,
    /// The shipped manifests and the valid own ones.
    pub agents: Agents,
    /// The file is missing or parsed. False for a broken or newer file, which a save must not replace.
    pub readable: bool,
    /// The entries that did not parse: what the editor was shown for each, and the entry as the file held it.
    broken: Vec<(AgentManifest, Value)>,
}

/// SHA-256 (hex) of the file's bytes, or "" when there is no file.
fn version_of(bytes: Option<&[u8]>) -> String {
    bytes.map(|b| hex::encode(Sha256::digest(b))).unwrap_or_default()
}

/// One entry of the file's `agents` list.
struct Slot {
    /// The entry as parsed. For one that could not be, what could be read of it (see `placeholder`), so the list keeps
    /// its places and an index names the entry the editor shows.
    manifest: AgentManifest,
    /// Why the entry could not be parsed, in words.
    broken: Option<String>,
    /// The entry as the file held it.
    raw: Value,
}

/// What can be read of an entry that does not parse, for the editor to show beside its problem.
fn placeholder(entry: &Value) -> AgentManifest {
    let text = |key: &str| entry.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let words = |key: &str| -> Vec<String> {
        entry
            .get(key)
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    };
    AgentManifest {
        id: text("id"),
        name: text("name"),
        cli: text("cli"),
        mark: "generic".into(),
        commands: words("commands"),
        reject_paths: Vec::new(),
        version_shape: None,
        exe_names: words("exe_names"),
        packages: words("packages"),
        prompt: PromptStyle::Positional,
        continue_args: None,
        modes: Default::default(),
    }
}

/// The entries in the file's bytes, each on its own, or why the file as a whole can't be read. One entry that does not
/// fit (a typo in a field name, a missing `cli`) is that entry's problem and leaves the others alone.
fn parse(bytes: &[u8]) -> Result<Vec<Slot>, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| format!("it isn't valid JSON (line {})", e.line()))?;
    let version = value.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version > u64::from(FileDoc::VERSION) {
        return Err("it was written by a newer Marshell".into());
    }
    let Some(agents) = value.get("agents") else {
        return Err("it has an unexpected shape (missing field `agents`)".into());
    };
    let Some(list) = agents.as_array() else {
        return Err("it has an unexpected shape (`agents` isn't a list)".into());
    };
    Ok(list
        .iter()
        .map(|entry| match serde_json::from_value::<AgentManifest>(entry.clone()) {
            Ok(manifest) => Slot {
                manifest,
                broken: None,
                raw: entry.clone(),
            },
            Err(e) => Slot {
                manifest: placeholder(entry),
                broken: Some(format!("It has an unexpected shape ({e}).")),
                raw: entry.clone(),
            },
        })
        .collect())
}

/// Reads `agents.json`. The file is never written here.
pub fn load_full(home: &Path) -> Loaded {
    let bytes = match std::fs::read(paths::agents_file(home)) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => {
            return unreadable(String::new(), e.to_string());
        }
    };
    let version = version_of(bytes.as_deref());
    let slots = match bytes.as_deref().map(parse).transpose() {
        Ok(slots) => slots.unwrap_or_default(),
        Err(why) => return unreadable(version, why),
    };
    // Only the entries that parsed are checked, and a problem is moved back to the entry's place in the file.
    let places: Vec<usize> = (0..slots.len()).filter(|&i| slots[i].broken.is_none()).collect();
    let parsed: Vec<AgentManifest> = places.iter().map(|&i| slots[i].manifest.clone()).collect();
    let (valid, mut problems) = validate_own(&parsed, &Reserved::builtin());
    for problem in &mut problems {
        problem.index = places[problem.index as usize] as u32;
    }
    for (index, slot) in slots.iter().enumerate() {
        if let Some(reason) = &slot.broken {
            problems.push(OwnAgentProblem {
                index: index as u32,
                id: Some(slot.manifest.id.clone()).filter(|id| !id.is_empty()),
                reason: reason.clone(),
            });
        }
    }
    problems.sort_by_key(|p| p.index);
    let broken: Vec<(AgentManifest, Value)> = slots
        .iter()
        .filter(|s| s.broken.is_some())
        .map(|s| (s.manifest.clone(), s.raw.clone()))
        .collect();
    let list: Vec<AgentManifest> = slots.into_iter().map(|s| s.manifest).collect();
    Loaded {
        agents: Agents::with_own(valid),
        own: OwnAgents {
            agents: list,
            problems,
            version,
        },
        readable: true,
        broken,
    }
}

fn unreadable(version: String, why: String) -> Loaded {
    Loaded {
        agents: Agents::shipped(),
        own: OwnAgents {
            agents: Vec::new(),
            problems: vec![OwnAgentProblem {
                index: 0,
                id: None,
                reason: format!("agents.json can't be read: {why}."),
            }],
            version,
        },
        readable: false,
        broken: Vec::new(),
    }
}

/// `agents.json` as `GET /v1/agents/own` shows it.
pub fn load(home: &Path) -> OwnAgents {
    load_full(home).own
}

/// Why a save was refused.
#[derive(Debug)]
pub enum SaveError {
    /// The file changed since the editor read it.
    Stale,
    /// The file can't be read, so it is left for the person to repair.
    Unreadable(Vec<OwnAgentProblem>),
    /// Entries failed validation. Nothing was written.
    Invalid(Vec<OwnAgentProblem>),
    Io(anyhow::Error),
}

/// Replaces the own agents with the validated entries. The caller serialises saves.
pub fn save(home: &Path, req: OwnAgentsSave) -> Result<OwnAgents, SaveError> {
    let current = load_full(home);
    if current.own.version != req.version {
        return Err(SaveError::Stale);
    }
    if !current.readable {
        return Err(SaveError::Unreadable(current.own.problems));
    }
    let (valid, problems) = validate_own(&req.agents, &Reserved::builtin());
    if !problems.is_empty() {
        return Err(SaveError::Invalid(problems));
    }
    // An entry sent back exactly as a broken one was shown goes back to the file as it was there, with its modes and flags
    // and whatever field made it fail: saving another agent must not edit it. One the person changed is written as sent,
    // and one left out is gone.
    let mut untouched: Vec<Option<&(AgentManifest, Value)>> = current.broken.iter().map(Some).collect();
    let mut agents = Vec::with_capacity(valid.len());
    for (sent, valid) in req.agents.iter().zip(valid) {
        let original = untouched
            .iter_mut()
            .find(|slot| slot.is_some_and(|(shown, _)| shown == sent))
            .and_then(Option::take);
        agents.push(match original {
            Some((_, raw)) => raw.clone(),
            None => serde_json::to_value(valid).map_err(|e| SaveError::Io(e.into()))?,
        });
    }
    store::write(&paths::agents_file(home), &FileDoc { agents }).map_err(SaveError::Io)?;
    Ok(load(home))
}

/// When the file was last read: its modified time and length, or `None` for no file.
type Stamp = Option<(SystemTime, u64)>;

/// The parsed `agents.json`, read again only when the file's modified time or length changes. There is no watcher.
#[derive(Default)]
pub struct OwnCache(Mutex<Option<(Stamp, Arc<Loaded>)>>);

impl OwnCache {
    /// One `metadata` call when nothing changed.
    pub fn get(&self, home: &Path) -> Arc<Loaded> {
        // None: the stamp can't be had, so nothing is cached for this read.
        let stamp: Option<Stamp> = match std::fs::metadata(paths::agents_file(home)) {
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
