import { describe, expect, it } from "vitest";
import { shellPanelAsk } from "./shellPanelAsk";

describe("shellPanelAsk", () => {
  it("hidden asks nothing", () => {
    expect(shellPanelAsk({ shown: false, sessions: 0, focusedCwd: undefined })).toBeNull();
    expect(shellPanelAsk({ shown: false, sessions: 2, focusedCwd: "C:\\work" })).toBeNull();
  });
  it("a focused session gives its folder", () => {
    expect(shellPanelAsk({ shown: true, sessions: 2, focusedCwd: "C:\\work" })).toEqual({ cwd: "C:\\work" });
  });
  it("no session gives the default folder", () => {
    expect(shellPanelAsk({ shown: true, sessions: 0, focusedCwd: undefined })).toEqual({ cwd: null });
  });
  it("sessions with none focused waits", () => {
    expect(shellPanelAsk({ shown: true, sessions: 2, focusedCwd: undefined })).toBeNull();
  });
});
