import { describe, expect, it } from "vitest";
import { coverPage, pageCovered, subscribeCover } from "./pageCover";

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
});
