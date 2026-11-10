import { describe, expect, it } from "vitest";
import type { PaneView } from "../../generated/PaneView";
import { paneReducer } from "../pane/paneState";
import { pageReducer, type PageEvent, type PageState } from "./pageStore";
import { openKeys, owners, popupActions, unload } from "./useBrowserPages";
import { slotStep } from "./usePageSlot";

const empty: PaneView = { open: false, width: null, sessions: {}, recent: [] };
const file = (s: PaneView, session: string, path: string) => paneReducer(s, { type: "open", session, project: "p", target: { path } });
const page = (s: PaneView, session: string, url: string) => paneReducer(s, { type: "openPage", session, url });

describe("openKeys", () => {
  it("counts every browser tab of every session, and no file tab", () => {
    let s = file(empty, "a", "C:\\x\\a.md");
    s = page(s, "a", "https://example.com/");
    s = page(s, "b", "http://localhost:3000/");
    s = page(s, "b", "about:blank");
    s = file(s, "b", "C:\\x\\b.md");
    expect(openKeys(s)).toEqual(new Set(["a/t2", "b/t1", "b/t2"]));
  });
  it("is empty with no tabs", () => {
    expect(openKeys(empty).size).toBe(0);
  });
});

describe("owners", () => {
  it("names the session and the pane tab of each key", () => {
    const s = page(page(empty, "a", "https://example.com/"), "b", "https://example.org/");
    expect(owners(s).get("b/t1")).toEqual({ session: "b", paneTab: "t1" });
  });
});

describe("popupActions", () => {
  // openPage shows the pane, so the closed pane is made by hiding it after.
  const open = page(page(empty, "a", "https://one.example/"), "a", "https://two.example/");
  const base = paneReducer(open, { type: "hide" });
  it("opens a tab in the popup's own session and keeps it selected when that session is on screen", () => {
    expect(popupActions(base, "a", "https://pop.example/", true)).toEqual([{ type: "openPage", session: "a", url: "https://pop.example/" }]);
  });
  it("leaves the selected tab and the pane as they were when the session is not on screen", () => {
    expect(popupActions(base, "a", "https://pop.example/", false)).toEqual([
      { type: "openPage", session: "a", url: "https://pop.example/" },
      { type: "select", session: "a", paneTab: "t2" },
      { type: "hide" },
    ]);
  });
  it("keeps the pane open when it was open", () => {
    expect(popupActions(open, "a", "https://pop.example/", false)).toEqual([
      { type: "openPage", session: "a", url: "https://pop.example/" },
      { type: "select", session: "a", paneTab: "t2" },
    ]);
  });
});

// The ninth page unloads the one shown longest ago. Its tab must open it again when shown (found in the 6q-3a live check: the
// store kept the closed page, so the tab asked the shell to show a page that no longer existed, and stayed blank).
describe("unload", () => {
  const loaded = (key: string): ReadonlyMap<string, PageState> =>
    pageReducer(new Map(), { type: "state", key, url: "http://localhost:3000/", title: "T", canBack: false, canForward: false, loading: false });

  it("closes each page in the shell and forgets it, so its tab opens it again when shown", async () => {
    let pages = loaded("a/t1");
    const closed: string[] = [];
    expect(slotStep(pages.get("a/t1"), true, false)).toBe("show");
    await unload(
      ["a/t1"],
      async (key) => void closed.push(key),
      (e: PageEvent) => (pages = pageReducer(pages, e)),
    );
    expect(closed).toEqual(["a/t1"]);
    expect(slotStep(pages.get("a/t1"), true, false)).toBe("open");
  });

  it("forgets a page whose close failed: the shell has no such page either way", async () => {
    let pages = loaded("a/t1");
    await unload(
      ["a/t1"],
      () => Promise.reject(new Error("The page isn't open.")),
      (e: PageEvent) => (pages = pageReducer(pages, e)),
    );
    expect(pages.has("a/t1")).toBe(false);
  });
});
