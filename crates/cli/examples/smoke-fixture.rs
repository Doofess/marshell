//! `smoke-fixture <marshell home>`: puts Marshell's hooks into Claude Code's and Codex's files, the way the app does
//! (the app's own plans, each previewed and applied through the config service), so the release smoke test
//! (`scripts/release/smoke.ps1`) has something for the uninstaller to take out.
//!
//! The files are found as the CLI finds them: from the process environment (`MARSHELL_CLAUDE_SETTINGS`, `CODEX_HOME`)
//! and the user's home folder. Both variables must be set, so a run can never reach the real `~/.claude` or
//! `~/.codex`; without them it changes nothing and exits 1. Claude Code's `settings.json` must exist already. Codex's
//! `hooks.json` may be missing, but its folder must exist. It also makes one git sandbox of the Changes panel in
//! the home (and the folder `smoke-checkout` it belongs to). Nothing else is changed. Exit 0 when both were changed,
//! 1 with the reason on stderr.

use marshell_core::agents::claude::hooks as claude;
use marshell_core::agents::codex::hooks as codex;
use marshell_core::agents::common::Bridge;
use marshell_core::config::service::ConfigService;
use marshell_core::gitbox::sandbox_dir;
use marshell_core::gitbox::sweep::OWNER_FILE;
use marshell_core::paths;
use marshell_protocol::agents::HookPlanRequest;
use marshell_protocol::config::{ApplyRequest, ChangeSet, ExpectedHash};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn user_home() -> Result<PathBuf, String> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("{var} is not set."))
}

/// The text of `path`, or `None` when the file is not there.
fn read_text(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| format!("{} is not UTF-8 text.", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Couldn't read {}: {e}", path.display())),
    }
}

/// Previews `changes` and applies them, the way the app does.
fn apply(service: &ConfigService, changes: ChangeSet, now_ms: u64) -> Result<(), String> {
    let preview = service.preview(changes);
    if !preview.can_apply {
        return Err(format!("The config service refused the change: {preview:?}"));
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
    service
        .apply(&request, now_ms)
        .map(|_| ())
        .map_err(|e| format!("Applying the change failed: {e:?}"))
}

fn run(home: &Path) -> Result<(), String> {
    for name in ["MARSHELL_CLAUDE_SETTINGS", "CODEX_HOME"] {
        if env(name).is_none_or(|v| v.trim().is_empty()) {
            return Err(format!(
                "{name} is not set, so the real config would be changed. Nothing was changed."
            ));
        }
    }
    let user_home = user_home()?;
    let bridge = Bridge::local(home);
    let service = ConfigService::new(home.to_path_buf());

    // Claude Code: hooks and the status line.
    let settings = claude::settings_path(&env, &user_home);
    let text = read_text(&settings)?.ok_or_else(|| format!("{} does not exist.", settings.display()))?;
    let chain = paths::statusline_chain(home, "claude");
    let want = HookPlanRequest {
        status: true,
        statusline: true,
        approve: false,
        skill: false,
        distro: None,
    };
    let changes = claude::install_plan(&settings, &user_home, Some(&text), None, Some(&chain), &bridge, &want)
        .map_err(|e| format!("Planning Claude Code's hooks failed: {e}"))?;
    apply(&service, changes, 1)?;
    println!("changed {}", settings.display());

    // Codex: hooks only. The folder must exist; the file may not.
    let hooks = codex::hooks_path(&env, &user_home);
    let folder = hooks.parent().ok_or("Codex's hooks.json has no folder.")?;
    if !folder.is_dir() {
        return Err(format!("{} does not exist.", folder.display()));
    }
    let text = read_text(&hooks)?;
    let want = HookPlanRequest {
        status: true,
        statusline: false,
        approve: false,
        skill: false,
        distro: None,
    };
    let changes = codex::install_plan(&hooks, text.as_deref(), &bridge, &want)
        .map_err(|e| format!("Planning Codex's hooks failed: {e}"))?;
    apply(&service, changes, 2)?;
    println!("changed {}", hooks.display());

    // One git sandbox of the Changes panel, for the uninstall to remove. Its checkout must exist: the app starts
    // once more before the uninstall, and its sweep removes a sandbox whose checkout is gone.
    let checkout = home.join("smoke-checkout");
    std::fs::create_dir_all(&checkout).map_err(|e| format!("Couldn't make {}: {e}", checkout.display()))?;
    let dir = sandbox_dir(home, &checkout);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make {}: {e}", dir.display()))?;
    let owner = std::path::absolute(&checkout).map_err(|e| format!("Couldn't resolve {}: {e}", checkout.display()))?;
    let owner = owner
        .to_str()
        .ok_or_else(|| format!("{} is not UTF-8 text.", checkout.display()))?;
    for (name, text) in [("HEAD", "ref: refs/heads/none\n"), (OWNER_FILE, owner)] {
        std::fs::write(dir.join(name), text)
            .map_err(|e| format!("Couldn't write {}: {e}", dir.join(name).display()))?;
    }
    println!("made {}", dir.display());
    Ok(())
}

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(home), None) = (args.next(), args.next()) else {
        eprintln!("usage: smoke-fixture <marshell home>");
        return ExitCode::from(1);
    };
    match run(Path::new(&home)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}
