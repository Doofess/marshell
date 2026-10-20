//! The diagnostics export: Marshell's own logs and setup, redacted, held in memory for the person to read
//! before it is zipped. Nothing here writes to disk; the app saves the zip where the person chooses.

use crate::diagnostics::redact::Redactor;
use crate::server::AppState;
use crate::server::agents::setup_of;
use crate::{auth, paths};
use serde_json::json;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a prepared export is kept.
pub const PREVIEW_TTL: Duration = Duration::from_secs(15 * 60);

/// The agents `about.json` lists, in this order.
const CLIS: [&str; 3] = ["claude", "codex", "antigravity"];

const README: &str = "Marshell diagnostics. This holds Marshell's logs and setup only: no transcripts, recordings, prompts or hook events. Keys, tokens, your home folder and user name were replaced before saving. Read the files before you share them.\n";

/// One file of the bundle: its name inside the zip and its redacted bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The bundle for this process, redacted with the global redactor. Blocking: reads files and may run
/// `--version` on a cold cache.
pub fn collect(st: &AppState) -> Vec<BundleFile> {
    collect_with(st, crate::diagnostics::redact::global())
}

/// `collect` with the redactor named, so a test can tell it which folder to hide.
pub(crate) fn collect_with(st: &AppState, redactor: &Redactor) -> Vec<BundleFile> {
    let mut files = Vec::new();
    let mut add_text = |name: &str, text: &str| {
        files.push(BundleFile {
            name: name.to_string(),
            bytes: redactor.redact(text).into_owned().into_bytes(),
        });
    };

    add_text("README.txt", README);
    add_text("about.json", &pretty(&about(st)));
    add_text("settings.json", &pretty(&st.settings_snapshot()));

    // An environment value is never kept, whatever it looks like; the names stay.
    let mut profiles = st.profiles_snapshot();
    for profile in &mut profiles.profiles {
        for arg in &mut profile.command {
            *arg = redactor.redact(arg).into_owned();
        }
        for (_, value) in &mut profile.env {
            *value = "<removed>".to_string();
        }
    }
    add_text("profiles.json", &pretty(&profiles));

    if let Ok(setup) = setup_of(st) {
        add_text("agents/claude.json", &pretty(&setup));
    }
    add_text("perf.json", &pretty(&crate::diagnostics::perf::snapshot(st)));

    let home = st.home();
    let logs = [
        ("logs/marshell.log", paths::marshell_log(home), 4),
        ("logs/crash.log", paths::crash_log(home), 1),
        ("logs/bridge.log", paths::bridge_log(home), 1),
    ];
    for (name, path, older) in logs {
        for n in 0..=older {
            let (name, path) = if n == 0 {
                (name.to_string(), path.clone())
            } else {
                let mut with_n = path.clone().into_os_string();
                with_n.push(format!(".{n}"));
                (format!("{name}.{n}"), with_n.into())
            };
            // `fs::read` opens with read, write and delete sharing on Windows, so the core's own writer
            // (or the bridge's) holding the log open does not stop it. A missing file is left out.
            if let Ok(bytes) = std::fs::read(&path) {
                add_text(&name, &String::from_utf8_lossy(&bytes));
            }
        }
    }
    files
}

fn pretty(value: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn about(st: &AppState) -> serde_json::Value {
    let clis: Vec<_> = CLIS
        .iter()
        .map(|agent| {
            let exe = st.agent_exe_of(agent);
            let version = st.agent_version(agent, exe.as_deref());
            json!({ "agent": agent, "found": exe.is_some(), "version": version })
        })
        .collect();
    json!({
        "app_version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "os_version": os_version(),
        "arch": std::env::consts::ARCH,
        "clis": clis,
    })
}

/// `24H2 (build 26300)` from the registry; `None` when neither value can be read.
#[cfg(windows)]
fn os_version() -> Option<String> {
    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let display = registry_string(KEY, "DisplayVersion");
    let build = registry_string(KEY, "CurrentBuild");
    match (display, build) {
        (Some(display), Some(build)) => Some(format!("{display} (build {build})")),
        (Some(display), None) => Some(display),
        (None, Some(build)) => Some(format!("build {build}")),
        (None, None) => None,
    }
}

#[cfg(not(windows))]
fn os_version() -> Option<String> {
    None
}

/// A string value under `HKEY_LOCAL_MACHINE`. A missing key or value, a value of another type and one
/// longer than the buffer are all `None`.
#[cfg(windows)]
fn registry_string(subkey: &str, value: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};

    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (subkey, value) = (wide(subkey), wide(value));
    let mut buf = [0u16; 256];
    let mut size = std::mem::size_of_val(&buf) as u32;
    // SAFETY: `subkey` and `value` are NUL-terminated and outlive the call; `buf` is valid for `size`
    // bytes, and `size` is a valid in/out length. The type pointer may be null.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    // `size` counts bytes and includes the terminating NUL.
    let units = (size as usize / 2).min(buf.len());
    let text = String::from_utf16_lossy(&buf[..units]);
    let text = text.trim_end_matches('\0').trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The files as a zip, every entry deflated. Built in memory.
pub fn zip(files: &[BundleFile]) -> anyhow::Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for file in files {
        writer.start_file(file.name.as_str(), options)?;
        writer.write_all(&file.bytes)?;
    }
    Ok(writer.finish()?.into_inner())
}

