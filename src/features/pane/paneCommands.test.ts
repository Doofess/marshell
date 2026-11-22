import { describe, expect, it } from "vitest";
import type { PaneView } from "../../generated/PaneView";
import { closeAsk, closeNothingText, closeTabOutcome, commandPanel, firstWith, hideFocus, noTabsText, openedByText, panelButtonsFor, panelHiddenText, panelOrder, panelToggleOutcome, leavePaneOutcome, closedText, coverText, movedText, openCheck, openedText, pageOpenCheck, pageOpenedText, paneBody, paneHiddenText, paneTabFocus, receiptWithPaneHidden, paneTabOutcome, projectKeyOf, shouldPin, noTabText, PAGE_REFUSED, TAB_LIMIT } from "./paneCommands";
import { paneReducer, recentFor } from "./paneState";
import type { PanelId } from "./panels";
import { landingPanel, unseenPanels } from "./paneCommands";

const EMPTY: PaneView = { open: false, width: null, sessions: {}, recent: [] };
const withTabs = (session: string, paths: string[], open = true): PaneView => {
  let s = paths.reduce((acc, path) => paneReducer(acc, { type: "open", session, project: "p", target: { path } }), EMPTY);
  if (!open) s = paneReducer(s, { type: "hide" });
  return s;
};

