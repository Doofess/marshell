//! Which paths the config service may write. Only the user's home folder, a project folder,
//! `<marshell home>/agents` and the agent folders of a WSL distro whose home was looked up are open.
//! Managed and organization paths are always refused, and so is the rest of the Marshell home.
//! Elevation is never run.

use crate::projects::{canonical, is_inside};
use std::path::{Component, Path, PathBuf};

/// File names an organization uses to manage a CLI. They are refused wherever they are.
pub const MANAGED_NAMES: [&str; 3] = ["managed-settings.json", "managed-mcp.json", "managed_config.toml"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NotAbsolute,
    Managed,
    OutsideAllowed,
    MarshellOwn,
}

/// The folders of a WSL distro's home that hold the agent CLIs' settings: nothing else in the distro is open.
const DISTRO_AGENT_FOLDERS: [&str; 5] = [".claude", ".codex", ".gemini", ".qwen", ".copilot"];

/// A WSL distro whose home Marshell has looked up (through `wsl.exe`), as the config service records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistroHome {
    pub distro: String,
    /// The home as a Windows path, such as `\\wsl.localhost\Ubuntu\home\me`.
    pub home: PathBuf,
    /// A test pinned this home, and it is a plain folder rather than the distro's share.
    pub pinned: bool,
}

#[derive(Clone)]
pub struct Guard {
    allowed: Vec<PathBuf>,
    /// Each opened distro with its home as the distro's own Linux path: its agent folders are open, by that spelling.
    distro_homes: Vec<(String, String)>,
    denied: Vec<PathBuf>,
    marshell_home: PathBuf,
}

impl Guard {
    pub fn new(allowed: Vec<PathBuf>, denied: Vec<PathBuf>, marshell_home: PathBuf) -> Self {
        Self {
            allowed: allowed.iter().map(|p| canonical(p)).collect(),
            distro_homes: Vec::new(),
            denied: denied.iter().map(|p| canonical(p)).collect(),
            marshell_home: canonical(&marshell_home),
        }
    }

    /// Allowed: the user's home (`dirs::home_dir`), every folder in `projects.json` and `<home>/agents`.
    /// A drive root in `projects.json` is skipped, also when it is spelt `C:\x\..` or reached through
    /// a link: it would open the whole drive.
    /// Also allowed: the folder each CLI's home variable names (`moved_agent_homes`).
    /// Denied: `denied_for_os()`. The project list is read once, here.
    pub fn for_home(home: &Path) -> Self {
        let mut allowed: Vec<PathBuf> = dirs::home_dir().into_iter().collect();
        allowed.extend(
            crate::projects::list(home)
                .into_iter()
                .map(|p| PathBuf::from(p.path))
                .filter(|p| !crate::projects::is_root(&canonical(p))),
        );
        allowed.push(home.join("agents"));
        allowed.extend(moved_agent_homes(&|k| std::env::var(k).ok()));
        Self::new(allowed, denied_for_os(), home.to_path_buf())
    }

    /// Also opens `.claude`, `.codex`, `.gemini`, `.qwen` and `.copilot` in each of these distro homes, and nothing else in the distro.
    /// A home that is not under its own distro's share (`\\wsl.localhost\<distro>` or `\\wsl$\<distro>`) opens
    /// nothing, except a pinned one, which a test says is a plain folder.
    pub fn with_distro_homes(mut self, homes: &[DistroHome]) -> Self {
        for home in homes {
            if home.pinned {
                let folders = DISTRO_AGENT_FOLDERS.map(|folder| canonical(&home.home.join(folder)));
                self.allowed.extend(folders);
            } else if let Some(linux) = linux_in_share(&home.distro, &home.home) {
                self.distro_homes.push((home.distro.clone(), linux));
            }
        }
        self
    }

