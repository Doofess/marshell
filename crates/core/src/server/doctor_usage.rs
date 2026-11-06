//! The usage scan every check runs: each Windows CLI's history is read from where the last scan stopped, what it
//! showed is merged into the Doctor's usage store, and the store is kept in `<home>/doctor/usage.json`. The history
//! formats live in `agents/<cli>/usage.rs` and the rules in `doctor::usage`; this is the server side that joins them.

use super::AppState;
use crate::doctor::ext::{ExtRoots, Inventory, item_key};
use crate::doctor::usage::{UsageStore, absorb, merge};
use crate::paths;
use crate::store;
use std::sync::Mutex;

/// One writer of `usage.json` at a time, so a slower run can't put an older value over a newer one. It guards the
/// file only; the state's lock is never held while it writes.
static WRITING: Mutex<()> = Mutex::new(());

/// The store as it was kept: in memory, else `usage.json`. A file that can't be read, or is from a newer Marshell, gives
/// an empty store, and a newer one is never written over (`store::write` refuses).
fn load(st: &AppState) -> UsageStore {
    let kept = st.doctor().usage.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(kept) = kept {
        return kept;
    }
    let loaded = match store::read::<UsageStore>(&paths::doctor_usage(&st.0.home)) {
        Ok(found) => found.unwrap_or_default(),
        Err(e) => {
            tracing::warn!("the doctor's usage.json can't be read, so its history starts again: {e:#}");
            UsageStore::default()
        }
    };
    // Another run may have loaded it meanwhile: the first value kept stands.
    let mut slot = st.doctor().usage.lock().unwrap_or_else(|e| e.into_inner());
    slot.get_or_insert(loaded).clone()
}

/// Reads what is new in every Windows CLI's history, keeps it, and answers the store. With no `roots` nothing is
/// scanned or written. Reads and writes files, so callers go through `spawn_blocking`. The state's lock is taken three
/// times, each only to clone or fold a value, and never while a file is read or written.
pub(crate) fn scan_all(st: &AppState, inv: &[Inventory], roots: Option<&ExtRoots>, now_ms: u64) -> UsageStore {
    let Some(roots) = roots else {
        return UsageStore::default();
    };
    // 1. The snapshot to scan from (lock, clone, drop; the file is read outside the lock).
    let mut found = load(st);
    // 2. Each found Windows CLI's new history, from where the snapshot left off.
    let windows: Vec<&Inventory> = inv.iter().filter(|i| i.distro.is_none() && i.found).collect();
    for i in &windows {
        let before = found.agents.get(i.agent).cloned().unwrap_or_default();
        let scan = match i.agent {
            "claude" => crate::agents::claude::usage::scan(roots, &before),
            "codex" => crate::agents::codex::usage::scan(roots, &before),
            "antigravity" => crate::agents::antigravity::usage::scan(roots, &before),
            "qwen" => crate::agents::qwen::usage::scan(roots, &before),
            _ => continue,
        };
        merge(found.agents.entry(i.agent.to_string()).or_default(), scan, now_ms);
    }
    // 3. When each item was first seen.
    for i in &windows {
        let keys = i
            .servers
            .iter()
            .map(|s| item_key(&s.file, &s.origin, &s.name))
            .chain(i.skills.iter().map(|s| item_key(&s.file, &s.origin, &s.name)));
        for key in keys {
            found.first_sight.entry(format!("{}:{key}", i.agent)).or_insert(now_ms);
        }
    }
    // 4. Fold into what is kept now, which a run that overlapped may have changed (lock, fold, clone, drop).
    {
        let mut kept = st.doctor().usage.lock().unwrap_or_else(|e| e.into_inner());
        absorb(kept.get_or_insert_with(UsageStore::default), &found);
    }
    // 5. One writer at a time, and each writes the newest value, so a slower run never puts back an older one.
    let _writing = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let latest = st
        .doctor()
        .usage
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default();
    if let Err(e) = store::write(&paths::doctor_usage(&st.0.home), &latest) {
        tracing::warn!("the doctor could not keep usage.json: {e:#}");
    }
    latest
}

