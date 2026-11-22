import { afterEach, describe, expect, it, vi } from "vitest";
import type { FilterMode } from "./filterText";
import { createShownAfter, filesWait, focusToFilter, LISTING, listFailed, SEARCHING } from "./filesWait";

const MODES: FilterMode[] = [{ kind: "none" }, { kind: "names", query: "a" }, { kind: "search", query: "ab" }, { kind: "short" }, { kind: "long" }];

describe("filesWait", () => {
  it("waits for the name list only in names mode, and for a search only while it is busy", () => {
    for (const mode of MODES) {
      for (const names of ["loading", "error", "ready"] as const) {
        for (const searchBusy of [false, true]) {
          const want = mode.kind === "names" && names === "loading" ? LISTING : mode.kind === "search" && searchBusy ? SEARCHING : null;
          expect(filesWait({ mode, names, searchBusy })).toBe(want);
        }
      }
    }
    expect(LISTING).toBe("Listing files…");
    expect(SEARCHING).toBe("Searching…");
  });
  it("sends focus to the filter box only when a wait begins with focus in the tree", () => {
    expect(focusToFilter({ waitBegan: true, focusWasInTree: true })).toBe(true);
    expect(focusToFilter({ waitBegan: true, focusWasInTree: false })).toBe(false);
    expect(focusToFilter({ waitBegan: false, focusWasInTree: true })).toBe(false);
  });
  it("says a failed list with the folder, or for the session when there is none", () => {
    expect(listFailed("src")).toBe("Can't list the files in src.");
    expect(listFailed(null)).toBe("Can't list files for this session.");
  });
});

describe("createShownAfter", () => {
  afterEach(() => vi.useRealTimers());
  it("shows the ring only after 150 ms and hides it at once", () => {
    vi.useFakeTimers();
    const set = vi.fn();
    const s = createShownAfter(150, set);
    s.update(true);
    vi.advanceTimersByTime(149);
    expect(set).not.toHaveBeenCalledWith(true);
    vi.advanceTimersByTime(1);
    expect(set).toHaveBeenLastCalledWith(true);
    s.update(false);
    expect(set).toHaveBeenLastCalledWith(false);
  });
  it("a wait that ends inside 150 ms never shows the ring", () => {
    vi.useFakeTimers();
    const set = vi.fn();
    const s = createShownAfter(150, set);
    s.update(true);
    vi.advanceTimersByTime(100);
    s.update(false);
    vi.advanceTimersByTime(500);
    expect(set).not.toHaveBeenCalledWith(true);
    s.dispose();
  });
});
