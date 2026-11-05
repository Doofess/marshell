//! Paths and distros for agents running inside WSL. CLI-neutral: nothing here knows which agent runs.
//! It works on strings, so it behaves the same on every host.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use crate::profiles::detect::{parse_wsl_list, run_wsl};

/// How long `wsl.exe --list` may take.
const LIST_TIMEOUT: Duration = Duration::from_secs(8);

/// The distro a `wsl.exe` argv names: the value after `-d`, `--distribution` or `--distribution=`.
/// Options after `--exec`, `-e` or `--` belong to the Linux command, so they don't count.
pub fn distro_of_argv(argv: &[String]) -> Option<String> {
    let mut args = argv.iter().skip(1).map(String::as_str);
    while let Some(arg) = args.next() {
        let value = match arg {
            "--exec" | "-e" | "--" => return None,
            "-d" | "--distribution" => args.next(),
            _ => arg.strip_prefix("--distribution="),
        };
        if let Some(value) = value {
            return Some(value.to_string()).filter(|v| !v.is_empty());
        }
    }
    None
}

/// The default distro, asked of the default distro itself (`$WSL_DISTRO_NAME`), so it works on any Windows language.
/// Only a running tab's bare `wsl.exe` asks, so that distro is already running. Remembered once found; a lookup that
/// fails is tried again next time. None off Windows or when WSL won't say.
pub fn default_distro() -> Option<String> {
    static DEFAULT: Mutex<Option<String>> = Mutex::new(None);
    cached_default(&DEFAULT, || {
        let out = run_wsl(&["--exec", "sh", "-c", r#"printf %s "$WSL_DISTRO_NAME""#], LIST_TIMEOUT);
        let name = String::from_utf8_lossy(&out);
        let name = name.trim_matches(|c: char| c == '\0' || c.is_whitespace());
        Some(name.to_string()).filter(|n| !n.is_empty())
    })
}

/// The remembered default distro, else what `fetch` finds, which is remembered only when it found one. The lock is
/// not held while `fetch` runs `wsl.exe`.
fn cached_default(cache: &Mutex<Option<String>>, fetch: impl FnOnce() -> Option<String>) -> Option<String> {
    if let Some(found) = cache.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Some(found);
    }
    let found = fetch()?;
    *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(found.clone());
    Some(found)
}

/// Docker Desktop's internal distros, which are not shells to launch in nor homes to clean.
fn is_internal_distro(name: &str) -> bool {
    name == "docker-desktop" || name == "docker-desktop-data"
}

/// The distros worth listing: `names` without Docker Desktop's internal ones.
pub fn user_distros(names: Vec<String>) -> Vec<String> {
    names.into_iter().filter(|name| !is_internal_distro(name)).collect()
}

/// The user's distros from the raw bytes of `wsl.exe -l -q` (UTF-16LE, usually with a BOM).
pub fn parse_distro_list(bytes: &[u8]) -> Vec<String> {
    user_distros(parse_wsl_list(bytes))
}

/// The installed distros, from `wsl.exe -l -q`. Blocks up to `timeout`, so async callers go through `spawn_blocking`.
/// Empty off Windows and when `wsl.exe` is missing, fails or is too slow.
pub fn list_distros(timeout: Duration) -> Vec<String> {
    parse_distro_list(&run_wsl(&["-l", "-q"], timeout))
}

/// The distros that are running now, from `wsl.exe -l --running -q`. Listing starts nothing. Blocks up to `timeout`, so
/// async callers go through `spawn_blocking`. Empty off Windows and when `wsl.exe` is missing, fails or is too slow;
/// when none runs the output is a sentence, not a name, which the caller never matches to a detected distro.
pub fn list_running(timeout: Duration) -> Vec<String> {
    parse_distro_list(&run_wsl(&["-l", "--running", "-q"], timeout))
}

/// `linux_home` as a path under the distro's own share. None for an empty or relative home, one with `..`, and one
/// outside the share (a home under `/mnt/c` is a folder of Windows, and a distro's files must never land there).
pub fn home_in_share(distro: &str, linux_home: &str) -> Option<PathBuf> {
    to_windows(distro, linux_home).filter(|home| under_distro(distro, home))
}

/// A distro's home as a path under its share, asked of the distro itself (`$HOME`). Starts the distro if it is
/// stopped and blocks up to `timeout`, so async callers go through `spawn_blocking`.
pub fn distro_home(distro: &str, timeout: Duration) -> Option<PathBuf> {
    // A Linux program's output through `--exec` is UTF-8.
    let out = run_wsl(
        &["-d", distro, "--exec", "sh", "-c", r#"printf "%s\n" "$HOME""#],
        timeout,
    );
    let out = String::from_utf8_lossy(&out);
    let home = out.lines().next()?.trim_end_matches('\r');
    home_in_share(distro, home)
}

/// A distro name that can sit in a UNC path as one component.
fn valid_distro(distro: &str) -> bool {
    !distro.is_empty() && !distro.contains(['/', '\\', '\0'])
}

/// The components of a path, split on the separators. None when one is `..`. Empty and `.` components are dropped.
fn components<'a>(text: &'a str, separators: &[char]) -> Option<Vec<&'a str>> {
    let parts: Vec<&str> = text
        .split(separators)
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    (!parts.contains(&"..")).then_some(parts)
}

