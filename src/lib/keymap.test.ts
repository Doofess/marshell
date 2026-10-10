import { describe, expect, it } from "vitest";
import { COMMANDS, commandById, shortcutLabel } from "./keymap";

describe("shortcutLabel", () => {
  it("is Ctrl+Shift+key on Windows and Linux, Cmd+key on macOS", () => {
    expect(shortcutLabel({ key: "K" }, "windows")).toBe("Ctrl+Shift+K");
    expect(shortcutLabel({ key: "K" }, "linux")).toBe("Ctrl+Shift+K");
    expect(shortcutLabel({ key: "K" }, "mac")).toBe("⌘K");
  });
  it("adds Alt and Option", () => {
    expect(shortcutLabel({ key: "T", alt: true }, "windows")).toBe("Ctrl+Shift+Alt+T");
    expect(shortcutLabel({ key: "T", alt: true }, "mac")).toBe("⌘⌥T");
  });
  it("keeps keys the plan leaves as written (F2, Ctrl+1)", () => {
    expect(shortcutLabel({ key: "F2", plain: { win: "F2", mac: "F2" } }, "windows")).toBe("F2");
    expect(shortcutLabel({ key: "1", plain: { win: "Ctrl+1", mac: "⌘1" } }, "mac")).toBe("⌘1");
  });
});

describe("COMMANDS", () => {
  it("lists the bindings from docs/PLAN.md section 7", () => {
    const win = (id: string) => shortcutLabel(commandById(id).binding!, "windows");
    expect(win("palette")).toBe("Ctrl+Shift+K");
    expect(win("history")).toBe("Ctrl+Shift+H");
    expect(win("find")).toBe("Ctrl+Shift+F");
    expect(win("split")).toBe("Ctrl+Shift+\\");
    expect(win("bookmark")).toBe("Ctrl+Shift+B");
    expect(win("launcher")).toBe("Ctrl+Shift+T");
    expect(win("reopen")).toBe("Ctrl+Shift+Alt+T");
    expect(win("close")).toBe("Ctrl+Shift+W");
    expect(win("cheatsheet")).toBe("Ctrl+Shift+/");
    expect(win("global-search")).toBe("Ctrl+Shift+Alt+F");
    expect(win("next-waiting")).toBe("Ctrl+Shift+N");
    expect(win("rename")).toBe("F2");
  });
  it("binds the split-view commands: add a pane, zoom one, and move between them", () => {
    const win = (id: string) => shortcutLabel(commandById(id).binding!, "windows");
    expect(win("zoom-pane")).toBe("Ctrl+Shift+Z");
    expect(win("next-pane")).toBe("Ctrl+Shift+]");
    expect(win("prev-pane")).toBe("Ctrl+Shift+[");
    expect(shortcutLabel(commandById("zoom-pane").binding!, "mac")).toBe("⌘Z");
    for (const id of ["layout", "unsplit"]) expect(commandById(id).group).toBe("View");
  });
  it("gives no two commands the same binding", () => {
    const seen = COMMANDS.filter((c) => c.binding).map((c) => `${shortcutLabel(c.binding!, "windows")}`);
    expect(new Set(seen).size).toBe(seen.length);
  });
  it("has unique ids and a label for every command", () => {
    expect(new Set(COMMANDS.map((c) => c.id)).size).toBe(COMMANDS.length);
    for (const c of COMMANDS) expect(c.label.length, c.id).toBeGreaterThan(0);
  });
  it("throws a clear error for an unknown id", () => {
    expect(() => commandById("nope")).toThrow("Unknown command: nope");
  });
});
