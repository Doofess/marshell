//! Spike 6q-0: the pure rules the browser child follows. Kept apart so 6q-3 can lift them as they are.

use tauri::Url;

/// Where the page may go: web pages and a blank page. The app's own `*.localhost` hosts (its assets, its IPC) are never a
/// page, and `localhost` itself is, for port chips. Every way in (typed, a link, a redirect, a drop) passes through here.
pub fn allowed_navigation(url: &Url) -> bool {
    match url.scheme() {
        "http" | "https" => !url
            .host_str()
            .is_some_and(|h| h.to_ascii_lowercase().ends_with(".localhost")),
        "about" => url.as_str() == "about:blank",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_pages_and_blank_are_allowed() {
        for u in [
            "https://example.com/",
            "http://127.0.0.1:47620/",
            "http://localhost:3000/",
            "about:blank",
        ] {
            assert!(allowed_navigation(&Url::parse(u).unwrap()), "{u}");
        }
    }

    #[test]
    fn files_and_the_apps_own_hosts_are_refused() {
        for u in [
            "file:///C:/Windows/win.ini",
            "http://tauri.localhost/",
            "https://tauri.localhost/",
            "http://ipc.localhost/x",
            "http://asset.localhost/x",
            "tauri://localhost/",
            "data:text/html,hi",
            "javascript:alert(1)",
        ] {
            assert!(!allowed_navigation(&Url::parse(u).unwrap()), "{u}");
        }
    }

    #[test]
    fn redirects_and_drops_are_checked_like_typed_urls() {
        // One rule for every way in: on_navigation sees redirects and drops as plain URLs.
        assert!(!allowed_navigation(&Url::parse("file://server/share/a.pdf").unwrap()));
        assert!(!allowed_navigation(&Url::parse("http://sub.tauri.localhost/").unwrap()));
    }
}
