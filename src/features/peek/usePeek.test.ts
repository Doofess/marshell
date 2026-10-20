import { afterEach, describe, expect, it, vi } from "vitest";
import { clampTop, peekName, usePeek } from "./usePeek";
import { mountHook } from "../../test/hooks";

vi.mock("react", async () => ({ ...await vi.importActual("react"), ...(await import("../../test/hooks")).hooks }));
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

describe("usePeek helpers", () => {
  it("opens on hover after 350 ms", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("window", { innerHeight: 800 });
    const read = vi.fn().mockResolvedValue(["last terminal row"]);
    const el = { getBoundingClientRect: () => ({ right: 200, top: 100 }) } as HTMLElement;
    const h = mountHook(() => usePeek(read));
    h.current.hoverRow("tab-a", el);
    vi.advanceTimersByTime(349); h.flush();
    expect(h.current.state).toBeNull();
    expect(read).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1); h.flush();
    expect(h.current.state).toMatchObject({ id: "tab-a", via: "hover", at: { x: 208, y: 100 } });
    expect(read).toHaveBeenCalledExactlyOnceWith("tab-a");
    await Promise.resolve(); h.flush();
    expect(h.current.state?.rows).toEqual(["last terminal row"]);
    h.unmount();
  });
  it("names the dialog by session number and name", () => {
    expect(peekName({ number: 6, name: "api-server" })).toBe("Peek: Session 6, api-server");
  });
  it("keeps the popover inside the window", () => {
    expect(clampTop(100, 800, 300)).toBe(100);
    expect(clampTop(700, 800, 300)).toBe(492);
    expect(clampTop(10, 200, 300)).toBe(8);
  });
});
