//! Paths and distros for agents running inside WSL. CLI-neutral: nothing here knows which agent runs.
//! It works on strings, so it behaves the same on every host.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use crate::profiles::detect::{decode_wsl_output, run_wsl};

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

/// The default distro, from `wsl.exe --list`. Read once per process. None off Windows or when WSL won't say.
pub fn default_distro() -> Option<String> {
    static DEFAULT: OnceLock<Option<String>> = OnceLock::new();
    DEFAULT
        .get_or_init(|| parse_default_distro(&run_wsl(&["--list"], LIST_TIMEOUT)))
        .clone()
}

/// The distro marked `(Default)` in `wsl.exe --list` output.
fn parse_default_distro(stdout: &[u8]) -> Option<String> {
    decode_wsl_output(stdout).lines().find_map(|line| {
        let line = line.trim_matches(|c: char| c == '\0' || c == '\u{feff}' || c.is_whitespace());
        let name = line.strip_suffix("(Default)")?.trim_end();
        Some(name.to_string()).filter(|n| !n.is_empty())
    })
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
    fn the_default_distro_is_the_one_marked_default() {
        let utf16 = |text: &str| -> Vec<u8> { text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect() };
        let list = utf16("\u{feff}Windows Subsystem for Linux Distributions:\r\nMy Distro (Default)\r\nDebian\r\n");
        assert_eq!(parse_default_distro(&list).as_deref(), Some("My Distro"));
        assert_eq!(
            parse_default_distro(b"Ubuntu (Default)\nDebian\n").as_deref(),
            Some("Ubuntu")
        );
        assert_eq!(parse_default_distro(b"Ubuntu\nDebian\n"), None);
        assert_eq!(parse_default_distro(b""), None);
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
