import { describe, expect, it } from "vitest";
import { findCommand, problemsIn, type KeyEventLike } from "./bindings";
import { COMMANDS, MOVED_COMMANDS, NAV_COMMAND_IDS, commandById, keyText, osOfPlatform, shortcutLabel } from "./keymap";

describe("osOfPlatform", () => {
  it("maps the core's platform names to the shortcut families", () => {
    expect([osOfPlatform("windows"), osOfPlatform("macos"), osOfPlatform("linux")]).toEqual(["windows", "mac", "linux"]);
  });
});

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
  it("writes Alt+Shift without the app modifier for a shift binding", () => {
    expect(shortcutLabel({ code: "ArrowUp", alt: true, shift: true }, "windows")).toBe("Alt+Shift+↑");
    expect(shortcutLabel({ code: "ArrowDown", alt: true, shift: true }, "linux")).toBe("Alt+Shift+↓");
    expect(shortcutLabel({ code: "ArrowUp", alt: true, shift: true }, "mac")).toBe("⌥⇧↑");
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
    expect(COMMANDS.some((c) => c.binding?.code === "KeyF" && c.binding.alt)).toBe(false);
    expect(win("next-waiting")).toBe("Ctrl+Shift+N");
    expect(win("rename")).toBe("F2");
  });

  it("moved the commands whose keys clashed on macOS, with Windows and Linux following so one letter means one thing everywhere", () => {
    // ⌘M is Minimize, ⌘H is Hide, ⌘Z is Undo, and Ctrl+Shift+Z is Redo in a text box.
    expect(mac("focus-prompt")).toBe("⌘L");
    expect(win("focus-prompt")).toBe("Ctrl+Shift+L");
    expect(mac("history")).toBe("⌘Y");
    expect(win("history")).toBe("Ctrl+Shift+Y");
    expect(mac("shell-panel")).toBe("⌘S");
    expect(win("shell-panel")).toBe("Ctrl+Shift+S");
  });

  it("binds Review this task's changes to Ctrl+Shift+R (Cmd+R on macOS) in the Sessions group", () => {
    const c = commandById("review-task");
    expect(c).toMatchObject({ label: "Review this task's changes", group: "Session", binding: { code: "KeyR" } });
    expect(win("review-task")).toBe("Ctrl+Shift+R");
    expect(mac("review-task")).toBe("⌘R");
    // The cheat sheet lists every command in COMMANDS, so this one is on it.
    expect(COMMANDS.some((x) => x.id === "review-task")).toBe(true);
  });

  it("uses no punctuation key anywhere, because punctuation sits on different keys (or behind AltGr) on other layouts", () => {
    for (const c of COMMANDS) if (c.binding) expect(c.binding.code, c.id).toMatch(/^(Key[A-Z]|Digit[0-9]|F[0-9]{1,2}|Tab|Space|Backspace|Arrow(Left|Right|Up|Down)|Page(Up|Down))$/);
  });

  it("moves between panes with the navigation chord, not with a bracket key", () => {
    expect(() => commandById("next-pane")).toThrow();
    expect(() => commandById("prev-pane")).toThrow();
    expect(win("nav-next-pane")).toBe("Ctrl+Shift+Tab");
    expect(mac("nav-next-pane")).toBe("⌃⇧Tab");
    expect(commandById("nav-next-pane").label).toBe("Select the terminal");
    // The navigation modifier already holds Shift, so Tab cannot also mean "backwards": the arrows go the other way.
    expect(() => commandById("nav-prev-pane")).toThrow();
    expect(win("nav-none")).toBe("Ctrl+Shift+Backspace");
    expect(commandById("nav-none").label).toBe("Select no terminal");
    expect(NAV_COMMAND_IDS).toEqual(["nav-next-pane", "nav-none"]);
  });

  it("moves a session up or down the list with Alt+Shift+Up and Down", () => {
    expect(win("move-up")).toBe("Alt+Shift+↑");
    expect(win("move-down")).toBe("Alt+Shift+↓");
    expect(commandById("move-up").group).toBe("Session");
  });

  it("the split commands are gone", () => {
    for (const id of ["split", "unsplit", "layout", "zoom-pane"]) expect(COMMANDS.some((c) => c.id === id), id).toBe(false);
    expect(commandById("nav-next-pane").label).toBe("Select the terminal");
    expect(win("cheatsheet")).toBe("Ctrl+Shift+O");
  });
  it("the freed keys run no command", () => {
    const e = (code: string, m: object): KeyEventLike => ({ code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...m });
    expect(findCommand(e("KeyD", { ctrlKey: true, shiftKey: true }), "windows")).toBeUndefined();
    expect(findCommand(e("KeyP", { ctrlKey: true, shiftKey: true }), "windows")).toBeUndefined();
    expect(findCommand(e("KeyD", { metaKey: true }), "mac")).toBeUndefined();
    expect(findCommand(e("KeyP", { metaKey: true }), "mac")).toBeUndefined();
  });

  it("binds the shell panel under Window", () => {
    const c = commandById("shell-panel");
    expect(c.group).toBe("Window");
    expect(c.label).toBe("Show or focus the shell panel");
  });

  it("jumps between prompts with PageUp and PageDown under Terminal", () => {
    const prev = commandById("prompt-prev");
    const next = commandById("prompt-next");
    expect(prev).toMatchObject({ label: "Jump to the previous prompt", group: "Terminal", binding: { code: "PageUp" } });
    expect(next).toMatchObject({ label: "Jump to the next prompt", group: "Terminal", binding: { code: "PageDown" } });
    expect(shortcutLabel(prev.binding!, "windows")).toBe("Ctrl+Shift+PageUp");
    expect(shortcutLabel(next.binding!, "mac")).toBe("⌘PageDown");
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

describe("focus mode key", () => {
  it("Ctrl+Shift+I is focus mode (Cmd+I on macOS), and the sidebar command has no key of its own", () => {
    const c = commandById("focus-mode");
    expect(c.binding).toEqual({ code: "KeyI" });
    expect(shortcutLabel(c.binding!, "windows")).toBe("Ctrl+Shift+I");
    expect(shortcutLabel(c.binding!, "mac")).toBe("⌘I");
    expect(commandById("sidebar").binding).toBeUndefined();
  });
  it("no binding uses U, E, C, V, Z or J, which the platforms or the terminal take", () => {
    for (const c of COMMANDS) expect(["KeyU", "KeyE", "KeyC", "KeyV", "KeyZ", "KeyJ"], c.id).not.toContain(c.binding?.code);
  });
});

describe("capture", () => {
  it("is Capture screen region under Terminal, on Ctrl+Shift+X, and clashes with nothing on any system", () => {
    const c = commandById("capture");
    expect(c.label).toBe("Capture screen region");
    expect(c.group).toBe("Terminal");
    expect(c.binding?.code).toBe("KeyX");
    expect(shortcutLabel(c.binding!, "windows")).toBe("Ctrl+Shift+X");
    expect(shortcutLabel(c.binding!, "linux")).toBe("Ctrl+Shift+X");
    // Cmd+X is Cut on macOS.
    expect(shortcutLabel(c.binding!, "mac")).toBe("⌘⌥X");
    for (const os of ["windows", "mac", "linux"] as const) expect(problemsIn(COMMANDS, os), os).toEqual([]);
  });
});

describe("diagnostics command", () => {
  it("sits right after agents, in App, with no key of its own", () => {
    const i = COMMANDS.findIndex((c) => c.id === "agents");
    expect(COMMANDS[i + 1]).toMatchObject({ id: "diagnostics", label: "Settings: Diagnostics…", group: "App" });
    expect(commandById("diagnostics")?.binding).toBeUndefined();
  });
});

describe("edit-references command", () => {
  it("is in Tools with no key of its own", () => {
    expect(commandById("edit-references")).toMatchObject({ label: "Edit this project's references", group: "Session" });
    expect(commandById("edit-references").binding).toBeUndefined();
  });
});

describe("repo script commands", () => {
  it("are Run and Stop in App, with no key of their own", () => {
    expect(commandById("run-script")).toMatchObject({ label: "Run the repo's run script", group: "Session" });
    expect(commandById("stop-script")).toMatchObject({ label: "Stop the repo's run script", group: "Session" });
    expect(commandById("run-script").binding).toBeUndefined();
    expect(commandById("stop-script").binding).toBeUndefined();
  });
});

describe("history settings command", () => {
  it("is History settings… in App, with no key of its own", () => {
    expect(commandById("history-settings")).toMatchObject({ label: "Settings: History…", group: "App" });
    expect(commandById("history-settings")?.binding).toBeUndefined();
  });
});

describe("settings command", () => {
  it("is Settings… in App, with no key of its own, and every id is listed once", () => {
    expect(commandById("settings")).toMatchObject({ label: "Settings…", group: "App" });
    expect(commandById("settings")?.binding).toBeUndefined();
    const ids = COMMANDS.map((c) => c.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("the four groups", () => {
  it("puts every command in one of the four groups, each group contiguous and in order", () => {
    const order = ["Session", "Terminal", "Window", "App"];
    const seen = COMMANDS.map((c) => order.indexOf(c.group));
    expect(seen.every((g) => g >= 0)).toBe(true);
    expect([...seen].sort((a, b) => a - b)).toEqual(seen);
  });
  it("names the Settings pages so typing a section finds them", () => {
    expect(["appearance", "agents", "profiles", "notifications", "history-settings", "diagnostics"].map((id) => commandById(id).label)).toEqual([
      "Settings: Appearance…", "Settings: Agents…", "Settings: Shell profiles…", "Settings: Notifications…", "Settings: History…", "Settings: Diagnostics…",
    ]);
  });
  it("has no Search all sessions command", () => {
    expect(COMMANDS.some((c) => c.id === "global-search")).toBe(false);
  });
  it("lists the commands the menu holds", () => {
    expect(MOVED_COMMANDS).toEqual(["settings", "appearance", "agents", "profiles", "notifications", "history-settings", "diagnostics", "history", "doctor", "departures"]);
    for (const id of MOVED_COMMANDS) expect(commandById(id).id).toBe(id);
  });
});

describe("the menu command", () => {
  it("reads Ctrl+Shift+M on Windows and Linux and ⌘E on macOS, with no clash", () => {
    const c = commandById("menu");
    expect(c.label).toBe("Open menu");
    expect(c.group).toBe("App");
    expect(shortcutLabel(c.binding!, "windows")).toBe("Ctrl+Shift+M");
    expect(shortcutLabel(c.binding!, "linux")).toBe("Ctrl+Shift+M");
    expect(shortcutLabel(c.binding!, "mac")).toBe("⌘E");
    for (const os of ["windows", "mac", "linux"] as const) expect(problemsIn(COMMANDS, os), os).toEqual([]);
  });
});
