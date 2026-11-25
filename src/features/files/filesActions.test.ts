import { describe, expect, it } from "vitest";
import type { PaneTab } from "../../generated/PaneTab";
import { EDITOR_DEFAULTS } from "../editor/editorPrefs";
import { FILES_DEFAULTS } from "./filesSettings";
import { afterOpen, editorPrefsOf, filesCommandSteps, filterTakesKeyboard, treeAfter, followTarget, previewExtras, keepFirst, keyboardToFile, leavingSession, openHow, rowOpens, treeOpen } from "./filesActions";

const tab = (path: string, preview = true): PaneTab => ({ id: "t1", kind: "file", history: [{ path, title: null }], index: 0, ...(preview ? { preview: true } : {}) }) as unknown as PaneTab;

describe("treeOpen", () => {
  it("previews when the setting is on and the open isn't a keep", () => {
    expect(treeOpen({ previewTabs: true, keep: false })).toEqual({ preview: true });
  });
  it("never previews with the setting off", () => {
    expect(treeOpen({ previewTabs: false, keep: false })).toEqual({});
  });
  it("never previews a keep", () => {
    expect(treeOpen({ previewTabs: true, keep: true })).toEqual({});
  });
});

describe("keepFirst", () => {
  it("is true when the preview tab's file is unsaved", () => {
    expect(keepFirst(tab("C:\\p\\a.ts"), (p) => p === "C:\\p\\a.ts")).toBe(true);
  });
  it("is false when it is saved, or there is no preview tab", () => {
    expect(keepFirst(tab("C:\\p\\a.ts"), () => false)).toBe(false);
    expect(keepFirst(undefined, () => true)).toBe(false);
  });
});

describe("followTarget", () => {
  it("is the relative path inside the root", () => {
    expect(followTarget("C:\\p", "C:\\p\\src\\a.ts", true)).toBe("src/a.ts");
    expect(followTarget("/p", "/p/src/a.ts", false)).toBe("src/a.ts");
  });
  it("is null outside the root, with no root or no file, and for the root itself", () => {
    expect(followTarget("C:\\p", "C:\\other\\a.ts", true)).toBeNull();
    expect(followTarget(null, "/p/a.ts", false)).toBeNull();
    expect(followTarget("/p", null, false)).toBeNull();
    expect(followTarget("/p", "/p", false)).toBeNull();
  });
});

describe("leavingSession", () => {
  it("is true only for a select of another session", () => {
    expect(leavingSession({ type: "select", tabId: "b" }, "a")).toBe(true);
    expect(leavingSession({ type: "select", tabId: "a" }, "a")).toBe(false);
    expect(leavingSession({ type: "select", tabId: "a" }, undefined)).toBe(true);
    expect(leavingSession({ type: "toggleShell" }, "a")).toBe(false);
    expect(leavingSession({ type: "sessions", openIds: [] }, "a")).toBe(false);
  });
});

describe("rowOpens", () => {
  it("opens a file and nothing else: a link row never opens", () => {
    expect(rowOpens({ kind: "file" })).toBe(true);
    for (const kind of ["link", "folder", "more", "empty"] as const) expect(rowOpens({ kind })).toBe(false);
    expect(rowOpens(undefined)).toBe(false);
  });
});

describe("how an open was made", () => {
  it("Enter keeps, a click and Space preview", () => {
    expect(openHow(true, false)).toBe("enter");
    expect(openHow(false, true)).toBe("click");
    expect(openHow(false, false)).toBe("space");
  });
  it("only Enter and a double-click move the keyboard to the file", () => {
    expect(keyboardToFile("enter")).toBe(true);
    expect(keyboardToFile("double")).toBe(true);
    expect(keyboardToFile("click")).toBe(false);
    expect(keyboardToFile("space")).toBe(false);
  });
});

describe("the Preview of each panel", () => {
  const reveal = () => {};
  it("the Files panel's gets the settings, Auto save, the file manager and the path bar's header", () => {
    const prefs = editorPrefsOf({ ...FILES_DEFAULTS, word_wrap: true, tab_size: 2, auto_save: true });
    expect(prefs).toEqual({ ...EDITOR_DEFAULTS, word_wrap: true, tab_size: 2 });
    expect(previewExtras("files", { prefs, autoSave: true, onReveal: reveal })).toEqual({ prefs, autoSave: true, onReveal: reveal, pathShown: true });
  });
  it("the Browser panel's gets none of them", () => {
    expect(previewExtras("browser", { prefs: EDITOR_DEFAULTS, autoSave: true, onReveal: reveal })).toEqual({});
  });
  it("the settings not read yet are the defaults", () => {
    expect(editorPrefsOf(null)).toBe(EDITOR_DEFAULTS);
  });
});

