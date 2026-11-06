//! "Measure MCP servers": starts each approved local server once, lists its tools and keeps the result for the report.
//! The server side of `doctor::measure` (the cost line and the failure items) and `doctor::probe` (the start itself).
//! Only the server reads the clock and the environment.

use super::AppState;
use super::doctor::{facts_of, now_ms, request_run};
use crate::doctor::ext::{CommandState, Expand, Inherit, Launch, Origin, Transport, item_key};
use crate::doctor::measure::{Measured, Measurements, fingerprint};
use crate::doctor::probe::{Failure, Listed, Ready};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Starts one server and lists its tools. A test swaps it (`AppState::with_probe`), so none starts a real server.
pub(crate) type Prober = fn(&Ready) -> Result<Listed, Failure>;

/// How many servers are started at once.
const AT_ONCE: usize = 4;

/// Starts "Measure MCP servers" unless one is going, in which case the request joins that run: nothing starts twice.
/// The summary reads `measuring` as soon as this returns. When the run ends, the results are stored, a run of the
/// checks is asked for, and only then does `measuring` clear, so the page that reloads on it finds the new items. Needs
/// a Tokio runtime.
pub(crate) fn request_measure(st: &AppState) {
    let doctor = st.doctor();
    {
        let mut run = doctor.measure.lock().unwrap_or_else(|e| e.into_inner());
        if run.running {
            return;
        }
        run.running = true;
        doctor.summary.send_modify(|s| s.measuring = true);
    }
    let st = st.clone();
    tokio::spawn(async move {
        let reader = st.clone();
        let found = tokio::task::spawn_blocking(move || measure_all(&reader)).await;
        let doctor = st.doctor();
        match found {
            Ok(measured) => {
                doctor.measure.lock().unwrap_or_else(|e| e.into_inner()).results = Some(measured);
            }
            // The old results stay, and the flag still clears below.
            Err(e) => tracing::warn!("measuring the MCP servers failed: {e}"),
        }
        // `running` goes true here, before `measuring` goes false.
        request_run(&st);
        let mut run = doctor.measure.lock().unwrap_or_else(|e| e.into_inner());
        run.running = false;
        doctor.summary.send_modify(|s| s.measuring = false);
    });
}

/// Marshell's own environment, with `PATH` replaced by the one a tab will get when that is known.
fn base_env(st: &AppState) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v.to_string_lossy().into_owned()))
        .collect();
    let finder = st.agent_finder(&[]);
    if let Some(path) = finder.path_in_use() {
        put(&mut env, "PATH", &path.to_string_lossy());
    }
    env
}

/// Sets `name` to `value`, replacing a variable of the same name in any case (Windows' names ignore case).
fn put(env: &mut Vec<(String, String)>, name: &str, value: &str) {
    match env.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
        Some(entry) => *entry = (name.to_string(), value.to_string()),
        None => env.push((name.to_string(), value.to_string())),
    }
}

fn lookup<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_name(s: &str) -> bool {
    s.chars().next().is_some_and(is_name_start) && s.chars().all(is_name_char)
}

/// `text` with its variables expanded as the CLI does. A name that isn't in `env` stays as written.
fn expand(text: &str, how: Expand, env: &[(String, String)]) -> String {
    if how == Expand::None || !text.contains('$') {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '$' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // `${NAME}`, and `${NAME:-default}` for Braces.
        if chars.get(i + 1) == Some(&'{')
            && let Some(len) = chars[i + 2..].iter().position(|c| *c == '}')
        {
            let inner: String = chars[i + 2..i + 2 + len].iter().collect();
            let (name, default) = match (how, inner.split_once(":-")) {
                (Expand::Braces, Some((n, d))) => (n, Some(d)),
                _ => (inner.as_str(), None),
            };
            if is_name(name) {
                match (lookup(env, name), default) {
                    (Some(v), _) => out.push_str(v),
                    (None, Some(d)) => out.push_str(d),
                    (None, None) => out.extend(&chars[i..i + 3 + len]),
                }
                i += 3 + len;
                continue;
            }
        }
        // `$NAME`, Dollar only.
        if how == Expand::Dollar && chars.get(i + 1).copied().is_some_and(is_name_start) {
            let len = chars[i + 1..].iter().take_while(|c| is_name_char(**c)).count();
            let name: String = chars[i + 1..i + 1 + len].iter().collect();
            match lookup(env, &name) {
                Some(v) => out.push_str(v),
                None => out.extend(&chars[i..i + 1 + len]),
            }
            i += 1 + len;
            continue;
        }
        out.push('$');
        i += 1;
    }
    out
}

