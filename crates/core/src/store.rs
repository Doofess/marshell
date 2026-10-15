//! Files Marshell keeps in `~/.marshell`. Every file carries a top-level `"version"`, is written
//! atomically (temp file + rename), keeps the previous copy as `<name>.bak`, and is migrated when
//! an older version is read.

use anyhow::Context;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A type stored in a versioned file.
pub trait Versioned: Serialize + DeserializeOwned {
    /// The version this build writes. Bump it when the shape changes, and teach `migrate` the step.
    const VERSION: u32;

    /// Turns the JSON of version `from` into the JSON of version `from + 1`.
    /// A file without a `"version"` field counts as version 0.
    fn migrate(_from: u32, value: Value) -> anyhow::Result<Value> {
        Ok(value)
    }
}

/// `prefs.json` → `prefs.json.bak`, in the same folder.
pub fn bak_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Writes `value` with its version. The file that was there before is kept as `.bak`
/// (unless it was not valid JSON, so a good backup is never replaced by a broken file).
pub fn write<T: Versioned>(path: &Path, value: &T) -> anyhow::Result<()> {
    let mut json = serde_json::to_value(value)?;
    let fields = json
        .as_object_mut()
        .with_context(|| format!("{} must be a JSON object", path.display()))?;
    fields.insert("version".into(), T::VERSION.into());

    let dir = path
        .parent()
        .with_context(|| format!("{} has no parent folder", path.display()))?;
    create_private_dir(dir)?;
    if let Ok(old) = std::fs::read(path)
        && let Ok(old) = serde_json::from_slice::<Value>(&old)
    {
        let version = old.get("version").and_then(Value::as_u64).unwrap_or(0);
        if version > u64::from(T::VERSION) {
            anyhow::bail!(
                "{} was written by a newer Marshell (version {version}); this one knows up to {}",
                path.display(),
                T::VERSION
            );
        }
        std::fs::copy(path, bak_path(path))?;
    }

    let name = path.file_name().unwrap_or_default().to_string_lossy();
    // Unique per call, so two writers in this process never share a temp file.
    let n = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".{name}.{}.{n}.tmp", std::process::id()));
    let result =
        write_private_file(&tmp, &serde_json::to_vec_pretty(&json)?).and_then(|()| Ok(std::fs::rename(&tmp, path)?));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Reads the file, or its `.bak` when the file is missing or broken. `Ok(None)` when neither exists.
/// An older version is migrated and written back. A newer version is an error and the file is left alone.
pub fn read<T: Versioned>(path: &Path) -> anyhow::Result<Option<T>> {
    let main = match load::<T>(path)? {
        Load::Ok { value, migrated } => {
            if migrated {
                write(path, &value)?;
            }
            return Ok(Some(value));
        }
        other => other,
    };
    match load::<T>(&bak_path(path))? {
        Load::Ok { value, .. } => Ok(Some(value)),
        Load::Missing => match main {
            Load::Broken(err) => Err(err),
            _ => Ok(None),
        },
        Load::Broken(err) => match main {
            Load::Broken(main_err) => Err(main_err),
            _ => Err(err),
        },
    }
}

enum Load<T> {
    Missing,
    Broken(anyhow::Error),
    Ok { value: T, migrated: bool },
}

/// Only a newer version comes back as `Err`. Anything else that goes wrong is `Load::Broken`,
/// so the caller can try the backup.
fn load<T: Versioned>(path: &Path) -> anyhow::Result<Load<T>> {
    let broken = |err: anyhow::Error, what: &str| Ok(Load::Broken(err.context(format!("{} {what}", path.display()))));
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Load::Missing),
        Err(err) => return broken(err.into(), "cannot be read"),
    };
    let mut json: Value = match serde_json::from_slice(&bytes) {
        Ok(json) => json,
        Err(err) => return broken(err.into(), "is not valid JSON"),
    };
    let version = json.get("version").and_then(Value::as_u64).unwrap_or(0) as u32;
    if version > T::VERSION {
        anyhow::bail!(
            "{} was written by a newer Marshell (version {version}); this one knows up to {}",
            path.display(),
            T::VERSION
        );
    }
    if let Some(fields) = json.as_object_mut() {
        fields.remove("version");
    }
    for from in version..T::VERSION {
        json = match T::migrate(from, json) {
            Ok(next) => next,
            Err(err) => return broken(err, "cannot be migrated"),
        };
    }
    match serde_json::from_value(json) {
        Ok(value) => Ok(Load::Ok {
            value,
            migrated: version < T::VERSION,
        }),
        Err(err) => broken(err.into(), "has an unexpected shape"),
    }
}

