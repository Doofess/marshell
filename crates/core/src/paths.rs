use marshell_protocol::brand;
use std::path::{Path, PathBuf};

/// Root of everything Marshell stores: `$MARSHELL_HOME` if set (tests), else `~/.marshell`.
pub fn home_dir() -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os(brand::env_var("HOME")) {
        return Ok(PathBuf::from(dir));
    }
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("could not find the user's home directory"))?;
    Ok(home.join(brand::HOME_DIR_NAME))
}

/// `<home>/run/endpoint.json`: port + token for the bridge and CLI.
pub fn endpoint_file(home: &Path) -> PathBuf {
    home.join("run").join("endpoint.json")
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
