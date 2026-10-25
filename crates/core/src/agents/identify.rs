//! Finding a manifest agent on this machine: its command on PATH, minus the install folders and version shapes that belong to
//! another program with the same name.

pub use crate::agents::manifest::AgentManifest;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// How long `--version` may run before the cache gives up on it.
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// An agent's command and where it resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub command: String,
    pub path: PathBuf,
    /// Read only for a manifest with a `version_shape`.
    pub version: Option<String>,
}

/// Why no command of a manifest is usable. With several commands, the first refusal wins over "not on PATH".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    NotOnPath,
    /// The command resolved inside an install folder that belongs to another program.
    Rejected {
        path: PathBuf,
    },
    /// The command's `--version` doesn't look like this agent's; `seen` is its first line.
    WrongVersion {
        path: PathBuf,
        seen: Option<String>,
    },
}

/// `/` and case don't matter: `C:/X/Copilot-Shim/` and `c:\x\copilot-shim\` are one folder.
fn normalised(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

/// The first of `m.commands` that resolves outside every `reject_paths` folder and, when the manifest has a version shape,
/// answers `--version` in that shape. `version_of` runs only for a manifest with a shape.
pub fn find(
    m: &AgentManifest,
    path_of: &dyn Fn(&str) -> Option<PathBuf>,
    version_of: &dyn Fn(&Path) -> Option<String>,
) -> Result<Found, Missing> {
    // A shape that doesn't compile can never match, so the agent reads as the wrong program rather than as found.
    let shape = m.version_shape.as_deref().map(|s| regex::Regex::new(s).ok());
    let rejects: Vec<String> = m.reject_paths.iter().map(|r| normalised(r)).collect();
    let mut refusal = None;
    for command in &m.commands {
        let Some(path) = path_of(command) else { continue };
        let resolved = normalised(&path.to_string_lossy());
        if rejects.iter().any(|r| resolved.contains(r.as_str())) {
            refusal.get_or_insert(Missing::Rejected { path });
            continue;
        }
        let Some(shape) = &shape else {
            return Ok(Found {
                command: command.clone(),
                path,
                version: None,
            });
        };
        let text = version_of(&path);
        let first = text
            .as_deref()
            .and_then(|t| t.lines().next())
            .map(|l| l.trim().to_string());
        match (shape, &first) {
            (Some(re), Some(line)) if re.is_match(line) => {
                return Ok(Found {
                    command: command.clone(),
                    version: super::parse_any_version(line),
                    path,
                });
            }
            _ => {
                refusal.get_or_insert(Missing::WrongVersion { path, seen: first });
            }
        }
    }
    Err(refusal.unwrap_or(Missing::NotOnPath))
}

type Reader = Box<dyn Fn(&Path) -> Option<String> + Send + Sync>;

/// `--version` output per program file, read again only when the file's modified time changes.
pub struct VersionCache {
    seen: Mutex<HashMap<(PathBuf, SystemTime), Option<String>>>,
    read: Reader,
}

impl Default for VersionCache {
    fn default() -> Self {
        Self::with_reader(|path| super::run_version_text(path, VERSION_TIMEOUT))
    }
}

impl VersionCache {
    pub fn with_reader(read: impl Fn(&Path) -> Option<String> + Send + Sync + 'static) -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
            read: Box::new(read),
        }
    }

    /// The program's `--version` text; None when it has no mtime (gone), fails or times out.
    pub fn get(&self, path: &Path) -> Option<String> {
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
        let key = (path.to_path_buf(), mtime);
        if let Some(hit) = self.seen.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return hit.clone();
        }
        // The lock is not held while the program runs: two misses at once may both read, and the second write wins.
        let text = (self.read)(path);
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        seen.retain(|(p, _), _| p != path);
        seen.insert(key, text.clone());
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::manifest::shipped;
    use std::cell::Cell;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn manifest(id: &str) -> &'static AgentManifest {
        shipped().iter().find(|m| m.id == id).expect("shipped manifest")
    }

    fn no_version(_: &Path) -> Option<String> {
        panic!("version_of runs only when a shape is set")
    }

    #[test]
    fn cursor_is_not_found_through_groks_agent_exe() {
        let cursor = manifest("cursor");
        let grok_agent = |c: &str| (c == "agent").then(|| PathBuf::from(r"C:\Users\Home Office\.grok\bin\agent.exe"));
        match find(cursor, &grok_agent, &no_version) {
            Err(Missing::Rejected { path }) => assert!(path.ends_with("agent.exe")),
            other => panic!("expected Rejected, got {other:?}"),
        }
        let real = |c: &str| {
            (c == "cursor-agent").then(|| PathBuf::from(r"C:\Users\x\AppData\Local\cursor-agent\cursor-agent.exe"))
        };
        let found = find(cursor, &real, &no_version).expect("cursor-agent is found");
        assert_eq!(found.command, "cursor-agent");
        assert_eq!(found.version, None);
    }

    #[test]
    fn copilot_shim_is_not_copilot() {
        let copilot = manifest("copilot");
        for shim in [
            r"C:\Program Files\Microsoft VS Code\bin\copilot-shim\copilot.cmd",
            "C:/Program Files/Microsoft VS Code/bin/copilot-shim/copilot.cmd",
            r"C:\PROGRAM FILES\Microsoft VS Code\bin\Copilot-Shim\copilot.cmd",
        ] {
            let path_of = |c: &str| (c == "copilot").then(|| PathBuf::from(shim));
            assert!(
                matches!(find(copilot, &path_of, &no_version), Err(Missing::Rejected { .. })),
                "{shim} is rejected"
            );
        }
        let none = |_: &str| None;
        assert!(matches!(find(copilot, &none, &no_version), Err(Missing::NotOnPath)));
    }

    #[test]
    fn grok_needs_its_version_shape() {
        let grok = manifest("grok");
        let path_of = |c: &str| (c == "grok").then(|| PathBuf::from(r"C:\Users\x\.grok\bin\grok.exe"));
        let calls = Cell::new(0);
        let real = |_: &Path| {
            calls.set(calls.get() + 1);
            Some("grok 1.0.50 (c58f321264ba) [stable]\n".to_string())
        };
        let found = find(grok, &path_of, &real).expect("grok is found");
        assert_eq!(found.command, "grok");
        assert_eq!(found.version.as_deref(), Some("1.0.50"));
        assert_eq!(calls.get(), 1);

        let other = |_: &Path| Some("grok-dev 1.1.7\n".to_string());
        match find(grok, &path_of, &other) {
            Err(Missing::WrongVersion { seen, .. }) => assert_eq!(seen.as_deref(), Some("grok-dev 1.1.7")),
            other => panic!("expected WrongVersion, got {other:?}"),
        }
        let silent = |_: &Path| None;
        assert!(matches!(
            find(grok, &path_of, &silent),
            Err(Missing::WrongVersion { seen: None, .. })
        ));
    }

    #[test]
    fn version_cache_reads_once_per_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool.exe");
        std::fs::write(&exe, b"one").unwrap();
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();
        let cache = VersionCache::with_reader(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            Some("tool 1.2.3".to_string())
        });
        assert_eq!(cache.get(&exe).as_deref(), Some("tool 1.2.3"));
        assert_eq!(cache.get(&exe).as_deref(), Some("tool 1.2.3"));
        assert_eq!(runs.load(Ordering::SeqCst), 1);

        // A new mtime (an update in place) reads again.
        let later = std::fs::metadata(&exe).unwrap().modified().unwrap() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&exe)
            .unwrap()
            .set_modified(later)
            .unwrap();
        cache.get(&exe);
        assert_eq!(runs.load(Ordering::SeqCst), 2);

        // A path that doesn't exist has no mtime and is never run.
        assert_eq!(cache.get(&dir.path().join("gone.exe")), None);
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }
}