describe("the announcements", () => {
  it("use the plan's words", () => {
    expect(openedText("a.md", "files")).toBe("Opened a.md in Files.");
    expect(openedText("localhost:3000", "browser")).toBe("Opened localhost:3000 in Browser.");
    expect(openedByText("Claude Code", "report.html", "browser")).toBe("Claude Code opened report.html in Browser.");
    expect(noTabsText("browser")).toBe("No tabs open in Browser.");
    expect(panelHiddenText("browser")).toBe("Browser is hidden.");
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
  it("at 32 tabs an open that replaces the panel's preview tab is not refused", () => {
    const paths = Array.from({ length: 32 }, (_, i) => `/f${i}`);
    const full = withTabs("s1", paths);
    expect(openCheck(full, "s1", "/new.ts")).toBe("refused");
    expect(openCheck(full, "s1", "/new.ts", true)).toBe("ok");
  });
});

describe("Ctrl+Shift+Q", () => {
  it("with no tabs shows the empty pane", () => {
    expect(paneBody(EMPTY, "s1")).toBe("empty");
    expect(paneReducer(EMPTY, { type: "toggle", session: "s1" }).open).toBe(true);
  });
  it("with tabs shows them, and hides an open pane", () => {
    const hidden = withTabs("s1", ["/a"], false);
    expect(paneBody(hidden, "s1")).toBe("tabs");
    expect(paneReducer(hidden, { type: "toggle", session: "s1" }).open).toBe(true);
    expect(paneReducer(withTabs("s1", ["/a"]), { type: "toggle", session: "s1" }).open).toBe(false);
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
    expect(paneTabOutcome(EMPTY, "s1", "files")).toBe("none");
    expect(paneTabOutcome(withTabs("s1", ["/a", "/b"], false), "s1", "files")).toBe("show");
    expect(paneTabOutcome(withTabs("s1", ["/a", "/b"]), "s1", "files")).toBe("step");
  });
  it("is none with an open pane when the panel is null or holds no tabs", () => {
    const s = withTabs("s1", ["/a", "/b"]);
    expect(paneTabOutcome(s, "s1", null)).toBe("none");
    expect(paneTabOutcome(s, "s1", "browser")).toBe("none");
  });
  it("shows a hidden pane when the session has a tab in any panel", () => {
    expect(paneTabOutcome(withTabs("s1", ["/a"], false), "s1", null)).toBe("show");
    expect(paneTabOutcome(withTabs("s1", ["/a"], false), "s1", "browser")).toBe("show");
    expect(paneTabOutcome({ ...EMPTY, open: false }, "s1", "files")).toBe("none");
  });
});

describe("commandPanel", () => {
  const holds = (p: PanelId) => p === "files";
  it("is null with nothing on screen", () => {
    expect(commandPanel([], "files", "files", holds)).toBeNull();
  });
  it("is the panel with the keyboard", () => {
    expect(commandPanel(["browser", "files"], "browser", "files", holds)).toBe("browser");
  });
  it("else the last one that had it, when it is on screen and holds tabs", () => {
    expect(commandPanel(["browser", "files"], null, "files", holds)).toBe("files");
  });
  it("else the first on screen that holds tabs", () => {
    expect(commandPanel(["browser", "files"], null, "browser", holds)).toBe("files");
    expect(commandPanel(["browser", "files"], null, null, holds)).toBe("files");
  });
  it("else the first on screen", () => {
    expect(commandPanel(["browser", "files"], null, null, () => false)).toBe("browser");
  });
  it("skips a last panel that is not on screen", () => {
    expect(commandPanel(["files"], null, "browser", () => true)).toBe("files");
  });
});

describe("panelToggleOutcome", () => {
  const two = (open = true): PaneView => ({ ...EMPTY, open, panels: { shown: ["browser", "files"], weights: {}, filled: null } });
  it("shows a panel that is not on screen", () => {
    const s: PaneView = { ...EMPTY, open: true, panels: { shown: ["files"], weights: {}, filled: null } };
    expect(panelToggleOutcome(s, "s1", "browser")).toEqual({ shows: true, paneHides: false });
  });
  it("hides one of two and keeps the pane", () => {
    expect(panelToggleOutcome(two(), "s1", "browser")).toEqual({ shows: false, paneHides: false });
  });
  it("hides the last one and the pane", () => {
    const s: PaneView = { ...EMPTY, open: true, panels: { shown: ["files"], weights: {}, filled: null } };
    expect(panelToggleOutcome(s, "s1", "files")).toEqual({ shows: false, paneHides: true });
  });
  it("shows a panel of a hidden pane", () => {
    expect(panelToggleOutcome(two(false), "s1", "files").shows).toBe(true);
  });
});

describe("hideFocus", () => {
  it("a hide from inside the panel gives the terminal the keyboard", () => {
    expect(hideFocus("browser", "browser", false, ["files"])).toEqual({ to: "terminal" });
  });
  it("a hide from elsewhere leaves the keyboard alone", () => {
    expect(hideFocus("browser", null, true, ["files"])).toEqual({ to: "stay" });
    expect(hideFocus("browser", "files", true, ["files"])).toEqual({ to: "stay" });
  });
  it("a hide in a pane that covers the terminal gives the other panel the keyboard", () => {
    expect(hideFocus("browser", "browser", true, ["files"])).toEqual({ to: "panel", panel: "files" });
  });
  it("and the terminal when no panel stays", () => {
    expect(hideFocus("files", "files", true, [])).toEqual({ to: "terminal" });
  });
});

describe("panelOrder and firstWith", () => {
  it("puts the current panel first, then the others in order", () => {
    expect(panelOrder(["browser", "files"], "files")).toEqual(["files", "browser"]);
    expect(panelOrder(["browser", "files"], "browser")).toEqual(["browser", "files"]);
    expect(panelOrder(["browser", "files"], null)).toEqual(["browser", "files"]);
    expect(panelOrder(["files"], "browser")).toEqual(["files"]);
  });
  it("the editor's commands fall to the other panel on screen", () => {
    const editor = (p: PanelId) => (p === "files" ? "editor" : null);
    expect(firstWith(panelOrder(["browser", "files"], "browser"), editor)).toBe("editor");
    expect(firstWith(["browser"], editor)).toBeNull();
  });
});

describe("panelButtonsFor", () => {
  const keys = (p: PanelId) => (p === "browser" ? "Ctrl+Shift+A" : "Ctrl+Shift+D");
  it("a button is shown for a panel on screen", () => {
    expect(panelButtonsFor(["files"], new Set(), keys)).toEqual([
      { id: "browser", label: "Browser", shown: false, news: false, keys: "Ctrl+Shift+A" },
      { id: "files", label: "Files", shown: true, news: false, keys: "Ctrl+Shift+D" },
    ]);
  });
  it("news is per panel", () => {
    const b = panelButtonsFor(["files"], new Set(["browser:s1"]), keys);
    expect(b.map((x) => x.news)).toEqual([true, false]);
  });
  it("with nothing on screen no button is shown", () => {
    expect(panelButtonsFor([], new Set(), keys).every((b) => !b.shown)).toBe(true);
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
    expect(closeTabOutcome(EMPTY, "s1", "files")).toBeNull();
    const s = withTabs("s1", ["/a/x.md", "/a/y.md"]);
    expect(closeTabOutcome(s, "s1", "files")).toEqual({ paneTab: "t2", path: "/a/y.md" });
    expect(closeTabOutcome(s, "s1", null)).toBeNull();
    expect(closeTabOutcome(s, "s1", "browser")).toBeNull();
  });
});

describe("closeNothingText", () => {
  it("says the pane is hidden, with its key, when the session has tabs behind a hidden pane", () => {
    const hidden = withTabs("s1", ["/a/x.md"], false);
    expect(closeNothingText(hidden, "s1", null, false, "Ctrl+Shift+Q")).toBe("The side pane is hidden. Ctrl+Shift+Q shows it.");
    // Another session has the tabs, or none has: there is nothing to close.
    expect(closeNothingText(hidden, "s2", null, false, "Ctrl+Shift+Q")).toBe("No tabs open.");
    expect(closeNothingText(EMPTY, "s1", null, false, "Ctrl+Shift+Q")).toBe("No tabs open.");
    expect(closeNothingText(hidden, undefined, null, false, "Ctrl+Shift+Q")).toBe("No tabs open.");
  });
  it("says what a tab command says in a pane that is shown, or behind a view that replaces the terminal", () => {
    const shown = withTabs("s1", ["/a/x.md"]);
    expect(closeNothingText(shown, "s1", "browser", false, "Ctrl+Shift+Q")).toBe("No tabs open in Browser.");
    expect(closeNothingText(withTabs("s1", ["/a/x.md"], false), "s1", null, true, "Ctrl+Shift+Q")).toBe("The side pane shows beside a session.");
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
    expect(pageOpenedText("https://example.com/a")).toBe("Opened example.com in Browser.");
    expect(pageOpenedText("http://localhost:3000/")).toBe("Opened localhost:3000 in Browser.");
    expect(pageOpenedText("about:blank")).toBe("Opened New tab in Browser.");
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

it("says the hidden line after the cursor's receipt, in one notice, so neither replaces the other", () => {
  expect(receiptWithPaneHidden("Selected session 2, api", "Ctrl+Shift+Q")).toBe("Selected session 2, api. The side pane is hidden. Ctrl+Shift+Q shows it.");
});

describe("shouldPin", () => {
  const open: PaneView = { ...EMPTY, open: true };
  it("nothing is pinned with no session on screen", () => {
    expect(shouldPin(open, null, true)).toBe(false);
    expect(shouldPin(open, undefined, true)).toBe(false);
  });
  it("an absent stack with the pane open and a session on screen is pinned, once everything has loaded", () => {
    expect(shouldPin(open, "t1", true)).toBe(true);
    expect(shouldPin(open, "t1", false)).toBe(false);
  });
  it("a saved stack is never pinned again", () => {
    expect(shouldPin({ ...open, panels: { shown: ["files"], weights: {}, filled: null } }, "t1", true)).toBe(false);
  });
  it("a hidden pane is not pinned", () => {
    expect(shouldPin(EMPTY, "t1", true)).toBe(false);
  });
});

describe("unseenPanels", () => {
  const bg = (v: PaneView, target: string, kind: "file" | "browser" = "file") => paneReducer(v, { type: "openBackground", session: "s1", kind, target, by: "claude", project: "p", onScreen: false });
  it("names only the panels on screen whose selected tab is unseen", () => {
    const v = bg(bg(EMPTY, "/site/a.html"), "/notes/a.md");
    expect(unseenPanels(v, "s1", ["browser", "files"])).toEqual(["browser", "files"]);
    expect(unseenPanels(v, "s1", ["files"])).toEqual(["files"]);
    expect(unseenPanels(v, "s1", [])).toEqual([]);
    expect(unseenPanels(v, "s2", ["browser", "files"])).toEqual([]);
  });
});

describe("landingPanel", () => {
  it("is the panel of the entry a back or forward step lands on, and null at the ends", () => {
    const base = withTabs("s1", ["/notes/a.md"]).sessions.s1!.tabs[0]!;
    // An old layout's history can cross panels; a navigate never makes one.
    const tab = { ...base, history: [base.history[0]!, { ...base.history[0]!, path: "/site/index.html" }], index: 1 };
    expect(landingPanel(tab, -1)).toBe("files");
    expect(landingPanel(tab, 1)).toBeNull();
    expect(landingPanel({ ...tab, index: 0 }, 1)).toBe("browser");
    expect(landingPanel({ ...tab, index: 0 }, -1)).toBeNull();
  });
});

describe("noTabText", () => {
  it("names the panel that has no tabs, or says no tabs are open", () => {
    expect(noTabText("files", false)).toBe(noTabsText("files"));
    expect(noTabText("browser", false)).toBe(noTabsText("browser"));
    expect(noTabText(null, false)).toBe("No tabs open.");
  });
  it("says where the pane shows while a view replaces the terminal: the session may have tabs", () => {
    expect(noTabText(null, true)).toBe("The side pane shows beside a session.");
    expect(noTabText("files", true)).toBe("The side pane shows beside a session.");
  });
});
