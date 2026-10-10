import { describe, expect, it } from "vitest";
import { COMMANDS, NAV_COMMAND_IDS, commandById, keyText, shortcutLabel } from "./keymap";

describe("keyText", () => {
  it("names a physical key the way it is printed on the keycap", () => {
    expect(keyText("KeyK")).toBe("K");
    expect(keyText("Digit7")).toBe("7");
    expect(keyText("F2")).toBe("F2");
    expect(keyText("Tab")).toBe("Tab");
    expect(keyText("Backspace")).toBe("Backspace");
    expect(keyText("ArrowLeft")).toBe("←");
  });
});

describe("shortcutLabel", () => {
  it("is Ctrl+Shift+key on Windows and Linux, Cmd+key on macOS", () => {
    expect(shortcutLabel({ code: "KeyK" }, "windows")).toBe("Ctrl+Shift+K");
    expect(shortcutLabel({ code: "KeyK" }, "linux")).toBe("Ctrl+Shift+K");
    expect(shortcutLabel({ code: "KeyK" }, "mac")).toBe("⌘K");
  });
  it("adds Alt and Option", () => {
    expect(shortcutLabel({ code: "KeyT", alt: true }, "windows")).toBe("Ctrl+Shift+Alt+T");
    expect(shortcutLabel({ code: "KeyT", alt: true }, "mac")).toBe("⌘⌥T");
  });
  it("keeps a plain key as it is (F2)", () => {
    expect(shortcutLabel({ code: "F2", plain: true }, "windows")).toBe("F2");
    expect(shortcutLabel({ code: "F2", plain: true }, "mac")).toBe("F2");
  });
  it("writes a navigation chord with the navigation modifier: Ctrl+Shift on Windows and Linux, ⌃⇧ on macOS", () => {
    expect(shortcutLabel({ code: "Tab", nav: true }, "windows")).toBe("Ctrl+Shift+Tab");
    expect(shortcutLabel({ code: "Backspace", nav: true }, "linux")).toBe("Ctrl+Shift+Backspace");
    expect(shortcutLabel({ code: "Tab", nav: true }, "mac")).toBe("⌃⇧Tab");
  });
});

describe("COMMANDS", () => {
  const win = (id: string) => shortcutLabel(commandById(id).binding!, "windows");
  const mac = (id: string) => shortcutLabel(commandById(id).binding!, "mac");

  it("lists the bindings from docs/PLAN.md section 7", () => {
    expect(win("palette")).toBe("Ctrl+Shift+K");
    expect(win("find")).toBe("Ctrl+Shift+F");
    expect(win("bookmark")).toBe("Ctrl+Shift+B");
    expect(win("launcher")).toBe("Ctrl+Shift+T");
    expect(win("reopen")).toBe("Ctrl+Shift+Alt+T");
    expect(win("close")).toBe("Ctrl+Shift+W");
    expect(win("global-search")).toBe("Ctrl+Shift+Alt+F");
    expect(win("next-waiting")).toBe("Ctrl+Shift+N");
    expect(win("rename")).toBe("F2");
  });

  it("moved the commands whose keys clashed on macOS, with Windows and Linux following so one letter means one thing everywhere", () => {
    // ⌘M is Minimize, ⌘H is Hide, ⌘Z is Undo, and Ctrl+Shift+Z is Redo in a text box.
    expect(mac("focus-prompt")).toBe("⌘L");
    expect(win("focus-prompt")).toBe("Ctrl+Shift+L");
    expect(mac("history")).toBe("⌘Y");
    expect(win("history")).toBe("Ctrl+Shift+Y");
    expect(mac("zoom-pane")).toBe("⌘P");
    expect(win("zoom-pane")).toBe("Ctrl+Shift+P");
    expect(mac("shell-panel")).toBe("⌘S");
    expect(win("shell-panel")).toBe("Ctrl+Shift+S");
  });

  it("uses no punctuation key anywhere, because punctuation sits on different keys (or behind AltGr) on other layouts", () => {
    for (const c of COMMANDS) if (c.binding) expect(c.binding.code, c.id).toMatch(/^(Key[A-Z]|Digit[0-9]|F[0-9]{1,2}|Tab|Space|Backspace|Arrow(Left|Right|Up|Down))$/);
  });

  it("moves between panes with the navigation chord, not with a bracket key", () => {
    expect(() => commandById("next-pane")).toThrow();
    expect(() => commandById("prev-pane")).toThrow();
    expect(win("nav-next-pane")).toBe("Ctrl+Shift+Tab");
    expect(mac("nav-next-pane")).toBe("⌃⇧Tab");
    expect(commandById("nav-next-pane").label).toBe("Select the next terminal on screen");
    // The navigation modifier already holds Shift, so Tab cannot also mean "backwards": the arrows go the other way.
    expect(() => commandById("nav-prev-pane")).toThrow();
    expect(win("nav-none")).toBe("Ctrl+Shift+Backspace");
    expect(commandById("nav-none").label).toBe("Select no terminal");
    expect(NAV_COMMAND_IDS).toEqual(["nav-next-pane", "nav-none"]);
  });

  it("keeps the other split commands", () => {
    expect(win("split")).toBe("Ctrl+Shift+D");
    expect(win("cheatsheet")).toBe("Ctrl+Shift+O");
    for (const id of ["layout", "unsplit"]) expect(commandById(id).group).toBe("View");
  });

  it("binds the shell panel under Tools", () => {
    const c = commandById("shell-panel");
    expect(c.group).toBe("Tools");
    expect(c.label).toBe("Show or focus the shell panel");
  });

  it("gives no two commands the same binding on any system", () => {
    for (const os of ["windows", "mac", "linux"] as const) {
      const seen = COMMANDS.filter((c) => c.binding).map((c) => shortcutLabel(c.binding!, os));
      expect(new Set(seen).size, os).toBe(seen.length);
    }
  });
  it("has unique ids and a label for every command", () => {
    expect(new Set(COMMANDS.map((c) => c.id)).size).toBe(COMMANDS.length);
    for (const c of COMMANDS) expect(c.label.length, c.id).toBeGreaterThan(0);
  });
  it("throws a clear error for an unknown id", () => {
    expect(() => commandById("nope")).toThrow("Unknown command: nope");
  });
});

describe("session numbers", () => {
  it("goes to sessions 1 to 9 with Ctrl+Shift+1 to 9 (Cmd+1 to 9 on macOS), not Ctrl+digit, which terminals read as control codes", () => {
    for (let n = 1; n <= 9; n++) {
      const c = commandById(`focus-session-${n}`);
      expect(c.label).toBe(`Go to session ${n}`);
      expect(shortcutLabel(c.binding!, "windows")).toBe(`Ctrl+Shift+${n}`);
      expect(shortcutLabel(c.binding!, "linux")).toBe(`Ctrl+Shift+${n}`);
      expect(shortcutLabel(c.binding!, "mac")).toBe(`⌘${n}`);
    }
  });
  it("reaches sessions past nine from the palette, by their number", () => {
    const c = commandById("go-to-session");
    expect(c.binding).toBeUndefined();
    expect(c.label).toContain("Go to session");
    expect(c.label).toContain("12");
  });
});
