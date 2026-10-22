import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { windowTitle } from "./windowTitle";

describe("windowTitle: what the OS shows in the title bar and the taskbar", () => {
  it("is the focused session's name and then the app's", () => {
    expect(windowTitle(FIXTURES.find((f) => f.id === "working")!, 0)).toBe("api-server — Marshell");
  });
  it("is just the app's name when no session is focused", () => {
    expect(windowTitle(null, 0)).toBe("Marshell");
  });
  it("leaves a blank name out rather than showing a dangling dash", () => {
    expect(windowTitle({ ...FIXTURES[0]!, name: "  " }, 0)).toBe("Marshell");
  });
  it("leads with the needs-you count, for the focused session or for none", () => {
    expect(windowTitle(FIXTURES.find((f) => f.id === "working")!, 2)).toBe("(2) api-server — Marshell");
    expect(windowTitle(null, 1)).toBe("(1) Marshell");
  });
  it("counts with a blank name too", () => {
    expect(windowTitle({ ...FIXTURES[0]!, name: "  " }, 3)).toBe("(3) Marshell");
  });
});
