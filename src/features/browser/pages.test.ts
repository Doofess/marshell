import { describe, expect, it } from "vitest";
import { LIVE_PAGES, nextLive } from "./pages";

describe("nextLive", () => {
  it("puts the shown page first", () => {
    expect(nextLive(["a", "b"], "c", new Set(["a", "b", "c"]))).toEqual({ live: ["c", "a", "b"], close: [] });
  });

  it("evicts the page shown longest ago", () => {
    const keys = Array.from({ length: LIVE_PAGES + 1 }, (_, i) => `k${i}`);
    const open = new Set(keys);
    let live: string[] = [];
    let close: string[] = [];
    for (const k of keys) ({ live, close } = nextLive(live, k, open));
    expect(live).toHaveLength(LIVE_PAGES);
    expect(close).toEqual(["k0"]);
  });

  it("drops a page whose tab is gone", () => {
    expect(nextLive(["a", "b"], null, new Set(["b"]))).toEqual({ live: ["b"], close: ["a"] });
  });

  it("with nothing shown keeps the order", () => {
    expect(nextLive(["b", "a"], null, new Set(["a", "b"]))).toEqual({ live: ["b", "a"], close: [] });
  });

  it("shows a page already first without a change", () => {
    expect(nextLive(["a", "b"], "a", new Set(["a", "b"]))).toEqual({ live: ["a", "b"], close: [] });
  });

  it("does not load a shown page whose tab is not open", () => {
    expect(nextLive(["a"], "z", new Set(["a"]))).toEqual({ live: ["a"], close: [] });
  });
});