    /// The resolved path (links resolved) when the service may write it. A path with a `..` part is
    /// refused as `NotAbsolute` before anything is resolved: Win32 folds `..` by the path text before
    /// it opens a file, and a physical walk can't match that once a folder is missing or a link is
    /// involved, so the guard and the write could name different files.
    pub fn check(&self, path: &Path) -> Result<PathBuf, Refusal> {
        if !path.is_absolute() || path.components().any(|c| c == Component::ParentDir) {
            return Err(Refusal::NotAbsolute);
        }
        let resolved = resolve(path)?;
        let text = resolved.to_string_lossy();
        if is_inside(&text, &self.marshell_home.to_string_lossy())
            && !is_inside(&text, &self.marshell_home.join("agents").to_string_lossy())
        {
            return Err(Refusal::MarshellOwn);
        }
        let in_denied = self.denied.iter().any(|d| is_inside(&text, &d.to_string_lossy()));
        if in_denied || resolved.file_name().is_some_and(is_managed_name) {
            return Err(Refusal::Managed);
        }
        let in_allowed = self.allowed.iter().any(|a| is_inside(&text, &a.to_string_lossy()));
        if in_allowed
            || self
                .distro_homes
                .iter()
                .any(|(d, home)| in_agent_folder(d, home, &resolved))
        {
            Ok(resolved)
        } else {
            Err(Refusal::OutsideAllowed)
        }
    }
}

/// The folder each of `COPILOT_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `QWEN_HOME` names, for a CLI whose home its
/// variable moves. A variable adds its folder only when it is set, not blank, absolute, free of `..` and not a drive
/// root; one that fails adds nothing and the others still count. The value is read by the CLI's own adapter code, so the
/// guard opens the folder the adapter writes to and no other. A `~` form of `QWEN_HOME` is read against the user's home,
/// and with no known home it stays relative and adds nothing. On Windows the folder must be on a drive of the PC: a
/// network share adds nothing, and neither does a WSL distro's share, whose agent folders have their own rule
/// (`with_distro_homes`). That holds for the path as it is written and for where it really is, so a drive letter
/// mapped to a share and a link that leads into one add nothing either.
pub fn moved_agent_homes(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    moved_agent_homes_as(env, &canonical)
}

/// `moved_agent_homes`, with the lookup of where a folder really is passed in, so a test can stand in for a mapped
/// drive or a link.
fn moved_agent_homes_as(env: &dyn Fn(&str) -> Option<String>, resolve: &dyn Fn(&Path) -> PathBuf) -> Vec<PathBuf> {
    use crate::agents::{codex, copilot, qwen, skill};
    let user_home = dirs::home_dir().unwrap_or_default();
    let set = |name: &str| env(name).is_some_and(|v| !v.trim().is_empty());
    let candidates = [
        set("COPILOT_HOME").then(|| copilot::home(env, &user_home)),
        set("CLAUDE_CONFIG_DIR")
            .then(|| skill::skill_path("claude", env, &user_home))
            .flatten()
            // `<dir>/skills/marshell/SKILL.md`: three levels up is the folder itself.
            .and_then(|file| file.ancestors().nth(3).map(Path::to_path_buf)),
        set("CODEX_HOME")
            .then(|| {
                codex::hooks::hooks_path(env, &user_home)
                    .parent()
                    .map(Path::to_path_buf)
            })
            .flatten(),
        set("QWEN_HOME").then(|| qwen::home(env, &user_home)),
    ];
    candidates
        .into_iter()
        .flatten()
        .filter(|folder| {
            let real = resolve(folder);
            folder.is_absolute()
                && on_a_drive(folder)
                && on_a_drive(&real)
                && !folder.components().any(|c| c == Component::ParentDir)
                && !crate::projects::is_root(&real)
        })
        .collect()
}

/// True on Windows for a path that starts with a drive letter (`D:\x`): not a share (`\\server\share\x`, a WSL
/// distro's `\\wsl.localhost\<distro>\x`) and not a verbatim spelling (`\\?\D:\x`). Other systems spell no share in a
/// path, so there every path counts.
fn on_a_drive(path: &Path) -> bool {
    #[cfg(windows)]
    {
        matches!(
            path.components().next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::Disk(_))
        )
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        true
    }
}

/// `\\?\UNC\server\share\x` as `\\server\share\x`: the verbatim spelling of a share, which `canonicalize` gives.
pub fn unverbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => PathBuf::from(format!(r"\\{rest}")),
        None => path.to_path_buf(),
    }
}

