import { describe, expect, it } from "vitest";
import type { PaneView } from "../../generated/PaneView";
import { closeAsk, closeTabOutcome, leavePaneOutcome, closedText, coverText, movedText, openCheck, openedText, pageOpenCheck, pageOpenedText, paneBody, paneHiddenText, paneTabFocus, paneTabOutcome, projectKeyOf, toggleOutcome, PAGE_REFUSED, TAB_LIMIT } from "./paneCommands";
import { paneReducer, recentFor } from "./paneState";

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

describe("paneTabFocus", () => {
  it("a step taken from inside a tab's body moves focus to the tab it shows", () => {
    expect(paneTabFocus({ outcome: "step", from: "panel", covers: false })).toBe(true);
  });
  it("a step from the terminal, the strip or anywhere else leaves focus where it is", () => {
    expect(paneTabFocus({ outcome: "step", from: "elsewhere", covers: false })).toBe(false);
    expect(paneTabFocus({ outcome: "step", from: "elsewhere", covers: true })).toBe(false);
  });
  it("showing the hidden pane takes focus only when the pane covers the terminal", () => {
    expect(paneTabFocus({ outcome: "show", from: "elsewhere", covers: true })).toBe(true);
    expect(paneTabFocus({ outcome: "show", from: "elsewhere", covers: false })).toBe(false);
  });
  it("nothing moves with no tabs", () => {
    expect(paneTabFocus({ outcome: "none", from: "panel", covers: true })).toBe(false);
  });
});

describe("projectKeyOf", () => {
  const rows = [
    { id: "s1", project: "api", projectKey: "C:\\work\\api" },
    { id: "s2", project: "api", projectKey: "D:\\client\\api" },
  ];
  it("is the session's repository, so two projects whose folders share a name keep their own recent files", () => {
    expect(projectKeyOf(rows, "s1")).toBe("C:\\work\\api");
    expect(projectKeyOf(rows, "s2")).toBe("D:\\client\\api");
    const s = paneReducer(EMPTY, { type: "open", session: "s1", project: projectKeyOf(rows, "s1"), target: { path: "C:\\work\\api\\a.md" } });
    expect(recentFor(s, projectKeyOf(rows, "s2"), "s2")).toEqual([]);
    expect(recentFor(s, projectKeyOf(rows, "s1"), "other")).toEqual(["C:\\work\\api\\a.md"]);
  });
  it("is empty with no session on screen", () => {
    expect(projectKeyOf(rows, undefined)).toBe("");
    expect(projectKeyOf(rows, "gone")).toBe("");
  });
});

describe("closeTabOutcome", () => {
  it("is the selected tab, or null with none", () => {
    expect(closeTabOutcome(EMPTY, "s1")).toBeNull();
    const s = withTabs("s1", ["/a/x.md", "/a/y.md"]);
    expect(closeTabOutcome(s, "s1")).toEqual({ paneTab: "t2", path: "/a/y.md" });
  });
});

describe("pageOpenCheck", () => {
  const urls = (n: number) => Array.from({ length: n }, (_, i) => `https://site${i}.example/`);
  const withPages = (session: string, list: string[]): PaneView => list.reduce((acc, url) => paneReducer(acc, { type: "openPage", session, url }), EMPTY);
  it("refuses with no session on screen first", () => {
    expect(pageOpenCheck(EMPTY, undefined, "https://example.com/", [])).toBe("no-session");
    expect(pageOpenCheck(EMPTY, null, "https://example.com/", [])).toBe("no-session");
  });
  it("refuses an address that is not a web page: another scheme, a path, userinfo", () => {
    for (const url of ["file:///C:/x", "javascript:alert(1)", "C:\\x\\a.md", "https://user:pw@example.com/", "mailto:a@b.c"]) expect(pageOpenCheck(EMPTY, "s1", url, []), url).toBe("refused");
  });
  it("refuses the 33rd tab, but not an address already open or a tab of another session", () => {
    const full = withPages("s1", urls(32));
    expect(pageOpenCheck(full, "s1", "https://new.example/", [])).toBe("limit");
    expect(pageOpenCheck(full, "s1", "about:blank", [])).toBe("limit");
    expect(pageOpenCheck(full, "s1", "https://site3.example/", [])).toBe("ok");
    expect(pageOpenCheck(full, "s2", "https://new.example/", [])).toBe("ok");
  });
  it("an_own_origin_is_refused_before_a_tab_opens", () => {
    const own = [["localhost", 1420], ["127.0.0.1", 49152]] as const;
    for (const url of ["http://localhost:1420/", "http://127.0.0.1:49152/v1/health", "http://tauri.localhost/", "http://[::1]:1420/"]) expect(pageOpenCheck(EMPTY, "s1", url, own), url).toBe("refused");
    for (const url of ["http://localhost:3000/", "https://example.com/", "about:blank"]) expect(pageOpenCheck(EMPTY, "s1", url, own), url).toBe("ok");
    expect(pageOpenCheck(EMPTY, undefined, "http://localhost:1420/", own)).toBe("no-session");
  });
  it("accepts http, https and about:blank", () => {
    for (const url of ["http://localhost:3000/", "https://example.com/a?b=1", "about:blank"]) expect(pageOpenCheck(EMPTY, "s1", url, []), url).toBe("ok");
  });
});

describe("the page announcements", () => {
  it("use the plan's words", () => {
    expect(PAGE_REFUSED).toBe("That address can't be opened in the side pane.");
    expect(pageOpenedText("https://example.com/a")).toBe("Opened example.com in the side pane.");
    expect(pageOpenedText("http://localhost:3000/")).toBe("Opened localhost:3000 in the side pane.");
    expect(pageOpenedText("about:blank")).toBe("Opened New tab in the side pane.");
  });
});

describe("closeAsk", () => {
  it("closing_an_unsaved_tab_asks_first", () => {
    const t = { id: "t1", path: "C:\\w\\a.txt", kind: "text" as const };
    expect(closeAsk(t, new Set(["C:\\w\\a.txt"]), "windows")).toBe("ask");
    // The same file by another spelling.
    expect(closeAsk(t, new Set(["c:/w/A.txt"]), "windows")).toBe("ask");
    expect(closeAsk(t, new Set(["C:\\w\\b.txt"]), "windows")).toBe("close");
    expect(closeAsk(t, new Set(), "windows")).toBe("close");
    // A page is never asked about, whatever the set holds.
    expect(closeAsk({ id: "b1", path: "https://a.test/", kind: "web" }, new Set(["https://a.test/"]), "windows")).toBe("close");
  });

  it("closeAsk_asks_for_an_unsaved_pdf", () => {
    const pdf = { id: "p1", path: "C:\\w\\report.pdf", kind: "pdf" as const };
    expect(closeAsk(pdf, new Set(["c:/w/Report.pdf"]), "windows")).toBe("ask");
    expect(closeAsk(pdf, new Set(), "windows")).toBe("close");
    // An image has nothing to save.
    expect(closeAsk({ id: "i1", path: "C:\\w\\a.png", kind: "image" }, new Set(["C:\\w\\a.png"]), "windows")).toBe("close");
  });
});

describe("leavePaneOutcome", () => {
  it("hides a pane that covers the terminal", () => {
    expect(leavePaneOutcome(true)).toBe("hide");
  });
  it("only moves the keyboard when the terminal shows beside the pane", () => {
    expect(leavePaneOutcome(false)).toBe("focus");
  });
});

it("says the pane is hidden and which keys show it", () => {
  expect(paneHiddenText("Ctrl+Shift+Q")).toBe("The side pane is hidden. Ctrl+Shift+Q shows it.");
});
