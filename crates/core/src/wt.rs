//! Finds Windows Terminal's settings files so the user can import its colour schemes.
//! Read-only: Marshell never writes to another program's config.

use marshell_protocol::api::FoundFile;
use std::path::{Path, PathBuf};

/// Files larger than this are skipped: a real settings file is far smaller.
const MAX_BYTES: u64 = 1024 * 1024;

/// Where Windows Terminal keeps `settings.json` under `%LOCALAPPDATA%`: the Store version,
/// the Preview version, then the unpackaged one.
pub fn candidates(local_app_data: &Path) -> Vec<PathBuf> {
    let packaged = |package: &str| {
        local_app_data
            .join("Packages")
            .join(package)
            .join("LocalState")
            .join("settings.json")
    };
    vec![
        packaged("Microsoft.WindowsTerminal_8wekyb3d8bbwe"),
        packaged("Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe"),
        local_app_data
            .join("Microsoft")
            .join("Windows Terminal")
            .join("settings.json"),
    ]
}

/// The candidate files that exist and can be read. A file over 1 MB, or one that fails to read
/// (not text, no permission), is skipped.
pub fn find(local_app_data: &Path) -> Vec<FoundFile> {
    let mut found = Vec::new();
    for path in candidates(local_app_data) {
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.len() > MAX_BYTES {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            found.push(FoundFile {
                path: path.to_string_lossy().into_owned(),
                text,
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn candidates_in_order() {
        let local = Path::new("local");
        let packages = local.join("Packages");
        assert_eq!(
            candidates(local),
            vec![
                packages.join("Microsoft.WindowsTerminal_8wekyb3d8bbwe/LocalState/settings.json"),
                packages.join("Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe/LocalState/settings.json"),
                local.join("Microsoft/Windows Terminal/settings.json"),
            ]
        );
    }

    #[test]
    fn find_reads_existing_and_skips_missing() {
        let local = tempfile::tempdir().unwrap();
        let preview = candidates(local.path())[1].clone();
        write(&preview, "{\"profiles\":{}}");
        let found = find(local.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, preview.to_string_lossy());
        assert_eq!(found[0].text, "{\"profiles\":{}}");
    }

    #[test]
    fn find_skips_files_over_1mb() {
        let local = tempfile::tempdir().unwrap();
        let [stable, preview, _] = candidates(local.path()).try_into().unwrap();
        write(&stable, &" ".repeat(MAX_BYTES as usize + 1));
        write(&preview, &" ".repeat(MAX_BYTES as usize));
        let found = find(local.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, preview.to_string_lossy());
    }
}
