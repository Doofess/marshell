//! Which paths the config service may write. Only the user's home folder, a project folder,
//! `<marshell home>/agents` and the agent folders of a WSL distro whose home was looked up are open.
//! Managed and organization paths are always refused, and so is the rest of the Marshell home.
//! The folder a CLI's home variable names is open too, unless it holds the user profile or Marshell's home: such a
//! variable opens nothing, and the refusal names it. Elevation is never run.

use crate::projects::{canonical, is_inside, same_path};
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
    /// Each project folder in a distro that was not running when the guard was built: the distro and the folder's Linux
    /// path. The folder is matched by that text and never resolved, because resolving it would start the distro.
    unread: Vec<(String, String)>,
    denied: Vec<PathBuf>,
    marshell_home: PathBuf,
    /// The home variables refused for naming a folder that holds the profile or Marshell's home. They open nothing;
    /// they are kept so a refusal can name the variable (`wide_home_of`).
    wide: Vec<WideHome>,
}

impl Guard {
    pub fn new(allowed: Vec<PathBuf>, denied: Vec<PathBuf>, marshell_home: PathBuf) -> Self {
        Self {
            allowed: allowed.iter().map(|p| canonical(p)).collect(),
            distro_homes: Vec::new(),
            unread: Vec::new(),
            denied: denied.iter().map(|p| canonical(p)).collect(),
            marshell_home: canonical(&marshell_home),
            wide: Vec::new(),
        }
    }

    /// Allowed: the user's home (`dirs::home_dir`), every folder in `projects.json` and `<home>/agents`.
    /// A drive root in `projects.json` is skipped, also when it is spelt `C:\x\..` or reached through
    /// a link: it would open the whole drive.
    /// Also allowed: the folder each CLI's home variable names (`agent_homes`), except a folder that holds the user
    /// profile or Marshell's home: it opens nothing, and a refusal under it names the variable (`wide_home_of`).
    /// A project in a WSL distro that is not running is not read: it does not enter `allowed` (resolving it would start
    /// the distro) and is matched by its text instead (`with_unread`).
    /// Denied: `denied_for_os()`. The project list is read once, here.
    pub fn for_home(home: &Path) -> Self {
        let mut allowed: Vec<PathBuf> = dirs::home_dir().into_iter().collect();
        let mut unread = Vec::new();
        for listed in crate::projects::entries(home, &crate::wsl::running_recent) {
            let path = PathBuf::from(listed.info.path);
            if !listed.read {
                unread.push(path);
            } else if !crate::projects::is_root(&canonical(&path)) {
                allowed.push(path);
            }
        }
        allowed.push(home.join("agents"));
        let homes = agent_homes(&|k| std::env::var(k).ok(), home);
        allowed.extend(homes.open);
        Self::new(allowed, denied_for_os(), home.to_path_buf())
            .with_wide_homes(homes.wide)
            .with_unread(&unread)
    }

    /// Also opens each of these project folders, all of them in a distro that is not running, by the text of the path
    /// alone: the distro and the Linux path, so `\\wsl$\ubuntu\x` and `\\wsl.localhost\Ubuntu\x` are one folder. Nothing
    /// is read from the disk. A folder that is not under a distro's share, or is the share's root (it would open the
    /// whole distro), opens nothing.
    pub fn with_unread(mut self, folders: &[PathBuf]) -> Self {
        for folder in folders {
            let folder = unverbatim(folder);
            let Some(distro) = folder.to_str().and_then(crate::wsl::share_distro) else {
                continue;
            };
            let Some(linux) = crate::wsl::share_to_linux(&folder) else {
                continue;
            };
            if linux != "/" {
                self.unread.push((distro.to_string(), linux));
            }
        }
        self
    }

