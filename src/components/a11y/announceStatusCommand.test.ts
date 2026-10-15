import { describe, expect, it } from "vitest";
import { problemsIn } from "../../lib/bindings";
import { COMMANDS, commandById, shortcutLabel, type Os } from "../../lib/keymap";

describe("the 'Read the focused session's status' command", () => {
  it("exists, with the words the palette shows", () => {
    expect(commandById("announce-status").label).toBe("Read the focused session's status");
  });
  it("is Ctrl+Shift+G on Windows and Linux and ⌘G on macOS, matched by physical key", () => {
    const b = commandById("announce-status").binding!;
    expect(b.code).toBe("KeyG");
    expect(shortcutLabel(b, "windows")).toBe("Ctrl+Shift+G");
    expect(shortcutLabel(b, "mac")).toBe("⌘G");
  });
  it("clashes with nothing on any platform", () => {
    for (const os of ["windows", "mac", "linux"] as Os[]) expect(problemsIn(COMMANDS, os), os).toEqual([]);
  });
});
