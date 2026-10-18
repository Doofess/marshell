//! `marshell uninstall`: shows what taking Marshell out of Claude Code would change, asks, and does it
//! through the config service (so it is hashed, snapshotted and undoable), then removes `<home>/bin`.
//!
//! Exit codes: 0 done or nothing to do, 1 refused, failed or declined.

use marshell_core::agents::claude::hooks::{remove_plan, settings_path};
use marshell_core::config::service::{ApplyError, ConfigService};
use marshell_core::paths;
use marshell_protocol::config::{ApplyRequest, ExpectedHash, FileStatus};
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Removes the files in `<home>/bin` one by one, then the folder if it is empty. A file that stays (the
/// running `marshell` itself, on Windows) is reported and is not a failure.
fn remove_bin(home: &Path, out: &mut dyn Write) {
    let bin = paths::bin_dir(home);
    let Ok(entries) = std::fs::read_dir(&bin) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let removed = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if removed.is_err() {
            let _ = writeln!(
                out,
                "Couldn't remove {}; delete it once this window is closed.",
                path.display()
            );
        }
    }
    let _ = std::fs::remove_dir(&bin);
}

pub fn run(
    home: &Path,
    user_home: &Path,
    env: &dyn Fn(&str) -> Option<String>,
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
    let plan = match remove_plan(&settings, user_home, text.as_deref(), chain_text.as_deref(), home) {
        Ok(plan) => plan,
        Err(e) => {
            let _ = writeln!(out, "{e}");
            return 1;
        }
    };
    let bin = paths::bin_dir(home);
    let has_bin = bin.exists();
    let left_note = "Your status line was changed since the setup and was left as it is.";

    if plan.changes.edits.is_empty() {
        let _ = writeln!(out, "Marshell has no hooks in {}.", settings.display());
        if plan.statusline_left {
            let _ = writeln!(out, "{left_note}");
        }
        if !has_bin {
            return 0;
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
        return 0;
    }

    let service = ConfigService::new(home.to_path_buf());
    let preview = service.preview(plan.changes);
    let refused: Vec<_> = preview
        .files
        .iter()
        .filter_map(|f| match &f.status {
            FileStatus::Refused { detail, .. } => Some((f.path.as_str(), detail.as_str())),
            _ => None,
        })
        .collect();
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
    remove_bin(home, out);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use marshell_core::agents::claude::hooks::{install_plan, settings_path};
    use marshell_core::config::service::ConfigService;
    use marshell_protocol::agents::HookPlanRequest;
    use marshell_protocol::config::{ApplyRequest, ExpectedHash};
    use std::path::PathBuf;

    const ORIGINAL: &str =
        "{\n  \"theme\": \"dark\",\n  \"statusLine\": {\"type\": \"command\", \"command\": \"my-line\"}\n}\n";

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
        };
        let changes = install_plan(&settings, &user_home, Some(original), None, &home, &want).unwrap();
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
        let code = run(&f.home, &f.user_home, &no_env, &mut input.as_bytes(), &mut out, yes);
        (code, String::from_utf8(out).unwrap())
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
        let code = run(&home, &user_home, &no_env, &mut "".as_bytes(), &mut out, false);
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
        let code = run(&home, &user_home, &no_env, &mut "n\n".as_bytes(), &mut out, false);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(code, 1, "{text}");
        assert!(bin.exists(), "{text}");
        let mut out = Vec::new();
        let code = run(&home, &user_home, &no_env, &mut "".as_bytes(), &mut out, true);
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
        let code = run(&f.home, &f.user_home, &env, &mut "y\n".as_bytes(), &mut out, true);
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
}
