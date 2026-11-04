//! `marshell uninstall`: shows what taking Marshell out of each agent CLI (on Windows and in the WSL distros) would
//! change, asks, and does it through the config service (so it is hashed, snapshotted and undoable), then removes
//! `<home>/bin`.
//!
//! Exit codes: 0 done or nothing to do, 1 refused, failed or declined, or a WSL distro that didn't answer.

use marshell_core::agents::antigravity::hooks as agy_hooks;
use marshell_core::agents::claude::hooks::{remove_plan, settings_path};
use marshell_core::agents::codex::hooks as codex_hooks;
use marshell_core::agents::distro as distro_hooks;
use marshell_core::agents::qwen::hooks as qwen_hooks;
use marshell_core::agents::skill;
use marshell_core::config::guard::unverbatim;
use marshell_core::config::managed;
use marshell_core::config::preview::{MAX_CONFIG_BYTES, TOO_LARGE_DETAIL};
use marshell_core::config::service::{ApplyError, ConfigService};
use marshell_core::{paths, wsl};
use marshell_protocol::agents::SkillInstall;
use marshell_protocol::config::{ApplyRequest, ChangeSet, ConfigPreview, ExpectedHash, FileStatus};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// How `run` looks into WSL: the installed distros, a distro's home as a path under its share (None when it doesn't
/// answer), and whether that home is a plain folder a test pinned.
pub struct Wsl<'a> {
    pub distros: &'a dyn Fn() -> Vec<String>,
    pub home: &'a (dyn Fn(&str) -> Option<PathBuf> + Sync),
    pub pinned: bool,
}

/// The distro whose share `path` is under (`\\wsl.localhost\<distro>\...` or `\\wsl$\<distro>\...`).
fn distro_of_share(path: &Path) -> Option<String> {
    let path = unverbatim(path);
    let text = path.to_str()?;
    let rest = text.strip_prefix(r"\\").or_else(|| text.strip_prefix("//"))?;
    let mut parts = rest.split(['\\', '/']);
    let (host, share) = (parts.next()?, parts.next()?);
    let on_wsl = host.eq_ignore_ascii_case("wsl.localhost") || host.eq_ignore_ascii_case("wsl$");
    (on_wsl && wsl::under_distro(share, &path)).then(|| share.to_string())
}

/// The distros `managed.json` has entries in: Marshell wrote hooks there.
fn managed_distros(home: &Path) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for entry in managed::list(home).unwrap_or_default() {
        if let Some(name) = distro_of_share(Path::new(&entry.path))
            && !names.iter().any(|n| n.eq_ignore_ascii_case(&name))
        {
            names.push(name);
        }
    }
    names
}

/// The config service for this run. A distro's home that was looked up is opened to it only while the distro is in
/// `distros`; without this wiring every distro file comes back refused.
fn uninstall_service(home: &Path, distros: &[String], homes: &[(String, PathBuf)], pinned: bool) -> ConfigService {
    let detected = distros.to_vec();
    let service = ConfigService::with_distros(home.to_path_buf(), Arc::new(move || detected.clone()));
    for (distro, distro_home) in homes {
        service.record_distro_home(distro, distro_home, pinned);
    }
    service
}

/// The text of `path`, or `None` when the file is not there. Any other failure is an error.
fn read_text(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| format!("{} is not UTF-8 text, so nothing was changed.", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Couldn't read {}: {e}", path.display())),
    }
}

/// The files a preview refuses, as (path, why).
fn refusals(preview: &ConfigPreview) -> Vec<(&str, &str)> {
    preview
        .files
        .iter()
        .filter_map(|f| match &f.status {
            FileStatus::Refused { detail, .. } => Some((f.path.as_str(), detail.as_str())),
            _ => None,
        })
        .collect()
}

fn confirmed(input: &mut dyn BufRead, out: &mut dyn Write, question: &str) -> bool {
    let _ = write!(out, "{question}");
    let _ = out.flush();
    let mut line = String::new();
    if input.read_line(&mut line).unwrap_or(0) == 0 {
        // End of input is a no.
        let _ = writeln!(out);
        return false;
    }
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Removes the files in `<home>/bin` one by one, then the folder if it is empty. On Windows the
/// running executable is removed after exit. Other files that stay are reported, not a failure.
fn remove_bin(home: &Path, out: &mut dyn Write) {
    let bin = paths::bin_dir(home);
    let Ok(entries) = std::fs::read_dir(&bin) else {
        return;
    };
    #[cfg(windows)]
    let mut failed = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let removed = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if removed.is_err() {
            #[cfg(windows)]
            failed.push(path);
            #[cfg(not(windows))]
            let _ = writeln!(
                out,
                "Couldn't remove {}; delete it once this window is closed.",
                path.display()
            );
        }
    }
    #[cfg(windows)]
    if failed.len() == 1 && defer_self_removal(&bin) {
        let _ = writeln!(out, "marshell.exe will be removed when this window closes.");
    } else {
        for path in failed {
            let _ = writeln!(
                out,
                "Couldn't remove {}; delete it once this window is closed.",
                path.display()
            );
        }
    }
    let _ = std::fs::remove_dir(&bin);
}

/// Only literal paths: PowerShell single quotes are escaped by doubling them.
#[cfg(windows)]
fn self_removal_command(pid: u32, exe: &str, bin: &str) -> String {
    let exe = exe.replace('\'', "''");
    let bin = bin.replace('\'', "''");
    format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
         Remove-Item -LiteralPath '{exe}' -ErrorAction Stop; \
         if (-not (Get-ChildItem -LiteralPath '{bin}' -Force -ErrorAction Stop)) {{ \
         Remove-Item -LiteralPath '{bin}' -ErrorAction Stop }}"
    )
}

/// Build nothing unless the running exe is a direct child of bin and its only remaining entry.
#[cfg(windows)]
fn checked_self_removal_command(pid: u32, exe: &Path, bin: &Path) -> Option<String> {
    // The plain `C:\dir` form, not `\\?\C:\dir`, so PowerShell gets an ordinary path.
    let exe = marshell_core::projects::canonical(exe);
    let bin = marshell_core::projects::canonical(bin);
    if exe.parent()?.to_str()?.to_lowercase() != bin.to_str()?.to_lowercase() {
        return None;
    }
    let mut entries = std::fs::read_dir(&bin).ok()?;
    let remaining = marshell_core::projects::canonical(&entries.next()?.ok()?.path());
    if entries.next().is_some() || remaining.to_str()?.to_lowercase() != exe.to_str()?.to_lowercase() {
        return None;
    }
    Some(self_removal_command(pid, exe.to_str()?, bin.to_str()?))
}

#[cfg(windows)]
fn defer_self_removal(bin: &Path) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(command) = checked_self_removal_command(std::process::id(), &exe, bin) else {
        return false;
    };
    // A hidden console of its own, so closing this window doesn't end it. Not DETACHED_PROCESS:
    // powershell.exe started without a console exits before running the command.
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &command,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        // Don't keep bin open if uninstall was launched with bin as its working directory.
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

