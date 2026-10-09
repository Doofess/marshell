/// The shell a new tab runs when the request names none.
/// Windows: PowerShell 7 if installed, else Windows PowerShell. Unix: `$SHELL -l`.
/// (Full shell detection and profiles come in phase 2.)
pub fn default_shell() -> Vec<String> {
    #[cfg(windows)]
    {
        let exe = if on_path("pwsh.exe") { "pwsh.exe" } else { "powershell.exe" };
        vec![exe.to_string(), "-NoLogo".to_string()]
    }
    #[cfg(unix)]
    {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        vec![sh, "-l".to_string()]
    }
}

#[cfg(windows)]
fn on_path(exe: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|dir| dir.join(exe).is_file()))
        .unwrap_or(false)
}
