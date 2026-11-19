import { describe, expect, it } from "vitest";
import { listKey, nextToDraw } from "./thumbs";

describe("nextToDraw", () => {
  it("nextToDraw_takes_the_wanted_page_nearest_the_current_one", () => {
    expect(nextToDraw(new Set([2, 5, 9]), new Set(), 6)).toBe(5);
    expect(nextToDraw(new Set([2, 5, 9]), new Set(), 1)).toBe(2);
    expect(nextToDraw(new Set([2, 5, 9]), new Set(), 20)).toBe(9);
  });
  it("nextToDraw_skips_pages_already_drawn", () => {
    expect(nextToDraw(new Set([4, 5, 6]), new Set([5]), 5)).toBe(6);
  });
  it("nextToDraw_gives_a_tie_to_the_later_page", () => {
    expect(nextToDraw(new Set([3, 7]), new Set(), 5)).toBe(7);
    expect(nextToDraw(new Set([4, 6]), new Set([5]), 5)).toBe(6);
  });
  it("nextToDraw_returns_null_when_nothing_is_left", () => {
    expect(nextToDraw(new Set(), new Set(), 1)).toBeNull();
    expect(nextToDraw(new Set([1, 2]), new Set([1, 2]), 1)).toBeNull();
  });
});

describe("listKey", () => {
  it("listKey_moves_one_with_up_and_down_and_stops_at_the_ends", () => {
    expect(listKey(3, 10, "ArrowDown", 4)).toBe(4);
    expect(listKey(3, 10, "ArrowUp", 4)).toBe(2);
    expect(listKey(0, 10, "ArrowUp", 4)).toBeNull();
    expect(listKey(9, 10, "ArrowDown", 4)).toBeNull();
  });
  it("listKey_home_and_end_go_to_the_first_and_last", () => {
    expect(listKey(5, 10, "Home", 4)).toBe(0);
    expect(listKey(5, 10, "End", 4)).toBe(9);
    expect(listKey(0, 10, "Home", 4)).toBeNull();
    expect(listKey(9, 10, "End", 4)).toBeNull();
  });
  it("listKey_page_keys_move_a_view_and_clamp_at_the_ends", () => {
    expect(listKey(5, 10, "PageDown", 3)).toBe(8);
    expect(listKey(8, 10, "PageDown", 3)).toBe(9);
    expect(listKey(9, 10, "PageDown", 3)).toBeNull();
    expect(listKey(5, 10, "PageUp", 3)).toBe(2);
    expect(listKey(1, 10, "PageUp", 3)).toBe(0);
    expect(listKey(0, 10, "PageUp", 3)).toBeNull();
  });
  it("listKey_moves_at_least_one_when_no_thumbnail_fits_whole", () => {
    expect(listKey(5, 10, "PageDown", 0)).toBe(6);
  });
  it("listKey_ignores_other_keys_and_an_empty_list", () => {
    expect(listKey(2, 10, "a", 3)).toBeNull();
    expect(listKey(0, 0, "ArrowDown", 3)).toBeNull();
  });
});
