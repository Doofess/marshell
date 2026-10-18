use marshell_core::bridge::{self, BridgeInstall};
use marshell_core::paths;
use std::fs;
use std::path::{Path, PathBuf};

fn write_source(dir: &Path, bytes: &[u8]) -> PathBuf {
    let source = dir.join(format!("source-hook{}", std::env::consts::EXE_SUFFIX));
    fs::write(&source, bytes).unwrap();
    source
}

fn leftovers(home: &Path, prefix: &str) -> Vec<String> {
    fs::read_dir(paths::bin_dir(home))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with(prefix))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn installs_into_an_empty_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let source = write_source(dir.path(), b"one");
    assert_eq!(bridge::install(&source, &home).unwrap(), BridgeInstall::Installed);
    assert_eq!(fs::read(paths::bridge_exe(&home)).unwrap(), b"one");
}

#[test]
fn same_bytes_are_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let source = write_source(dir.path(), b"one");
    bridge::install(&source, dir.path()).unwrap();
    assert_eq!(bridge::install(&source, dir.path()).unwrap(), BridgeInstall::Unchanged);
    assert!(leftovers(dir.path(), "marshell-hook.old-").is_empty());
}

#[test]
fn new_bytes_update_and_leave_an_old_copy() {
    let dir = tempfile::tempdir().unwrap();
    let source = write_source(dir.path(), b"one");
    bridge::install(&source, dir.path()).unwrap();
    let source = write_source(dir.path(), b"two");
    assert_eq!(bridge::install(&source, dir.path()).unwrap(), BridgeInstall::Updated);
    assert_eq!(fs::read(paths::bridge_exe(dir.path())).unwrap(), b"two");
    assert_eq!(leftovers(dir.path(), "marshell-hook.old-").len(), 1);
    assert!(leftovers(dir.path(), "marshell-hook.new").is_empty());
}

#[test]
fn sweep_removes_leftovers() {
    let dir = tempfile::tempdir().unwrap();
    let bin = paths::bin_dir(dir.path());
    fs::create_dir_all(&bin).unwrap();
    for name in [
        "marshell-hook.old-1.exe",
        "marshell-hook.new.exe",
        "marshell-hook.exe",
        "other.txt",
    ] {
        fs::write(bin.join(name), b"x").unwrap();
    }
    bridge::sweep_leftovers(dir.path());
    assert!(leftovers(dir.path(), "marshell-hook.old-").is_empty());
    assert!(leftovers(dir.path(), "marshell-hook.new").is_empty());
    assert!(bin.join("marshell-hook.exe").exists());
    assert!(bin.join("other.txt").exists());
}

#[cfg(windows)]
struct KillOnDrop(std::process::Child);

#[cfg(windows)]
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(windows)]
#[test]
fn updates_while_the_old_bridge_is_running() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    fs::create_dir_all(paths::bin_dir(home)).unwrap();
    let installed = paths::bridge_exe(home);
    fs::copy(env!("CARGO_BIN_EXE_fake-agent"), &installed).unwrap();
    // fake-agent's `sleep` runs for 60 s; the guard kills it first.
    let mut child = KillOnDrop(std::process::Command::new(&installed).arg("sleep").spawn().unwrap());
    std::thread::sleep(std::time::Duration::from_millis(300));

    let source = write_source(dir.path(), b"a different bridge");
    assert_eq!(bridge::install(&source, home).unwrap(), BridgeInstall::Updated);
    assert_eq!(fs::read(&installed).unwrap(), b"a different bridge");
    assert_eq!(leftovers(home, "marshell-hook.old-").len(), 1);

    child.0.kill().unwrap();
    child.0.wait().unwrap();
    for _ in 0..20 {
        bridge::sweep_leftovers(home);
        if leftovers(home, "marshell-hook.old-").is_empty() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("the old copy is still there after the bridge exited");
}