/// A distro's Linux path as a Windows path: a drive under `/mnt`, otherwise the distro's UNC share.
/// None for a relative or empty path, or one with a `..` component.
pub fn to_windows(distro: &str, linux: &str) -> Option<PathBuf> {
    if !linux.starts_with('/') {
        return None;
    }
    let parts = components(linux, &['/'])?;
    // A backslash would turn one Linux component into several Windows ones.
    if parts.iter().any(|part| part.contains(['\\', '\0'])) {
        return None;
    }
    if let [mnt, drive, rest @ ..] = parts.as_slice()
        && *mnt == "mnt"
        && let [letter] = drive.as_bytes()
        && letter.is_ascii_alphabetic()
    {
        let drive = letter.to_ascii_uppercase() as char;
        return Some(PathBuf::from(format!("{drive}:\\{}", rest.join("\\"))));
    }
    if !valid_distro(distro) {
        return None;
    }
    Some(PathBuf::from(format!(r"\\wsl.localhost\{distro}\{}", parts.join("\\"))))
}

/// What follows `\\wsl.localhost\<distro>` or `\\wsl$\<distro>` in the path. None for any other path or a `..`.
fn under_share<'a>(distro: &str, text: &'a str) -> Option<Vec<&'a str>> {
    if !valid_distro(distro) {
        return None;
    }
    let rest = text.strip_prefix(r"\\").or_else(|| text.strip_prefix("//"))?;
    let (host, rest) = rest.split_once(['\\', '/'])?;
    let (share, rest) = rest.split_once(['\\', '/']).unwrap_or((rest, ""));
    let on_wsl = host.eq_ignore_ascii_case("wsl.localhost") || host.eq_ignore_ascii_case("wsl$");
    if !on_wsl || share.to_lowercase() != distro.to_lowercase() {
        return None;
    }
    components(rest, &['\\', '/'])
}

/// A Windows path as the distro sees it. None for another distro, a network share, or a relative path.
pub fn to_linux(distro: &str, windows: &Path) -> Option<String> {
    let text = windows.to_str()?;
    if let Some(parts) = under_share(distro, text) {
        return Some(format!("/{}", parts.join("/")));
    }
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let rest = &text[2..];
        if !rest.is_empty() && !rest.starts_with(['\\', '/']) {
            return None;
        }
        let parts = components(rest, &['\\', '/'])?;
        let drive = bytes[0].to_ascii_lowercase() as char;
        return Some(
            std::iter::once(format!("/mnt/{drive}"))
                .chain(parts.iter().map(|part| format!("/{part}")))
                .collect(),
        );
    }
    None
}

