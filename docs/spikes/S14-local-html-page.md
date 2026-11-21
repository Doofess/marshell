# S14: A local HTML file as a real page, and three WebView2 facts for the Browser menu (Windows)

Slice 6r-1b, run 2026-11-21 before the slice's plan was written ("Slice 6r-1b decisions" 5). The spike's code is the never-merged branch `phase-6r-1b-spike`, commit 923fc9fb, on the dev box: Windows 11 Pro (build 26300), one monitor at 100%, WebView2 Runtime 157.0.4341.60.
Build: `phase-6r-1b-spike` under `pnpm tauri dev`, started by `live-check.ps1` with scratch homes and CDP on 9333.
Decisions: `docs/PLAN.md` §9, "Slice 6r-1a decisions" 2 and "Slice 6r-1b decisions".

**Verdict: go for a folder mapped to a host name of its own, with cross-origin access denied. No-go for `file://`.** The viewport sizes, a full-page screenshot and two of the three cookie rules are also a go; WebView2 has no way to refuse cookies outright.

## The questions

1. Today an HTML file shows in a viewer where no script runs, and a page may load only `http`, `https` and `about:blank` (6q-3 decision 12). Can a local HTML file be a real page, and what can its scripts then reach?
2. Can a page be given a fixed viewport width, wider or narrower than its panel?
3. What can "keep cookies: always, until quit, never" mean in WebView2?
4. Can a screenshot hold the whole page, not only the part on screen?

## Driver

- The spike adds one probe to the page's Rust, reached through `browser_capture` with a key that starts `spike:`, so no command, capability or CSP changed. Its operations: `cdp` (`ICoreWebView2::CallDevToolsProtocolMethod`), `zoom` (`ICoreWebView2Controller::SetZoomFactor`), `map`, `mapcors` and `mapdeny` (`ICoreWebView2_3::SetVirtualHostNameToFolderMapping` with `DENY_CORS`, `ALLOW` and `DENY`), `unmap` and `go` (`Navigate`). The allow-list let `file:` and `*.marshell-spike.example` through, and a page made while `<home>/spike-inprivate` existed was InPrivate (`WebviewBuilder::incognito`).
- A page was read by `Runtime.evaluate` through that probe, called from the app's own webview over CDP. A page still has no CDP of its own from outside (S12 finding 2); the in-process call works, in a debug build with `devtools(false)`.
- The test site is `spikes/local-html-page/` on the spike branch: `proj-a/` (an `index.html` with an inline script, a sibling `app.js`, `style.css` with width media queries, an image, `data.json`, `sub/page2.html`), `outside/` (a text file, a script and an HTML file that stand for the rest of the disk), and `server.mjs`, a loopback site on port 47631 that logs every request and sets and echoes cookies. A junction `proj-a/link-out` pointed at `outside/`.
- No real key, mouse or desktop input was sent. The session was a PowerShell tab with no agent; hooks were skipped in onboarding.

## Results

### 1. A local HTML file as a real page

The same probe script ran in a normal web page (the baseline), in the file opened as `file:///…/proj-a/index.html`, and in the file opened as `https://<name>.marshell-spike.example/index.html` with `proj-a` mapped to that host.

