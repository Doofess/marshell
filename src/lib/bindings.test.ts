import { describe, expect, it } from "vitest";
import { bindingConflicts, chordOf, findCommand, matchesBinding, problemsIn, reservedReason, type KeyEventLike } from "./bindings";
import { COMMANDS, type Binding, type Command, type Os } from "./keymap";

const ev = (code: string, m: Partial<Omit<KeyEventLike, "code">> = {}): KeyEventLike => ({ code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...m });
const OSES: Os[] = ["windows", "mac", "linux"];

describe("chordOf", () => {
  it("is Ctrl+Shift on Windows and Linux and Cmd on macOS, with Alt added when asked", () => {
    expect(chordOf({ code: "KeyK" }, "windows")).toEqual({ code: "KeyK", ctrl: true, shift: true, alt: false, meta: false });
    expect(chordOf({ code: "KeyK" }, "mac")).toEqual({ code: "KeyK", ctrl: false, shift: false, alt: false, meta: true });
    expect(chordOf({ code: "KeyT", alt: true }, "linux").alt).toBe(true);
  });
  it("is the key alone for a plain binding and the navigation modifier for a navigation chord", () => {
    expect(chordOf({ code: "F2", plain: true }, "windows")).toEqual({ code: "F2", ctrl: false, shift: false, alt: false, meta: false });
    expect(chordOf({ code: "Tab", nav: true }, "mac")).toEqual({ code: "Tab", ctrl: true, shift: true, alt: false, meta: false });
    expect(chordOf({ code: "Tab", nav: true }, "windows", "ctrl+alt")).toEqual({ code: "Tab", ctrl: true, shift: false, alt: true, meta: false });
  });
});

describe("matchesBinding: by physical key", () => {
  it("matches the key by its position, whatever character the layout puts there", () => {
    // On a French (AZERTY) keyboard the key at the QWERTY "A" position types "q"; KeyboardEvent.code stays KeyA.
    expect(matchesBinding(ev("KeyK", { ctrlKey: true, shiftKey: true }), { code: "KeyK" }, "windows")).toBe(true);
    // On a Hebrew keyboard the same key types "ל" and `key` would never equal "K"; `code` still does.
    expect(matchesBinding(ev("Digit3", { metaKey: true }), { code: "Digit3" }, "mac")).toBe(true);
  });
  it("needs exactly the modifiers, so Ctrl+Shift+Alt+T is not Ctrl+Shift+T", () => {
    const t: Binding = { code: "KeyT" };
    expect(matchesBinding(ev("KeyT", { ctrlKey: true, shiftKey: true, altKey: true }), t, "windows")).toBe(false);
    expect(matchesBinding(ev("KeyT", { ctrlKey: true, shiftKey: true, altKey: true }), { code: "KeyT", alt: true }, "windows")).toBe(true);
    expect(matchesBinding(ev("KeyT", { ctrlKey: true }), t, "windows")).toBe(false);
    expect(matchesBinding(ev("KeyT"), t, "windows")).toBe(false);
  });
  it("lets plain Tab and plain Esc through: no binding matches them, so they reach the terminal", () => {
    for (const os of OSES) {
      expect(findCommand(ev("Tab"), os)).toBeUndefined();
      expect(findCommand(ev("Escape"), os)).toBeUndefined();
      expect(findCommand(ev("Tab", { shiftKey: true }), os)).toBeUndefined();
    }
  });
  it("finds the command a key press belongs to", () => {
    expect(findCommand(ev("KeyK", { ctrlKey: true, shiftKey: true }), "windows")?.id).toBe("palette");
    expect(findCommand(ev("KeyK", { metaKey: true }), "mac")?.id).toBe("palette");
    expect(findCommand(ev("Digit6", { ctrlKey: true, shiftKey: true }), "linux")?.id).toBe("focus-session-6");
    expect(findCommand(ev("F2"), "windows")?.id).toBe("rename");
  });
});