/// The Linux path of `path` when it is under the distro's own share. `to_linux` also maps a drive to `/mnt/<drive>`,
/// and a folder of Windows is not the distro's.
fn linux_in_share(distro: &str, path: &Path) -> Option<String> {
    let path = unverbatim(path);
    crate::wsl::under_distro(distro, &path)
        .then(|| crate::wsl::to_linux(distro, &path))
        .flatten()
}

/// Whether `path` is one of the agent folders of the distro's home, or below one. `linux_home` is the home as the
/// distro spells it, and the folders are matched by that spelling: a Linux file system tells `.Claude` from `.claude`.
/// It compares text, so `\\wsl.localhost\x` and `\\wsl$\x` say the same, and a path outside the distro's share says no.
fn in_agent_folder(distro: &str, linux_home: &str, path: &Path) -> bool {
    let Some(linux) = linux_in_share(distro, path) else {
        return false;
    };
    // `/home/me2` starts with `/home/me`, so what is left must start at a separator.
    let Some(rest) = linux
        .strip_prefix(linux_home.trim_end_matches('/'))
        .and_then(|rest| rest.strip_prefix('/'))
    else {
        return false;
    };
    let folder = rest.split('/').next().unwrap_or("");
    DISTRO_AGENT_FOLDERS.contains(&folder)
}

/// Resolves the path part by part: what exists goes through `canonical` (links and short names) and
/// what does not is appended. It never sees a `..`, because `check` refuses those first.
fn resolve(path: &Path) -> Result<PathBuf, Refusal> {
    let mut out = PathBuf::new();
    let mut exists = true;
    for part in path.components() {
        match part {
            Component::CurDir => continue,
            other => out.push(other),
        }
        if exists {
            if out.exists() {
                out = canonical(&out);
            } else if std::fs::symlink_metadata(&out).is_ok() {
                // A link whose target is gone: a write would create the target wherever it points.
                return Err(Refusal::OutsideAllowed);
            } else {
                exists = false;
            }
        }
    }
    Ok(out)
}

fn is_managed_name(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    // Windows drops trailing dots and spaces, and `name:stream` is the same file's other stream.
    let name = if cfg!(windows) {
        name.split(':').next().unwrap_or("").trim_end_matches(['.', ' '])
    } else {
        &name
    };
    MANAGED_NAMES.iter().any(|m| {
        if cfg!(windows) {
            name.eq_ignore_ascii_case(m)
        } else {
            name == *m
        }
    })
}