| What the page's script tried | Web page (baseline) | `file://` | Mapped host |
| --- | --- | --- | --- |
| Inline script, sibling script, stylesheet, image | n/a | All ran and loaded | All ran and loaded |
| `<script type="module">` | n/a | **Refused** | Loaded |
| `fetch` of a file beside it (`data.json`) | n/a | **Refused** | 200, read |
| `fetch` or XHR of a file outside the folder | Refused | Refused | Refused (`../outside/secret.txt`, `/..%2f…`, `/sub/../../…`, and the junction `/link-out/secret.txt`) |
| `<script src>` of a file outside the folder | Refused | **Ran** (`outside/secret.js`) | Refused |
| `<img>` of a file anywhere on disk | Refused | **Loaded** (`C:\Windows\Web\…\img0.jpg`, 3840 px wide) | Refused |
| `<iframe>` of an HTML file outside the folder | Refused | **Loaded**, its content not readable | Refused |
| Navigating itself to `file:///C:/Windows/win.ini` | Refused | Allowed by the spike's loosened allow-list | Refused by Chromium |
| A POST to the internet (the loopback log site) | 200 | 200 | 200 |
| Marshell's API on loopback, no token | Not readable | Not readable | Not readable |
| `fetch` of `<home>/run/endpoint.json` (the API's port and token) | Refused | Refused | Refused |
| Tauri IPC: `core_endpoint`, `plugin:app\|version` | Denied by the ACL | Denied ("Origin header is not a valid URL") | Denied by the ACL |
| `localStorage` | Its own origin | **One store for every local file**: a value set by `proj-a/index.html` was read by `outside/other.html` | Its own origin, one per mapped host |
| Service worker | Available | Available in name | Registered |

More on the mapped host:

- **The mapping belongs to one webview.** A second tab could not load an image from the mapped host, fetch from it or navigate to it ("can't reach this page").
- **`DENY_CORS` leaks inside its own webview; `DENY` does not.** With `DENY_CORS`, after the mapped tab navigated to a web page, that web page loaded the folder's image and script and read `data.json` with a `no-cors` fetch (the response came back readable, type `basic`). `ClearVirtualHostNameToFolderMapping` did not stop it for the page already loaded. With `DENY`, the web page in the same webview, and a sandboxed frame inside the local page, got nothing: image error, script error, both fetches refused. The local page itself lost nothing under `DENY`: its scripts, module, and same-origin fetch all worked.
- **Edits show at once.** After `data.json` and `index.html` changed on disk, the next fetch and the next reload had the new content, with a service worker registered. The responses carry `Last-Modified` and no cache header.
- **A missing file fails the request** (no 404 page of WebView2's own was seen by `fetch`).
- A page under the mapped host is a secure context with an `https` origin, so every web API that needs one is available to it.

### 2. Viewport width

| Check | Result |
| --- | --- |
| `Emulation.setDeviceMetricsOverride {width: 1280, height: 0, deviceScaleFactor: 0, mobile: false}` in a 317 px panel | `innerWidth` 1280, the `min-width` media queries follow, `devicePixelRatio` stays 1. The page is drawn at full size and cut off at the panel's edge. |
| The same with `scale: 317 / 1280` | The whole 1280 px page is drawn scaled into the panel; `innerHeight` grows to 2713 (672 / scale). |
| Width 390 and 768 | `innerWidth` 390 and 768, the phone and tablet media queries apply. |
| Reload, and a navigation to another address | The override stays. `Emulation.clearDeviceMetricsOverride` ends it. |
| `mobile: true` | A page with no viewport meta lays out 980 px wide, as on a phone. No touch points, and the user agent doesn't change. |
| `SetZoomFactor(317 / 1280)` instead | `innerWidth` 1268, not 1280, and `devicePixelRatio` reads 0.25, which a page can see. The emulation is the better tool. |

### 3. Cookies

| Check | Result |
| --- | --- |
| A cookie with `Max-Age` and a session cookie, in the shared profile | Both sent back; `Storage.getCookies` tells them apart (`session: true`). |
| `Network.setCookiesEnabled` | No such method (0x80070057). |
| `Emulation.setDocumentCookieDisabled` | `document.cookie` reads empty and writes nothing; the cookies set over HTTP are still stored and sent. Not a way to refuse cookies. |
| `Storage.clearDataForOrigin {origin, storageTypes: "all"}` | That site's cookies are gone at once. |
| `Network.clearBrowserCookies` | Every cookie in the profile is gone at once. |
| An InPrivate page beside a normal one, same user-data folder | It starts with no cookies and no `localStorage`, and the normal page keeps its own. Two InPrivate pages share one jar. After the last InPrivate page closed, a new one started empty. |

So "always" is today's behaviour; "until quit" is a clear of the cookies at quit and at the next start; and the nearest thing to "never" is a private page, whose cookies live only while a private tab is open and are never written to disk. Nothing found refuses cookies outright.

### 4. Screenshot

| Check | Result |
| --- | --- |
| `Page.captureScreenshot {format: "png", captureBeyondViewport: true}` on a page 5164 px tall in a 672 px panel | A 302 × 5164 PNG in 56 ms. The page did not scroll. |
| The same while the viewport is emulated at 1280 | A 1280 × 2688 PNG: the screenshot has the emulated width at full size, whatever the panel's width. |
| A tab whose page isn't loaded | "The page isn't open." A screenshot needs a loaded page. |

## What 6r-1b and 6r-1c take from this

- **A local page, if the user approves it (6r-1c), is a folder mapped with `DENY`** to a host name of its own in that tab's webview. `file://` stays refused: on it modules and `fetch` don't work, any script, image or HTML file on the disk can be pulled in, and every local file shares one `localStorage`.
- **What a local page can still do** is what any web page can: talk to the internet, so a script can send out whatever it can read, which is every file under the mapped folder. Which folder is mapped (the file's own, or the project's) is the size of that reach, and `sub/page2.html` needs its parent mapped for `../app.js`.
- **A mapped tab should not go to a web address in the same webview.** `DENY` held in the checks, but a link to the web is better opened as a tab of its own.
- **The viewport sizes use `Emulation.setDeviceMetricsOverride` with `scale`** when the width is wider than the panel, per page; it survives navigation and must be set again for a page that is loaded again.
- **"Save screenshot" can use `Page.captureScreenshot`**, for the whole page or the part on screen.
- **CDP from the shell is new surface**: until now the page's Rust called no DevTools-protocol method.

## Not checked

- Tauri's `register_uri_scheme_protocol` and a loopback file server as other ways to serve a folder (both would also work on macOS and Linux, where pages aren't available yet).
- How the first load of a mapped host compares in time with a web page (no delay was noticed; it was not timed).
- A permission prompt's wording for a mapped host, a download from a local page, and printing.
- Whether a service worker a local page registers can outlive an edit in a way that matters (it did not in the one check above, whose worker had an empty fetch handler).
- `Network.setCookieControls` (third-party cookies only) was accepted; its effect was not measured.
- A real high-DPI display: every reading is at 100%.
