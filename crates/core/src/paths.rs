use marshell_protocol::brand;
use std::path::{Path, PathBuf};

/// Root of everything Marshell stores: `$MARSHELL_HOME` if set (tests), else `~/.marshell`.
pub fn home_dir() -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os(brand::env_var("HOME")) {
        return Ok(PathBuf::from(dir));
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not find the user's home directory"))?;
    Ok(home.join(brand::HOME_DIR_NAME))
}

/// `<home>/run/endpoint.json`: port + token for the bridge and CLI.
pub fn endpoint_file(home: &Path) -> PathBuf {
    home.join("run").join("endpoint.json")
}

/// `<home>/settings.json`: how the app looks (theme, accent, terminal colours and font).
pub fn settings_file(home: &Path) -> PathBuf {
    home.join("settings.json")
}

/// `<home>/layout.json`: the tabs that were open and how the window was laid out.
pub fn layout_file(home: &Path) -> PathBuf {
    home.join("layout.json")
}

/// `<home>/logs/crash.log`: one line per panic.
pub fn crash_log(home: &Path) -> PathBuf {
    home.join("logs").join("crash.log")
}

/// `<home>/history/recordings`: one asciicast file per session run, `<tab>-<unix ms>.cast`.
pub fn recordings_dir(home: &Path) -> PathBuf {
    home.join("history").join("recordings")
}

/// `<home>/attachments`: the images saved for a session, one folder per tab.
pub fn attachments_dir(home: &Path) -> PathBuf {
    home.join("attachments")
}

/// `<home>/shell`: the shell integration scripts, owned and rewritten by Marshell.
pub fn shell_dir(home: &Path) -> PathBuf {
    home.join("shell")
}

/// `<home>/bin`: the bridge program the agents' hooks run.
pub fn bin_dir(home: &Path) -> PathBuf {
    home.join("bin")
}

/// `<home>/bin/marshell-hook` (`.exe` on Windows).
pub fn bridge_exe(home: &Path) -> PathBuf {
    bin_dir(home).join(format!("marshell-hook{}", std::env::consts::EXE_SUFFIX))
}

/// `<home>/logs/bridge.log`: one line per bridge failure.
pub fn bridge_log(home: &Path) -> PathBuf {
    home.join("logs").join("bridge.log")
}

/// `<home>/agents/<agent>/statusline-chain.json`: the status line command the bridge forwards to.
pub fn statusline_chain(home: &Path, agent: &str) -> PathBuf {
    home.join("agents").join(agent).join("statusline-chain.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_file_is_under_run() {
        let p = endpoint_file(Path::new("/x/.marshell"));
        assert!(p.ends_with("run/endpoint.json") || p.ends_with("run\\endpoint.json"));
    }
}
