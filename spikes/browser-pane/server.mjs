// Spike 6q-0: the loopback test site the child webview opens. Node built-ins only.
// node spikes/browser-pane/server.mjs --log <scratch>\site.jsonl [--port 47620]
import { appendFileSync, readFileSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const opt = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : fallback;
};
const port = Number(opt("--port", "47620"));
const logPath = opt("--log", null);
if (!logPath) {
  console.error("--log <path> is required");
  process.exit(2);
}
const here = dirname(fileURLToPath(import.meta.url));
const log = (entry) => appendFileSync(logPath, JSON.stringify({ t: Date.now(), by: "site", ...entry }) + "\n");

// Every page reports its keys, focus and input back here, so the checks can see what reached the page.
const reporter = `<script>
const report = (event, data = {}) => fetch("/report", { method: "POST", body: JSON.stringify({ event, page: location.pathname, ...data }) });
addEventListener("keydown", (e) => report("keydown", { code: e.code, key: e.key, ctrl: e.ctrlKey, shift: e.shiftKey, alt: e.altKey, repeat: e.repeat }));
addEventListener("focusin", (e) => report("focusin", { id: e.target.id || e.target.tagName }));
addEventListener("input", (e) => report("input", { id: e.target.id, value: e.target.value }));
addEventListener("load", () => report("load", { title: document.title }));
addEventListener("drop", (e) => report("drop", { files: [...(e.dataTransfer?.files ?? [])].map((f) => f.name) }));
</script>`;

const page = (title, body) => `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title}</title>${reporter}
<style>body{font:16px system-ui;margin:24px}a,input,button{display:block;margin:8px 0}</style></head><body><h1>${title}</h1>${body}</body></html>`;

const home = page(
  "Spike home",
  `<a id="first" href="/one">Page one</a>
<a id="popup" href="/two" target="_blank">Page two in a new window</a>
<input id="box" aria-label="Text box">
<button id="btn" type="button">A button</button>
${Array.from({ length: 30 }, (_, i) => `<a id="l${i}" href="/one?${i}">Link ${i}</a>`).join("\n")}
<a id="last" href="/two">Last link</a>`,
);

const server = createServer((req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  const send = (status, type, body, headers = {}) => {
    res.writeHead(status, { "content-type": type, "cache-control": "no-store", ...headers });
    res.end(body);
  };
  if (req.method === "POST" && url.pathname === "/report") {
    let body = "";
    req.on("data", (c) => (body += c));
    req.on("end", () => {
      try {
        log({ event: "report", ...JSON.parse(body) });
      } catch {
        log({ event: "report-unparsed", body });
      }
      send(204, "text/plain", "");
    });
    return;
  }
  log({ event: "request", method: req.method, path: url.pathname + url.search, cookie: req.headers.cookie ?? null });
  switch (url.pathname) {
    case "/":
      return send(200, "text/html", home);
    case "/one":
      return send(200, "text/html", page("Page one", `<a id="first" href="/two">Page two</a><a href="/">Home</a>`));
    case "/two":
      return send(200, "text/html", page("Page two", `<a id="first" href="/">Home</a>`));
    case "/cookie": {
      const had = req.headers.cookie ?? "(none)";
      const value = `spike=${Date.now()}; Max-Age=86400; Path=/`;
      const keep = /spike=/.test(had) ? {} : { "set-cookie": value };
      return send(200, "text/html", page("Cookie", `<p id="had">Cookie received: ${had}</p>`), keep);
    }
    case "/slow": {
      // A page that answers late, for a freeze while a navigation is still waiting (Task 5 c and d).
      const ms = Math.min(Number(url.searchParams.get("ms") ?? "1500") || 0, 10000);
      setTimeout(() => send(200, "text/html", page("Slow page", `<p>Answered after ${ms} ms.</p>`)), ms);
      return;
    }
    case "/redirect-file":
      return send(302, "text/plain", "", { location: "file:///C:/Windows/win.ini" });
    case "/probe":
      return send(200, "text/html", readFileSync(join(here, "probe.html")));
    default:
      return send(404, "text/plain", "not found");
  }
});
server.listen(port, "127.0.0.1", () => {
  log({ event: "listening", port, pid: process.pid });
  console.log(`spike site on http://127.0.0.1:${port}/ (pid ${process.pid})`);
});