/// On unix the folder is 0700, so no other user can list or read what is in it.
pub(crate) fn create_private_dir(dir: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    Ok(())
}

/// Creates the file 0600 on unix from the start, and flushes it to disk before returning.
pub(crate) fn write_private_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Prefs {
        name: String,
    }

    impl Versioned for Prefs {
        const VERSION: u32 = 2;
        fn migrate(from: u32, mut value: serde_json::Value) -> anyhow::Result<serde_json::Value> {
            if from == 1
                && let Some(obj) = value.as_object_mut()
                && let Some(title) = obj.remove("title")
            {
                obj.insert("name".into(), title);
            }
            Ok(value)
        }
    }

    fn prefs(name: &str) -> Prefs {
        Prefs { name: name.into() }
    }

    fn json_at(path: &Path) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn writes_version_at_top_level() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        write(&path, &prefs("a")).unwrap();
        let v = json_at(&path);
        assert_eq!(v["version"], 2);
        assert_eq!(v["name"], "a");
    }

    #[test]
    fn keeps_previous_as_bak() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        write(&path, &prefs("a")).unwrap();
        write(&path, &prefs("b")).unwrap();
        assert_eq!(read::<Prefs>(&path).unwrap(), Some(prefs("b")));
        assert_eq!(json_at(&bak_path(&path))["name"], "a");
    }

    #[test]
    fn reads_bak_when_main_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        write(&path, &prefs("a")).unwrap();
        write(&path, &prefs("b")).unwrap();
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(read::<Prefs>(&path).unwrap(), Some(prefs("a")));
    }

    #[test]
    fn migrates_old_version_and_rewrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, r#"{"version":1,"title":"x"}"#).unwrap();
        assert_eq!(read::<Prefs>(&path).unwrap(), Some(prefs("x")));
        assert_eq!(json_at(&path)["version"], 2);
        assert_eq!(json_at(&bak_path(&path))["version"], 1);
    }

    #[test]
    fn refuses_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        let newer = r#"{"version":3,"name":"x"}"#;
        std::fs::write(&path, newer).unwrap();
        let err = read::<Prefs>(&path).unwrap_err();
        assert!(err.to_string().contains("newer Marshell"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
    }

    #[test]
    fn write_refuses_to_replace_a_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        let newer = r#"{"version":3,"name":"x"}"#;
        std::fs::write(&path, newer).unwrap();
        let err = write(&path, &Prefs { name: "old".into() }).unwrap_err();
        assert!(err.to_string().contains("newer Marshell"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
        assert!(!bak_path(&path).exists());
    }

    #[test]
    fn missing_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read::<Prefs>(&dir.path().join("prefs.json")).unwrap(), None);
    }

    #[test]
    fn no_temp_file_left() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        write(&path, &prefs("a")).unwrap();
        write(&path, &prefs("b")).unwrap();
        let mut names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["prefs.json", "prefs.json.bak"]);
    }

    #[test]
    fn concurrent_writes_to_one_path_both_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prefs.json");
        write(&path, &prefs("seed")).unwrap();
        for round in 0..20 {
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let handles: Vec<_> = ["one", "two"]
                .into_iter()
                .map(|name| {
                    let (path, barrier) = (path.clone(), barrier.clone());
                    std::thread::spawn(move || {
                        barrier.wait();
                        write(&path, &prefs(name))
                    })
                })
                .collect();
            for handle in handles {
                handle.join().unwrap().unwrap_or_else(|e| panic!("round {round}: {e:#}"));
            }
            let saved = read::<Prefs>(&path).unwrap().unwrap();
            assert!(saved == prefs("one") || saved == prefs("two"), "round {round}: {saved:?}");
        }
    }
}