    /// Whether `path` is one of the unread folders or below one, by text. The Linux path is compared with case, and what
    /// follows the folder must start at a `/`, so `/home/me2` is not in `/home/me`.
    fn in_unread(&self, path: &Path) -> bool {
        self.unread.iter().any(|(distro, folder)| {
            linux_in_share(distro, path).is_some_and(|linux| {
                linux == *folder
                    || linux
                        .strip_prefix(folder.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            })
        })
    }

    /// Records the home variables the guard refused for being wide, so `wide_home_of` can name them. They open nothing.
    pub fn with_wide_homes(mut self, wide: Vec<WideHome>) -> Self {
        self.wide = wide;
        self
    }

    /// The refused home variable whose folder `path` is in, if any. None for a path inside an allowed folder (as
    /// `check` tests it, on the path as written), so a path the guard opens never blames a variable. Otherwise the
    /// first wide home that holds the path by its text, or by where the path really is, so a file that doesn't exist
    /// yet under a link to the wide folder is still found.
    pub fn wide_home_of(&self, path: &Path) -> Option<&WideHome> {
        let text = path.to_string_lossy();
        // A folder in an unread distro is tested first, by text: testing `allowed` resolves the path itself.
        if self.in_unread(path) || self.allowed.iter().any(|a| is_inside(&text, &a.to_string_lossy())) {
            return None;
        }
        let resolved = resolve(path).ok();
        self.wide.iter().find(|wide| {
            text_inside(path, &wide.folder)
                || resolved
                    .as_ref()
                    .is_some_and(|r| is_inside(&r.to_string_lossy(), &wide.folder.to_string_lossy()))
        })
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
        let in_allowed =
            self.in_unread(&resolved) || self.allowed.iter().any(|a| is_inside(&text, &a.to_string_lossy()));
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

/// What the folder of a refused home variable holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holds {
    Profile,
    MarshellHome,
}

/// A home variable that opens nothing because its folder holds the user profile or Marshell's home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WideHome {
    pub variable: &'static str,
    /// The folder as the variable spells it (after the CLI adapter's own reading, so Qwen Code's `~` is expanded).
    pub folder: PathBuf,
    pub holds: Holds,
}

/// The folders the home variables open, and the variables refused for being wide.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AgentHomes {
    pub open: Vec<PathBuf>,
    pub wide: Vec<WideHome>,
}

/// The folder each of `COPILOT_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `QWEN_HOME` names, for a CLI whose home its
/// variable moves, as `agent_homes` gives them, with no Marshell home to protect. See `agent_homes` for the rule.
pub fn moved_agent_homes(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    moved_agent_homes_as(env, &canonical)
}

/// `moved_agent_homes`, with the lookup of where a folder really is passed in, so a test can stand in for a mapped
/// drive or a link.
fn moved_agent_homes_as(env: &dyn Fn(&str) -> Option<String>, resolve: &dyn Fn(&Path) -> PathBuf) -> Vec<PathBuf> {
    agent_homes_as(env, resolve, &profiles_of(env), None).open
}

/// The folder each of `COPILOT_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `QWEN_HOME` names, for a CLI whose home its
/// variable moves. A variable counts only when it is set, not blank, absolute and free of `..`; one that fails adds
/// nothing and the others still count. The value is read by the CLI's own adapter code, so the guard opens the folder
/// the adapter writes to and no other. A `~` form of `QWEN_HOME` is read against the user's home, and with no known
/// home it stays relative and adds nothing. On Windows the folder must be on a drive of the PC: a network share adds
/// nothing, and neither does a WSL distro's share, whose agent folders have their own rule (`with_distro_homes`). That
/// holds for the path as it is written and for where it really is, so a drive letter mapped to a share and a link that
/// leads into one add nothing either.
///
/// A folder that holds the user profile (both spellings: `dirs::home_dir` and `USERPROFILE`, or `HOME` elsewhere) or
/// Marshell's home opens nothing and is recorded in `wide`, so a refusal can name the variable. The profile itself
/// and the folders inside it are not wide: the guard opens the profile anyway. A drive root adds nothing, and is
/// recorded only when it holds the profile or Marshell's home.
pub fn agent_homes(env: &dyn Fn(&str) -> Option<String>, marshell_home: &Path) -> AgentHomes {
    agent_homes_as(env, &canonical, &profiles_of(env), Some(marshell_home))
}

