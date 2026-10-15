import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Tauri expects a fixed dev port (tauri.conf.json devUrl).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  // cssTarget: the webviews Marshell ships on (WebView2, WKWebView, WebKitGTK) all handle light-dark() and oklch()
  // natively. An older target makes the minifier rewrite light-dark() into variables that ignore a nested
  // color-scheme, which breaks forced-dark or side-by-side light panels.
  build: { target: "es2022", cssTarget: ["chrome123", "safari17.5"] },
  test: { environment: "node" },
});
