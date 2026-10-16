import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { LIVE_COMMANDS } from "./LivePalette";
import { closesShortcuts, LiveShortcuts } from "./LiveShortcuts";

const ev = (code: string, mods: Partial<{ ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean }> = {}) => ({
  code,
  key: code,
  ctrlKey: false,
  shiftKey: false,
  altKey: false,
  metaKey: false,
  ...mods,
});

describe("LiveShortcuts", () => {
  const h = renderToStaticMarkup(<LiveShortcuts os="windows" onClose={() => {}} />);
  it("is a modal dialog named by the sheet's title, that takes focus itself", () => {
    expect(h).toMatch(/<div[^>]*role="dialog"[^>]*aria-modal="true"/);
    expect(h).toMatch(/<div[^>]*role="dialog"[^>]*tabindex="-1"/);
    const id = /role="dialog"[^>]*aria-labelledby="([^"]+)"/.exec(h)?.[1];
    expect(id).toBeDefined();
    expect(h).toContain(`id="${id}"`);
  });
  it("lists the window's keys, and leaves out the ones it does not run yet", () => {
    expect(h).toContain("Ctrl+Shift+K");
    expect(h).toContain("Ctrl+Shift+O");
    expect(h).toContain("Go to session 1 to 9");
    expect(h).not.toContain("Search in scrollback");
    expect(h).not.toContain("History");
  });
  it("is a command the palette runs", () => {
    expect(LIVE_COMMANDS.map((c) => c.id)).toContain("cheatsheet");
  });
  it("closes on a second Ctrl+Shift+O (Cmd+O), and on no other key", () => {
    expect(closesShortcuts(ev("KeyO", { ctrlKey: true, shiftKey: true }), "windows")).toBe(true);
    expect(closesShortcuts(ev("KeyO", { metaKey: true }), "mac")).toBe(true);
    expect(closesShortcuts(ev("KeyO", { ctrlKey: true }), "windows")).toBe(false);
    expect(closesShortcuts(ev("KeyK", { ctrlKey: true, shiftKey: true }), "windows")).toBe(false);
  });
  // No DOM here, so the wiring is pinned at the source: Escape, Tab and focus return are useDialogFocus's (tested in
  // lib/dialogFocus.test.ts: Escape calls onClose and is prevented, so it never reaches the terminal, and focus goes back to it).
  it("hands Escape, Tab and the return of focus to useDialogFocus, with its onClose", () => {
    const src = readFileSync(new URL("./LiveShortcuts.tsx", import.meta.url), "utf8");
    expect(src).toMatch(/useDialogFocus\(dialog, \w+, onClose\)/);
  });
});
