import { describe, expect, it, vi } from "vitest";
import { onFontsLoaded } from "./useTerminalFloor";

function fakeFonts(ready: Promise<unknown> = Promise.resolve()) {
  const listeners = new Set<() => void>();
  return {
    ready,
    addEventListener: vi.fn((_type: "loadingdone", l: () => void) => void listeners.add(l)),
    removeEventListener: vi.fn((_type: "loadingdone", l: () => void) => void listeners.delete(l)),
    fire: () => listeners.forEach((l) => l()),
    count: () => listeners.size,
  };
}

describe("onFontsLoaded", () => {
  it("tells once when the fonts are ready, and again each time a font finishes loading", async () => {
    const fonts = fakeFonts();
    const tell = vi.fn();
    onFontsLoaded(fonts, tell);
    expect(tell).not.toHaveBeenCalled();
    await Promise.resolve();
    expect(tell).toHaveBeenCalledTimes(1);
    fonts.fire();
    fonts.fire();
    expect(tell).toHaveBeenCalledTimes(3);
    expect(fonts.addEventListener).toHaveBeenCalledWith("loadingdone", expect.any(Function));
  });

  it("stops telling once it is let go, even if the fonts become ready afterwards", async () => {
    let resolve = () => {};
    const fonts = fakeFonts(new Promise<void>((r) => (resolve = r)));
    const tell = vi.fn();
    const stop = onFontsLoaded(fonts, tell);
    stop();
    expect(fonts.count()).toBe(0);
    resolve();
    await Promise.resolve();
    await Promise.resolve();
    fonts.fire();
    expect(tell).not.toHaveBeenCalled();
  });

  it("does nothing where there is no font set", () => {
    const tell = vi.fn();
    expect(() => onFontsLoaded(undefined, tell)()).not.toThrow();
    expect(tell).not.toHaveBeenCalled();
  });

  it("a font set whose ready promise fails still tells on a load", async () => {
    const fonts = fakeFonts(Promise.reject(new Error("no fonts")));
    const tell = vi.fn();
    onFontsLoaded(fonts, tell);
    await Promise.resolve();
    await Promise.resolve();
    fonts.fire();
    expect(tell).toHaveBeenCalledTimes(1);
  });
});
