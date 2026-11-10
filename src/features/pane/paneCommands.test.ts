import { describe, expect, it } from "vitest";
import type { PaneView } from "../../generated/PaneView";
import { closeTabOutcome, closedText, coverText, movedText, openCheck, openedText, paneBody, paneTabOutcome, toggleOutcome, TAB_LIMIT } from "./paneCommands";
import { paneReducer } from "./paneState";

const EMPTY: PaneView = { open: false, width: null, sessions: {}, recent: [] };
const withTabs = (session: string, paths: string[], open = true): PaneView => {
  let s = paths.reduce((acc, path) => paneReducer(acc, { type: "open", session, project: "p", target: { path } }), EMPTY);
  if (!open) s = paneReducer(s, { type: "hide" });
  return s;
};

describe("the announcements", () => {
  it("use the plan's words", () => {
    expect(openedText("a.md")).toBe("Opened a.md in the side pane.");
    expect(closedText("a.md")).toBe("Closed a.md.");
    expect(movedText("a.md", 0, 3)).toBe("Moved a.md to position 1 of 3.");
    expect(coverText("the launcher")).toBe("Close the launcher to open a file.");
    expect(TAB_LIMIT).toBe("This session has 32 tabs. Close one to open another.");
  });
});

describe("openCheck", () => {
  it("needs a session on screen", () => {
    expect(openCheck(EMPTY, null, "/a")).toBe("no-session");
    expect(openCheck(EMPTY, undefined, "/a")).toBe("no-session");
    expect(openCheck(EMPTY, "s1", "/a")).toBe("ok");
  });
  it("refuses a 33rd tab but not one already open", () => {
    const paths = Array.from({ length: 32 }, (_, i) => `/f${i}`);
    const full = withTabs("s1", paths);
    expect(openCheck(full, "s1", "/new")).toBe("refused");
    expect(openCheck(full, "s1", "/f3")).toBe("ok");
    expect(openCheck(full, "s2", "/new")).toBe("ok");
  });
});

describe("Ctrl+Shift+Q", () => {
  it("with no tabs shows the empty pane", () => {
    expect(toggleOutcome(EMPTY, "s1")).toEqual({ shown: true, body: "empty" });
    expect(paneReducer(EMPTY, { type: "toggle" }).open).toBe(true);
  });
  it("with tabs shows them, and hides an open pane", () => {
    const hidden = withTabs("s1", ["/a"], false);
    expect(toggleOutcome(hidden, "s1")).toEqual({ shown: true, body: "tabs" });
    expect(toggleOutcome(withTabs("s1", ["/a"]), "s1").shown).toBe(false);
  });
});

describe("paneBody", () => {
  it("is the empty state for a session with no tabs while the pane is open, and with no session at all", () => {
    const other = withTabs("s2", ["/a"]);
    expect(other.open).toBe(true);
    expect(paneBody(other, "s1")).toBe("empty");
    expect(paneBody(other, null)).toBe("empty");
    expect(paneBody(other, "s2")).toBe("tabs");
  });
});

describe("paneTabOutcome", () => {
  it("does nothing with no tabs, shows a hidden pane first, and steps an open one", () => {
    expect(paneTabOutcome(EMPTY, "s1")).toBe("none");
    expect(paneTabOutcome(withTabs("s1", ["/a", "/b"], false), "s1")).toBe("show");
    expect(paneTabOutcome(withTabs("s1", ["/a", "/b"]), "s1")).toBe("step");
  });
});

describe("closeTabOutcome", () => {
  it("is the selected tab, or null with none", () => {
    expect(closeTabOutcome(EMPTY, "s1")).toBeNull();
    const s = withTabs("s1", ["/a/x.md", "/a/y.md"]);
    expect(closeTabOutcome(s, "s1")).toEqual({ paneTab: "t2", path: "/a/y.md" });
  });
});