/// A launch turned into what the probe starts: the environment built and expanded, the program found. The server's
/// environment is `base` filtered by `inherit`, then `set`, then the entry's own `env`; variables expand against the
/// environment built so far. A command that can't be found gives `CantStart`.
pub(crate) fn ready_of(launch: &Launch, base: &[(String, String)]) -> Result<Ready, Failure> {
    let mut env: Vec<(String, String)> = match &launch.inherit {
        Inherit::All => base.to_vec(),
        Inherit::Only(names) => base
            .iter()
            .filter(|(k, _)| names.iter().any(|n| n.eq_ignore_ascii_case(k)))
            .cloned()
            .collect(),
    };
    for (k, v) in &launch.set {
        put(&mut env, k, v);
    }
    // The entry's own values are placed as written first, so one can name another; then each is expanded.
    for (k, v) in &launch.env {
        put(&mut env, k, v);
    }
    for (k, v) in &launch.env {
        let expanded = expand(v, launch.expand, &env);
        put(&mut env, k, &expanded);
    }
    let command = expand(&launch.command, launch.expand, &env);
    let args: Vec<String> = launch.args.iter().map(|a| expand(a, launch.expand, &env)).collect();
    let cwd = launch
        .cwd
        .as_ref()
        .map(|c| PathBuf::from(expand(&c.to_string_lossy(), launch.expand, &env)));
    let program = if command.contains(['\\', '/']) {
        let path = PathBuf::from(&command);
        match (&cwd, path.is_absolute()) {
            (Some(cwd), false) => cwd.join(path),
            _ => path,
        }
    } else {
        let path = lookup(&env, "PATH").ok_or(Failure::CantStart)?;
        crate::profiles::detect::command_path_in(std::ffi::OsStr::new(path), &command).ok_or(Failure::CantStart)?
    };
    Ok(Ready {
        program,
        args,
        env,
        cwd,
        timeout: Duration::from_secs(launch.timeout_secs),
    })
}

/// One server to start.
struct Job<'a> {
    agent: &'static str,
    key: String,
    launch: &'a Launch,
}