/// The folders an organization manages on this OS.
pub fn denied_for_os() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let var = |name: &str, fallback: &str| PathBuf::from(std::env::var_os(name).unwrap_or_else(|| fallback.into()));
        vec![
            var("ProgramFiles", r"C:\Program Files"),
            var("ProgramFiles(x86)", r"C:\Program Files (x86)"),
            var("ProgramData", r"C:\ProgramData"),
            var("SystemRoot", r"C:\Windows"),
        ]
    }
    #[cfg(target_os = "macos")]
    {
        ["/Library", "/System", "/etc", "/private/etc"]
            .map(PathBuf::from)
            .to_vec()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        ["/etc", "/usr"].map(PathBuf::from).to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// `root`, plus a guard that allows `root`, denies `root/org` and keeps Marshell's home at `root/home`.
    fn setup() -> (tempfile::TempDir, PathBuf, Guard) {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical(dir.path());
        let guard = Guard::new(vec![root.clone()], vec![root.join("org")], root.join("home"));
        (dir, root, guard)
    }

    #[test]
    fn a_file_in_an_allowed_folder_passes() {
        let (_dir, root, guard) = setup();
        let path = root.join("proj").join(".claude").join("settings.json");
        let resolved = guard.check(&path).unwrap();
        assert!(resolved.ends_with("settings.json"));
    }

    #[test]
    fn a_managed_folder_is_refused_even_inside_an_allowed_one() {
        let (_dir, root, guard) = setup();
        assert_eq!(guard.check(&root.join("org").join("x.json")), Err(Refusal::Managed));
    }

    #[test]
    fn managed_file_names_are_refused_anywhere() {
        let (_dir, root, guard) = setup();
        for name in MANAGED_NAMES {
            assert_eq!(guard.check(&root.join("a").join(name)), Err(Refusal::Managed), "{name}");
        }
        if cfg!(windows) {
            let upper = root.join("a").join("MANAGED-SETTINGS.JSON");
            assert_eq!(guard.check(&upper), Err(Refusal::Managed));
            let dotted = root.join("a").join("managed-settings.json.");
            assert_eq!(guard.check(&dotted), Err(Refusal::Managed));
            let stream = root.join("a").join("managed-settings.json:x");
            assert_eq!(guard.check(&stream), Err(Refusal::Managed));
        }
    }

    #[test]
    fn a_root_in_projects_json_allows_nothing() {
        let home = tempfile::tempdir().unwrap();
        // The drive root of the temp folder: a file directly under it is outside the user's home.
        let drive_root = home.path().ancestors().last().unwrap().to_path_buf();
        let doc = serde_json::json!({
            "version": 1,
            "projects": [{ "path": drive_root.to_string_lossy(), "last_used_ms": 1 }],
        });
        fs::write(home.path().join("projects.json"), doc.to_string()).unwrap();
        let guard = Guard::for_home(home.path());
        let file = drive_root.join("marshell-guard-probe.json");
        assert_eq!(guard.check(&file), Err(Refusal::OutsideAllowed));
    }

    #[test]
    fn a_projects_json_entry_that_folds_to_a_root_allows_nothing() {
        let home = tempfile::tempdir().unwrap();
        let drive_root = home.path().ancestors().last().unwrap().to_path_buf();
        // `<root>\<existing folder>\..` is the drive root once `..` is applied.
        let first = home.path().components().nth(if cfg!(windows) { 2 } else { 1 }).unwrap();
        let spelt = drive_root.join(first.as_os_str()).join("..");
        let doc = serde_json::json!({
            "version": 1,
            "projects": [{ "path": spelt.to_string_lossy(), "last_used_ms": 1 }],
        });
        fs::write(home.path().join("projects.json"), doc.to_string()).unwrap();
        let guard = Guard::for_home(home.path());
        let file = drive_root.join("marshell-guard-probe.json");
        assert_eq!(guard.check(&file), Err(Refusal::OutsideAllowed));
    }

    #[test]
    fn outside_every_allowed_folder_is_refused() {
        let (_dir, _root, guard) = setup();
        let other = tempfile::tempdir().unwrap();
        assert_eq!(guard.check(&other.path().join("x.json")), Err(Refusal::OutsideAllowed));
    }

    #[test]
    fn dot_dot_is_refused() {
        let (_dir, root, guard) = setup();
        // Win32 resolves `..` by the path text and a physical walk would not, so no `..` is allowed,
        // whether the folder before it exists or not.
        assert_eq!(guard.check(&root.join("a/../../x.json")), Err(Refusal::NotAbsolute));
        fs::create_dir(root.join("a")).unwrap();
        assert_eq!(guard.check(&root.join("a/../../x.json")), Err(Refusal::NotAbsolute));
        assert_eq!(guard.check(&root.join("a/../x.json")), Err(Refusal::NotAbsolute));
    }

    #[test]
    fn marshell_home_is_closed_except_agents() {
        let (_dir, root, guard) = setup();
        assert_eq!(
            guard.check(&root.join("home").join("managed.json")),
            Err(Refusal::MarshellOwn)
        );
        assert_eq!(guard.check(&root.join("home")), Err(Refusal::MarshellOwn));
        let chain = root
            .join("home")
            .join("agents")
            .join("claude")
            .join("statusline-chain.json");
        assert!(guard.check(&chain).is_ok());
    }

    #[test]
    fn relative_path_is_refused() {
        let (_dir, _root, guard) = setup();
        assert_eq!(
            guard.check(Path::new(".claude/settings.json")),
            Err(Refusal::NotAbsolute)
        );
    }

    /// A folder link at `link` to `target`; a junction on Windows without developer mode.
    fn link_dir(link: &Path, target: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        return std::os::windows::fs::symlink_dir(target, link).or_else(|_| {
            // Without developer mode a junction still works, and `canonical` resolves it the same way.
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()?
                .status;
            status
                .success()
                .then_some(())
                .ok_or_else(|| std::io::Error::other("mklink /J failed"))
        });
    }

    #[test]
    fn a_link_out_of_the_allowed_folder_is_refused() {
        let (_dir, root, guard) = setup();
        let target = tempfile::tempdir().unwrap();
        if let Err(e) = link_dir(&root.join("link"), target.path()) {
            eprintln!("skipped: cannot create a symlink here: {e}");
            return;
        }
        assert_eq!(
            guard.check(&root.join("link").join("x.json")),
            Err(Refusal::OutsideAllowed)
        );
        // A `..` is refused before anything is resolved, so it can't leave through the link's target.
        assert_eq!(
            guard.check(&root.join("link").join("..").join("x.json")),
            Err(Refusal::NotAbsolute)
        );
    }

    #[test]
    fn missing_dot_dot_then_a_link_is_refused() {
        let (_dir, root, guard) = setup();
        let target = tempfile::tempdir().unwrap();
        if let Err(e) = link_dir(&root.join("link"), target.path()) {
            eprintln!("skipped: cannot create a symlink here: {e}");
            return;
        }
        // `nope` doesn't exist, so the old walk stopped resolving, stepped back over `..` and
        // never looked at `link`; the write would have landed in `target`.
        let sneaky = root.join("nope").join("..").join("link").join("new.json");
        assert_eq!(guard.check(&sneaky), Err(Refusal::NotAbsolute));
    }

    fn distro_home(distro: &str, home: &str) -> DistroHome {
        DistroHome {
            distro: distro.to_string(),
            home: PathBuf::from(home),
            pinned: false,
        }
    }

    #[test]
    fn a_distros_agent_folders_are_admitted() {
        let home = "/home/u";
        for path in [
            r"\\wsl.localhost\Ubuntu\home\u\.claude\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.codex\hooks.json",
            r"\\wsl.localhost\Ubuntu\home\u\.gemini\config\hooks.json",
            r"\\wsl.localhost\Ubuntu\home\u\.claude",
            // The other spellings of the same share: `\\wsl$`, a case-insensitive distro name, `/`, and the
            // verbatim form that `canonicalize` gives.
            r"\\wsl$\Ubuntu\home\u\.claude\settings.json",
            r"\\WSL.LOCALHOST\ubuntu\home\u\.codex\hooks.json",
            "//wsl.localhost/Ubuntu/home/u/.claude/settings.json",
            r"\\?\UNC\wsl.localhost\Ubuntu\home\u\.gemini\config\hooks.json",
        ] {
            assert!(in_agent_folder("Ubuntu", home, Path::new(path)), "{path}");
        }
        // A root user's home with a trailing slash, and a home that is the distro's root.
        let root_path = Path::new(r"\\wsl.localhost\Ubuntu\root\.claude\settings.json");
        assert!(in_agent_folder("Ubuntu", "/root/", root_path));
        assert!(in_agent_folder(
            "Ubuntu",
            "/",
            Path::new(r"\\wsl.localhost\Ubuntu\.codex\hooks.json")
        ));
        assert!(!in_agent_folder(
            "Ubuntu",
            "/",
            Path::new(r"\\wsl.localhost\Ubuntu\etc\x")
        ));
    }

    #[test]
    fn a_distro_qwen_folder_is_admitted() {
        let home = "/home/u";
        for path in [
            r"\\wsl.localhost\Ubuntu\home\u\.qwen\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.qwen\skills\marshell\SKILL.md",
            r"\\wsl$\Ubuntu\home\u\.qwen",
            "//wsl.localhost/Ubuntu/home/u/.qwen/settings.json",
        ] {
            assert!(in_agent_folder("Ubuntu", home, Path::new(path)), "{path}");
        }
        // Only the folder itself: a look-alike, another case, a nested one and the home's other files stay refused.
        for path in [
            r"\\wsl.localhost\Ubuntu\home\u\.qwen.bak\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.Qwen\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\qwen\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\notes\.qwen\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.qwen\..\.bashrc",
            r"\\wsl.localhost\Ubuntu\home\u\.config\qwen\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.agents\skills\marshell\SKILL.md",
            r"\\wsl.localhost\Ubuntu\home\u\.bashrc",
        ] {
            assert!(!in_agent_folder("Ubuntu", home, Path::new(path)), "{path}");
        }
    }

    #[test]
    fn a_distro_copilot_folder_is_admitted() {
        let home = "/home/u";
        for path in [
            r"\\wsl.localhost\Ubuntu\home\u\.copilot\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.copilot\hooks\marshell.json",
            r"\\wsl$\Ubuntu\home\u\.copilot",
            "//wsl.localhost/Ubuntu/home/u/.copilot/hooks/marshell.json",
        ] {
            assert!(in_agent_folder("Ubuntu", home, Path::new(path)), "{path}");
        }
        // Only the folder itself, and nothing else of the distro's home beside the five.
        for path in [
            r"\\wsl.localhost\Ubuntu\home\u\.copilot.bak\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.Copilot\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\copilot\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\notes\.copilot\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.copilot\..\.bashrc",
            r"\\wsl.localhost\Ubuntu\home\u\.config\copilot\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.agents\skills\marshell\SKILL.md",
            r"\\wsl.localhost\Ubuntu\home\u\.bashrc",
        ] {
            assert!(!in_agent_folder("Ubuntu", home, Path::new(path)), "{path}");
        }
        assert_eq!(DISTRO_AGENT_FOLDERS.len(), 5);
    }

    #[test]
    fn the_rest_of_a_distro_stays_refused() {
        let home = "/home/u";
        for path in [
            r"\\wsl.localhost\Ubuntu\home\u\.bashrc",
            r"\\wsl.localhost\Ubuntu\home\u",
            r"\\wsl.localhost\Ubuntu\etc\x",
            r"\\wsl.localhost\Ubuntu\home\u\notes\.claude\x",
            // The home's own text is a prefix of this one, but it is another folder.
            r"\\wsl.localhost\Ubuntu\home\u2\.claude\settings.json",
            // Linux tells these from the agent folders.
            r"\\wsl.localhost\Ubuntu\home\u\.Claude\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.claude.bak\x",
            r"\\wsl.localhost\Ubuntu\home\u\.claude.\x",
            // Another distro, a share on another server, and a Windows folder.
            r"\\wsl.localhost\Debian\home\u\.claude\settings.json",
            r"\\server\share\home\u\.claude\settings.json",
            r"C:\home\u\.claude\settings.json",
            r"\\wsl.localhost\Ubuntu\home\u\.claude\..\.bashrc",
        ] {
            assert!(!in_agent_folder("Ubuntu", home, Path::new(path)), "{path}");
        }
        // A distro's `/mnt/c` is a Windows folder, which the distro's share does not hold.
        let windows = Path::new(r"C:\Users\me\.claude\settings.json");
        assert!(!in_agent_folder("Ubuntu", "/mnt/c/Users/me", windows));
    }

    #[test]
    fn a_home_outside_its_distro_opens_nothing() {
        let guard = Guard::new(vec![], vec![], PathBuf::from("/marshell"));
        let homes = [
            distro_home("Ubuntu", r"C:\Users\me"),
            distro_home("Ubuntu", r"\\server\share\home\u"),
            distro_home("Ubuntu", r"\\wsl.localhost\Debian\home\u"),
            distro_home("Ubuntu", "relative"),
        ];
        assert!(guard.with_distro_homes(&homes).distro_homes.is_empty());
        let opened = Guard::new(vec![], vec![], PathBuf::from("/marshell"))
            .with_distro_homes(&[distro_home("Ubuntu", r"\\?\UNC\wsl$\ubuntu\home\u")]);
        assert_eq!(opened.distro_homes, [("Ubuntu".to_string(), "/home/u".to_string())]);
    }

    /// A guard with the fake distro's home open and its `locked` folder denied.
    #[cfg(windows)]
    fn distro_guard(denied: Vec<PathBuf>) -> Guard {
        let home = distro_home("MarshellTestDistro", r"\\wsl.localhost\MarshellTestDistro\home\u");
        Guard::new(vec![], denied, PathBuf::from(r"C:\marshell-test-home")).with_distro_homes(&[home])
    }

    #[cfg(windows)]
    #[test]
    fn the_guard_checks_a_distros_paths_the_same_way() {
        let guard = distro_guard(vec![]);
        let base = r"\\wsl.localhost\MarshellTestDistro\home\u";
        for path in [
            r".claude\settings.json",
            r".codex\hooks.json",
            r".gemini\config\hooks.json",
        ] {
            assert!(guard.check(Path::new(&format!(r"{base}\{path}"))).is_ok(), "{path}");
        }
        for path in [r".bashrc", r"..\..\etc\x", r".claude\..\.bashrc"] {
            assert!(guard.check(Path::new(&format!(r"{base}\{path}"))).is_err(), "{path}");
        }
        let other = r"\\wsl.localhost\MarshellOtherDistro\home\u\.claude\settings.json";
        assert_eq!(guard.check(Path::new(other)), Err(Refusal::OutsideAllowed));
    }

    #[cfg(windows)]
    #[test]
    fn managed_and_denied_rules_still_apply_in_a_distro() {
        let base = r"\\wsl.localhost\MarshellTestDistro\home\u";
        let denied = vec![PathBuf::from(format!(r"{base}\.codex\locked"))];
        let guard = distro_guard(denied);
        for name in MANAGED_NAMES {
            let path = format!(r"{base}\.claude\{name}");
            assert_eq!(guard.check(Path::new(&path)), Err(Refusal::Managed), "{name}");
        }
        let in_denied = format!(r"{base}\.codex\locked\hooks.json");
        assert_eq!(guard.check(Path::new(&in_denied)), Err(Refusal::Managed));
        // Its neighbour in the same agent folder is still open.
        let neighbour = format!(r"{base}\.codex\hooks.json");
        assert!(guard.check(Path::new(&neighbour)).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn windows_denies_program_files() {
        let (_dir, root, _guard) = setup();
        let guard = Guard::new(vec![PathBuf::from("C:\\")], denied_for_os(), root.join("home"));
        let settings = Path::new(r"C:\Program Files\ClaudeCode\settings.json");
        assert_eq!(guard.check(settings), Err(Refusal::Managed));
        let codex = Path::new(r"C:\ProgramData\OpenAI\Codex\requirements.toml");
        assert_eq!(guard.check(codex), Err(Refusal::Managed));
    }

    // ---- a CLI home that its variable moves outside the profile ----

    /// An environment of just these variables.
    fn vars<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    /// A guard that allows only what `moved_agent_homes` gives for these variables, with Marshell's home at
    /// `D:\m\home` and `D:\org` denied.
    #[cfg(windows)]
    fn moved_guard(pairs: &[(&str, &str)]) -> Guard {
        Guard::new(
            moved_agent_homes(&vars(pairs)),
            vec![PathBuf::from(r"D:\org")],
            PathBuf::from(r"D:\m\home"),
        )
    }

    #[cfg(windows)]
    #[test]
    fn a_copilot_home_outside_the_profile_is_admitted() {
        let home = r"D:\work\copilot-home";
        let guard = moved_guard(&[("COPILOT_HOME", home)]);
        for file in [r"settings.json", r"hooks\marshell.json", r"skills\marshell\SKILL.md"] {
            let path = PathBuf::from(format!(r"{home}\{file}"));
            assert!(guard.check(&path).is_ok(), "{file}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_beside_the_copilot_home_stays_closed() {
        let guard = moved_guard(&[("COPILOT_HOME", r"D:\work\copilot-home")]);
        for path in [r"D:\work\other\x.json", r"D:\work\copilot-home-2\x.json"] {
            assert_eq!(guard.check(Path::new(path)), Err(Refusal::OutsideAllowed), "{path}");
        }
    }

    #[test]
    fn moved_agent_homes_refuses_a_root_a_relative_path_a_dot_dot_and_a_blank() {
        for value in [r"D:\", "copilot-home", r"D:\a\..\b", "  ", ""] {
            for name in ["COPILOT_HOME", "CLAUDE_CONFIG_DIR", "CODEX_HOME", "QWEN_HOME"] {
                let given = [(name, value)];
                assert!(moved_agent_homes(&vars(&given)).is_empty(), "{name}={value:?}");
            }
        }
        assert!(moved_agent_homes(&|_| None).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn a_copilot_home_inside_marshells_home_is_still_marshells_own() {
        let guard = moved_guard(&[("COPILOT_HOME", r"D:\m\home\cp")]);
        let path = Path::new(r"D:\m\home\cp\settings.json");
        assert_eq!(guard.check(path), Err(Refusal::MarshellOwn));
    }

    #[cfg(windows)]
    #[test]
    fn a_copilot_home_under_a_denied_folder_is_still_managed() {
        let guard = moved_guard(&[("COPILOT_HOME", r"D:\org\cp")]);
        let path = Path::new(r"D:\org\cp\settings.json");
        assert_eq!(guard.check(path), Err(Refusal::Managed));
    }

    #[cfg(windows)]
    #[test]
    fn a_managed_file_name_in_the_copilot_home_is_still_refused() {
        let guard = moved_guard(&[("COPILOT_HOME", r"D:\work\copilot-home")]);
        let path = Path::new(r"D:\work\copilot-home\managed-settings.json");
        assert_eq!(guard.check(path), Err(Refusal::Managed));
    }

    #[cfg(windows)]
    #[test]
    fn each_cli_home_variable_opens_its_own_folder() {
        let given = [
            ("COPILOT_HOME", r"D:\w\cp"),
            ("CLAUDE_CONFIG_DIR", r"D:\w\cl"),
            ("CODEX_HOME", r"D:\w\cx"),
            ("QWEN_HOME", r"D:\w\qw"),
        ];
        let homes = moved_agent_homes(&vars(&given));
        let expected: Vec<PathBuf> = [r"D:\w\cp", r"D:\w\cl", r"D:\w\cx", r"D:\w\qw"]
            .map(PathBuf::from)
            .to_vec();
        assert_eq!(homes, expected);
        let guard = moved_guard(&given);
        for path in [
            r"D:\w\cl\settings.json",
            r"D:\w\cx\hooks.json",
            r"D:\w\qw\settings.json",
        ] {
            assert!(guard.check(Path::new(path)).is_ok(), "{path}");
        }
        assert_eq!(
            guard.check(Path::new(r"D:\w\other\settings.json")),
            Err(Refusal::OutsideAllowed)
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_bad_home_variable_does_not_close_the_others() {
        let given = [("CODEX_HOME", r"D:\"), ("QWEN_HOME", r"D:\w\qw")];
        assert_eq!(moved_agent_homes(&vars(&given)), vec![PathBuf::from(r"D:\w\qw")]);
    }

    /// A home on a network share or in a WSL distro is not opened this way: a distro's agent folders have their own
    /// rule (`with_distro_homes`), and a share is not the PC's own disk.
    #[cfg(windows)]
    #[test]
    fn a_home_on_a_share_or_in_a_distro_adds_nothing() {
        for value in [
            r"\\server\share\copilot-home",
            r"\\wsl.localhost\Ubuntu\home\u\.copilot",
            r"\\wsl$\Ubuntu\home\u",
            r"\\?\UNC\server\share\copilot-home",
            "//server/share/copilot-home",
            // A verbatim spelling is not how a person sets a home either.
            r"\\?\D:\w\cp",
        ] {
            for name in ["COPILOT_HOME", "CLAUDE_CONFIG_DIR", "CODEX_HOME", "QWEN_HOME"] {
                let given = [(name, value), ("QWEN_RUNTIME_DIR", r"D:\w\rt")];
                assert!(moved_agent_homes(&vars(&given)).is_empty(), "{name}={value}");
            }
        }
    }

    /// A drive letter can stand for a share (a mapped drive), and a link on a drive can lead into one: the rule is about
    /// where the folder really is.
    #[cfg(windows)]
    #[test]
    fn a_home_that_really_is_on_a_share_adds_nothing() {
        let really = |path: &Path| match path.to_str() {
            Some(r"Z:\copilot") => PathBuf::from(r"\\server\share\copilot"),
            Some(r"D:\link") => PathBuf::from(r"\\wsl.localhost\Ubuntu\home\u"),
            _ => path.to_path_buf(),
        };
        for value in [r"Z:\copilot", r"D:\link"] {
            for name in ["COPILOT_HOME", "CLAUDE_CONFIG_DIR", "CODEX_HOME", "QWEN_HOME"] {
                let given = [(name, value)];
                assert!(
                    moved_agent_homes_as(&vars(&given), &really).is_empty(),
                    "{name}={value}"
                );
            }
        }
        // A folder that really is where it says still counts.
        assert_eq!(
            moved_agent_homes_as(&vars(&[("COPILOT_HOME", r"D:\w\cp")]), &really),
            vec![PathBuf::from(r"D:\w\cp")]
        );
    }
}
