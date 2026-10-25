//! The agents the person defined themselves, in `~/.marshell/agents.json` (`{"version":1,"agents":[...]}`).
//!
//! Reading never repairs or rewrites the file: one that can't be read gives no agents and one problem, and a save is refused
//! until the person fixes it. Writing goes through `crate::store` (versioned, atomic) after a changed-since-read check.

use crate::agents::manifest::{AgentManifest, Agents, OwnAgentProblem, Reserved, validate_own};
use crate::paths;
use crate::store::{self, Versioned};
use marshell_protocol::manifest::{OwnAgents, OwnAgentsSave};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[derive(Serialize, Deserialize)]
struct FileDoc {
    agents: Vec<AgentManifest>,
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
}

/// SHA-256 (hex) of the file's bytes, or "" when there is no file.
fn version_of(bytes: Option<&[u8]>) -> String {
    bytes.map(|b| hex::encode(Sha256::digest(b))).unwrap_or_default()
}

/// The entries in the file's bytes, or why they can't be read.
fn parse(bytes: &[u8]) -> Result<Vec<AgentManifest>, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| format!("it isn't valid JSON (line {})", e.line()))?;
    let version = value.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version > u64::from(FileDoc::VERSION) {
        return Err("it was written by a newer Marshell".into());
    }
    serde_json::from_value::<FileDoc>(value)
        .map(|doc| doc.agents)
        .map_err(|e| format!("it has an unexpected shape ({e})"))
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
    let list = match bytes.as_deref().map(parse).transpose() {
        Ok(list) => list.unwrap_or_default(),
        Err(why) => return unreadable(version, why),
    };
    let (valid, problems) = validate_own(&list, &Reserved::builtin());
    Loaded {
        agents: Agents::with_own(valid),
        own: OwnAgents {
            agents: list,
            problems,
            version,
        },
        readable: true,
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
    store::write(&paths::agents_file(home), &FileDoc { agents: valid }).map_err(SaveError::Io)?;
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
