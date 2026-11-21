import { describe, expect, it, vi } from "vitest";
import { coverPage, pageCovered, subscribeCover, uncovered } from "./pageCover";

describe("pageCover", () => {
  it("cover_counts_two_covers_and_one_release_still_covers", () => {
    expect(pageCovered()).toBe(false);
    const a = coverPage();
    const b = coverPage();
    expect(pageCovered()).toBe(true);
    a();
    expect(pageCovered()).toBe(true);
    b();
    expect(pageCovered()).toBe(false);
  });

  it("a release called twice removes one cover", () => {
    const a = coverPage();
    const b = coverPage();
    a();
    a();
    expect(pageCovered()).toBe(true);
    b();
    expect(pageCovered()).toBe(false);
  });

  it("subscribers hear only a change of covered", () => {
    let heard = 0;
    const off = subscribeCover(() => heard++);
    const a = coverPage();
    const b = coverPage();
    expect(heard).toBe(1);
    a();
    expect(heard).toBe(1);
    b();
    expect(heard).toBe(2);
    off();
    const c = coverPage();
    c();
    expect(heard).toBe(2);
  });

  it("uncovered is true at once with no cover and after the last cover goes", async () => {
    expect(await uncovered(5000)).toBe(true);
    const a = coverPage();
    const b = coverPage();
    let result: boolean | null = null;
    const p = uncovered(5000).then((v) => (result = v));
    a();
    await Promise.resolve();
    expect(result).toBeNull();
    b();
    await p;
    expect(result).toBe(true);
  });

  it("uncovered is false when the limit passes first", async () => {
    vi.useFakeTimers();
    try {
      const a = coverPage();
      const p = uncovered(5000);
      await vi.advanceTimersByTimeAsync(5000);
      expect(await p).toBe(false);
      a();
    } finally {
      vi.useRealTimers();
    }
  });
});
