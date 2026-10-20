use crate::store::{self, Versioned};
use marshell_protocol::api::Endpoint;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(crate) fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("the OS random number generator failed");
    hex::encode(buf)
}

/// 32 random bytes as hex. A new token every launch.
pub fn new_token() -> String {
    random_hex(32)
}

/// Short random id such as `t3f9a0c41b2de` (prefix + 12 hex chars).
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}{}", random_hex(6))
}

/// Compares in constant time, so response timing does not leak how much of a guess was right.
pub fn tokens_match(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `endpoint.json` on disk: the endpoint, the PID of the core that wrote it, and a top-level `"version"`.
/// Readers that do not know `pid` ignore it, so adding it did not need a new version.
#[derive(Serialize, Deserialize)]
struct EndpointFile {
    #[serde(flatten)]
    endpoint: Endpoint,
    pid: u32,
}

impl Versioned for EndpointFile {
    const VERSION: u32 = 1;
}

/// Writes the endpoint file through `store::write`: atomic (temp file + rename), private from the start.
/// On unix the directory is 0700 and the file 0600, so no other user can read it, not even briefly.
/// On Windows the file inherits the ACL of its directory: the home's owner-only ACL (the user and SYSTEM),
/// set by `private::secure_home` before this write.
pub fn write_endpoint_file(path: &Path, ep: &Endpoint) -> anyhow::Result<()> {
    // The old file only names a core that is gone, even one from a newer Marshell, so it is never kept.
    let _ = std::fs::remove_file(path);
    let file = EndpointFile {
        endpoint: ep.clone(),
        pid: std::process::id(),
    };
    store::write(path, &file)
}

/// Deletes the endpoint file, but only if it still holds `token`. A restarted core writes its own file
/// before the old one is gone from memory, so a file with another token belongs to the new core and stays.
/// A missing or unreadable file, or a failed delete, is logged at most.
pub fn remove_endpoint_file_if_ours(path: &Path, token: &str) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let Ok(file) = serde_json::from_slice::<Endpoint>(&bytes) else {
        return;
    };
    if !tokens_match(&file.token, token) {
        return;
    }
    if let Err(e) = std::fs::remove_file(path) {
        tracing::warn!("could not delete {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_protocol::api::Platform;

    #[test]
    fn token_is_64_hex_and_unique() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn ids_have_prefix() {
        let id = new_id("t");
        assert!(id.starts_with('t'));
        assert_eq!(id.len(), 1 + 12);
    }

    #[test]
    fn tokens_match_only_exactly() {
        assert!(tokens_match("abc", "abc"));
        assert!(!tokens_match("abc", "abd"));
        assert!(!tokens_match("abc", "abcd"));
        assert!(!tokens_match("", "a"));
    }

    #[test]
    fn endpoint_file_round_trips_and_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join("endpoint.json");
        let ep = Endpoint {
            port: 4242,
            token: "t0k".into(),
            platform: Platform::current(),
        };
        write_endpoint_file(&path, &ep).unwrap();
        let back: Endpoint = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(back.port, 4242);
        assert_eq!(back.token, "t0k");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn endpoint_file_replaces_one_from_a_newer_marshell() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join("endpoint.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"version":99,"port":1,"token":"old"}"#).unwrap();
        let ep = Endpoint {
            port: 4242,
            token: "t0k".into(),
            platform: Platform::current(),
        };
        write_endpoint_file(&path, &ep).unwrap();
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["version"], 1);
        assert_eq!(raw["token"], "t0k");
    }

    #[test]
    fn endpoint_file_has_version_1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join("endpoint.json");
        let ep = Endpoint {
            port: 4242,
            token: "t0k".into(),
            platform: Platform::current(),
        };
        write_endpoint_file(&path, &ep).unwrap();
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["version"], 1);
        let back: Endpoint = serde_json::from_value(raw).unwrap();
        assert_eq!(back.port, 4242);
    }

    #[test]
    fn endpoint_file_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join("endpoint.json");
        let first = Endpoint {
            port: 1111,
            token: "a".into(),
            platform: Platform::current(),
        };
        let second = Endpoint {
            port: 2222,
            token: "b".into(),
            platform: Platform::current(),
        };
        write_endpoint_file(&path, &first).unwrap();
        write_endpoint_file(&path, &second).unwrap();
        let back: Endpoint = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(back.port, 2222);
        assert_eq!(back.token, "b");
    }

    fn sample(token: &str) -> Endpoint {
        Endpoint {
            port: 4242,
            token: token.into(),
            platform: Platform::current(),
        }
    }

    #[test]
    fn endpoint_file_names_this_process() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join("endpoint.json");
        write_endpoint_file(&path, &sample("t0k")).unwrap();
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["pid"], std::process::id());
        assert_eq!(raw["port"], 4242);
        assert_eq!(raw["token"], "t0k");
    }

    #[test]
    fn removes_the_endpoint_file_when_the_token_is_ours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join("endpoint.json");
        write_endpoint_file(&path, &sample("mine")).unwrap();
        remove_endpoint_file_if_ours(&path, "mine");
        assert!(!path.exists());
    }

    #[test]
    fn keeps_an_endpoint_file_with_another_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join("endpoint.json");
        write_endpoint_file(&path, &sample("newer")).unwrap();
        remove_endpoint_file_if_ours(&path, "older");
        assert!(path.exists());
    }

    #[test]
    fn removing_a_missing_or_broken_endpoint_file_is_harmless() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("endpoint.json");
        remove_endpoint_file_if_ours(&path, "x");
        std::fs::write(&path, "not json").unwrap();
        remove_endpoint_file_if_ours(&path, "x");
        assert!(path.exists());
    }
}
