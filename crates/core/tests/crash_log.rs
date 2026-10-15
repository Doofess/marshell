//! Its own test binary, because the panic hook is process-global. The hook is installed once for
//! one home, and the tests take turns (a panic in one would otherwise land in the other's log).

use marshell_core::{crash, paths};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn home() -> &'static Path {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let home = tempfile::tempdir().unwrap().keep();
        crash::install_panic_hook(&home);
        home
    })
}

fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn panic_in_thread(name: &str, message: &'static str) {
    let joined = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || panic!("{message}"))
        .unwrap()
        .join();
    assert!(joined.is_err());
}

#[test]
fn panics_are_appended_to_crash_log() {
    let _turn = one_at_a_time();
    let log = paths::crash_log(home());
    let _ = std::fs::remove_file(&log);
    panic_in_thread("probe", "boom 42");
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.contains("thread=probe"), "{text}");
    assert!(text.contains("boom 42"), "{text}");
    assert_eq!(text.lines().count(), 1, "{text}");
}

#[test]
fn rotates_past_one_mebibyte() {
    let _turn = one_at_a_time();
    let log = paths::crash_log(home());
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    std::fs::write(&log, vec![b'x'; 1024 * 1024 + 1]).unwrap();
    panic_in_thread("rotate", "boom again");
    assert!(log.with_file_name("crash.log.1").exists());
    let text = std::fs::read_to_string(&log).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
}
