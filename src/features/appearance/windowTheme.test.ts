import { describe, expect, it, vi } from "vitest";
import { themeSync, windowThemeOf } from "./windowTheme";

/** Lets the sync's send settle. */
const settle = () => new Promise<void>((done) => setTimeout(done, 0));

describe("windowThemeOf", () => {
  it("is the root's theme when Appearance names one", () => {
    expect(windowThemeOf("light")).toBe("light");
    expect(windowThemeOf("dark")).toBe("dark");
  });

  it("follows the system when the root names no theme", () => {
    expect(windowThemeOf(null)).toBe("system");
  });

  it("follows the system for a value it doesn't know", () => {
    expect(windowThemeOf("sepia")).toBe("system");
  });
});

describe("themeSync", () => {
  it("sends the theme the first time, following the system included", () => {
    const send = vi.fn(async () => {});
    themeSync(() => "system", send)();
    expect(send.mock.calls).toEqual([["system"]]);
  });

  it("sends an unchanged theme once", () => {
    const send = vi.fn(async () => {});
    const sync = themeSync(() => "light", send);
    sync();
    sync();
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("sends again when the theme changed", () => {
    const send = vi.fn(async () => {});
    let now: "light" | "system" = "light";
    const sync = themeSync(() => now, send);
    sync();
    now = "system";
    sync();
    expect(send.mock.calls).toEqual([["light"], ["system"]]);
  });

  it("sends the same theme again after a send that failed", async () => {
    const send = vi.fn(async () => {});
    send.mockRejectedValueOnce(new Error("no window"));
    const sync = themeSync(() => "light", send);
    sync();
    await settle();
    sync();
    expect(send.mock.calls).toEqual([["light"], ["light"]]);
  });

  it("keeps a newer theme when an older send fails late", async () => {
    let fail: (e: Error) => void = () => {};
    const send = vi.fn(async () => {});
    send.mockImplementationOnce(() => new Promise<void>((_, reject) => (fail = reject)));
    let now: "light" | "dark" = "light";
    const sync = themeSync(() => now, send);
    sync();
    now = "dark";
    sync();
    fail(new Error("no window"));
    await settle();
    sync();
    expect(send.mock.calls).toEqual([["light"], ["dark"]]);
  });
});
