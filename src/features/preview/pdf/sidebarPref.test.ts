import { beforeEach, describe, expect, it } from "vitest";
import { keyboardLeft, setSidebarPref, sidebarPref, startOpen, startView } from "./sidebarPref";

beforeEach(() => setSidebarPref({ open: false, view: null }));

describe("the sidebar pref", () => {
  it("sidebarPref_starts_closed_with_no_view_chosen", () => {
    expect(sidebarPref()).toEqual({ open: false, view: null });
  });
  it("sidebarPref_keeps_what_was_set_and_merges_partial_updates", () => {
    setSidebarPref({ open: true });
    setSidebarPref({ view: "pages" });
    expect(sidebarPref()).toEqual({ open: true, view: "pages" });
  });
});

describe("startOpen", () => {
  it("startOpen_opens_when_left_open_and_beside_the_pages", () => {
    expect(startOpen({ open: true, view: null }, false)).toBe(true);
  });
  it("startOpen_stays_closed_where_it_would_lie_over_the_pages", () => {
    expect(startOpen({ open: true, view: "outline" }, true)).toBe(false);
  });
  it("startOpen_stays_closed_when_left_closed", () => {
    expect(startOpen({ open: false, view: "pages" }, false)).toBe(false);
    expect(startOpen({ open: false, view: null }, true)).toBe(false);
  });
});

describe("startView", () => {
  it("startView_with_an_outline_follows_the_pref_and_defaults_to_outline", () => {
    expect(startView({ open: false, view: null }, true)).toBe("outline");
    expect(startView({ open: false, view: "outline" }, true)).toBe("outline");
    expect(startView({ open: false, view: "pages" }, true)).toBe("pages");
  });
  it("startView_without_an_outline_shows_pages_whatever_the_pref", () => {
    expect(startView({ open: false, view: null }, false)).toBe("pages");
    expect(startView({ open: false, view: "outline" }, false)).toBe("pages");
    expect(startView({ open: false, view: "pages" }, false)).toBe("pages");
  });
  it("startView_does_not_change_the_pref", () => {
    startView({ open: false, view: "outline" }, false);
    expect(sidebarPref().view).toBeNull();
  });
});

// Read a moment after the keyboard leaves: a window losing focus (Alt+Tab) leaves the keyboard where it was, in the sidebar.
describe("keyboardLeft", () => {
  const row = { in: true };
  const pages = { in: false };
  const inside = (t: unknown) => (t as { in?: boolean } | null)?.in === true;
  it("an_overlying_sidebar_stays_open_when_the_window_loses_focus", () => {
    expect(keyboardLeft(false, row, inside)).toBe(false);
  });
  it("keyboardLeft_is_the_keyboard_on_something_outside_the_sidebar", () => {
    expect(keyboardLeft(true, pages, inside)).toBe(true);
    expect(keyboardLeft(true, null, inside)).toBe(true);
    expect(keyboardLeft(true, row, inside)).toBe(false);
  });
});
