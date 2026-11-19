import { describe, expect, it } from "vitest";
import { clampPlace, placeAt, placeText, samePlace, scrollFor, type PageBox } from "./place";

// Two pages of 200 px with a 12 px gap between them, in client coordinates.
const pages: PageBox[] = [
  { page: 1, top: 100, height: 200 },
  { page: 2, top: 312, height: 200 },
];

describe("placeAt", () => {
  it("placeAt_inside_a_page", () => {
    expect(placeAt(150, pages)).toEqual({ page: 1, top: 0.25 });
    expect(placeAt(412, pages)).toEqual({ page: 2, top: 0.5 });
    expect(placeAt(100, pages)).toEqual({ page: 1, top: 0 });
  });
  it("placeAt_in_the_gap_takes_the_next_page", () => {
    expect(placeAt(305, pages)).toEqual({ page: 2, top: 0 });
  });
  it("placeAt_above_the_first_page", () => {
    expect(placeAt(40, pages)).toEqual({ page: 1, top: 0 });
  });
  it("placeAt_past_the_last_page", () => {
    expect(placeAt(600, pages)).toEqual({ page: 2, top: 1 });
    expect(placeAt(512, pages)).toEqual({ page: 2, top: 1 });
  });
  it("has no place without pages", () => {
    expect(placeAt(10, [])).toBeNull();
  });
});

describe("scrollFor", () => {
  it("scrollFor_puts_the_place_at_the_view_top", () => {
    // Page 2 starts 312 px down; a place half way in is at 412, and the view's top is at 160.
    expect(scrollFor({ page: 2, top: 0.5 }, pages[1]!, 160)).toBe(252);
    // A place already at the view's top needs no scroll.
    expect(scrollFor({ page: 1, top: 0 }, pages[0]!, 100)).toBe(0);
  });
});

describe("clampPlace", () => {
  it("clampPlace_past_the_end_is_the_last_page", () => {
    expect(clampPlace({ page: 9, top: 0.4 }, 5)).toEqual({ page: 5, top: 0 });
    expect(clampPlace({ page: 3, top: 0.4 }, 5)).toEqual({ page: 3, top: 0.4 });
    expect(clampPlace({ page: 0, top: 2 }, 5)).toEqual({ page: 1, top: 1 });
  });
});

describe("samePlace", () => {
  it("samePlace_within_half_a_percent", () => {
    expect(samePlace({ page: 2, top: 0.3 }, { page: 2, top: 0.304 })).toBe(true);
    expect(samePlace({ page: 2, top: 0.3 }, { page: 2, top: 0.31 })).toBe(false);
    expect(samePlace({ page: 2, top: 0.3 }, { page: 3, top: 0.3 })).toBe(false);
  });
});

describe("placeText", () => {
  it("placeText_reads_page_of", () => {
    expect(placeText(3, 12)).toBe("Page 3 of 12.");
  });
});
