import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (name: string) => JSON.parse(readFileSync(new URL(name, import.meta.url), "utf8"));

// Tauri merges tauri.windows.conf.json into tauri.conf.json with JSON Merge Patch (RFC 7396): objects merge key by key, but
// an array such as app.windows is replaced whole, so the Windows entry must repeat the base entry.
describe("Windows window config", () => {
  const base = read("./tauri.conf.json").app.windows;
  const windows = read("./tauri.windows.conf.json").app?.windows;
  it("is the base window plus exactly a frameless window that keeps the system shadow", () => {
    expect(windows).toEqual([{ ...base[0], decorations: false, shadow: true }]);
  });
  it("leaves the other platforms' window to the base config", () => {
    expect(base[0]).not.toHaveProperty("decorations");
  });
  it("keeps the bundled ConPTY resources", () => {
    expect(Object.keys(read("./tauri.windows.conf.json").bundle.resources)).toHaveLength(3);
  });
});

describe("main window capability", () => {
  const permissions: unknown[] = read("./capabilities/default.json").permissions;
  it("lets the custom title bar minimize, maximize, close, drag and name the window", () => {
    for (const p of ["core:window:allow-minimize", "core:window:allow-toggle-maximize", "core:window:allow-close", "core:window:allow-start-dragging", "core:window:allow-set-title"])
      expect(permissions, p).toContain(p);
  });
  it("gets is-maximized and the drag region's double-click from core:default", () => {
    expect(permissions).toContain("core:default");
  });
});
