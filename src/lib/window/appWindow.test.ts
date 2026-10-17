import { describe, expect, it, vi } from "vitest";
import { close, isMaximized, minimize, onMaximizedChange, setTitle, toggleMaximize } from "./appWindow";

describe("appWindow outside Tauri", () => {
  it("does nothing and says the window is not maximized", async () => {
    await expect(minimize()).resolves.toBeUndefined();
    await expect(toggleMaximize()).resolves.toBeUndefined();
    await expect(close()).resolves.toBeUndefined();
    await expect(setTitle("x")).resolves.toBeUndefined();
    await expect(isMaximized()).resolves.toBe(false);
  });
  it("never calls back and returns a stop function", () => {
    const cb = vi.fn();
    const stop = onMaximizedChange(cb);
    expect(typeof stop).toBe("function");
    stop();
    expect(cb).not.toHaveBeenCalled();
  });
});
