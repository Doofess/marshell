use marshell_protocol::api::Endpoint;
use std::path::Path;

fn random_hex(bytes: usize) -> String {
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
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Writes the endpoint file atomically (temp file + rename) and creates it private from the start.
/// On unix the directory is 0700 and the file 0600, so no other user can read it, not even briefly.
/// On Windows the file inherits the ACL of its directory, which is user-only under the default
/// `~/.marshell`. An explicit owner-only ACL comes when the hook bridge starts reading this file (phase 3).
pub fn write_endpoint_file(path: &Path, ep: &Endpoint) -> anyhow::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("endpoint path has no parent"))?;
    create_private_dir(dir)?;
    let tmp = dir.join(format!(".endpoint.{}.tmp", std::process::id()));
    let result = write_private_file(&tmp, &serde_json::to_vec_pretty(ep)?)
        .and_then(|()| Ok(std::fs::rename(&tmp, path)?));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn create_private_dir(dir: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    Ok(())
}

fn write_private_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
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
}
