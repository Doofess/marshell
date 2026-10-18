use std::path::PathBuf;

use marshell_protocol::{api::Endpoint, brand};

/// `MARSHELL_HOME` if set and not empty, else `<user home>/.marshell`.
pub fn home(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    match env(&brand::env_var("HOME")).filter(|h| !h.is_empty()) {
        Some(h) => Some(PathBuf::from(h)),
        None => std::env::home_dir().map(|h| h.join(brand::HOME_DIR_NAME)),
    }
}

/// Reads `<home>/run/endpoint.json`. A missing or unreadable file gives `None`: the app is closed.
pub fn discover(env: &dyn Fn(&str) -> Option<String>) -> Option<(PathBuf, u16, String)> {
    let home = home(env)?;
    let text = std::fs::read_to_string(home.join("run").join("endpoint.json")).ok()?;
    let ep: Endpoint = serde_json::from_str(&text).ok()?;
    Some((home, ep.port, ep.token))
}