/// Both spellings of the user profile, each once: where the OS says it is, and what the CLI adapters read.
fn profiles_of(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let named = env(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let mut profiles: Vec<PathBuf> = Vec::new();
    for profile in dirs::home_dir().into_iter().chain(named) {
        let text = profile.to_string_lossy();
        if !profiles.iter().any(|p| same_path(&p.to_string_lossy(), &text)) {
            profiles.push(profile);
        }
    }
    profiles
}

/// `agent_homes`, with the lookup of where a folder really is and the profiles passed in, so a test can stand in for a
/// mapped drive, a link or a profile on another drive.
fn agent_homes_as(
    env: &dyn Fn(&str) -> Option<String>,
    resolve: &dyn Fn(&Path) -> PathBuf,
    profiles: &[PathBuf],
    marshell_home: Option<&Path>,
) -> AgentHomes {
    use crate::agents::{codex, copilot, qwen, skill};
    let user_home = dirs::home_dir().unwrap_or_default();
    let set = |name: &str| env(name).is_some_and(|v| !v.trim().is_empty());
    let candidates = [
        (
            "COPILOT_HOME",
            set("COPILOT_HOME").then(|| copilot::home(env, &user_home)),
        ),
        (
            "CLAUDE_CONFIG_DIR",
            set("CLAUDE_CONFIG_DIR")
                .then(|| skill::skill_path("claude", env, &user_home))
                .flatten()
                // `<dir>/skills/marshell/SKILL.md`: three levels up is the folder itself.
                .and_then(|file| file.ancestors().nth(3).map(Path::to_path_buf)),
        ),
        (
            "CODEX_HOME",
            set("CODEX_HOME")
                .then(|| {
                    codex::hooks::hooks_path(env, &user_home)
                        .parent()
                        .map(Path::to_path_buf)
                })
                .flatten(),
        ),
        ("QWEN_HOME", set("QWEN_HOME").then(|| qwen::home(env, &user_home))),
    ];
    let mut homes = AgentHomes::default();
    for (variable, folder) in candidates {
        let Some(folder) = folder else { continue };
        let real = resolve(&folder);
        let usable = folder.is_absolute()
            && on_a_drive(&folder)
            && on_a_drive(&real)
            && !folder.components().any(|c| c == Component::ParentDir);
        if !usable {
            continue;
        }
        // 2 comes first: one spelling of the profile can hold the other (a `USERPROFILE` above the profile the OS
        // reports), and a folder that holds a profile is wide whatever else it is.
        let holds = if profiles.iter().any(|p| holds_folder(&folder, resolve, p)) {
            Some(Holds::Profile)
        } else if profiles.iter().any(|p| is_folder(&folder, resolve, p)) {
            // 1. The profile itself adds nothing and stays open: the guard opens the whole profile whatever a
            // variable says.
            None
        } else if marshell_home.is_some_and(|m| holds_folder(&folder, resolve, m)) {
            // 3.
            Some(Holds::MarshellHome)
        } else {
            None
        };
        match holds {
            Some(holds) => homes.wide.push(WideHome {
                variable,
                folder,
                holds,
            }),
            // 4. A drive root adds nothing.
            None if crate::projects::is_root(&real) => {}
            None => homes.open.push(folder),
        }
    }
    homes
}

/// The path text with `.` and `..` folded and every separator spelt `\` on Windows, without touching the disk.
fn tidy_text(path: &Path) -> String {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out.to_string_lossy().into_owned()
}

/// Whether `path` is `folder` or in it by the text of the two alone, which `projects::is_inside` is when neither side is
/// on the disk: trailing separators are dropped, case is folded on Windows, and what follows the folder's text is a
/// separator (`task-2` is not in `task`). Nothing is resolved, so it holds for a folder that is a link or is gone.
fn text_inside(path: &Path, folder: &Path) -> bool {
    let fold = |text: String| if cfg!(windows) { text.to_ascii_lowercase() } else { text };
    let (path, folder) = (fold(tidy_text(path)), fold(tidy_text(folder)));
    if same_path(&path, &folder) {
        return true;
    }
    path.strip_prefix(folder.trim_end_matches(['/', '\\']))
        .is_some_and(|rest| rest.starts_with(['/', '\\']))
}

/// Whether `folder` is the folder `other`: the same text, or the same text once `folder` is resolved, or each is
/// inside the other (`is_inside` resolves a side that exists, and a folder can be a link to another).
fn is_folder(folder: &Path, resolve: &dyn Fn(&Path) -> PathBuf, other: &Path) -> bool {
    let (folder_text, other_text) = (folder.to_string_lossy(), other.to_string_lossy());
    same_path(&folder_text, &other_text)
        || same_path(&resolve(folder).to_string_lossy(), &other_text)
        || (is_inside(&folder_text, &other_text) && is_inside(&other_text, &folder_text))
}

/// Whether `folder` holds `other` from above: it is not `other` and `other` is in it, by the text of the two, by where
/// they are on disk, or by where `folder` really is (a link).
fn holds_folder(folder: &Path, resolve: &dyn Fn(&Path) -> PathBuf, other: &Path) -> bool {
    if is_folder(folder, resolve, other) {
        return false;
    }
    let other_text = other.to_string_lossy();
    text_inside(other, folder)
        || is_inside(&other_text, &folder.to_string_lossy())
        || is_inside(&other_text, &resolve(folder).to_string_lossy())
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
    fn an_unread_folder_opens_what_is_in_it_by_either_spelling() {
        let (_dir, _root, guard) = setup();
        let guard = guard.with_unread(&[r"\\wsl$\ubuntu\home\me\app".into()]);
        for inside in [
            r"\\wsl.localhost\Ubuntu\home\me\app\.claude\settings.json",
            r"\\wsl$\UBUNTU\home\me\app",
            r"\\?\UNC\wsl.localhost\Ubuntu\home\me\app\x",
        ] {
            assert!(guard.in_unread(Path::new(inside)), "{inside}");
        }
        for outside in [
            r"\\wsl.localhost\Ubuntu\home\me\app2\x",
            r"\\wsl.localhost\Ubuntu\home\me",
            r"\\wsl.localhost\Debian\home\me\app\x",
            r"C:\home\me\app",
        ] {
            assert!(!guard.in_unread(Path::new(outside)), "{outside}");
        }
    }

    #[test]
    fn a_share_root_is_never_unread_allowed() {
        let (_dir, _root, guard) = setup();
        let guard = guard.with_unread(&[r"\\wsl.localhost\Ubuntu".into(), r"\\wsl.localhost\Ubuntu\".into()]);
        assert!(guard.unread.is_empty());
    }

    #[test]
    fn an_unread_folder_is_never_resolved() {
        let (_dir, root, guard) = setup();
        let guard = guard.with_unread(&[r"\\wsl.localhost\Ubuntu\home\me\app".into()]);
        assert!(guard.check(&root.join("proj").join("x.json")).is_ok());
        // A check could reach the share only through a folder held in `allowed`.
        assert!(
            guard
                .allowed
                .iter()
                .all(|a| crate::wsl::share_distro(&a.to_string_lossy()).is_none())
        );
    }

    #[test]
    fn wide_home_of_leaves_a_path_in_an_unread_folder_alone() {
        let (_dir, _root, guard) = setup();
        let wide = WideHome {
            variable: "CODEX_HOME",
            folder: PathBuf::from(r"\\wsl.localhost\Ubuntu\home"),
            holds: Holds::Profile,
        };
        let guard = guard
            .with_unread(&[r"\\wsl.localhost\Ubuntu\home\me\app".into()])
            .with_wide_homes(vec![wide]);
        let inside = Path::new(r"\\wsl.localhost\Ubuntu\home\me\app\.codex\config.toml");
        assert!(guard.wide_home_of(inside).is_none());
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

    // ---- a home variable that names a folder holding the profile or Marshell's home (slice 6s) ----

    const HOME_VARIABLES: [&str; 4] = ["COPILOT_HOME", "CLAUDE_CONFIG_DIR", "CODEX_HOME", "QWEN_HOME"];

    /// `agent_homes_as` with nothing resolved (a folder is where its text says) and Marshell's home at `D:\m\home`.
    #[cfg(windows)]
    fn homes_of(pairs: &[(&str, &str)], profiles: &[&str]) -> AgentHomes {
        let profiles: Vec<PathBuf> = profiles.iter().map(PathBuf::from).collect();
        agent_homes_as(
            &vars(pairs),
            &|p| p.to_path_buf(),
            &profiles,
            Some(Path::new(r"D:\m\home")),
        )
    }

    #[cfg(windows)]
    fn wide(variable: &'static str, folder: &str, holds: Holds) -> WideHome {
        WideHome {
            variable,
            folder: PathBuf::from(folder),
            holds,
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_home_variable_that_holds_the_profile_opens_nothing() {
        for name in HOME_VARIABLES {
            let homes = homes_of(&[(name, r"D:\u")], &[r"D:\u\me"]);
            assert!(homes.open.is_empty(), "{name}");
            assert_eq!(homes.wide, vec![wide(name, r"D:\u", Holds::Profile)], "{name}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_profile_itself_and_a_folder_in_it_stay_open() {
        let given = [("CODEX_HOME", r"D:\u\me"), ("COPILOT_HOME", r"D:\u\me\.copilot")];
        let homes = homes_of(&given, &[r"D:\u\me"]);
        assert_eq!(
            homes.open,
            vec![PathBuf::from(r"D:\u\me\.copilot"), PathBuf::from(r"D:\u\me")]
        );
        assert!(homes.wide.is_empty());
    }

    /// `USERPROFILE` can name a folder that holds the profile the OS reports. A home variable naming that folder
    /// still holds a profile, so it opens nothing, although it is "a profile" by the other spelling.
    #[cfg(windows)]
    #[test]
    fn a_profile_spelling_that_holds_the_other_profile_opens_nothing() {
        for name in HOME_VARIABLES {
            let homes = homes_of(&[(name, r"D:\u")], &[r"D:\u\me", r"D:\u"]);
            assert!(homes.open.is_empty(), "{name}");
            assert_eq!(homes.wide, vec![wide(name, r"D:\u", Holds::Profile)], "{name}");
        }
        // The inner profile itself still opens, whichever spelling comes first.
        for profiles in [[r"D:\u\me", r"D:\u"], [r"D:\u", r"D:\u\me"]] {
            let homes = homes_of(&[("CODEX_HOME", r"D:\u\me")], &profiles);
            assert_eq!(homes.open, vec![PathBuf::from(r"D:\u\me")]);
            assert!(homes.wide.is_empty());
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_home_variable_that_holds_marshells_home_opens_nothing() {
        let homes = homes_of(&[("CODEX_HOME", r"D:\m")], &[r"D:\u\me"]);
        assert!(homes.open.is_empty());
        assert_eq!(homes.wide, vec![wide("CODEX_HOME", r"D:\m", Holds::MarshellHome)]);
        // Marshell's home and a folder in it are not wide: `check` calls them Marshell's own.
        for folder in [r"D:\m\home", r"D:\m\home\x"] {
            let homes = homes_of(&[("CODEX_HOME", folder)], &[r"D:\u\me"]);
            assert!(homes.wide.is_empty(), "{folder}");
            assert_eq!(homes.open, vec![PathBuf::from(folder)], "{folder}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_that_holds_both_names_the_profile() {
        let homes = agent_homes_as(
            &vars(&[("CODEX_HOME", r"D:\w")]),
            &|p| p.to_path_buf(),
            &[PathBuf::from(r"D:\w\me")],
            Some(Path::new(r"D:\w\mh")),
        );
        assert!(homes.open.is_empty());
        assert_eq!(homes.wide, vec![wide("CODEX_HOME", r"D:\w", Holds::Profile)]);
    }

    #[cfg(windows)]
    #[test]
    fn case_and_a_trailing_separator_do_not_hide_a_wide_home() {
        for value in [r"d:\U\", r"d:\U", r"D:\u\", r"D:/u/"] {
            let homes = homes_of(&[("CODEX_HOME", value)], &[r"D:\u\me"]);
            assert!(homes.open.is_empty(), "{value}");
            assert_eq!(homes.wide.len(), 1, "{value}");
            assert_eq!(homes.wide[0].holds, Holds::Profile, "{value}");
            assert_eq!(homes.wide[0].variable, "CODEX_HOME", "{value}");
        }
        // The profile spelt in another case is still the profile, so it stays open.
        let homes = homes_of(&[("CODEX_HOME", r"d:\U\ME\")], &[r"D:\u\me"]);
        assert!(homes.wide.is_empty());
        assert_eq!(homes.open.len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn a_link_to_a_wide_folder_opens_nothing() {
        let really = |path: &Path| match path.to_str() {
            Some(r"D:\link") => PathBuf::from(r"D:\u"),
            _ => path.to_path_buf(),
        };
        let profiles = [PathBuf::from(r"D:\u\me")];
        let homes = agent_homes_as(
            &vars(&[("CODEX_HOME", r"D:\link")]),
            &really,
            &profiles,
            Some(Path::new(r"D:\m\home")),
        );
        assert!(homes.open.is_empty());
        assert_eq!(homes.wide, vec![wide("CODEX_HOME", r"D:\link", Holds::Profile)]);
    }

    #[test]
    fn a_profile_that_is_a_link_is_still_held_by_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = canonical(dir.path());
        fs::create_dir_all(tmp.join("real").join("me")).unwrap();
        fs::create_dir_all(tmp.join("Users")).unwrap();
        if let Err(e) = link_dir(&tmp.join("Users").join("me"), &tmp.join("real").join("me")) {
            eprintln!("skipped: cannot create a symlink here: {e}");
            return;
        }
        let profiles = [tmp.join("Users").join("me")];
        let mh = tmp.join("mh");
        for folder in [tmp.join("Users"), tmp.join("real")] {
            let given = [("CODEX_HOME", folder.to_string_lossy().into_owned())];
            let env = |k: &str| given.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone());
            let homes = agent_homes_as(&env, &canonical, &profiles, Some(&mh));
            assert!(homes.open.is_empty(), "{}", folder.display());
            assert_eq!(homes.wide.len(), 1, "{}", folder.display());
            assert_eq!(homes.wide[0].holds, Holds::Profile, "{}", folder.display());
        }
    }

    #[test]
    fn a_file_under_a_link_to_a_wide_folder_names_the_variable() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = canonical(dir.path());
        fs::create_dir_all(tmp.join("u").join("me")).unwrap();
        if let Err(e) = link_dir(&tmp.join("link"), &tmp.join("u")) {
            eprintln!("skipped: cannot create a symlink here: {e}");
            return;
        }
        let profiles = [tmp.join("u").join("me")];
        let mh = tmp.join("mh");
        let value = tmp.join("link").to_string_lossy().into_owned();
        let env = |k: &str| (k == "CODEX_HOME").then(|| value.clone());
        let homes = agent_homes_as(&env, &canonical, &profiles, Some(&mh));
        assert!(homes.open.is_empty());
        assert_eq!(homes.wide.len(), 1);
        let guard = Guard::new(homes.open, vec![], mh).with_wide_homes(homes.wide);
        let file = tmp.join("link").join("hooks.json");
        assert!(!file.exists());
        assert_eq!(guard.wide_home_of(&file).map(|w| w.variable), Some("CODEX_HOME"));
    }

    #[cfg(windows)]
    #[test]
    fn a_drive_root_that_holds_the_profile_is_named() {
        let homes = homes_of(&[("CODEX_HOME", r"D:\")], &[r"D:\u\me"]);
        assert!(homes.open.is_empty());
        assert_eq!(homes.wide, vec![wide("CODEX_HOME", r"D:\", Holds::Profile)]);
        // A root that holds neither is dropped without a record, as before.
        let homes = homes_of(&[("CODEX_HOME", r"E:\")], &[r"D:\u\me"]);
        assert_eq!(homes, AgentHomes::default());
    }

    #[cfg(windows)]
    #[test]
    fn a_path_in_an_allowed_folder_never_names_a_wide_home() {
        let guard = Guard::new(vec![PathBuf::from(r"D:\u\me")], vec![], PathBuf::from(r"D:\m\home"))
            .with_wide_homes(vec![wide("CODEX_HOME", r"D:\u", Holds::Profile)]);
        assert_eq!(guard.wide_home_of(Path::new(r"D:\u\me\x.json")), None);
        assert!(guard.wide_home_of(Path::new(r"D:\u\x.json")).is_some());
    }

    #[cfg(windows)]
    #[test]
    fn the_second_spelling_of_the_profile_counts() {
        let homes = homes_of(&[("CODEX_HOME", r"E:\scratch")], &[r"D:\u\me", r"E:\scratch\home"]);
        assert!(homes.open.is_empty());
        assert_eq!(homes.wide, vec![wide("CODEX_HOME", r"E:\scratch", Holds::Profile)]);
    }

    #[cfg(windows)]
    #[test]
    fn a_wide_home_does_not_close_the_others() {
        let given = [("CODEX_HOME", r"D:\u"), ("QWEN_HOME", r"D:\w\qw")];
        let homes = homes_of(&given, &[r"D:\u\me"]);
        assert_eq!(homes.open, vec![PathBuf::from(r"D:\w\qw")]);
        assert_eq!(homes.wide, vec![wide("CODEX_HOME", r"D:\u", Holds::Profile)]);
    }

    #[test]
    fn qwens_tilde_is_the_profile_and_stays_open() {
        let Some(profile) = dirs::home_dir() else {
            return;
        };
        let mh = tempfile::tempdir().unwrap();
        let homes = agent_homes_as(
            &vars(&[("QWEN_HOME", "~")]),
            &canonical,
            std::slice::from_ref(&profile),
            Some(mh.path()),
        );
        assert!(homes.wide.is_empty());
        assert_eq!(homes.open, vec![profile]);
    }

    #[cfg(windows)]
    #[test]
    fn a_write_under_a_wide_home_is_refused_and_names_it() {
        let homes = homes_of(&[("CODEX_HOME", r"D:\u")], &[r"D:\u\me"]);
        let guard = Guard::new(homes.open, vec![], PathBuf::from(r"D:\m\home")).with_wide_homes(homes.wide);
        let file = Path::new(r"D:\u\hooks.json");
        assert_eq!(guard.check(file), Err(Refusal::OutsideAllowed));
        assert_eq!(guard.wide_home_of(file).map(|w| w.variable), Some("CODEX_HOME"));
        assert_eq!(guard.wide_home_of(Path::new(r"D:\elsewhere\x.json")), None);
    }
}