describe("bindingConflicts", () => {
  it("is true for the same chord and false for any difference", () => {
    for (const os of OSES) {
      expect(bindingConflicts({ code: "KeyK" }, { code: "KeyK" }, os)).toBe(true);
      expect(bindingConflicts({ code: "KeyK" }, { code: "KeyJ" }, os)).toBe(false);
      expect(bindingConflicts({ code: "KeyK" }, { code: "KeyK", alt: true }, os)).toBe(false);
    }
  });
  it("sees a navigation chord and an app chord on the same key as one chord where both use Ctrl+Shift, and as two on macOS", () => {
    // Why no app command may ever be bound to Tab, Backspace or an arrow: the navigation layer owns them under Ctrl+Shift.
    expect(bindingConflicts({ code: "Tab", nav: true }, { code: "Tab" }, "windows")).toBe(true);
    expect(bindingConflicts({ code: "Tab", nav: true }, { code: "Tab" }, "mac")).toBe(false);
    expect(bindingConflicts({ code: "F2", plain: true }, { code: "F2", nav: true }, "windows")).toBe(false);
  });
});

describe("reservedReason: keys the system, a text box or a terminal program already owns", () => {
  const reason = (b: Binding, os: Os) => reservedReason(b, os);
  it("knows the macOS clashes that sent M, H and Z away", () => {
    expect(reason({ code: "KeyM" }, "mac")).toMatch(/Minimize/);
    expect(reason({ code: "KeyH" }, "mac")).toMatch(/Hide/);
    expect(reason({ code: "KeyZ" }, "mac")).toMatch(/Undo/);
    expect(reason({ code: "KeyQ" }, "mac")).toMatch(/Quit/);
    expect(reason({ code: "KeyC" }, "mac")).toMatch(/Copy/);
  });
  it("knows the Windows and Linux ones", () => {
    expect(reason({ code: "KeyZ" }, "windows")).toMatch(/Redo/);
    expect(reason({ code: "KeyC" }, "windows")).toMatch(/copies/i);
    expect(reason({ code: "KeyV" }, "linux")).toMatch(/pastes/i);
    expect(reason({ code: "Escape", nav: true }, "windows")).toMatch(/Task Manager/);
    expect(reason({ code: "KeyU" }, "linux")).toMatch(/Unicode/i);
    expect(reason({ code: "KeyE" }, "linux")).toMatch(/emoji/i);
    expect(reason({ code: "KeyI" }, "windows")).toMatch(/developer tools/);
    expect(reason({ code: "KeyJ" }, "linux")).toMatch(/developer console/);
  });
  it("knows a bare key a terminal program needs", () => {
    for (const code of ["Tab", "Escape", "Enter", "ArrowUp", "Backspace"]) expect(reason({ code, plain: true }, "windows"), code).toMatch(/terminal/i);
  });
  it("is null for a binding nobody else wants", () => {
    for (const os of OSES) expect(reason({ code: "KeyK" }, os)).toBeNull();
    expect(reason({ code: "F2", plain: true }, "linux")).toBeNull();
  });
  it("does not let Ctrl+Shift+arrows be navigation text selection", () => {
    expect(reason({ code: "ArrowLeft", nav: true }, "windows")).toMatch(/select/i);
  });
});

describe("the shipped commands", () => {
  it("clash with nothing and with each other on any system", () => {
    for (const os of OSES) expect(problemsIn(COMMANDS, os), os).toEqual([]);
  });
  it("reports a duplicate and a reserved key when there are some", () => {
    const bad: Command[] = [
      { id: "a", label: "A", group: "Tools", binding: { code: "KeyK" } },
      { id: "b", label: "B", group: "Tools", binding: { code: "KeyK" } },
      { id: "c", label: "C", group: "Tools", binding: { code: "KeyM" } },
    ];
    const p = problemsIn(bad, "mac");
    expect(p).toHaveLength(2);
    expect(p[0]).toMatch(/a and b/);
    expect(p[1]).toMatch(/c.*Minimize/);
  });
});
