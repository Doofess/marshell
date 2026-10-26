//! How an agent CLI was installed, and the command that updates it.

use marshell_protocol::install::InstallKind;
use std::path::{Component, Path};
use std::sync::LazyLock;

/// The npm package that ships an agent's CLI, when it has one.
pub fn npm_package(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("@anthropic-ai/claude-code"),
        "codex" => Some("@openai/codex"),
        _ => None,
    }
}

/// The folders and file of `exe`, lower-cased. A drive or `\\?\` prefix is left out, so a canonicalized path
/// compares like the path as found.
fn parts(exe: &Path) -> Vec<String> {
    exe.components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

fn has_sequence(parts: &[String], want: &[&str]) -> bool {
    parts
        .windows(want.len())
        .any(|w| w.iter().zip(want).all(|(a, b)| a == b))
}

const WINGET_SUFFIX: &str = "_microsoft.winget.source";

/// The WinGet package id of the `Packages\<id>_Microsoft.Winget.Source_<hash>` folder `exe` sits in.
/// `None` when it is not a WinGet path; `Some(None)` when it is one whose id can't be trusted.
fn winget_id(exe: &Path) -> Option<Option<String>> {
    static ID: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*$").expect("valid id pattern"));
    let names: Vec<String> = exe
        .components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let lower = parts(exe);
    let at = lower
        .windows(3)
        .position(|w| w[0] == "microsoft" && w[1] == "winget" && w[2] == "packages")?;
    // The folder after `Packages`, with the file after it.
    let dir = match names.get(at + 3) {
        Some(dir) if at + 4 < names.len() => dir,
        _ => return None,
    };
    if !dir.is_ascii() {
        return Some(None);
    }
    let id = dir
        .to_ascii_lowercase()
        .find(WINGET_SUFFIX)
        .map(|end| dir[..end].to_string());
    Some(id.filter(|id| ID.is_match(id)))
}

/// How `exe` was installed. `exe` is canonicalized by the caller when it could be, so a WinGet `Links` symlink
/// shows the `Packages` folder it points into. `exists` is the disk check, so tests need no files.
pub fn install_kind(agent: &str, exe: &Path, exists: &dyn Fn(&Path) -> bool) -> InstallKind {
    if let Some(id) = winget_id(exe) {
        return id.map_or(InstallKind::Unknown, |id| InstallKind::Winget { id });
    }
    if let Some(package) = npm_package(agent)
        && let Some(folder) = exe.parent()
    {
        let mut manifest = folder.join("node_modules");
        for segment in package.split('/') {
            manifest.push(segment);
        }
        manifest.push("package.json");
        if exists(&manifest) {
            return InstallKind::Npm {
                package: package.to_string(),
            };
        }
    }
    let parts = parts(exe);
    let (folder, file) = match parts.split_last() {
        Some((file, folder)) => (folder, file.as_str()),
        None => return InstallKind::Unknown,
    };
    let native = match agent {
        "claude" => file == "claude.exe" && folder.ends_with(&[".local".into(), "bin".into()]),
        "codex" => has_sequence(folder, &["programs", "openai", "codex", "bin"]),
        "antigravity" => file == "agy.exe" && folder.ends_with(&["agy".into(), "bin".into()]),
        _ => false,
    };
    if native {
        InstallKind::Native
    } else {
        InstallKind::Unknown
    }
}

/// The program's bare name, as the person types it.
fn bare_name(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        "antigravity" => Some("agy"),
        _ => None,
    }
}

/// The command that updates the CLI, ready to run: the program and its arguments.
pub fn update_argv(_agent: &str, kind: &InstallKind, exe: &Path) -> Option<Vec<String>> {
    let words = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match kind {
        InstallKind::Native => Some(vec![exe.to_string_lossy().into_owned(), "update".into()]),
        InstallKind::Npm { package } => {
            let mut argv = words(&["cmd.exe", "/d", "/c"]);
            argv.extend(words(&["npm", "install", "-g"]));
            argv.push(format!("{package}@latest"));
            Some(argv)
        }
        InstallKind::Winget { id } => {
            let mut argv = words(&["winget", "upgrade", "--id"]);
            argv.push(id.clone());
            argv.push("--exact".into());
            Some(argv)
        }
        InstallKind::Unknown => None,
    }
}

/// The update command as shown to the person.
pub fn update_text(agent: &str, kind: &InstallKind) -> Option<String> {
    match kind {
        InstallKind::Native => bare_name(agent).map(|name| format!("{name} update")),
        InstallKind::Npm { package } => Some(format!("npm install -g {package}@latest")),
        InstallKind::Winget { id } => Some(format!("winget upgrade --id {id} --exact")),
        InstallKind::Unknown => None,
    }
}

/// `X.Y.Z`, with a `-suffix` ignored.
fn triple(version: &str) -> Option<(u64, u64, u64)> {
    static V: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^(\d+)\.(\d+)\.(\d+)(?:-.*)?$").expect("valid version pattern"));
    let c = V.captures(version.trim())?;
    Some((c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?))
}