#[cfg(test)]
pub(in crate::server) mod tests {
    use super::super::doctor::tests::pinned;
    use super::super::doctor::{facts_of, now_ms};
    use super::*;
    use crate::agents::claude::usage::scan as claude_scan;
    use crate::doctor::usage::{Cursor, new_lines};
    use std::path::{Path, PathBuf};

    pub(in crate::server) const DAY: u64 = 24 * 60 * 60 * 1000;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// `2026-11-01T10:00:00.000Z` for an epoch time in ms (days to a civil date, after Howard Hinnant).
    pub(in crate::server) fn iso(ms: u64) -> String {
        let secs = ms / 1000;
        let (days, rem) = ((secs / 86400) as i64, secs % 86400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        )
    }

    pub(in crate::server) fn quiet_line(at_ms: u64) -> String {
        format!(
            r#"{{"type":"user","timestamp":"{}","cwd":"c:/dev/x","message":{{"content":"hi"}}}}"#,
            iso(at_ms)
        )
    }

    pub(in crate::server) fn call_line(at_ms: u64, tool: &str) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{}","cwd":"c:/dev/x","message":{{"content":[{{"type":"tool_use","name":"{tool}","input":{{}}}}]}}}}"#,
            iso(at_ms)
        )
    }

    pub(in crate::server) fn transcript(ext: &Path, name: &str) -> PathBuf {
        ext.join(".claude").join("projects").join("p1").join(name)
    }

    pub(in crate::server) fn put_lines(path: &Path, lines: &[String]) {
        write(path, &lines.iter().map(|l| format!("{l}\n")).collect::<String>());
    }

    fn ext_state(root: &Path) -> (AppState, PathBuf) {
        let ext = crate::projects::canonical(root).join("ext-home");
        std::fs::create_dir_all(&ext).unwrap();
        (pinned(root).with_ext_home(ext.clone()), ext)
    }

    /// A program file that exists, so a server whose command is its path reads as found.
    pub(in crate::server) fn program(root: &Path) -> PathBuf {
        let p = crate::projects::canonical(root).join("bin").join("srv.exe");
        write(&p, "");
        p
    }

    /// A run of the checks, ended.
    pub(in crate::server) async fn run_and_wait(st: &AppState) {
        let mut rx = st.doctor().summary.subscribe();
        super::super::doctor::request_run(st);
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            rx.wait_for(|s| !s.running && !s.measuring && !s.cli_running),
        )
        .await
        .expect("the run ends")
        .unwrap();
    }

    pub(in crate::server) async fn plan_of(st: &AppState, ids: &[String]) -> marshell_protocol::doctor::DoctorFixPlan {
        let resp = super::super::doctor::fix(
            axum::extract::State(st.clone()),
            axum::Json(marshell_protocol::doctor::DoctorFixRequest { item_ids: ids.to_vec() }),
        )
        .await;
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn usage_file(st: &AppState) -> PathBuf {
        paths::doctor_usage(&st.0.home)
    }

    fn roots_of(st: &AppState) -> ExtRoots {
        super::super::doctor_ext::windows_roots(st).expect("a fake home")
    }

    /// Writes `usage.json` with every server and skill of Claude Code first seen `days` days ago, and drops what the
    /// state kept in memory, so the next scan loads the file.
    pub(in crate::server) fn seed_first_sight(st: &AppState, days: u64) {
        let mut store = UsageStore::default();
        for inv in facts_of(st).ext.iter().filter(|i| i.found && i.distro.is_none()) {
            for s in &inv.servers {
                store.first_sight.insert(
                    format!("{}:{}", inv.agent, item_key(&s.file, &s.origin, &s.name)),
                    now_ms() - days * DAY,
                );
            }
            for s in &inv.skills {
                store.first_sight.insert(
                    format!("{}:{}", inv.agent, item_key(&s.file, &s.origin, &s.name)),
                    now_ms() - days * DAY,
                );
            }
        }
        std::fs::remove_file(usage_file(st)).ok();
        store::write(&usage_file(st), &store).unwrap();
        *st.doctor().usage.lock().unwrap() = None;
    }

    fn on_disk(st: &AppState) -> UsageStore {
        store::read::<UsageStore>(&usage_file(st)).unwrap().expect("usage.json")
    }

    #[test]
    fn first_run_writes_usage_json_and_second_reads_only_new_lines() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        write(
            &ext.join(".claude").join("skills").join("mine").join("SKILL.md"),
            "---\nname: mine\ndescription: d\n---\nbody\n",
        );
        let file = transcript(&ext, "s1.jsonl");
        put_lines(
            &file,
            &[
                quiet_line(now_ms() - 2 * DAY),
                call_line(now_ms() - DAY, "mcp__gh__list"),
            ],
        );
        assert!(!usage_file(&st).exists());

        let first = facts_of(&st).usage;
        assert!(usage_file(&st).exists(), "usage.json is written");
        let claude = &first.agents["claude"];
        let cursor: Cursor = claude.cursors[&file.display().to_string()].clone();
        let len = std::fs::metadata(&file).unwrap().len();
        assert_eq!(cursor.read_to, len);
        assert!(claude.used.contains_key("tool:mcp__gh__list"), "{claude:?}");
        assert!(first.first_sight.keys().any(|k| k.starts_with("claude:")), "{first:?}");
        assert_eq!(on_disk(&st), first);

        // The second run reads nothing of that file again.
        let (lines, again) = new_lines(&file, Some(&cursor)).unwrap();
        assert!(lines.is_empty());
        let scan = claude_scan(&roots_of(&st), claude);
        assert_eq!(scan.cursors[&file.display().to_string()].read_to, len);
        assert!(scan.events.is_empty(), "{:?}", scan.events);
        assert_eq!(again.read_to, len);
        let second = facts_of(&st).usage;
        assert_eq!(second.agents["claude"].cursors, claude.cursors);
        assert_eq!(second.agents["claude"].used, claude.used);
    }

    // Found in the 7c2 final review: an unreadable scan leaves `scanned_ms` as it was, so `absorb` saw a tie and
    // dropped `unreadable`; the "can't be read" item never showed and old coverage kept judging.
    #[test]
    fn history_that_becomes_unreadable_is_kept_as_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        write(
            &ext.join(".claude").join("skills").join("mine").join("SKILL.md"),
            "---\nname: mine\ndescription: d\n---\nbody\n",
        );
        put_lines(&transcript(&ext, "s1.jsonl"), &[quiet_line(now_ms() - 2 * DAY)]);
        assert_eq!(facts_of(&st).usage.agents["claude"].unreadable, None);

        // The history folder can no longer be listed: a file stands where it was.
        let projects = ext.join(".claude").join("projects");
        std::fs::remove_dir_all(&projects).unwrap();
        write(&projects, "");
        let usage = facts_of(&st).usage;
        assert!(
            usage.agents["claude"].unreadable.is_some(),
            "{:?}",
            usage.agents["claude"]
        );
        assert!(on_disk(&st).agents["claude"].unreadable.is_some());

        // Readable again: the next scan clears it.
        std::fs::remove_file(&projects).unwrap();
        put_lines(&transcript(&ext, "s1.jsonl"), &[quiet_line(now_ms() - 2 * DAY)]);
        assert_eq!(facts_of(&st).usage.agents["claude"].unreadable, None);
    }

    #[test]
    fn corrupt_usage_json_starts_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        write(&usage_file(&st), "{ this is not json");
        put_lines(
            &transcript(&ext, "s1.jsonl"),
            &[call_line(now_ms() - DAY, "mcp__gh__list")],
        );
        let got = facts_of(&st).usage;
        assert!(got.agents["claude"].used.contains_key("tool:mcp__gh__list"));
        // The run goes on to the report, and the file is whole again.
        assert_eq!(on_disk(&st), got);
    }

    #[test]
    fn newer_usage_json_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let newer = br#"{"version": 99, "agents": {"claude": {"scanned_ms": 5}}}"#;
        write(&usage_file(&st), std::str::from_utf8(newer).unwrap());
        put_lines(
            &transcript(&ext, "s1.jsonl"),
            &[call_line(now_ms() - DAY, "mcp__gh__list")],
        );
        let got = facts_of(&st).usage;
        // A default store in memory, scanned from what is on disk.
        assert!(got.agents["claude"].scanned_ms.is_some());
        assert!(got.agents["claude"].used.contains_key("tool:mcp__gh__list"));
        assert_eq!(std::fs::read(usage_file(&st)).unwrap(), newer);
        assert!(!usage_file(&st).with_extension("json.bak").exists());
    }

    #[tokio::test]
    async fn unused_item_reaches_the_report_and_fix() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        let project = crate::projects::canonical(dir.path()).join("project");
        let exe = program(dir.path());
        write(
            &project.join(".mcp.json"),
            &serde_json::json!({"mcpServers": {"srv": {"command": exe.to_string_lossy()}}}).to_string(),
        );
        write(
            &project.join(".claude").join("settings.local.json"),
            r#"{"enabledMcpjsonServers":["srv"]}"#,
        );
        // Approval counts only in a folder whose trust dialog was accepted.
        write(
            &ext.join(".claude.json"),
            &serde_json::json!({"projects": {project.to_string_lossy().replace('\\', "/"): {"hasTrustDialogAccepted": true}}})
                .to_string(),
        );
        put_lines(&transcript(&ext, "s1.jsonl"), &[quiet_line(now_ms() - 40 * DAY)]);
        seed_first_sight(&st, 40);

        run_and_wait(&st).await;
        let report = st.doctor().report.lock().unwrap().clone().expect("a report");
        let item = report
            .items
            .iter()
            .find(|i| i.id.starts_with("mcpunused.claude."))
            .unwrap_or_else(|| panic!("{:#?}", report.items));
        assert_eq!(item.group, marshell_protocol::doctor::DoctorGroup::Waste);
        assert!(item.fixable && item.unchecked, "{item:?}");

        let plan = plan_of(&st, std::slice::from_ref(&item.id)).await;
        assert_eq!(plan.preview.files.len(), 1, "{:?}", plan.preview);
        let doc = &plan.preview.files[0];
        assert!(doc.path.ends_with("settings.local.json"), "{doc:?}");
        assert!(
            doc.diff.contains("disabledMcpjsonServers") && doc.diff.contains("srv"),
            "{}",
            doc.diff
        );
    }

    #[test]
    fn two_runs_at_once_lose_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (st, ext) = ext_state(dir.path());
        put_lines(
            &transcript(&ext, "a.jsonl"),
            &[call_line(now_ms() - DAY, "mcp__one__t")],
        );
        put_lines(
            &transcript(&ext, "b.jsonl"),
            &[call_line(now_ms() - DAY, "mcp__two__t")],
        );
        let inv = facts_of(&st).ext;
        let roots = roots_of(&st);
        // The first call's stored value must not hide the second's.
        *st.doctor().usage.lock().unwrap() = None;
        std::fs::remove_file(usage_file(&st)).ok();
        let barrier = std::sync::Barrier::new(2);
        let results: Vec<UsageStore> = std::thread::scope(|s| {
            let run = || {
                barrier.wait();
                scan_all(&st, &inv, Some(&roots), now_ms())
            };
            let a = s.spawn(run);
            let b = s.spawn(run);
            vec![a.join().unwrap(), b.join().unwrap()]
        });
        for r in results.iter().chain([&on_disk(&st)]) {
            let used = &r.agents["claude"].used;
            assert!(
                used.contains_key("tool:mcp__one__t") && used.contains_key("tool:mcp__two__t"),
                "{used:?}"
            );
        }
        let kept = st.doctor().usage.lock().unwrap().clone().unwrap();
        assert!(kept.agents["claude"].used.contains_key("tool:mcp__one__t"));
        assert!(kept.agents["claude"].used.contains_key("tool:mcp__two__t"));
    }
}
