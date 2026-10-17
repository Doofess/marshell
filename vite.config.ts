import { createRequire } from "node:module";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vitest/config";

const PDFJS_DIRS = ["cmaps", "standard_fonts", "wasm"];

/** Every file under `dir`, as paths relative to it with forward slashes. */
function filesUnder(dir: string, prefix = ""): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    return statSync(full).isDirectory() ? filesUnder(full, `${prefix}${name}/`) : [`${prefix}${name}`];
  });
}

/**
 * pdf.js fetches its character maps and standard fonts at run time. They are not part of any bundle, so this serves them
 * from node_modules at /pdfjs/ in dev and writes them under pdfjs/ in the build.
 */
function pdfjsAssets(): Plugin {
  const root = join(dirname(createRequire(import.meta.url).resolve("pdfjs-dist/package.json")));
  return {
    name: "marshell-pdfjs-assets",
    configureServer(server) {
      server.middlewares.use("/pdfjs", (req, res, next) => {
        const rel = decodeURIComponent((req.url ?? "").split("?")[0]!).replace(/^\/+/, "");
        const [dir] = rel.split("/");
        const file = join(root, rel);
        if (!dir || !PDFJS_DIRS.includes(dir) || rel.includes("..") || !existsSync(file) || !statSync(file).isFile()) return next();
        res.setHeader("Content-Type", file.endsWith(".wasm") ? "application/wasm" : "application/octet-stream");
        res.end(readFileSync(file));
      });
    },
    generateBundle() {
      for (const dir of PDFJS_DIRS) {
        const base = join(root, dir);
        if (!existsSync(base)) continue;
        for (const rel of filesUnder(base)) this.emitFile({ type: "asset", fileName: `pdfjs/${dir}/${rel}`, source: readFileSync(join(base, rel)) });
      }
    },
  };
}

// Tauri expects a fixed dev port (tauri.conf.json devUrl).
export default defineConfig({
  plugins: [react(), pdfjsAssets()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  // cssTarget: the webviews Marshell ships on (WebView2, WKWebView, WebKitGTK) all handle light-dark() and oklch()
  // natively. An older target makes the minifier rewrite light-dark() into variables that ignore a nested
  // color-scheme, which breaks forced-dark or side-by-side light panels.
  build: { target: "es2022", cssTarget: ["chrome123", "safari17.5"] },
  test: { environment: "node" },
});