describe("afterOpen", () => {
  it("Enter and a double-click move the keyboard to the file in Files", () => {
    expect(afterOpen("enter", { narrow: false, panel: "files" })).toEqual({ close: false, focus: "files" });
    expect(afterOpen("double", { narrow: false, panel: "files" })).toEqual({ close: false, focus: "files" });
  });
  it("a click and Space leave the keyboard in the wide tree", () => {
    expect(afterOpen("click", { narrow: false, panel: "files" })).toEqual({ close: false, focus: null });
    expect(afterOpen("space", { narrow: false, panel: "files" })).toEqual({ close: false, focus: null });
  });
  it("in the narrow layout every open shows the file; Space leaves the keyboard on the tree button, out of the editor", () => {
    expect(afterOpen("click", { narrow: true, panel: "files" })).toEqual({ close: true, focus: "files" });
    expect(afterOpen("enter", { narrow: true, panel: "files" })).toEqual({ close: true, focus: "files" });
    expect(afterOpen("double", { narrow: true, panel: "files" })).toEqual({ close: true, focus: "files" });
    expect(afterOpen("space", { narrow: true, panel: "files" })).toEqual({ close: true, focus: "tree-button" });
  });
  it("in the wide layout Space still leaves the tree and the keyboard as they are", () => {
    expect(afterOpen("space", { narrow: false, panel: "files" })).toEqual({ close: false, focus: null });
  });
  it("an HTML file opens in Browser: Enter moves the keyboard there and the narrow tree stays", () => {
    expect(afterOpen("enter", { narrow: true, panel: "browser" })).toEqual({ close: false, focus: "browser" });
    expect(afterOpen("click", { narrow: true, panel: "browser" })).toEqual({ close: false, focus: null });
  });
});

describe("filesCommandSteps", () => {
  it("Collapse all folders goes through a mounted panel, whose tree holds the live state, and shows a hidden one", () => {
    expect(filesCommandSteps("collapse", { mounted: true, shown: true })).toEqual({ via: "panel", showPanel: false });
    expect(filesCommandSteps("collapse", { mounted: true, shown: false })).toEqual({ via: "panel", showPanel: true });
  });
  it("Collapse all folders with no panel mounted changes the saved state and shows the panel", () => {
    expect(filesCommandSteps("collapse", { mounted: false, shown: false })).toEqual({ via: "saved", showPanel: true });
  });
  it("Show file tree toggles the tree of a panel on screen", () => {
    expect(filesCommandSteps("tree", { mounted: true, shown: true })).toEqual({ via: "panel", showPanel: false });
  });
  it("Show file tree on a hidden panel shows the panel with the tree on", () => {
    expect(filesCommandSteps("tree", { mounted: true, shown: false })).toEqual({ via: "saved", showPanel: true });
    expect(filesCommandSteps("tree", { mounted: false, shown: false })).toEqual({ via: "saved", showPanel: true });
  });
  it("never hides a panel on screen that has no panel mounted", () => {
    expect(filesCommandSteps("tree", { mounted: false, shown: true })).toEqual({ via: "saved", showPanel: false });
    expect(filesCommandSteps("collapse", { mounted: false, shown: true })).toEqual({ via: "saved", showPanel: false });
  });
});

describe("filterTakesKeyboard", () => {
  it("moves the keyboard to the filter box only when the narrow tree opens over a file that holds it (review 6)", () => {
    expect(filterTakesKeyboard({ narrow: true, opening: true, focusInFile: true })).toBe(true);
    expect(filterTakesKeyboard({ narrow: true, opening: true, focusInFile: false })).toBe(false);
    expect(filterTakesKeyboard({ narrow: true, opening: false, focusInFile: true })).toBe(false);
    expect(filterTakesKeyboard({ narrow: false, opening: true, focusInFile: true })).toBe(false);
  });
});
describe("treeAfter", () => {
  const at = (path: string | null, narrow = true) => ({ path, narrow });
  it("the last file closing brings the narrow tree back", () => {
    expect(treeAfter(false, at("C:\p\a.ts"), at(null))).toBe(true);
  });
  it("a file opened from outside the tree takes the tree's place (review 6w: Go to file left it hidden under the tree)", () => {
    expect(treeAfter(true, at(null), at("C:\p\a.ts"))).toBe(false);
    expect(treeAfter(true, at("C:\p\a.ts"), at("C:\p\b.ts"))).toBe(false);
    // Also in the wide layout, so the tree isn't left up for when the panel narrows.
    expect(treeAfter(true, at(null, false), at("C:\p\a.ts", false))).toBe(false);
  });
  it("a panel that narrows shows its file, and the tree only when there is no file (review 6w: a leftover tree covered the file and dropped the keyboard)", () => {
    expect(treeAfter(true, at("C:\p\a.ts", false), at("C:\p\a.ts", true))).toBe(false);
    expect(treeAfter(false, at(null, false), at(null, true))).toBe(true);
  });
  it("is as it was when neither the file nor the layout changed, and when the panel widens", () => {
    expect(treeAfter(true, at("C:\p\a.ts"), at("C:\p\a.ts"))).toBe(true);
    expect(treeAfter(false, at(null), at(null))).toBe(false);
    expect(treeAfter(true, at("C:\p\a.ts", true), at("C:\p\a.ts", false))).toBe(true);
  });
});
