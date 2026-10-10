import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { windowTitle } from "./windowTitle";

describe("windowTitle: what the OS shows in the title bar and the taskbar", () => {
  it("is the focused session's name and then the app's", () => {
    expect(windowTitle(FIXTURES.find((f) => f.id === "working")!)).toBe("api-server — Marshell");
  });
  it("is just the app's name when no session is focused", () => {
    expect(windowTitle(null)).toBe("Marshell");
  });
  it("leaves a blank name out rather than showing a dangling dash", () => {
    expect(windowTitle({ ...FIXTURES[0]!, name: "  " })).toBe("Marshell");
  });
});
