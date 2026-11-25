import { describe, expect, it } from "vitest";
import { shellPanelAsk } from "./shellPanelAsk";

describe("shellPanelAsk", () => {
  it("not ready asks nothing", () => {
    expect(shellPanelAsk({ ready: false, sessions: 0, focusedCwd: undefined })).toBeNull();
    expect(shellPanelAsk({ ready: false, sessions: 2, focusedCwd: "C:\work" })).toBeNull();
  });
  it("a hidden panel still asks", () => {
    expect(shellPanelAsk({ ready: true, sessions: 0, focusedCwd: undefined })).toEqual({ cwd: null });
  });
  it("a focused session gives its folder", () => {
    expect(shellPanelAsk({ ready: true, sessions: 2, focusedCwd: "C:\work" })).toEqual({ cwd: "C:\work" });
  });
  it("no session gives the default folder", () => {
    expect(shellPanelAsk({ ready: true, sessions: 0, focusedCwd: undefined })).toEqual({ cwd: null });
  });
  it("sessions with none focused waits", () => {
    expect(shellPanelAsk({ ready: true, sessions: 2, focusedCwd: undefined })).toBeNull();
  });
});