/// Starts each approved local server of the Windows inventories once, at most `AT_ONCE` at a time, and keeps what each
/// answered. Runs processes, so callers go through `spawn_blocking`.
fn measure_all(st: &AppState) -> Measurements {
    let gathered = facts_of(st);
    let mut jobs: Vec<Job> = Vec::new();
    for inv in gathered.ext.iter().filter(|inv| inv.distro.is_none()) {
        for s in &inv.servers {
            let startable = matches!(
                &s.transport,
                Transport::Stdio {
                    state: CommandState::Found | CommandState::NotChecked,
                    ..
                }
            );
            if s.off || s.origin == Origin::Bundled || !startable {
                continue;
            }
            let Ok(launch) = &s.launch else { continue };
            let key = item_key(&s.file, &s.origin, &s.name);
            // One start per server: two entries that name the same file, folder and name are one.
            if jobs.iter().any(|j| j.agent == inv.agent && j.key == key) {
                continue;
            }
            jobs.push(Job {
                agent: inv.agent,
                key,
                launch,
            });
        }
    }

    let base = base_env(st);
    let probe = st.0.probe;
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Measured)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..AT_ONCE.min(jobs.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(job) = jobs.get(i) else { return };
                    let outcome = ready_of(job.launch, &base).and_then(|ready| probe(&ready));
                    let measured = Measured {
                        agent: job.agent,
                        key: job.key.clone(),
                        fingerprint: fingerprint(job.launch),
                        outcome,
                    };
                    done.lock().unwrap_or_else(|e| e.into_inner()).push((i, measured));
                }
            });
        }
    });
    let mut done = done.into_inner().unwrap_or_else(|e| e.into_inner());
    done.sort_by_key(|(i, _)| *i);
    Measurements {
        at_ms: now_ms(),
        servers: done.into_iter().map(|(_, m)| m).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::doctor::tests::{pinned, pinned_with};
    use super::*;
    use axum::Json;
    use axum::extract::State;
    use marshell_protocol::doctor::{DoctorFixPlan, DoctorFixRequest, DoctorGroup, DoctorReport};
    use std::path::Path;
    use std::sync::atomic::AtomicBool;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn env_of(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn launch(command: &str) -> Launch {
        Launch {
            command: command.into(),
            args: vec![],
            env: vec![],
            inherit: Inherit::All,
            set: vec![],
            cwd: None,
            expand: Expand::None,
            timeout_secs: 7,
        }
    }

    fn get<'a>(ready: &'a Ready, name: &str) -> Option<&'a str> {
        ready
            .env
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn ready_of_builds_the_env_and_expands() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        let tool = bin.join(if cfg!(windows) { "tool.exe" } else { "tool" });
        write(&tool, "");
        let path = bin.to_string_lossy().into_owned();
        let base = env_of(&[("PATH", &path), ("SECRET", "s3"), ("HOME_X", "/h")]);

        // Only the named variables are inherited; `set` and `env` come on top.
        let mut l = launch("tool");
        l.inherit = Inherit::Only(vec!["path".into()]);
        l.set = env_of(&[("FROM_CLI", "1")]);
        l.env = env_of(&[("MINE", "2")]);
        let ready = ready_of(&l, &base).expect("ready");
        assert_eq!(get(&ready, "SECRET"), None);
        assert_eq!(get(&ready, "HOME_X"), None);
        assert_eq!(get(&ready, "PATH"), Some(path.as_str()));
        assert_eq!(get(&ready, "FROM_CLI"), Some("1"));
        assert_eq!(get(&ready, "MINE"), Some("2"));
        // PATHEXT's spelling of the ending can differ in case.
        assert_eq!(
            ready.program.to_string_lossy().to_lowercase(),
            tool.to_string_lossy().to_lowercase()
        );
        assert_eq!(ready.timeout, Duration::from_secs(7));

        // Braces: `${X:-d}` gives the default, an unknown `${MISSING}` stays as written, a set name expands.
        let mut l = launch("tool");
        l.expand = Expand::Braces;
        l.args = vec![
            "${X:-d}".into(),
            "${MISSING}".into(),
            "${HOME_X}/y".into(),
            "$HOME_X".into(),
        ];
        l.env = env_of(&[("E", "${SECRET}-${X:-z}")]);
        l.cwd = Some(PathBuf::from("${HOME_X}/w"));
        let ready = ready_of(&l, &base).expect("ready");
        assert_eq!(ready.args, ["d", "${MISSING}", "/h/y", "$HOME_X"]);
        assert_eq!(get(&ready, "E"), Some("s3-z"));
        assert_eq!(ready.cwd, Some(PathBuf::from("/h/w")));

        // Dollar: `$HOME_X` and `${HOME_X}` expand, `$MISSING` stays.
        let mut l = launch("tool");
        l.expand = Expand::Dollar;
        l.args = vec![
            "$HOME_X/a".into(),
            "${HOME_X}".into(),
            "$MISSING".into(),
            "${X:-d}".into(),
        ];
        let ready = ready_of(&l, &base).expect("ready");
        assert_eq!(ready.args, ["/h/a", "/h", "$MISSING", "${X:-d}"]);

        // With no expansion nothing changes.
        let mut l = launch("tool");
        l.args = vec!["${HOME_X}".into()];
        assert_eq!(ready_of(&l, &base).unwrap().args, ["${HOME_X}"]);

        // A bare command missing from PATH can't start; a relative path is joined to the cwd.
        assert_eq!(ready_of(&launch("ghost"), &base).unwrap_err(), Failure::CantStart);
        let mut l = launch("sub/run.exe");
        l.cwd = Some(PathBuf::from("/proj"));
        assert_eq!(
            ready_of(&l, &base).unwrap().program,
            PathBuf::from("/proj").join("sub/run.exe")
        );
    }

    // The measure runs. `pinned` finds every CLI at a path that doesn't exist, and every probe is a pinned `fn`.

    fn tool_named(name: &str) -> Listed {
        Listed {
            instructions: String::new(),
            tools: vec![crate::doctor::probe::Tool {
                name: name.into(),
                description: String::new(),
                schema_chars: 0,
                meta_true: vec![],
            }],
        }
    }

    /// The argument that names a server in these tests: each server's `args` is `[name]`.
    fn named(ready: &Ready) -> String {
        ready.args.first().cloned().unwrap_or_default()
    }

    /// A program file that exists, so a server whose command is its path reads as found.
    fn program(root: &Path) -> PathBuf {
        let p = crate::projects::canonical(root).join("bin").join("srv.exe");
        write(&p, "");
        p
    }

    fn entry(program: &Path, name: &str) -> serde_json::Value {
        serde_json::json!({"command": program.to_string_lossy(), "args": [name]})
    }

    fn ext_state(root: &Path) -> (AppState, PathBuf) {
        let ext = crate::projects::canonical(root).join("ext-home");
        std::fs::create_dir_all(&ext).unwrap();
        let st = pinned(root)
            .with_ext_home(ext.clone())
            .with_agent_check(|command| command != "ghost");
        (st, ext)
    }

    async fn wait_idle(st: &AppState) {
        let mut rx = st.doctor().summary.subscribe();
        tokio::time::timeout(
            Duration::from_secs(10),
            rx.wait_for(|s| !s.running && !s.measuring && !s.cli_running),
        )
        .await
        .expect("the runs end")
        .unwrap();
    }

    fn last_report(st: &AppState) -> DoctorReport {
        st.doctor().report.lock().unwrap().clone().expect("a report")
    }

    static PICKED: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn recording(ready: &Ready) -> Result<Listed, Failure> {
        PICKED.lock().unwrap().push(named(ready));
        Ok(tool_named("t"))
    }

    #[tokio::test]
    async fn measure_picks_only_approved_local_windows_servers() {
        use marshell_protocol::api::{Profile, ShellFamily};
        let dir = tempfile::tempdir().unwrap();
        let root = crate::projects::canonical(dir.path());
        let exe = program(dir.path());
        let ubuntu = Profile {
            id: "ubuntu".into(),
            name: "Ubuntu".into(),
            command: vec!["wsl.exe".into(), "-d".into(), "Ubuntu".into()],
            family: ShellFamily::Wsl,
            cwd: None,
            env: Vec::new(),
            shell_integration: false,
            terminal_theme: None,
            detected: true,
        };
        let ubuntu_home = root.join("ubuntu-home");
        let ext = root.join("ext-home");
        let project = root.join("project");
        let st = pinned_with(dir.path(), vec![ubuntu])
            .with_ext_home(ext.clone())
            .with_agent_check(|command| command != "ghost")
            .with_wsl_version(Some("1.0.0".into()))
            .with_running_distros(|| vec!["Ubuntu".to_string()])
            .with_distro_target(Some(super::super::DistroTarget {
                home: ubuntu_home.clone(),
                bridge: "/mnt/c/marshell-hook.exe".into(),
            }))
            .with_probe(recording);
        crate::config::managed::save(
            &st.0.home,
            &[crate::config::managed::ManagedEntry {
                path: r"\\wsl.localhost\Ubuntu\home\u\.claude\settings.json".into(),
                pointer: Some("/hooks/Stop/-".into()),
                changeset_id: "c".into(),
                applied_ms: 1,
            }],
        )
        .unwrap();
        // Claude Code at the user level: one on, one remote, one whose command isn't there.
        write(
            &ext.join(".claude.json"),
            &serde_json::json!({"mcpServers": {
                "on": entry(&exe, "on"),
                "rem": {"type": "http", "url": "https://example.invalid/mcp"},
                "ghost": {"command": "ghost", "args": ["ghost"]},
            }})
            .to_string(),
        );
        // The project's shared file: one approved in settings.local.json, one not approved, one approved and then
        // turned off.
        write(
            &project.join(".mcp.json"),
            &serde_json::json!({"mcpServers": {
                "appr": entry(&exe, "appr"),
                "unappr": entry(&exe, "unappr"),
                "off": entry(&exe, "off"),
            }})
            .to_string(),
        );
        write(
            &project.join(".claude").join("settings.local.json"),
            r#"{"enabledMcpjsonServers":["appr","off"],"disabledMcpjsonServers":["off"]}"#,
        );
        // A server in the distro's Qwen Code settings, which must never start.
        write(
            &ubuntu_home.join(".qwen").join("settings.json"),
            &serde_json::json!({"mcpServers": {"in-distro": entry(&exe, "in-distro")}}).to_string(),
        );

        let gathered = facts_of(&st);
        assert!(
            gathered
                .ext
                .iter()
                .any(|i| i.distro.as_deref() == Some("Ubuntu") && i.servers.iter().any(|s| s.name == "in-distro")),
            "the distro's server was read"
        );
        let measured = tokio::task::spawn_blocking({
            let st = st.clone();
            move || measure_all(&st)
        })
        .await
        .unwrap();
        let mut picked = PICKED.lock().unwrap().clone();
        picked.sort();
        assert_eq!(picked, ["appr", "on"]);
        assert_eq!(measured.servers.len(), 2);
        assert!(
            measured
                .servers
                .iter()
                .all(|m| m.agent == "claude" && m.outcome.is_ok())
        );
    }

    static ONE_FAILS: AtomicUsize = AtomicUsize::new(0);

    fn one_fails(ready: &Ready) -> Result<Listed, Failure> {
        ONE_FAILS.fetch_add(1, Ordering::SeqCst);
        if named(ready) == "bad" {
            Err(Failure::Exited(Some(1)))
        } else {
            Ok(tool_named("good_tool"))
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn measure_results_ride_along_and_fail_unchecked() {
        let dir = tempfile::tempdir().unwrap();
        let exe = program(dir.path());
        let (st, ext) = ext_state(dir.path());
        let st = st.with_probe(one_fails);
        write(
            &ext.join(".claude.json"),
            &serde_json::json!({"mcpServers": {"bad": entry(&exe, "bad"), "good": entry(&exe, "good")}}).to_string(),
        );

        crate::server::doctor::request_run(&st);
        wait_idle(&st).await;
        assert_eq!(last_report(&st).measured_at_ms, None);
        assert!(!last_report(&st).items.iter().any(|i| i.id.starts_with("mcpcost.")));

        request_measure(&st);
        assert!(st.doctor().summary.borrow().measuring);
        // A thread watches the summary as fast as it can: the first time `measuring` is false, the report that holds
        // the results is already stored.
        let watched = st.clone();
        let seen = std::thread::spawn(move || {
            let started = std::time::Instant::now();
            loop {
                let s = watched.doctor().summary.borrow().clone();
                if !s.measuring && !s.running {
                    return watched.doctor().report.lock().unwrap().clone();
                }
                assert!(started.elapsed().as_secs() < 10, "the runs did not end");
                std::thread::yield_now();
            }
        });
        let report = seen.join().unwrap().expect("a report");
        assert!(report.measured_at_ms.is_some(), "{report:?}");
        let key = item_key(&ext.join(".claude.json"), &Origin::User, "bad");
        let fail = report
            .items
            .iter()
            .find(|i| i.id == format!("mcpfail.claude.{key}"))
            .unwrap_or_else(|| panic!("no failure item: {:?}", report.items));
        assert!(fail.unchecked);
        assert_eq!(fail.group, DoctorGroup::Broken);
        let cost = report
            .items
            .iter()
            .find(|i| i.id == "mcpcost.claude")
            .expect("the cost line");
        assert!(cost.title.contains("MCP servers add about"), "{}", cost.title);
        assert_eq!(ONE_FAILS.load(Ordering::SeqCst), 2);
        // A later background run keeps the results.
        crate::server::doctor::request_run(&st);
        wait_idle(&st).await;
        let again = last_report(&st);
        assert_eq!(again.measured_at_ms, report.measured_at_ms);
        assert!(again.items.iter().any(|i| i.id == "mcpcost.claude"));
    }

    static JOIN_CALLS: AtomicUsize = AtomicUsize::new(0);
    static JOIN_HOLD: AtomicBool = AtomicBool::new(true);

    fn held(_: &Ready) -> Result<Listed, Failure> {
        JOIN_CALLS.fetch_add(1, Ordering::SeqCst);
        while JOIN_HOLD.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(tool_named("t"))
    }

    #[tokio::test]
    async fn measure_joins_a_running_measure() {
        let dir = tempfile::tempdir().unwrap();
        let exe = program(dir.path());
        let (st, ext) = ext_state(dir.path());
        let st = st.with_probe(held);
        write(
            &ext.join(".claude.json"),
            &serde_json::json!({"mcpServers": {"a": entry(&exe, "a"), "b": entry(&exe, "b")}}).to_string(),
        );
        request_measure(&st);
        assert!(st.doctor().summary.borrow().measuring);
        while JOIN_CALLS.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        request_measure(&st);
        request_measure(&st);
        assert!(st.doctor().summary.borrow().measuring);
        JOIN_HOLD.store(false, Ordering::SeqCst);
        wait_idle(&st).await;
        // One start per server, whatever the number of requests.
        assert_eq!(JOIN_CALLS.load(Ordering::SeqCst), 2);
        assert!(!st.doctor().summary.borrow().measuring);
    }

    fn all_fail(_: &Ready) -> Result<Listed, Failure> {
        Err(Failure::NotMcp)
    }

    async fn plan_of(st: &AppState, ids: &[String]) -> DoctorFixPlan {
        let resp =
            super::super::doctor::fix(State(st.clone()), Json(DoctorFixRequest { item_ids: ids.to_vec() })).await;
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// The usage half (an unused item and a failed one for one server, picked together) comes with Task 9, when the
    /// report carries usage; what exists now is a failed measurement's switch, which the fix reads again, and one edit
    /// for the list it adds to.
    #[tokio::test]
    async fn fix_picks_unused_and_failed_once() {
        let dir = tempfile::tempdir().unwrap();
        let exe = program(dir.path());
        let (st, _ext) = ext_state(dir.path());
        let st = st.with_probe(all_fail);
        let project = crate::projects::canonical(dir.path()).join("project");
        write(
            &project.join(".mcp.json"),
            &serde_json::json!({"mcpServers": {"p1": entry(&exe, "p1"), "p2": entry(&exe, "p2")}}).to_string(),
        );
        write(
            &project.join(".claude").join("settings.local.json"),
            r#"{"enabledMcpjsonServers":["p1","p2"]}"#,
        );
        request_measure(&st);
        wait_idle(&st).await;
        let report = last_report(&st);
        let ids: Vec<String> = report
            .items
            .iter()
            .filter(|i| i.id.starts_with("mcpfail.claude.") && i.fixable)
            .map(|i| i.id.clone())
            .collect();
        assert_eq!(ids.len(), 2, "{:?}", report.items);
        // One failed server: one edit, in the local settings file.
        let one = plan_of(&st, &ids[..1]).await;
        assert_eq!(one.preview.files.len(), 1, "{:?}", one.preview);
        assert_eq!(one.item_ids, ids[..1]);
        // Both: still one edit, since they add to one list.
        let both = plan_of(&st, &ids).await;
        assert_eq!(both.preview.files.len(), 1, "{:?}", both.preview);
        let doc = &both.preview.files[0];
        assert!(doc.path.ends_with("settings.local.json"), "{doc:?}");
        assert!(doc.diff.contains("p1") && doc.diff.contains("p2"), "{}", doc.diff);
    }
}
