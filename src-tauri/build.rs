use std::path::Path;

/// Every command in main.rs's `generate_handler!`. Listing them makes Tauri check each call against the
/// capability, which names only the app's own webview: a browser page in the same window can call none.
/// A test in main.rs holds this list, the handler list and the capability equal.
const COMMANDS: &[&str] = &[
    "save_diagnostics",
    "save_conversation",
    "pick_file",
    "pick_folder",
    "core_endpoint",
    "elevated_input",
    "elevated_resize",
    "elevated_create",
    "elevated_restart",
    "elevated_resume",
    "elevated_reopen",
    "elevated_decide",
    "close_once",
    "capture_screen_region",
    "cancel_capture",
    "read_clipboard_image",
    "set_badge",
    "set_tray",
    "notification_link",
    "set_notification_link",
    "set_window_background",
];

fn main() {
    drop_conpty_if_not_fetched();
    let attributes = tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}

/// On Windows, `tauri.windows.conf.json` bundles the fetched conpty.dll and OpenConsole.exe, and
/// tauri-build copies them next to the built exe, where portable-pty looks for them first.
/// Before `pnpm fetch:conpty` they are missing: then this removes them from the config so the
/// build still works, and the app uses the ConPTY built into Windows.
fn drop_conpty_if_not_fetched() {
    // `resources` is always there (it holds a README); a missing path would rerun on every build.
    println!("cargo:rerun-if-changed=resources");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let fetched = ["x64/conpty.dll", "x64/OpenConsole.exe", "LICENSE"]
        .iter()
        .all(|f| Path::new("resources/conpty").join(f).exists());
    if fetched {
        return;
    }
    println!("cargo:warning=ConPTY files not fetched; using the inbox ConPTY (run pnpm fetch:conpty)");
    if std::env::var_os("TAURI_CONFIG").is_some() {
        // A `--config` override is set; leave it alone, and tauri-build reports the missing files.
        return;
    }
    // tauri-build merges TAURI_CONFIG into the config; null removes the resources.
    // SAFETY: the build script has only this one thread, so nothing reads the environment meanwhile.
    unsafe { std::env::set_var("TAURI_CONFIG", r#"{"bundle":{"resources":null}}"#) };
}
