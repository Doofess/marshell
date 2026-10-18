use std::path::{Path, PathBuf};

/// The shell that runs the user's status line command: `program args_before_command <command>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCmd {
    pub program: PathBuf,
    pub args_before_command: Vec<String>,
}

/// Picks the shell the way Claude Code does on Windows: `CLAUDE_CODE_GIT_BASH_PATH`, then the
/// `bash.exe` of the Git for Windows that holds the `git.exe` on `PATH`, then the default install,
/// then PowerShell. The first `bash` on `PATH` is never used: on a machine with WSL it is the
/// launcher in `WindowsApps`, and nothing under `WindowsApps` is used at all. Off Windows it is `sh`.
/// `exists` and `git_on_path` come from the caller, so this stays pure.
pub fn resolve(
    env: &dyn Fn(&str) -> Option<String>,
    exists: &dyn Fn(&Path) -> bool,
    git_on_path: Option<PathBuf>,
) -> ShellCmd {
    if !cfg!(windows) {
        return ShellCmd {
            program: PathBuf::from("sh"),
            args_before_command: vec!["-c".into()],
        };
    }
    let from_env = env("CLAUDE_CODE_GIT_BASH_PATH").filter(|p| !p.is_empty());
    let candidates = from_env
        .map(PathBuf::from)
        .into_iter()
        .chain(git_on_path.as_deref().and_then(bash_next_to_git))
        .chain([PathBuf::from("C:/Program Files/Git/bin/bash.exe")]);
    for candidate in candidates {
        let candidate = PathBuf::from(candidate.to_string_lossy().replace('\\', "/"));
        if !in_windowsapps(&candidate) && exists(&candidate) {
            return ShellCmd {
                program: candidate,
                args_before_command: vec!["-c".into()],
            };
        }
    }
    ShellCmd {
        program: PathBuf::from("powershell.exe"),
        args_before_command: vec!["-NoProfile".into(), "-Command".into()],
    }
}

/// `<root>/bin/bash.exe` for a `git.exe` in `<root>/cmd`, `<root>/bin` or `<root>/mingw64/bin`.
fn bash_next_to_git(git: &Path) -> Option<PathBuf> {
    let dir = git.parent()?;
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase());
    let root = match name(dir)?.as_str() {
        "cmd" => dir.parent()?,
        "bin" => {
            let up = dir.parent()?;
            if name(up).as_deref() == Some("mingw64") {
                up.parent()?
            } else {
                up
            }
        }
        _ => return None,
    };
    Some(root.join("bin").join("bash.exe"))
}

fn in_windowsapps(path: &Path) -> bool {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
        .contains("/windowsapps/")
}

/// Scans `PATH` for `git.exe`. This and the `exists` closure are the only parts that touch the
/// real filesystem.
pub fn find_git_on_path(path_var: Option<&str>) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    std::env::split_paths(path_var?)
        .map(|dir| dir.join("git.exe"))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn only(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let paths: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
        move |p| paths.contains(&p.to_string_lossy().replace('\\', "/"))
    }

    #[cfg(windows)]
    mod windows {
        use super::*;

        fn ps() -> ShellCmd {
            ShellCmd {
                program: PathBuf::from("powershell.exe"),
                args_before_command: vec!["-NoProfile".into(), "-Command".into()],
            }
        }

        fn bash(path: &str) -> ShellCmd {
            ShellCmd {
                program: PathBuf::from(path),
                args_before_command: vec!["-c".into()],
            }
        }

        #[test]
        fn git_bash_path_env_wins() {
            let env = |k: &str| (k == "CLAUDE_CODE_GIT_BASH_PATH").then(|| "D:/g/bin/bash.exe".to_string());
            let got = resolve(
                &env,
                &only(&["D:/g/bin/bash.exe", "C:/Program Files/Git/bin/bash.exe"]),
                Some(PathBuf::from("C:/Program Files/Git/cmd/git.exe")),
            );
            assert_eq!(got, bash("D:/g/bin/bash.exe"));
        }

        #[test]
        fn env_path_that_does_not_exist_is_skipped() {
            let env = |k: &str| (k == "CLAUDE_CODE_GIT_BASH_PATH").then(|| "D:/g/bin/bash.exe".to_string());
            let got = resolve(&env, &only(&["C:/Program Files/Git/bin/bash.exe"]), None);
            assert_eq!(got, bash("C:/Program Files/Git/bin/bash.exe"));
        }

        #[test]
        fn bash_is_found_next_to_git() {
            for git in [
                "C:/Program Files/Git/cmd/git.exe",
                "C:/Program Files/Git/bin/git.exe",
                "C:/Program Files/Git/mingw64/bin/git.exe",
            ] {
                let got = resolve(
                    &no_env,
                    &only(&["C:/Program Files/Git/bin/bash.exe"]),
                    Some(PathBuf::from(git)),
                );
                assert_eq!(got, bash("C:/Program Files/Git/bin/bash.exe"), "{git}");
            }
        }

        #[test]
        fn a_git_in_another_place_is_followed() {
            let got = resolve(
                &no_env,
                &only(&["D:/tools/git/bin/bash.exe", "C:/Program Files/Git/bin/bash.exe"]),
                Some(PathBuf::from("D:/tools/git/cmd/git.exe")),
            );
            assert_eq!(got, bash("D:/tools/git/bin/bash.exe"));
        }

        #[test]
        fn default_git_install_is_tried() {
            let got = resolve(&no_env, &only(&["C:/Program Files/Git/bin/bash.exe"]), None);
            assert_eq!(got, bash("C:/Program Files/Git/bin/bash.exe"));
        }

        #[test]
        fn never_the_windowsapps_bash() {
            let wsl = "C:/Users/me/AppData/Local/Microsoft/WindowsApps/bash.exe";
            let got = resolve(&no_env, &only(&[wsl]), None);
            assert_eq!(got, ps());
            // Even when the env var, or a git inside WindowsApps, points at it.
            let env = |k: &str| (k == "CLAUDE_CODE_GIT_BASH_PATH").then(|| wsl.to_string());
            let got = resolve(&env, &only(&[wsl]), None);
            assert_eq!(got, ps());
            let apps_git = "C:\\Users\\me\\AppData\\Local\\Microsoft\\WindowsApps\\git.exe";
            let got = resolve(
                &no_env,
                &only(&["C:/Users/me/AppData/Local/Microsoft/bin/bash.exe"]),
                Some(PathBuf::from(apps_git)),
            );
            assert_eq!(got, ps());
        }

        #[test]
        fn windowsapps_check_ignores_case_and_slash_style() {
            let env = |k: &str| (k == "CLAUDE_CODE_GIT_BASH_PATH").then(|| "C:\\x\\windowsapps\\bash.exe".to_string());
            let got = resolve(&env, &only(&["C:/x/windowsapps/bash.exe"]), None);
            assert_eq!(got, ps());
        }

        #[test]
        fn nothing_found_gives_powershell() {
            assert_eq!(resolve(&no_env, &|_| false, None), ps());
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_it_is_sh() {
        let got = resolve(&no_env, &|_| true, None);
        assert_eq!(
            got,
            ShellCmd {
                program: PathBuf::from("sh"),
                args_before_command: vec!["-c".into()],
            }
        );
    }
}
