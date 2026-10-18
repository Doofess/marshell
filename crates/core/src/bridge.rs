use crate::paths;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const EXE_NAME: &str = "marshell-hook";

/// What `install` did to `<home>/bin/marshell-hook`.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BridgeInstall {
    Installed,
    Updated,
    Unchanged,
}

/// The `marshell-hook` program that sits next to the app's own exe, if it was built.
pub fn source_next_to(exe: &Path) -> Option<PathBuf> {
    let source = exe
        .parent()?
        .join(format!("{EXE_NAME}{}", std::env::consts::EXE_SUFFIX));
    source.is_file().then_some(source)
}

/// Deletes the `.old-*` and `.new*` copies an earlier update left in `<home>/bin`. Errors are ignored: a copy
/// whose program still runs stays locked on Windows, and the next launch tries again.
pub fn sweep_leftovers(home: &Path) {
    let Ok(entries) = std::fs::read_dir(paths::bin_dir(home)) else {
        return;
    };
    let old = format!("{EXE_NAME}.old-");
    let new = format!("{EXE_NAME}.new");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&old) || name.starts_with(&new) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Copies `source` to `<home>/bin/marshell-hook` when the bytes differ. A running copy cannot be overwritten on
/// Windows but can be renamed, so the old one moves aside and the new one takes its place.
pub fn install(source: &Path, home: &Path) -> io::Result<BridgeInstall> {
    let bin = paths::bin_dir(home);
    std::fs::create_dir_all(&bin)?;
    sweep_leftovers(home);

    let dest = paths::bridge_exe(home);
    let existed = dest.is_file();
    if existed && std::fs::read(&dest)? == std::fs::read(source)? {
        return Ok(BridgeInstall::Unchanged);
    }

    let suffix = std::env::consts::EXE_SUFFIX;
    let staged = bin.join(format!("{EXE_NAME}.new{suffix}"));
    std::fs::copy(source, &staged)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
    }

    if existed {
        let mut stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        // Never clobber an old copy that still runs under the same name.
        while bin.join(format!("{EXE_NAME}.old-{stamp}{suffix}")).exists() {
            stamp += 1;
        }
        std::fs::rename(&dest, bin.join(format!("{EXE_NAME}.old-{stamp}{suffix}")))?;
    }

    let mut tries = 0;
    loop {
        match std::fs::rename(&staged, &dest) {
            Ok(()) => break,
            Err(e) if tries >= 4 => return Err(e),
            Err(_) => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    Ok(if existed {
        BridgeInstall::Updated
    } else {
        BridgeInstall::Installed
    })
}
