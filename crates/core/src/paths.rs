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

/// `<home>/logs/marshell.log`: the core's own log. Older copies are `marshell.log.1` to `.4`.
pub fn marshell_log(home: &Path) -> PathBuf {
    home.join("logs").join("marshell.log")
}

/// `<home>/logs/crash.log`: one line per panic.
pub fn crash_log(home: &Path) -> PathBuf {
    home.join("logs").join("crash.log")
}

/// `<home>/history`: recordings, the transcript archive and the sessions index.
pub fn history_dir(home: &Path) -> PathBuf {
    home.join("history")
}

/// `<home>/history/archive/<agent>`: compressed transcript copies and their sidecars.
pub fn archive_dir(home: &Path, agent: &str) -> PathBuf {
    history_dir(home).join("archive").join(agent)
}

/// `<home>/history/index.sqlite`: the sessions index.
pub fn index_file(home: &Path) -> PathBuf {
    history_dir(home).join("index.sqlite")
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

/// `<home>/bin/marshell` (`.exe` on Windows): the command line program.
pub fn cli_exe(home: &Path) -> PathBuf {
    bin_dir(home).join(format!("{}{}", brand::CLI_BIN, std::env::consts::EXE_SUFFIX))
}

/// `<home>/logs/bridge.log`: one line per bridge failure.
pub fn bridge_log(home: &Path) -> PathBuf {
    home.join("logs").join("bridge.log")
}

/// `<home>/agents/<agent>/statusline-chain.json`: the status line command the bridge forwards to.
pub fn statusline_chain(home: &Path, agent: &str) -> PathBuf {
    home.join("agents").join(agent).join("statusline-chain.json")
}

/// `<home>/snapshots`: `index.json` and one folder of file copies per snapshot.
pub fn snapshots_dir(home: &Path) -> PathBuf {
    home.join("snapshots")
}

/// `<home>/managed.json`: the config entries Marshell wrote.
pub fn managed_file(home: &Path) -> PathBuf {
    home.join("managed.json")
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