/// The one prepared export, if any. A new `put` replaces it, and it expires `PREVIEW_TTL` after the `put`.
#[derive(Default)]
pub struct Previews {
    slot: Mutex<Option<Pending>>,
}

struct Pending {
    id: String,
    made: Instant,
    files: Arc<Vec<BundleFile>>,
}

impl Previews {
    /// Keeps `files` and returns the id that names them.
    pub fn put(&self, files: Vec<BundleFile>) -> String {
        self.put_at(files, Instant::now())
    }

    pub fn get(&self, id: &str) -> Option<Arc<Vec<BundleFile>>> {
        self.get_at(id, Instant::now())
    }

    pub(crate) fn put_at(&self, files: Vec<BundleFile>, now: Instant) -> String {
        let id = auth::random_hex(16);
        *self.slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(Pending {
            id: id.clone(),
            made: now,
            files: Arc::new(files),
        });
        id
    }

    pub(crate) fn get_at(&self, id: &str, now: Instant) -> Option<Arc<Vec<BundleFile>>> {
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if slot
            .as_ref()
            .is_some_and(|p| now.saturating_duration_since(p.made) >= PREVIEW_TTL)
        {
            *slot = None;
        }
        slot.as_ref().filter(|p| p.id == id).map(|p| p.files.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::{Profile, ShellFamily};
    use std::io::{Cursor, Read, Write};
    use std::path::Path;

    /// A state that runs no real `claude` and reads no real Claude Code settings.
    fn state(home: &Path, detected: Vec<Profile>) -> AppState {
        AppState::new("t".into(), 1, Vec::new(), home.to_path_buf(), detected)
            .with_claude_settings(home.join("claude-settings.json"))
            .with_claude_version(None)
            .with_agent_path(|_| None)
            .with_agent_check(|_| false)
    }

    fn text_of(files: &[BundleFile], name: &str) -> String {
        let file = files
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("no {name}"));
        String::from_utf8(file.bytes.clone()).unwrap()
    }

    fn names(files: &[BundleFile]) -> Vec<&str> {
        files.iter().map(|f| f.name.as_str()).collect()
    }

    #[test]
    fn export_without_logs_or_claude_still_builds() {
        let home = tempfile::tempdir().unwrap();
        let st = state(home.path(), Vec::new());
        let files = collect_with(&st, &Redactor::new(None, None, None));
        assert_eq!(
            names(&files),
            [
                "README.txt",
                "about.json",
                "settings.json",
                "profiles.json",
                "agents/claude.json",
                "perf.json"
            ]
        );
        let about: serde_json::Value = serde_json::from_str(&text_of(&files, "about.json")).unwrap();
        let clis = about["clis"].as_array().unwrap();
        let agents: Vec<_> = clis.iter().map(|c| c["agent"].clone()).collect();
        assert_eq!(agents, ["claude", "codex", "antigravity"]);
        for cli in clis {
            assert_eq!(cli["found"], false);
            assert!(cli["version"].is_null());
        }
        assert_eq!(about["app_version"], env!("CARGO_PKG_VERSION"));
        assert!(text_of(&files, "README.txt").starts_with("Marshell diagnostics. This holds"));
    }

    #[test]
    fn profiles_lose_quoted_secrets() {
        let home = tempfile::tempdir().unwrap();
        let profile = Profile {
            id: "p1".into(),
            name: "work".into(),
            command: vec![
                "pwsh".into(),
                "-c".into(),
                r#"$env:MY_TOKEN="abc123secret"; claude"#.into(),
                // A backslash in the value: after JSON escaping (`\\`) the escaped-quote rule no longer
                // sees the closing quote, so only the argument on its own is redacted whole.
                r#"$env:MY_API_KEY="C:\keys\hidden-9 two""#.into(),
            ],
            family: ShellFamily::Posix,
            cwd: None,
            env: Vec::new(),
            shell_integration: true,
            terminal_theme: None,
            detected: false,
        };
        let files = collect_with(&state(home.path(), vec![profile]), &Redactor::new(None, None, None));
        let text = text_of(&files, "profiles.json");
        assert!(!text.contains("abc123secret"), "{text}");
        assert!(!text.contains("hidden-9") && !text.contains("two"), "{text}");
        serde_json::from_str::<serde_json::Value>(&text).unwrap();
    }

    #[test]
    fn profiles_lose_env_values_and_secrets() {
        let home = tempfile::tempdir().unwrap();
        let profile = Profile {
            id: "p1".into(),
            name: "work".into(),
            command: vec!["pwsh".into(), "-c".into(), "echo token=abc12345".into()],
            family: ShellFamily::Posix,
            cwd: None,
            env: vec![("ANTHROPIC_API_KEY".into(), "sk-ant-api03-abcdefghijkl".into())],
            shell_integration: true,
            terminal_theme: None,
            detected: false,
        };
        let st = state(home.path(), vec![profile]);
        let files = collect_with(&st, &Redactor::new(None, None, None));
        let text = text_of(&files, "profiles.json");
        assert!(text.contains("\"<removed>\""), "{text}");
        assert!(text.contains("ANTHROPIC_API_KEY"), "the name stays: {text}");
        assert!(!text.contains("sk-ant-"), "{text}");
        assert!(!text.contains("abc12345"), "{text}");
    }

    #[test]
    fn logs_are_included_and_redacted() {
        let home = tempfile::tempdir().unwrap();
        let logs = home.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        // The writer stays open while the export reads: the log is live.
        let mut writer = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(logs.join("marshell.log"))
            .unwrap();
        writeln!(
            writer,
            "started in {} with launch-token-0123456789abcdef",
            home.path().display()
        )
        .unwrap();
        writer.flush().unwrap();
        std::fs::write(logs.join("marshell.log.2"), "older line\n").unwrap();
        std::fs::write(logs.join("crash.log"), "panic at 1\n").unwrap();
        std::fs::write(logs.join("bridge.log.1"), "bridge failed\n").unwrap();

        let redactor = Redactor::new(Some(home.path().to_path_buf()), None, None);
        redactor.remember_token("launch-token-0123456789abcdef");
        let st = state(home.path(), Vec::new());
        let files = collect_with(&st, &redactor);

        assert_eq!(
            names(&files)[6..],
            [
                "logs/marshell.log",
                "logs/marshell.log.2",
                "logs/crash.log",
                "logs/bridge.log.1"
            ]
        );
        let log = text_of(&files, "logs/marshell.log");
        assert!(log.starts_with("started in ~ with <token>"), "{log}");
        assert!(!log.contains(&home.path().display().to_string()), "{log}");
        drop(writer);
    }

    #[test]
    fn zip_round_trips() {
        let files = vec![
            BundleFile {
                name: "README.txt".into(),
                bytes: b"hello hello hello hello hello hello".to_vec(),
            },
            BundleFile {
                name: "logs/marshell.log".into(),
                bytes: "line\n".repeat(500).into_bytes(),
            },
            BundleFile {
                name: "empty.txt".into(),
                bytes: Vec::new(),
            },
        ];
        let bytes = zip(&files).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), files.len());
        for (i, want) in files.iter().enumerate() {
            let mut entry = archive.by_index(i).unwrap();
            assert_eq!(entry.name().unwrap(), want.name);
            assert_eq!(entry.compression(), zip::CompressionMethod::Deflated);
            let mut got = Vec::new();
            entry.read_to_end(&mut got).unwrap();
            assert_eq!(got, want.bytes);
        }
    }

    #[test]
    fn previews_expire_and_replace() {
        let previews = Previews::default();
        let file = |n: &str| {
            vec![BundleFile {
                name: n.into(),
                bytes: vec![1],
            }]
        };
        let t0 = Instant::now();
        let first = previews.put_at(file("a"), t0);
        assert_eq!(first.len(), 32);
        assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
        let almost = t0 + PREVIEW_TTL - Duration::from_secs(1);
        assert_eq!(previews.get_at(&first, almost).unwrap()[0].name, "a");
        assert!(previews.get_at(&first, t0 + PREVIEW_TTL).is_none());
        assert!(previews.get_at("not-an-id", t0).is_none());

        let a = previews.put_at(file("a"), t0);
        let b = previews.put_at(file("b"), t0);
        assert_ne!(a, b);
        assert!(previews.get_at(&a, t0).is_none(), "a new export replaces the old one");
        assert_eq!(previews.get_at(&b, t0).unwrap()[0].name, "b");
    }

    #[cfg(windows)]
    #[test]
    fn the_windows_version_comes_from_the_registry() {
        let key = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        assert!(registry_string(key, "CurrentBuild").is_some());
        assert!(os_version().unwrap().contains("build"));
        assert_eq!(registry_string(key, "NoSuchValueMarshell"), None);
        assert_eq!(registry_string(r"SOFTWARE\NoSuchKeyMarshell", "x"), None);
    }
}
