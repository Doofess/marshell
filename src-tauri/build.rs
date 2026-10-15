use std::path::Path;

fn main() {
    drop_conpty_if_not_fetched();
    tauri_build::build()
}

/// On Windows, `tauri.windows.conf.json` bundles the fetched conpty.dll and OpenConsole.exe, and
/// tauri-build copies them next to the built exe, where portable-pty looks for them first.
/// Before `pnpm fetch:conpty` they are missing: then this removes them from the config so the
/// build still works, and the app uses the ConPTY built into Windows.
fn drop_conpty_if_not_fetched() {
    println!("cargo:rerun-if-changed=resources/conpty");
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