pub fn run(
    home: &Path,
    user_home: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    wsl: &Wsl,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    yes: bool,
) -> i32 {
    let settings = settings_path(env, user_home);
    let chain = paths::statusline_chain(home, "claude");
    let (text, chain_text) = match (read_text(&settings), read_text(&chain)) {
        (Ok(text), Ok(chain_text)) => (text, chain_text),
        (Err(e), _) | (_, Err(e)) => {
            let _ = writeln!(out, "{e}");
            return 1;
        }
    };
    let mut plan = match remove_plan(
        &settings,
        user_home,
        text.as_deref(),
        chain_text.as_deref(),
        Some(&chain),
        None,
    ) {
        Ok(plan) => plan,
        Err(e) => {
            let _ = writeln!(out, "{e}");
            return 1;
        }
    };
    // Codex's hooks.json. Nothing about its config.toml is read or written here. A file that can't be
    // read or planned from is reported and left alone; Claude Code's removal goes on without it, and
    // the bin folder stays, because Codex's entries may still point at the bridge.
    let codex_file = codex_hooks::hooks_path(env, user_home);
    let codex = read_text(&codex_file).and_then(|text| {
        codex_hooks::remove_plan(&codex_file, text.as_deref(), home)
            .map(|changes| (text, changes))
            .map_err(|e| e.to_string())
    });
    // Codex's, Antigravity's or Qwen Code's hooks could not be planned, so their entries may still point at the bridge.
    let mut hooks_failed = false;
    // A skill file could not be read: the removal goes on without it, but the exit code says it was left.
    let mut skill_unread = false;
    let mut extras = Vec::new();
    match codex {
        Ok((text, changes)) => {
            if !changes.edits.is_empty() {
                extras.push(changes);
            } else if text.is_some() {
                let _ = writeln!(out, "Marshell has no hooks in {}.", codex_file.display());
            }
        }
        Err(e) => {
            let _ = writeln!(out, "{e}");
            hooks_failed = true;
        }
    }
    // Antigravity's hooks.json and settings.json, the same way: a problem is reported and the rest goes on.
    let agy_file = agy_hooks::hooks_path(user_home);
    let agy = read_text(&agy_file).and_then(|hooks_text| {
        let settings_text = read_text(&agy_hooks::settings_path(user_home))?;
        let chain = paths::statusline_chain(home, "antigravity");
        let chain_text = read_text(&chain)?;
        agy_hooks::remove_plan(
            user_home,
            hooks_text.as_deref(),
            settings_text.as_deref(),
            chain_text.as_deref(),
            Some(&chain),
            None,
        )
        .map(|changes| (hooks_text, changes))
        .map_err(|e| e.to_string())
    });
    match agy {
        Ok((text, changes)) => {
            if !changes.edits.is_empty() {
                extras.push(changes);
            } else if text.is_some() {
                let _ = writeln!(out, "Marshell has no hooks in {}.", agy_file.display());
            }
        }
        Err(e) => {
            let _ = writeln!(out, "{e}");
            hooks_failed = true;
        }
    }
    // Qwen Code's user settings.json and its chain, the same way: a problem is reported and the rest goes on.
    let qwen_file = qwen_hooks::settings_path(env, user_home);
    let qwen = read_text(&qwen_file).and_then(|text| {
        let chain = paths::statusline_chain(home, "qwen");
        let chain_text = read_text(&chain)?;
        qwen_hooks::remove_plan(
            &qwen_file,
            user_home,
            text.as_deref(),
            chain_text.as_deref(),
            Some(&chain),
            None,
        )
        .map(|plan| (text, plan))
        .map_err(|e| e.to_string())
    });
    match qwen {
        Ok((text, qwen_plan)) => {
            plan.statusline_left |= qwen_plan.statusline_left;
            if !qwen_plan.changes.edits.is_empty() {
                extras.push(qwen_plan.changes);
            } else if text.is_some() {
                let _ = writeln!(out, "Marshell has no hooks in {}.", qwen_file.display());
            }
        }
        Err(e) => {
            let _ = writeln!(out, "{e}");
            hooks_failed = true;
        }
    }
    // The WSL distros: the installed ones, and any `managed.json` names that the list missed. Every home is asked for at
    // once, each with its own time limit, because a stopped distro is started by the question.
    let managed_names = managed_distros(home);
    let listed = (wsl.distros)();
    let mut names: Vec<String> = Vec::new();
    for name in listed.iter().cloned().chain(managed_names.iter().cloned()) {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            names.push(name);
        }
    }
    if !names.is_empty() {
        let _ = writeln!(out, "Looking in WSL distros: {}.", names.join(", "));
    }
    let lookup = wsl.home;
    let answers: Vec<(String, Option<PathBuf>)> = std::thread::scope(|scope| {
        let asked: Vec<_> = names.iter().map(|name| scope.spawn(move || lookup(name))).collect();
        names
            .iter()
            .zip(asked)
            .map(|(name, asked)| (name.clone(), asked.join().unwrap_or(None)))
            .collect()
    });
    let homes: Vec<(String, PathBuf)> = answers
        .iter()
        .filter_map(|(name, answer)| answer.clone().map(|answer| (name.clone(), answer)))
        .collect();
    // Built once, with the looked-up homes, so the guard admits the distro files and the skills below share it.
    let service = uninstall_service(home, &names, &homes, wsl.pinned);
    // A distro that didn't answer, or whose files can't all be removed, may still have hooks pointing at bin.
    let mut distro_kept: Vec<String> = Vec::new();
    let mut distro_unanswered = false;
    for (name, answer) in &answers {
        let Some(distro_home) = answer else {
            // WSL answered with its distros and this one isn't among them: it was unregistered, and its files with it.
            // An empty list may be WSL itself failing, so it proves nothing.
            if !listed.is_empty() && !listed.iter().any(|l| l.eq_ignore_ascii_case(name)) {
                let _ = writeln!(out, "{name} isn't installed in WSL any more: nothing to remove there.");
                continue;
            }
            let _ = writeln!(out, "{name} didn't answer. Start it and run marshell uninstall again.");
            distro_unanswered = true;
            if managed_names.iter().any(|m| m.eq_ignore_ascii_case(name)) {
                distro_kept.push(name.clone());
            }
            continue;
        };
        // Each agent's removal is previewed on its own first, so a file that is refused or doesn't plan is reported by
        // path and left, and the rest of the distro, the other distros and Windows go on.
        let (mut found, mut failed) = (false, false);
        for (agent, plan) in distro_hooks::remove_plans(name, distro_home, home) {
            match plan {
                Err(e) => {
                    let _ = writeln!(out, "{e}");
                    failed = true;
                }
                Ok(None) => {}
                Ok(Some(changes)) => {
                    let preview = service.preview(changes.clone());
                    let refused = refusals(&preview);
                    if refused.is_empty() && preview.can_apply {
                        extras.push(changes);
                        found = true;
                    } else {
                        for (path, detail) in &refused {
                            let _ = writeln!(out, "Marshell can't change {path}: {detail}");
                        }
                        if refused.is_empty() {
                            let _ = writeln!(out, "{name}: Marshell can't change the {agent} files.");
                        }
                        failed = true;
                    }
                }
            }
        }
        if found {
            let _ = writeln!(out, "{name}: Marshell's hooks found.");
        } else if !failed {
            let _ = writeln!(out, "{name}: nothing of Marshell's.");
        }
        if failed {
            distro_kept.push(name.clone());
        }
    }
    // The marshell skill each CLI has: one Marshell wrote joins the removal, an edited one is left. Each removal
    // is previewed on its own first, so a skill the config service refuses is left out and the rest goes on.
    for (agent, name) in skill::AGENTS {
        let Some(path) = skill::skill_path(agent, env, user_home) else {
            continue;
        };
        let read = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_CONFIG_BYTES => Err(TOO_LARGE_DETAIL.to_string()),
            _ => match std::fs::read(&path) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.to_string()),
            },
        };
        let bytes = match read {
            Ok(bytes) => bytes,
            Err(why) => {
                let _ = writeln!(out, "Couldn't read {}: {why}", path.display());
                skill_unread = true;
                continue;
            }
        };
        match skill::state_of(bytes.as_deref()) {
            SkillInstall::None | SkillInstall::Unavailable => {}
            SkillInstall::Installed | SkillInstall::Outdated => {
                let changes = skill::remove_plan(name, &path, bytes.as_deref());
                let preview = service.preview(changes.clone());
                let refused = refusals(&preview);
                if refused.is_empty() && preview.can_apply {
                    extras.push(changes);
                } else {
                    for (path, detail) in refused {
                        let _ = writeln!(out, "Marshell can't change {path}: {detail}");
                    }
                }
            }
            SkillInstall::Edited => {
                let _ = writeln!(
                    out,
                    "Left {}: it was edited after Marshell installed it.",
                    path.display()
                );
            }
        }
    }
    let claude_empty = plan.changes.edits.is_empty();
    for changes in extras {
        if plan.changes.edits.is_empty() {
            plan.changes = changes;
        } else {
            plan.changes = ChangeSet {
                title: "Turn Marshell's hooks off".to_string(),
                reason: format!("{} {}", plan.changes.reason, changes.reason),
                edits: plan.changes.edits.into_iter().chain(changes.edits).collect(),
            };
        }
    }
    let bin = paths::bin_dir(home);
    let others_failed = hooks_failed || skill_unread || !distro_kept.is_empty();
    let has_bin = bin.exists() && !others_failed;
    // Non-zero when something was left, so a script notices.
    let ok = i32::from(others_failed || distro_unanswered);
    let keep_note = |out: &mut dyn Write| {
        if hooks_failed && bin.exists() {
            let _ = writeln!(
                out,
                "The folder {} was left in place, because Codex's, Antigravity's or Qwen Code's hooks may still point at it.",
                bin.display()
            );
        }
        if !distro_kept.is_empty() && bin.exists() {
            let _ = writeln!(
                out,
                "The folder {} was left in place, because hooks in {} may still point at it.",
                bin.display(),
                distro_kept.join(", ")
            );
        }
    };
    let left_note = "Your status line was changed since the setup and was left as it is.";

    if plan.changes.edits.is_empty() {
        if claude_empty {
            let _ = writeln!(out, "Marshell has no hooks in {}.", settings.display());
        }
        if plan.statusline_left {
            let _ = writeln!(out, "{left_note}");
        }
        if !has_bin {
            keep_note(out);
            return ok;
        }
        let _ = writeln!(
            out,
            "The folder {} and the programs in it will be removed.",
            bin.display()
        );
        if !yes && !confirmed(input, out, "Apply these changes? [y/N] ") {
            let _ = writeln!(out, "Nothing changed.");
            return 1;
        }
        remove_bin(home, out);
        return ok;
    }

    let preview = service.preview(plan.changes);
    let refused = refusals(&preview);
    if !refused.is_empty() || !preview.can_apply {
        for (path, detail) in &refused {
            let _ = writeln!(out, "Marshell can't change {path}: {detail}");
        }
        let _ = writeln!(out, "Nothing was changed.");
        return 1;
    }

    let _ = writeln!(out, "{}", preview.title);
    let _ = writeln!(out, "{}", preview.reason);
    for file in &preview.files {
        let _ = writeln!(out, "\n{}", file.path);
        let _ = write!(out, "{}", file.diff);
        if !file.diff.ends_with('\n') {
            let _ = writeln!(out);
        }
    }
    if plan.statusline_left {
        let _ = writeln!(out, "\n{left_note}");
    }
    if has_bin {
        let _ = writeln!(
            out,
            "\nThe folder {} and the programs in it will be removed.",
            bin.display()
        );
    }
    let _ = writeln!(out);

    if !yes && !confirmed(input, out, "Apply these changes? [y/N] ") {
        let _ = writeln!(out, "Nothing changed.");
        return 1;
    }

    let request = ApplyRequest {
        changeset_id: preview.changeset_id.clone(),
        expected_hashes: preview
            .files
            .iter()
            .map(|f| ExpectedHash {
                path: f.path.clone(),
                hash: f.before_hash.clone(),
            })
            .collect(),
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    match service.apply(&request, now_ms) {
        Ok(applied) => {
            let _ = writeln!(
                out,
                "Done. Snapshot {} holds the files as they were before this change.",
                applied.snapshot_id
            );
        }
        Err(ApplyError::Conflict { reason, paths }) => {
            let _ = writeln!(
                out,
                "Nothing was changed: {reason} ({}). Run it again.",
                paths.join(", ")
            );
            return 1;
        }
        Err(ApplyError::UnknownChangeSet) => {
            let _ = writeln!(out, "Nothing was changed: the preview expired. Run it again.");
            return 1;
        }
        Err(ApplyError::Failed(e)) => {
            let _ = writeln!(out, "Nothing was removed: {e:#}");
            return 1;
        }
    }
    if has_bin {
        remove_bin(home, out);
    } else {
        keep_note(out);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_core::agents::claude::hooks::{install_plan, settings_path};
    use marshell_core::agents::common::Bridge;
    use marshell_core::config::service::ConfigService;
    use marshell_protocol::agents::HookPlanRequest;
    use marshell_protocol::config::{ApplyRequest, ExpectedHash};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    const ORIGINAL: &str =
        "{\n  \"theme\": \"dark\",\n  \"statusLine\": {\"type\": \"command\", \"command\": \"my-line\"}\n}\n";

    #[cfg(windows)]
    #[test]
    fn self_removal_quotes_exact_paths_without_recursion_or_wildcards() {
        let command = self_removal_command(123, r"C:\Marshell's home\bin\marshell.exe", r"C:\Marshell's home\bin");
        assert_eq!(
            command,
            "Wait-Process -Id 123 -ErrorAction SilentlyContinue; \
             Remove-Item -LiteralPath 'C:\\Marshell''s home\\bin\\marshell.exe' -ErrorAction Stop; \
             if (-not (Get-ChildItem -LiteralPath 'C:\\Marshell''s home\\bin' -Force -ErrorAction Stop)) { \
             Remove-Item -LiteralPath 'C:\\Marshell''s home\\bin' -ErrorAction Stop }"
        );
        assert!(!command.to_lowercase().contains("-recurse"));
        assert!(!command.contains(['*', '?', '[', ']']));
    }

    #[cfg(windows)]
    #[test]
    fn self_removal_is_not_built_for_an_exe_outside_bin() {
        let dir = tempfile::tempdir().unwrap();
        let bin = paths::bin_dir(dir.path());
        let outside = dir.path().join("bin-other");
        std::fs::create_dir(&bin).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let exe = outside.join("marshell.exe");
        std::fs::write(&exe, b"exe").unwrap();
        std::fs::write(bin.join("marshell.exe"), b"exe").unwrap();
        assert!(checked_self_removal_command(123, &exe, &bin).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn self_removal_requires_only_the_running_exe_in_bin() {
        let dir = tempfile::tempdir().unwrap();
        let bin = paths::bin_dir(dir.path());
        std::fs::create_dir(&bin).unwrap();
        let exe = bin.join("marshell.exe");
        std::fs::write(&exe, b"exe").unwrap();
        assert!(checked_self_removal_command(123, &exe, &bin).is_some());
        let other = bin.join("marshell-hook.exe");
        std::fs::write(&other, b"hook").unwrap();
        assert!(checked_self_removal_command(123, &exe, &bin).is_none());
        std::fs::remove_file(&exe).unwrap();
        assert!(checked_self_removal_command(123, &exe, &bin).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn self_removal_canonicalizes_paths_and_ignores_case() {
        let dir = tempfile::tempdir().unwrap();
        let bin = paths::bin_dir(dir.path());
        std::fs::create_dir(&bin).unwrap();
        let exe = bin.join("marshell.exe");
        std::fs::write(&exe, b"exe").unwrap();
        let upper = PathBuf::from(exe.to_str().unwrap().to_uppercase());
        assert_eq!(
            checked_self_removal_command(123, &upper, &bin.join("..\\bin")),
            Some(self_removal_command(
                123,
                marshell_core::projects::canonical(&exe).to_str().unwrap(),
                marshell_core::projects::canonical(&bin).to_str().unwrap(),
            ))
        );
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        home: PathBuf,
        user_home: PathBuf,
        settings: PathBuf,
        install_snapshot: String,
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    /// A temp Marshell home and user home, with the hooks installed by the real install plan and config service.
    fn installed(original: &str) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("marshell-home");
        let user_home = dir.path().join("user");
        let settings = settings_path(&no_env, &user_home);
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(&settings, original).unwrap();
        let want = HookPlanRequest {
            status: true,
            statusline: true,
            approve: false,
            skill: false,
            distro: None,
        };
        let chain = paths::statusline_chain(&home, "claude");
        let bridge = Bridge::local(&home);
        let changes = install_plan(
            &settings,
            &user_home,
            Some(original),
            None,
            Some(&chain),
            &bridge,
            &want,
        )
        .unwrap();
        let service = ConfigService::new(home.clone());
        let preview = service.preview(changes);
        assert!(preview.can_apply, "{preview:?}");
        let request = ApplyRequest {
            changeset_id: preview.changeset_id.clone(),
            expected_hashes: preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        };
        let applied = service.apply(&request, 1).unwrap();
        assert_ne!(std::fs::read_to_string(&settings).unwrap(), original);
        // The programs the app copies in at startup.
        let bin = marshell_core::paths::bin_dir(&home);
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(marshell_core::paths::bridge_exe(&home), b"hook").unwrap();
        std::fs::write(marshell_core::paths::cli_exe(&home), b"cli").unwrap();
        Fixture {
            _dir: dir,
            home,
            user_home,
            settings,
            install_snapshot: applied.snapshot_id,
        }
    }

    fn run_with(f: &Fixture, input: &str, yes: bool) -> (i32, String) {
        let mut out = Vec::new();
        let code = run(
            &f.home,
            &f.user_home,
            &no_env,
            &no_wsl(),
            &mut input.as_bytes(),
            &mut out,
            yes,
        );
        (code, String::from_utf8(out).unwrap())
    }

    /// Installs Codex's hooks into `<user home>/.codex/hooks.json` through the real plan and service.
    fn install_codex(f: &Fixture, original: Option<&str>) -> PathBuf {
        use marshell_core::agents::codex::hooks as ch;
        let path = ch::hooks_path(&no_env, &f.user_home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if let Some(text) = original {
            std::fs::write(&path, text).unwrap();
        }
        let want = HookPlanRequest {
            status: true,
            statusline: false,
            approve: false,
            skill: false,
            distro: None,
        };
        let changes = ch::install_plan(&path, original, &Bridge::local(&f.home), &want).unwrap();
        let service = ConfigService::new(f.home.clone());
        let preview = service.preview(changes);
        assert!(preview.can_apply, "{preview:?}");
        let request = ApplyRequest {
            changeset_id: preview.changeset_id.clone(),
            expected_hashes: preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        };
        service.apply(&request, 2).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("mshl1"));
        path
    }

    #[test]
    fn removes_codex_hooks_too() {
        let f = installed(ORIGINAL);
        let mine = "{\"hooks\": {\"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"my-stop\"}]}]}}
";
        let codex = install_codex(&f, Some(mine));
        let (code, out) = run_with(
            &f, "y
", false,
        );
        assert_eq!(code, 0, "{out}");
        assert_eq!(out.matches("Apply these changes?").count(), 1, "{out}");
        assert!(out.contains(&*f.settings.to_string_lossy()), "{out}");
        assert!(out.contains("hooks.json"), "{out}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        let after = std::fs::read_to_string(&codex).unwrap();
        assert!(after.contains("my-stop") && !after.contains("mshl1"), "{after}");
    }

    /// A `hooks.json` Marshell can't plan from, in the place Codex's hooks go.
    fn broken_codex(f: &Fixture) -> (PathBuf, &'static str) {
        let broken = "{ \"hooks\": [oops\n";
        let path = marshell_core::agents::codex::hooks::hooks_path(&no_env, &f.user_home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, broken).unwrap();
        (path, broken)
    }

    #[test]
    fn a_codex_file_that_does_not_parse_does_not_stop_claudes_removal() {
        let f = installed(ORIGINAL);
        let (codex, broken) = broken_codex(&f);
        let (code, out) = run_with(&f, "y\n", false);
        // Codex's file is reported by name and left byte for byte as it was.
        assert!(
            out.contains(&*codex.to_string_lossy()) && out.contains("does not parse"),
            "{out}"
        );
        assert_eq!(std::fs::read_to_string(&codex).unwrap(), broken);
        // Claude's hooks still came out, after the usual confirmation.
        assert_eq!(out.matches("Apply these changes?").count(), 1, "{out}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        // Codex's entries may still point at the bridge, so bin stays, and the exit says so.
        assert!(marshell_core::paths::bridge_exe(&f.home).exists());
        assert!(out.contains("left in place"), "{out}");
        assert_ne!(code, 0, "{out}");
    }

    #[test]
    fn a_codex_file_that_does_not_parse_and_answer_no_changes_nothing() {
        let f = installed(ORIGINAL);
        let installed_text = std::fs::read_to_string(&f.settings).unwrap();
        let (codex, broken) = broken_codex(&f);
        let (code, out) = run_with(&f, "n\n", false);
        assert_ne!(code, 0, "{out}");
        assert!(out.contains("Nothing changed."), "{out}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), installed_text);
        assert_eq!(std::fs::read_to_string(&codex).unwrap(), broken);
        assert!(marshell_core::paths::bridge_exe(&f.home).exists());
    }

    #[test]
    fn a_codex_file_that_does_not_parse_keeps_bin_even_with_no_claude_hooks() {
        let f = installed(ORIGINAL);
        std::fs::write(&f.settings, ORIGINAL).unwrap();
        let (_, broken) = broken_codex(&f);
        let (code, out) = run_with(&f, "y\n", false);
        assert_ne!(code, 0, "{out}");
        assert!(marshell_core::paths::bridge_exe(&f.home).exists(), "{out}");
        assert!(out.contains("left in place"), "{out}");
        assert!(!out.contains("will be removed"), "{out}");
        let _ = broken;
    }

    /// Installs Antigravity's hooks under the fixture's user home with the real plan and config service.
    fn install_agy(f: &Fixture, mine: Option<&str>) -> PathBuf {
        use marshell_core::agents::antigravity::hooks as ah;
        let path = ah::hooks_path(&f.user_home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if let Some(text) = mine {
            std::fs::write(&path, text).unwrap();
        }
        let want = HookPlanRequest {
            status: true,
            statusline: false,
            approve: false,
            skill: false,
            distro: None,
        };
        let chain = paths::statusline_chain(&f.home, "antigravity");
        let bridge = Bridge::local(&f.home);
        let changes = ah::install_plan(&f.user_home, mine, None, None, Some(&chain), &bridge, &want).unwrap();
        let service = ConfigService::new(f.home.clone());
        let preview = service.preview(changes);
        assert!(preview.can_apply, "{preview:?}");
        let request = ApplyRequest {
            changeset_id: preview.changeset_id.clone(),
            expected_hashes: preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        };
        service.apply(&request, 3).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("mshl1"));
        path
    }

    #[test]
    fn uninstall_removes_antigravity_hooks() {
        let f = installed(ORIGINAL);
        let mine = "{\"mine\": {\"enabled\": true, \"Stop\": [{\"type\": \"command\", \"command\": \"my-stop\"}]}}\n";
        let agy = install_agy(&f, Some(mine));
        let (code, out) = run_with(&f, "y\n", false);
        assert_eq!(code, 0, "{out}");
        assert_eq!(out.matches("Apply these changes?").count(), 1, "{out}");
        assert!(out.contains("hooks.json"), "{out}");
        let after = std::fs::read_to_string(&agy).unwrap();
        assert!(after.contains("my-stop") && !after.contains("mshl1"), "{after}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        assert!(!marshell_core::paths::bin_dir(&f.home).exists(), "{out}");
    }

    /// Installs Qwen Code's hooks into `<user home>/.qwen/settings.json` through the real plan and config service.
    fn install_qwen(f: &Fixture, original: &str) -> PathBuf {
        use marshell_core::agents::qwen::hooks as qh;
        let path = qh::settings_path(&no_env, &f.user_home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, original).unwrap();
        let want = HookPlanRequest {
            status: true,
            statusline: true,
            approve: false,
            skill: false,
            distro: None,
        };
        let chain = paths::statusline_chain(&f.home, "qwen");
        let bridge = Bridge::local(&f.home);
        let changes =
            qh::install_plan(&path, &f.user_home, Some(original), None, Some(&chain), &bridge, &want).unwrap();
        let service = ConfigService::new(f.home.clone());
        let preview = service.preview(changes);
        assert!(preview.can_apply, "{preview:?}");
        let request = ApplyRequest {
            changeset_id: preview.changeset_id.clone(),
            expected_hashes: preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        };
        service.apply(&request, 4).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("mshl1"));
        path
    }

    #[test]
    fn uninstall_removes_qwens_hooks_and_restores_its_status_line() {
        let f = installed(ORIGINAL);
        let mine = "{\n  \"theme\": \"dark\",\n  \"ui\": {\"statusLine\": {\"type\": \"command\", \"command\": \"my-qwen-line\"}}\n}\n";
        let qwen = install_qwen(&f, mine);
        let during = std::fs::read_to_string(&qwen).unwrap();
        assert!(!during.contains("my-qwen-line"), "{during}");
        let (code, out) = run_with(&f, "y\n", false);
        assert_eq!(code, 0, "{out}");
        assert_eq!(out.matches("Apply these changes?").count(), 1, "{out}");
        assert!(out.contains("settings.json"), "{out}");
        assert_eq!(std::fs::read_to_string(&qwen).unwrap(), mine);
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        assert!(!marshell_core::paths::bin_dir(&f.home).exists(), "{out}");
    }

    #[test]
    fn uninstall_goes_on_past_a_qwen_settings_file_that_does_not_parse() {
        let f = installed(ORIGINAL);
        let path = marshell_core::agents::qwen::hooks::settings_path(&no_env, &f.user_home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ \"ui\": [oops\n").unwrap();
        let (code, out) = run_with(&f, "y\n", false);
        assert!(
            out.contains(&*path.to_string_lossy()) && out.contains("does not parse"),
            "{out}"
        );
        assert_eq!(out.matches("Apply these changes?").count(), 1, "{out}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ \"ui\": [oops\n");
        // Qwen's entries may still run the bridge, so bin stays and the exit says so.
        assert!(marshell_core::paths::bridge_exe(&f.home).exists());
        assert!(out.contains("left in place"), "{out}");
        assert_ne!(code, 0, "{out}");
    }

    #[test]
    fn uninstall_goes_on_past_an_unreadable_hooks_json() {
        let f = installed(ORIGINAL);
        // A folder where the file should be: it can't be read.
        let agy = marshell_core::agents::antigravity::hooks::hooks_path(&f.user_home);
        std::fs::create_dir_all(&agy).unwrap();
        let (code, out) = run_with(&f, "y\n", false);
        assert!(out.contains("hooks.json"), "{out}");
        assert_eq!(out.matches("Apply these changes?").count(), 1, "{out}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        assert!(agy.is_dir());
        // The bridge may still be what its entries run, so bin stays and the exit says so.
        assert!(marshell_core::paths::bridge_exe(&f.home).exists());
        assert!(out.contains("left in place"), "{out}");
        assert_ne!(code, 0, "{out}");
    }

    #[test]
    fn a_codex_only_install_is_found() {
        // Claude's hooks are gone; only Codex's are left.
        let f2 = installed(ORIGINAL);
        std::fs::write(&f2.settings, ORIGINAL).unwrap();
        let codex = install_codex(&f2, None);
        let (code, out) = run_with(
            &f2, "y
", false,
        );
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("hooks.json"), "{out}");
        assert!(!std::fs::read_to_string(&codex).unwrap().contains("mshl1"));
        assert_eq!(std::fs::read_to_string(&f2.settings).unwrap(), ORIGINAL);
    }

    #[test]
    fn prints_the_diff_and_stops_on_no() {
        let f = installed(ORIGINAL);
        let installed_text = std::fs::read_to_string(&f.settings).unwrap();
        let (code, out) = run_with(&f, "n\n", false);
        assert_eq!(code, 1, "{out}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), installed_text);
        assert!(out.contains("@@"), "the unified diff is missing: {out}");
        assert!(out.contains("marshell-hook"), "{out}");
        assert!(out.contains("Apply these changes? [y/N] "), "{out}");
        assert!(out.contains("Nothing changed."), "{out}");
        assert!(marshell_core::paths::bridge_exe(&f.home).exists());
    }

    #[test]
    fn end_of_input_is_a_no() {
        let f = installed(ORIGINAL);
        let (code, out) = run_with(&f, "", false);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Nothing changed."), "{out}");
    }

    #[test]
    fn yes_flag_applies_and_removes_bin() {
        let f = installed(ORIGINAL);
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 0, "{out}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        assert!(!marshell_core::paths::bin_dir(&f.home).exists(), "{out}");
        assert!(f.home.join("managed.json").exists(), "the rest of the home stays");
        assert!(f.home.join("snapshots").exists(), "the rest of the home stays");
        let service = ConfigService::new(f.home.clone());
        let ids: Vec<String> = service
            .snapshots()
            .list(None)
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        let uninstall_id = ids
            .iter()
            .find(|id| **id != f.install_snapshot)
            .expect("a second snapshot");
        assert!(out.contains(uninstall_id.as_str()), "{out}");
        assert!(!out.contains("Apply these changes?"), "--yes must not ask: {out}");
    }

    #[test]
    fn y_and_yes_in_any_case_apply() {
        for answer in ["y\n", "YES\n", "  Yes \r\n"] {
            let f = installed(ORIGINAL);
            let (code, out) = run_with(&f, answer, false);
            assert_eq!(code, 0, "{answer:?}: {out}");
            assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        }
        let f = installed(ORIGINAL);
        let (code, _) = run_with(&f, "maybe\n", false);
        assert_eq!(code, 1);
    }

    #[test]
    fn nothing_installed_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("marshell-home");
        let user_home = dir.path().join("user");
        let settings = settings_path(&no_env, &user_home);
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(&settings, ORIGINAL).unwrap();
        let mut out = Vec::new();
        let code = run(
            &home,
            &user_home,
            &no_env,
            &no_wsl(),
            &mut "".as_bytes(),
            &mut out,
            false,
        );
        let out = String::from_utf8(out).unwrap();
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains(&format!("Marshell has no hooks in {}.", settings.display())),
            "{out}"
        );
        assert_eq!(std::fs::read_to_string(&settings).unwrap(), ORIGINAL);
    }

    #[test]
    fn a_missing_settings_file_is_nothing_installed() {
        let dir = tempfile::tempdir().unwrap();
        let mut out = Vec::new();
        let code = run(
            &dir.path().join("h"),
            &dir.path().join("user"),
            &no_env,
            &no_wsl(),
            &mut "".as_bytes(),
            &mut out,
            false,
        );
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&out));
    }

    #[test]
    fn nothing_installed_still_offers_to_remove_bin() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("marshell-home");
        let bin = marshell_core::paths::bin_dir(&home);
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("marshell-hook.exe"), b"x").unwrap();
        let user_home = dir.path().join("user");
        let mut out = Vec::new();
        let code = run(
            &home,
            &user_home,
            &no_env,
            &no_wsl(),
            &mut "n\n".as_bytes(),
            &mut out,
            false,
        );
        let text = String::from_utf8(out).unwrap();
        assert_eq!(code, 1, "{text}");
        assert!(bin.exists(), "{text}");
        let mut out = Vec::new();
        let code = run(
            &home,
            &user_home,
            &no_env,
            &no_wsl(),
            &mut "".as_bytes(),
            &mut out,
            true,
        );
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&out));
        assert!(!bin.exists());
    }

    #[test]
    fn refused_file_exits_1() {
        let f = installed(ORIGINAL);
        // Not JSON any more: the plan fails closed and nothing changes.
        std::fs::write(&f.settings, "{ \"hooks\": ").unwrap();
        let (code, out) = run_with(&f, "y\n", true);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("does not parse"), "{out}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), "{ \"hooks\": ");
        assert!(marshell_core::paths::bin_dir(&f.home).exists());
    }

    #[test]
    fn a_file_the_service_refuses_exits_1() {
        let f = installed(ORIGINAL);
        // Enterprise-managed settings are never written, even though they hold Marshell's entries.
        let managed = f.settings.with_file_name("managed-settings.json");
        std::fs::copy(&f.settings, &managed).unwrap();
        let before = std::fs::read(&managed).unwrap();
        let path = managed.to_string_lossy().into_owned();
        let env = move |name: &str| (name == "MARSHELL_CLAUDE_SETTINGS").then(|| path.clone());
        let mut out = Vec::new();
        let code = run(
            &f.home,
            &f.user_home,
            &env,
            &no_wsl(),
            &mut "y\n".as_bytes(),
            &mut out,
            true,
        );
        let out = String::from_utf8(out).unwrap();
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("managed-settings.json"), "{out}");
        assert_eq!(std::fs::read(&managed).unwrap(), before);
        assert!(marshell_core::paths::bin_dir(&f.home).exists(), "nothing is removed");
    }

    #[test]
    fn status_line_left_is_reported() {
        let f = installed(ORIGINAL);
        // The person set their own status line after the setup.
        let text = std::fs::read_to_string(&f.settings).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["statusLine"]["command"] = "their-new-line".into();
        std::fs::write(&f.settings, serde_json::to_string_pretty(&value).unwrap()).unwrap();
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("status line"), "{out}");
        assert!(out.contains("left as it is"), "{out}");
        let after = std::fs::read_to_string(&f.settings).unwrap();
        assert!(after.contains("their-new-line"), "{after}");
        assert!(!after.contains("marshell-hook"), "{after}");
    }

    fn put_skill(f: &Fixture, agent: &str, bytes: &[u8]) -> PathBuf {
        let path = marshell_core::agents::skill::skill_path(agent, &no_env, &f.user_home).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn uninstall_removes_skills_marshell_wrote() {
        let f = installed(ORIGINAL);
        let text = marshell_core::agents::skill::SKILL_TEXT.as_bytes();
        let claude = put_skill(&f, "claude", text);
        let codex = put_skill(&f, "codex", text);
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 0, "{out}");
        for path in [&claude, &codex] {
            assert!(out.contains(&*path.to_string_lossy()), "{out}");
            assert!(!path.exists(), "{out}");
            assert!(!path.parent().unwrap().exists(), "{out}");
        }
        assert!(!out.contains("Left "), "{out}");
    }

    #[test]
    fn uninstall_removes_a_skill_when_there_are_no_hooks() {
        let f = installed(ORIGINAL);
        std::fs::write(&f.settings, ORIGINAL).unwrap();
        let path = put_skill(&f, "antigravity", marshell_core::agents::skill::SKILL_TEXT.as_bytes());
        let (code, out) = run_with(
            &f, "y
", false,
        );
        assert_eq!(code, 0, "{out}");
        assert!(!path.exists() && !path.parent().unwrap().exists(), "{out}");
    }

    #[test]
    fn uninstall_removes_qwens_skill() {
        let f = installed(ORIGINAL);
        let path = put_skill(&f, "qwen", marshell_core::agents::skill::SKILL_TEXT.as_bytes());
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains(&*path.to_string_lossy()), "{out}");
        assert!(!path.exists() && !path.parent().unwrap().exists(), "{out}");
    }

    #[test]
    fn an_edited_qwen_skill_is_left() {
        let f = installed(ORIGINAL);
        let path = put_skill(&f, "qwen", b"mine");
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 0, "{out}");
        assert_eq!(std::fs::read(&path).unwrap(), b"mine");
        assert!(
            out.contains(&format!(
                "Left {}: it was edited after Marshell installed it.",
                path.display()
            )),
            "{out}"
        );
    }

    #[test]
    fn uninstall_leaves_an_edited_skill() {
        let f = installed(ORIGINAL);
        let path = put_skill(&f, "codex", b"mine");
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 0, "{out}");
        assert_eq!(std::fs::read(&path).unwrap(), b"mine");
        assert!(
            out.contains(&format!(
                "Left {}: it was edited after Marshell installed it.",
                path.display()
            )),
            "{out}"
        );
    }

    /// An environment that moves Claude Code's skill folder under Marshell's own data folder, which the config
    /// guard refuses.
    fn refusing_env(f: &Fixture) -> impl Fn(&str) -> Option<String> + use<> {
        let dir = f.home.join("claude-config").to_string_lossy().into_owned();
        move |name: &str| (name == "CLAUDE_CONFIG_DIR").then(|| dir.clone())
    }

    #[test]
    fn a_skill_the_guard_refuses_is_left_and_the_rest_goes_on() {
        let f = installed(ORIGINAL);
        let env = refusing_env(&f);
        let refused = marshell_core::agents::skill::skill_path("claude", &env, &f.user_home).unwrap();
        std::fs::create_dir_all(refused.parent().unwrap()).unwrap();
        std::fs::write(&refused, marshell_core::agents::skill::SKILL_TEXT).unwrap();
        let codex = put_skill(&f, "codex", marshell_core::agents::skill::SKILL_TEXT.as_bytes());
        let mut out = Vec::new();
        let code = run(
            &f.home,
            &f.user_home,
            &env,
            &no_wsl(),
            &mut "".as_bytes(),
            &mut out,
            true,
        );
        let out = String::from_utf8(out).unwrap();
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains(&format!(
                "Marshell can't change {}: This is Marshell's own data folder.",
                refused.display()
            )),
            "{out}"
        );
        assert!(refused.exists(), "the refused skill stays: {out}");
        assert!(!codex.exists(), "the other skill goes: {out}");
        let after = std::fs::read_to_string(&f.settings).unwrap();
        assert!(!after.contains("marshell-hook"), "the hooks are removed: {after}");
        assert!(
            !marshell_core::paths::bin_dir(&f.home).exists(),
            "a skill does not point at bin"
        );
    }

    #[test]
    fn an_unreadable_skill_keeps_bin_without_blaming_the_other_hooks() {
        let f = installed(ORIGINAL);
        // A folder named SKILL.md cannot be read as a file.
        let path = marshell_core::agents::skill::skill_path("codex", &no_env, &f.user_home).unwrap();
        std::fs::create_dir_all(&path).unwrap();
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains(&format!("Couldn't read {}", path.display())), "{out}");
        assert!(!out.contains("Codex's, Antigravity's or Qwen Code's"), "{out}");
        assert!(marshell_core::paths::bin_dir(&f.home).exists(), "{out}");
    }

    #[test]
    fn a_skill_over_the_size_limit_is_reported_like_an_unreadable_one() {
        let f = installed(ORIGINAL);
        let path = put_skill(&f, "codex", b"");
        let big = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        big.set_len(marshell_core::config::preview::MAX_CONFIG_BYTES + 1)
            .unwrap();
        drop(big);
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 1, "{out}");
        assert!(
            out.contains(&format!(
                "Couldn't read {}: The file is larger than 4 MiB.",
                path.display()
            )),
            "{out}"
        );
        assert!(path.exists());
        assert!(marshell_core::paths::bin_dir(&f.home).exists(), "{out}");
    }

    #[test]
    fn an_unreadable_codex_hooks_file_still_blames_the_other_hooks() {
        let f = installed(ORIGINAL);
        let (path, _) = broken_codex(&f);
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Codex's, Antigravity's or Qwen Code's"), "{out}");
        assert!(path.exists());
    }

    #[test]
    fn uninstall_without_skills_says_nothing_about_them() {
        let f = installed(ORIGINAL);
        let (code, out) = run_with(&f, "", true);
        assert_eq!(code, 0, "{out}");
        assert!(!out.to_lowercase().contains("skill"), "{out}");
    }

    // ---- WSL distros (#82) ----

    fn no_wsl() -> Wsl<'static> {
        Wsl {
            distros: &|| Vec::new(),
            home: &|_| None,
            pinned: false,
        }
    }

    /// A fake distro home that is not under the user's home, so a missing guard wiring shows as a refusal.
    fn fake_distro_home() -> tempfile::TempDir {
        let target = marshell_core::projects::canonical(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
        tempfile::tempdir_in(target).unwrap()
    }

    const MINE_CLAUDE: &str = "{\n  \"theme\": \"dark\",\n  \"statusLine\": {\"type\": \"command\", \"command\": \"my-line\"},\n  \"hooks\": {\"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"my-stop\"}]}]}\n}\n";
    const MINE_CODEX: &str =
        "{\"hooks\": {\"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"my-stop\"}]}]}}\n";
    const MINE_AGY: &str =
        "{\"mine\": {\"enabled\": true, \"Stop\": [{\"type\": \"command\", \"command\": \"my-stop\"}]}}\n";
    const MINE_QWEN: &str = "{\n  \"theme\": \"dark\",\n  \"ui\": {\"statusLine\": {\"type\": \"command\", \"command\": \"my-qwen-line\"}}\n}\n";

    fn apply_pinned(service: &ConfigService, changes: ChangeSet, at: u64) {
        let preview = service.preview(changes);
        assert!(preview.can_apply, "{preview:?}");
        let request = ApplyRequest {
            changeset_id: preview.changeset_id.clone(),
            expected_hashes: preview
                .files
                .iter()
                .map(|f| ExpectedHash {
                    path: f.path.clone(),
                    hash: f.before_hash.clone(),
                })
                .collect(),
        };
        service.apply(&request, at).unwrap();
    }

    /// The four agents' files in a distro home: the person's own entries, with Marshell's hooks installed beside them
    /// through the real plans and a pinned service, as the Agents dialog does. Returns the four files.
    fn install_in_distro(f: &Fixture, home: &Path) -> [PathBuf; 4] {
        use marshell_core::agents::antigravity::hooks as ah;
        use marshell_core::agents::codex::hooks as ch;
        use marshell_core::agents::qwen::hooks as qh;
        let service = ConfigService::with_distros(f.home.clone(), Arc::new(|| vec!["Ubuntu".to_string()]));
        service.record_distro_home("Ubuntu", home, true);
        let bridge = Bridge::Distro {
            name: "Ubuntu".to_string(),
            path: "/mnt/c/Users/x/.marshell/bin/marshell-hook.exe".to_string(),
        };
        let want = HookPlanRequest {
            status: true,
            statusline: false,
            approve: false,
            skill: false,
            distro: Some("Ubuntu".to_string()),
        };
        let claude = home.join(".claude").join("settings.json");
        let codex = home.join(".codex").join("hooks.json");
        let agy = ah::hooks_path(home);
        let qwen = home.join(".qwen").join("settings.json");
        for (path, text) in [
            (&claude, MINE_CLAUDE),
            (&codex, MINE_CODEX),
            (&agy, MINE_AGY),
            (&qwen, MINE_QWEN),
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let changes = marshell_core::agents::claude::hooks::install_plan(
            &claude,
            home,
            Some(MINE_CLAUDE),
            None,
            None,
            &bridge,
            &want,
        )
        .unwrap();
        apply_pinned(&service, changes, 10);
        let changes = ch::install_plan(&codex, Some(MINE_CODEX), &bridge, &want).unwrap();
        apply_pinned(&service, changes, 11);
        let changes = ah::install_plan(home, Some(MINE_AGY), None, None, None, &bridge, &want).unwrap();
        apply_pinned(&service, changes, 12);
        let changes = qh::install_plan(&qwen, home, Some(MINE_QWEN), None, None, &bridge, &want).unwrap();
        apply_pinned(&service, changes, 13);
        for path in [&claude, &codex, &agy, &qwen] {
            assert!(
                std::fs::read_to_string(path).unwrap().contains("mshl1"),
                "{}",
                path.display()
            );
        }
        [claude, codex, agy, qwen]
    }

    /// Runs with `--yes`, the given listed distros and homes, and a pinned service. The third value is the distros the
    /// home lookup was asked for.
    fn run_in_wsl(f: &Fixture, names: &[&str], homes: &[(&str, &Path)]) -> (i32, String, Vec<String>) {
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        let homes: Vec<(String, PathBuf)> = homes.iter().map(|(n, h)| (n.to_string(), h.to_path_buf())).collect();
        let asked = Mutex::new(Vec::new());
        let list = || names.clone();
        let lookup = |distro: &str| {
            asked.lock().unwrap().push(distro.to_string());
            homes.iter().find(|(n, _)| n == distro).map(|(_, h)| h.clone())
        };
        let wsl = Wsl {
            distros: &list,
            home: &lookup,
            pinned: true,
        };
        let mut out = Vec::new();
        let code = run(&f.home, &f.user_home, &no_env, &wsl, &mut "".as_bytes(), &mut out, true);
        (code, String::from_utf8(out).unwrap(), asked.into_inner().unwrap())
    }

    /// Adds a `managed.json` entry for a file under a distro's share, as a real distro install leaves.
    fn manage_in_distro(f: &Fixture, distro: &str) {
        let file = marshell_core::paths::managed_file(&f.home);
        let mut json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        json["entries"].as_array_mut().unwrap().push(serde_json::json!({
            "path": format!(r"\\wsl.localhost\{distro}\home\u\.claude\settings.json"),
            "pointer": "/hooks/Stop",
            "changeset_id": "c-test",
            "applied_ms": 1,
        }));
        std::fs::write(&file, json.to_string()).unwrap();
    }

    fn bin_exists(f: &Fixture) -> bool {
        marshell_core::paths::bridge_exe(&f.home).exists()
    }

    #[test]
    fn removes_marshells_hooks_inside_a_distro() {
        let f = installed(ORIGINAL);
        let distro = fake_distro_home();
        let files = install_in_distro(&f, distro.path());
        let (code, out, asked) = run_in_wsl(&f, &["Ubuntu"], &[("Ubuntu", distro.path())]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(asked, ["Ubuntu"]);
        assert!(out.contains("Looking in WSL distros: Ubuntu."), "{out}");
        assert!(out.contains("Ubuntu: Marshell's hooks found."), "{out}");
        for file in &files {
            let after = std::fs::read_to_string(file).unwrap();
            assert!(!after.contains("mshl1"), "{}: {after}", file.display());
        }
        let claude = std::fs::read_to_string(&files[0]).unwrap();
        assert!(claude.contains("my-stop") && claude.contains("my-line"), "{claude}");
        assert!(std::fs::read_to_string(&files[1]).unwrap().contains("my-stop"));
        assert!(std::fs::read_to_string(&files[2]).unwrap().contains("my-stop"));
        assert!(std::fs::read_to_string(&files[3]).unwrap().contains("my-qwen-line"));
        // The Windows hooks came out too, and the program folder with them.
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        assert!(!marshell_core::paths::bin_dir(&f.home).exists(), "{out}");
    }

    #[test]
    fn a_distro_that_does_not_answer_is_named() {
        // Not in managed.json: the program folder goes with the Windows hooks.
        let f = installed(ORIGINAL);
        let (code, out, _) = run_in_wsl(&f, &["Ubuntu"], &[]);
        assert_eq!(code, 1, "{out}");
        assert!(
            out.contains("Ubuntu didn't answer. Start it and run marshell uninstall again."),
            "{out}"
        );
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        assert!(!marshell_core::paths::bin_dir(&f.home).exists(), "{out}");

        // Named in managed.json: hooks in it may still point at the bridge, so bin stays.
        let f = installed(ORIGINAL);
        manage_in_distro(&f, "Ubuntu");
        let (code, out, _) = run_in_wsl(&f, &["Ubuntu"], &[]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Ubuntu didn't answer."), "{out}");
        assert!(out.contains("because hooks in Ubuntu may still point at it."), "{out}");
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        assert!(bin_exists(&f), "{out}");
    }

    #[test]
    fn a_distro_file_that_does_not_parse_does_not_block_the_rest() {
        let f = installed(ORIGINAL);
        let distro = fake_distro_home();
        let files = install_in_distro(&f, distro.path());
        let broken = "{ \"hooks\": [oops\n";
        std::fs::write(&files[1], broken).unwrap();
        let (code, out, _) = run_in_wsl(&f, &["Ubuntu"], &[("Ubuntu", distro.path())]);
        assert_eq!(code, 1, "{out}");
        assert!(
            out.contains(&*files[1].to_string_lossy()) && out.contains("does not parse"),
            "{out}"
        );
        assert_eq!(std::fs::read_to_string(&files[1]).unwrap(), broken);
        for file in [&files[0], &files[2], &files[3]] {
            let after = std::fs::read_to_string(file).unwrap();
            assert!(!after.contains("mshl1"), "{}: {after}", file.display());
        }
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
        assert!(bin_exists(&f), "{out}");
        assert!(out.contains("because hooks in Ubuntu may still point at it."), "{out}");
    }

    #[test]
    fn a_refused_distro_file_is_left_out() {
        let f = installed(ORIGINAL);
        let distro = fake_distro_home();
        let files = install_in_distro(&f, distro.path());
        let before: Vec<String> = files.iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
        // Ubuntu is neither listed nor managed, so its home is never looked up and the guard keeps its files closed.
        let (code, out, asked) = run_in_wsl(&f, &[], &[("Ubuntu", distro.path())]);
        assert_eq!(code, 0, "{out}");
        assert!(asked.is_empty(), "{asked:?}");
        for (file, text) in files.iter().zip(&before) {
            assert_eq!(&std::fs::read_to_string(file).unwrap(), text);
        }
        assert_eq!(std::fs::read_to_string(&f.settings).unwrap(), ORIGINAL);
    }

    #[cfg(windows)]
    #[test]
    fn uninstalls_service_admits_a_recorded_distro_home() {
        let dir = tempfile::tempdir().unwrap();
        let home = PathBuf::from(r"\\wsl.localhost\MarshellTestDistro\home\u");
        let file = home.join(".claude").join("settings.json");
        let homes = [("MarshellTestDistro".to_string(), home.clone())];
        let service = uninstall_service(dir.path(), &["MarshellTestDistro".to_string()], &homes, false);
        assert_eq!(service.path_refusal(&file), None);
        let service = uninstall_service(dir.path(), &[], &homes, false);
        assert!(service.path_refusal(&file).is_some());
    }

    #[test]
    fn managed_json_names_a_distro_the_list_missed() {
        let f = installed(ORIGINAL);
        manage_in_distro(&f, "Ubuntu");
        let (_, out, asked) = run_in_wsl(&f, &[], &[]);
        assert_eq!(asked, ["Ubuntu"]);
        assert!(out.contains("Looking in WSL distros: Ubuntu."), "{out}");
    }

    #[test]
    fn a_managed_distro_that_is_no_longer_installed_does_not_keep_bin() {
        // WSL listed its distros and Gone isn't one of them: it was unregistered, and its files went with it.
        let f = installed(ORIGINAL);
        manage_in_distro(&f, "Gone");
        let ubuntu = fake_distro_home();
        let (code, out, asked) = run_in_wsl(&f, &["Ubuntu"], &[("Ubuntu", ubuntu.path())]);
        assert_eq!(code, 0, "{out}");
        assert!(asked.contains(&"Gone".to_string()), "{asked:?}");
        assert!(
            out.contains("Gone isn't installed in WSL any more: nothing to remove there."),
            "{out}"
        );
        assert!(!out.contains("Gone didn't answer"), "{out}");
        assert!(!marshell_core::paths::bin_dir(&f.home).exists(), "{out}");

        // An empty list may be WSL failing to answer, not a distro gone: the managed one still keeps bin.
        let f = installed(ORIGINAL);
        manage_in_distro(&f, "Gone");
        let (code, out, _) = run_in_wsl(&f, &[], &[]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Gone didn't answer."), "{out}");
        assert!(bin_exists(&f), "{out}");
    }

    #[test]
    fn no_wsl_says_nothing() {
        let f = installed(ORIGINAL);
        let (code, out, asked) = run_in_wsl(&f, &[], &[]);
        assert_eq!(code, 0, "{out}");
        assert!(asked.is_empty());
        assert!(!out.contains("WSL") && !out.contains("Ubuntu"), "{out}");
    }
}