/// The `latest_version` Codex recorded in its `version.json`, when it is newer than `installed` and the person has not
/// dismissed it.
pub fn codex_newer(version_json: &str, installed: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(version_json).ok()?;
    let latest = value.get("latest_version")?.as_str()?;
    if value.get("dismissed_version").and_then(|v| v.as_str()) == Some(latest) {
        return None;
    }
    (triple(latest)? > triple(installed)?).then(|| latest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn kind(agent: &str, exe: &str) -> InstallKind {
        install_kind(agent, Path::new(exe), &|_| false)
    }

    #[test]
    fn winget_id_comes_from_the_packages_folder() {
        let want = InstallKind::Winget {
            id: "Anthropic.ClaudeCode".into(),
        };
        assert_eq!(
            kind(
                "claude",
                r"C:\Users\u\AppData\Local\Microsoft\WinGet\Packages\Anthropic.ClaudeCode_Microsoft.Winget.Source_8wekyb3d8bbwe\claude.exe"
            ),
            want
        );
        assert_eq!(
            kind(
                "claude",
                r"C:\Users\u\AppData\Local\microsoft\winget\packages\Anthropic.ClaudeCode_Microsoft.Winget.Source_8wekyb3d8bbwe\claude.exe"
            ),
            want
        );
        // A verbatim prefix from canonicalize does not hide the folders.
        assert_eq!(
            kind(
                "claude",
                r"\\?\C:\Users\u\AppData\Local\Microsoft\WinGet\Packages\Anthropic.ClaudeCode_Microsoft.Winget.Source_8wekyb3d8bbwe\claude.exe"
            ),
            want
        );
    }

    #[test]
    fn a_winget_folder_with_an_odd_id_is_unknown() {
        for exe in [
            r"C:\Users\u\AppData\Local\Microsoft\WinGet\Packages\Bad&Id_Microsoft.Winget.Source_x\claude.exe",
            r"C:\Users\u\AppData\Local\Microsoft\WinGet\Packages\NoSuffix\claude.exe",
        ] {
            assert_eq!(kind("claude", exe), InstallKind::Unknown, "{exe}");
        }
    }

    #[test]
    fn npm_needs_the_package_beside_the_shim() {
        let shim = r"C:\Users\u\AppData\Roaming\npm\claude.cmd";
        let package: PathBuf = [
            r"C:\Users\u\AppData\Roaming\npm",
            "node_modules",
            "@anthropic-ai",
            "claude-code",
            "package.json",
        ]
        .iter()
        .collect();
        assert_eq!(
            install_kind("claude", Path::new(shim), &|p| p == package),
            InstallKind::Npm {
                package: "@anthropic-ai/claude-code".into()
            }
        );
        assert_eq!(
            install_kind("claude", Path::new(shim), &|_| false),
            InstallKind::Unknown
        );
        assert_eq!(
            install_kind(
                "antigravity",
                Path::new(r"C:\Users\u\AppData\Roaming\npm\agy.cmd"),
                &|_| true
            ),
            InstallKind::Unknown
        );
    }

    #[test]
    fn native_paths() {
        assert_eq!(kind("claude", r"C:\Users\u\.local\bin\claude.exe"), InstallKind::Native);
        assert_eq!(
            kind("codex", r"C:\Users\u\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe"),
            InstallKind::Native
        );
        assert_eq!(
            kind("antigravity", r"C:\Users\u\AppData\Local\agy\bin\agy.exe"),
            InstallKind::Native
        );
        assert_eq!(kind("claude", r"C:\tools\claude.exe"), InstallKind::Unknown);
        assert_eq!(kind("codex", r"C:\Users\u\.local\bin\claude.exe"), InstallKind::Unknown);
    }

    #[test]
    fn update_argv_per_kind() {
        let exe = Path::new(r"C:\Users\u\.local\bin\claude.exe");
        let npm = InstallKind::Npm {
            package: "@openai/codex".into(),
        };
        let winget = InstallKind::Winget {
            id: "Anthropic.ClaudeCode".into(),
        };
        let s = |v: &[&str]| Some(v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(
            update_argv("claude", &InstallKind::Native, exe),
            s(&[r"C:\Users\u\.local\bin\claude.exe", "update"])
        );
        assert_eq!(
            update_argv("codex", &npm, exe),
            s(&["cmd.exe", "/d", "/c", "npm", "install", "-g", "@openai/codex@latest"])
        );
        assert_eq!(
            update_argv("claude", &winget, exe),
            s(&["winget", "upgrade", "--id", "Anthropic.ClaudeCode", "--exact"])
        );
        assert_eq!(update_argv("claude", &InstallKind::Unknown, exe), None);

        assert_eq!(
            update_text("claude", &InstallKind::Native).as_deref(),
            Some("claude update")
        );
        assert_eq!(
            update_text("antigravity", &InstallKind::Native).as_deref(),
            Some("agy update")
        );
        assert_eq!(
            update_text("codex", &npm).as_deref(),
            Some("npm install -g @openai/codex@latest")
        );
        assert_eq!(
            update_text("claude", &winget).as_deref(),
            Some("winget upgrade --id Anthropic.ClaudeCode --exact")
        );
        assert_eq!(update_text("claude", &InstallKind::Unknown), None);
    }

    #[test]
    fn codex_newer_cases() {
        let json =
            |latest: &str, dismissed: &str| format!(r#"{{"latest_version":{latest},"dismissed_version":{dismissed}}}"#);
        let plain = json(r#""0.164.0""#, "null");
        assert_eq!(codex_newer(&plain, "0.163.2").as_deref(), Some("0.164.0"));
        assert_eq!(codex_newer(&plain, "0.164.0"), None);
        assert_eq!(codex_newer(&plain, "0.163.0-alpha.2").as_deref(), Some("0.164.0"));
        assert_eq!(codex_newer(&json(r#""0.164.0""#, r#""0.164.0""#), "0.163.2"), None);
        assert_eq!(codex_newer("nope", "0.163.2"), None);
        assert_eq!(codex_newer(&json(r#""x""#, "null"), "0.163.2"), None);
    }
}