/// Whether the path is under the distro's UNC share, as `\\wsl.localhost\<distro>` or `\\wsl$\<distro>`.
pub fn under_distro(distro: &str, path: &Path) -> bool {
    path.to_str().and_then(|text| under_share(distro, text)).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(text: &str) -> Option<PathBuf> {
        Some(PathBuf::from(text))
    }

    #[test]
    fn to_windows_maps_mnt_drives() {
        assert_eq!(to_windows("Ubuntu", "/mnt/c/Users/x"), win(r"C:\Users\x"));
        assert_eq!(to_windows("Ubuntu", "/mnt/D"), win(r"D:\"));
        assert_eq!(to_windows("Ubuntu", "/mnt/C/x"), win(r"C:\x"));
        assert_eq!(to_windows("Ubuntu", "/mnt/c/"), win(r"C:\"));
        assert_eq!(to_windows("Ubuntu", "/mnt/c/x/"), win(r"C:\x"));
    }

    #[test]
    fn to_windows_maps_the_rest_to_the_distro_unc() {
        assert_eq!(
            to_windows("Ubuntu", "/tmp/s11/proj"),
            win(r"\\wsl.localhost\Ubuntu\tmp\s11\proj")
        );
        assert_eq!(
            to_windows("Ubuntu", "/mnt/wsl/x"),
            win(r"\\wsl.localhost\Ubuntu\mnt\wsl\x")
        );
        assert_eq!(to_windows("Ubuntu", "/mnt"), win(r"\\wsl.localhost\Ubuntu\mnt"));
        assert_eq!(to_windows("Ubuntu", "/mnt/"), win(r"\\wsl.localhost\Ubuntu\mnt"));
        assert_eq!(to_windows("Ubuntu", "/"), win(r"\\wsl.localhost\Ubuntu\"));
        assert_eq!(to_windows("Ubuntu", "/home/a/"), win(r"\\wsl.localhost\Ubuntu\home\a"));
        assert_eq!(
            to_windows("My Distro", "/home/a"),
            win(r"\\wsl.localhost\My Distro\home\a")
        );
        assert_eq!(
            to_windows("Ubuntu-24.04", "/srv"),
            win(r"\\wsl.localhost\Ubuntu-24.04\srv")
        );
    }

    #[test]
    fn to_windows_refuses_relative_dotdot_and_empty() {
        assert_eq!(to_windows("Ubuntu", "tmp/x"), None);
        assert_eq!(to_windows("Ubuntu", "/tmp/../etc"), None);
        assert_eq!(to_windows("Ubuntu", "/mnt/c/../x"), None);
        assert_eq!(to_windows("Ubuntu", ".."), None);
        assert_eq!(to_windows("Ubuntu", ""), None);
        // A backslash would turn one Linux component into several Windows ones.
        assert_eq!(to_windows("Ubuntu", r"/tmp/a\..\b"), None);
        assert_eq!(to_windows("", "/tmp"), None);
        assert_eq!(to_windows(r"a\b", "/tmp"), None);
    }

    #[test]
    fn to_linux_inverts() {
        let linux = |distro, text| to_linux(distro, Path::new(text));
        assert_eq!(
            linux("Ubuntu", r"\\wsl.localhost\ubuntu\home\a"),
            Some("/home/a".into())
        );
        assert_eq!(linux("Ubuntu", r"\\wsl$\Ubuntu\home\a"), Some("/home/a".into()));
        assert_eq!(linux("Ubuntu", r"\\wsl.localhost\Ubuntu\"), Some("/".into()));
        assert_eq!(linux("Ubuntu", r"\\wsl.localhost\Ubuntu"), Some("/".into()));
        assert_eq!(
            linux("My Distro", r"\\wsl.localhost\My Distro\a b"),
            Some("/a b".into())
        );
        assert_eq!(linux("Ubuntu", r"C:\x\y"), Some("/mnt/c/x/y".into()));
        assert_eq!(linux("Ubuntu", r"c:\x"), Some("/mnt/c/x".into()));
        assert_eq!(linux("Ubuntu", r"D:\"), Some("/mnt/d".into()));
        assert_eq!(linux("Ubuntu", r"\\server\share\x"), None);
        assert_eq!(linux("Ubuntu", r"\\wsl.localhost\Debian\x"), None);
        assert_eq!(linux("Ubuntu", r"\\wsl.localhost\Ubuntu2\x"), None);
        assert_eq!(linux("Ubuntu", r"\\wsl.localhost\Ubuntu\a\..\b"), None);
        assert_eq!(linux("Ubuntu", r"C:\x\..\y"), None);
        assert_eq!(linux("Ubuntu", r"C:x"), None);
        assert_eq!(linux("Ubuntu", r"x\y"), None);
        assert_eq!(linux("Ubuntu", ""), None);
        assert_eq!(linux("", r"\\wsl.localhost\\x"), None);
    }

    #[test]
    fn to_linux_round_trips_to_windows() {
        for linux in ["/", "/home/a", "/mnt/wsl/x", "/mnt/c/Users/x", "/mnt/d"] {
            let windows = to_windows("My Distro", linux).unwrap();
            assert_eq!(to_linux("My Distro", &windows).as_deref(), Some(linux));
        }
    }

    #[test]
    fn under_distro_accepts_only_that_distro() {
        let under = |distro, text| under_distro(distro, Path::new(text));
        assert!(under("Ubuntu", r"\\wsl.localhost\Ubuntu\home\a\.claude\x.jsonl"));
        assert!(under("Ubuntu", r"\\wsl$\ubuntu\home\a"));
        assert!(under("Ubuntu", r"\\WSL.LOCALHOST\UBUNTU\home"));
        assert!(under("My Distro", r"\\wsl.localhost\My Distro\home"));
        assert!(!under("Ubuntu", r"\\wsl.localhost\Debian\home"));
        assert!(!under("Ubuntu", r"\\wsl.localhost\Ubuntu-24.04\home"));
        assert!(!under("Ubuntu", r"\\server\share\home"));
        assert!(!under("Ubuntu", r"\\server\wsl.localhost\Ubuntu\home"));
        assert!(!under("Ubuntu", r"C:\Users\a"));
        assert!(!under("Ubuntu", r"\\wsl.localhost\Ubuntu\home\..\..\Debian"));
        assert!(!under("Ubuntu", r"\\wsl.localhost\Ubuntu\a\..\b"));
        assert!(!under("Ubuntu", r"\\?\UNC\wsl.localhost\Ubuntu\home"));
        assert!(!under("Ubuntu", ""));
        assert!(!under("", r"\\wsl.localhost\\home"));
    }

    #[test]
    fn a_distro_home_is_the_distros_own_share() {
        assert_eq!(
            home_in_share("Ubuntu", "/home/me"),
            win(r"\\wsl.localhost\Ubuntu\home\me")
        );
        // A home on a Windows drive, an empty or relative one, and one with `..`: none.
        for home in [
            "/mnt/c/Users/me",
            "/mnt/c",
            "/mnt/c/Users/x y",
            "",
            "me",
            "/home/../mnt/c/x",
        ] {
            assert_eq!(home_in_share("Ubuntu", home), None, "{home:?}");
        }
        assert_eq!(home_in_share("", "/home/me"), None);
    }

    #[test]
    fn the_distro_list_drops_docker_desktops_own() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "Ubuntu\r\ndocker-desktop\r\nDebian\r\ndocker-desktop-data\r\n".encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        assert_eq!(parse_distro_list(&bytes), ["Ubuntu", "Debian"]);
        assert!(parse_distro_list(&[]).is_empty());
        assert_eq!(
            user_distros(vec!["docker-desktop".into(), "Docker-Desktop-Mine".into()]),
            ["Docker-Desktop-Mine"]
        );
    }

    #[test]
    fn a_failed_default_lookup_is_tried_again_and_a_found_one_is_kept() {
        let cache = Mutex::new(None);
        let calls = std::cell::Cell::new(0);
        let fetch = |answer: Option<&str>| {
            calls.set(calls.get() + 1);
            answer.map(String::from)
        };
        assert_eq!(cached_default(&cache, || fetch(None)), None);
        assert_eq!(cached_default(&cache, || fetch(None)), None);
        assert_eq!(calls.get(), 2);
        assert_eq!(
            cached_default(&cache, || fetch(Some("Ubuntu"))).as_deref(),
            Some("Ubuntu")
        );
        assert_eq!(calls.get(), 3);
        assert_eq!(
            cached_default(&cache, || fetch(Some("Debian"))).as_deref(),
            Some("Ubuntu")
        );
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn distro_of_argv_reads_each_form() {
        let argv = |parts: &[&str]| parts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            distro_of_argv(&argv(&["wsl.exe", "-d", "My Distro"])).as_deref(),
            Some("My Distro")
        );
        assert_eq!(
            distro_of_argv(&argv(&["wsl.exe", "--distribution", "Debian"])).as_deref(),
            Some("Debian")
        );
        assert_eq!(
            distro_of_argv(&argv(&["wsl.exe", "--distribution=Ubuntu-24.04"])).as_deref(),
            Some("Ubuntu-24.04")
        );
        assert_eq!(distro_of_argv(&argv(&["wsl.exe"])), None);
        assert_eq!(distro_of_argv(&argv(&["wsl.exe", "-d"])), None);
        assert_eq!(distro_of_argv(&argv(&["wsl.exe", "--distribution="])), None);
        // After --exec the rest is the Linux command, not wsl.exe's own options.
        assert_eq!(distro_of_argv(&argv(&["wsl.exe", "--exec", "tool", "-d", "x"])), None);
    }
}
