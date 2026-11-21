import { describe, expect, it } from "vitest";
import { browserMenuChoice, browserMenuItems, menuTarget, toggledText, type BrowserMenuState } from "./browserMenu";
import { BROWSER_DEFAULTS } from "./browserSettings";
import type { MenuItem } from "../../components/ContextMenu/ContextMenu";
import type { PaneTab } from "../../generated/PaneTab";
import type { MenuTarget } from "./browserMenu";

type Row = Extract<MenuItem, { id: string }>;
// Every id the menu has is present, so a lookup is never undefined.
type Rows = Record<"copy" | "shot" | "shot-full" | "outside" | "html" | "links" | "auto" | "viewport" | "cookies" | "clear", Row>;
const byId = (items: MenuItem[]): Rows => Object.fromEntries(items.filter((i): i is Row => "id" in i).map((r) => [r.id, r])) as Rows;
const state: BrowserMenuState = { pages: true, settings: BROWSER_DEFAULTS, failed: false, viewport: null, shot: null, autoNote: null };
const page: MenuTarget = { kind: "page", url: "https://example.com/" };
const NOT_YET = "Not available on this system yet";
const UNREAD = "Your settings couldn't be read";

describe("browserMenuItems", () => {
  it("lists the rows in the decided order", () => {
    expect(browserMenuItems(page, state).map((r) => ("separator" in r ? "—" : r.label))).toEqual([
      "Copy link",
      "Save screenshot",
      "Save full-page screenshot",
      "Open in the system browser",
      "—",
      "Open HTML file…",
      "—",
      "Open terminal links here",
      "Open HTML files agents write",
      "Viewport",
      "Cookies",
      "—",
      "Clear browsing data…",
    ]);
  });
  it("gives the rows their ids", () => {
    expect(Object.keys(byId(browserMenuItems(page, state)))).toEqual(["copy", "shot", "shot-full", "outside", "html", "links", "auto", "viewport", "cookies", "clear"]);
  });
  it("a page enables Copy link and Open in the system browser", () => {
    const rows = byId(browserMenuItems(page, state));
    expect(rows.copy.disabled).toBeUndefined();
    expect(rows.outside.disabled).toBeUndefined();
  });
  it("a blank tab and no tab say No page is open", () => {
    for (const t of [{ kind: "blank" }, { kind: "none" }] as const) {
      const rows = byId(browserMenuItems(t, state));
      expect(rows.copy.disabled).toBe("No page is open");
      expect(rows.outside.disabled).toBe("No page is open");
    }
  });
  it("an HTML file says it has no address", () => {
    const rows = byId(browserMenuItems({ kind: "file" }, state));
    expect(rows.copy.disabled).toBe("An HTML file has no address");
    expect(rows.outside.disabled).toBe("An HTML file has no address");
  });
  it("Open HTML file… is never disabled", () => {
    for (const t of [{ kind: "none" }, { kind: "blank" }, { kind: "file" }, page] as const) {
      for (const pages of [true, false]) expect(byId(browserMenuItems(t, { ...state, pages })).html.disabled).toBeUndefined();
    }
  });
  it("shows the two switches' state and the session's note", () => {
    const rows = byId(browserMenuItems(page, { ...state, settings: { ...BROWSER_DEFAULTS, terminal_links: false }, autoNote: "Not in this session: Codex's hooks aren't set up." }));
    expect(rows.links).toMatchObject({ checked: false });
    expect(rows.auto).toMatchObject({ checked: true, note: "Not in this session: Codex's hooks aren't set up." });
    expect(rows.auto.disabled).toBeUndefined();
  });
  it("holds the viewport and cookie rows as radio submenus with the current one checked", () => {
    const rows = byId(browserMenuItems(page, { ...state, viewport: "laptop", settings: { ...BROWSER_DEFAULTS, cookies: "until-quit" } }));
    expect(rows.viewport.rows!.filter((r) => "checked" in r && r.checked).map((r) => "id" in r && r.id)).toEqual(["viewport:laptop"]);
    expect(rows.cookies.rows!.filter((r) => "checked" in r && r.checked).map((r) => "id" in r && r.id)).toEqual(["cookies:until-quit"]);
  });
  it("turns rows off with their reasons", () => {
    expect(byId(browserMenuItems({ kind: "file" }, { ...state, shot: "Not for an HTML file" })).shot.disabled).toBe("Not for an HTML file");
    expect(byId(browserMenuItems({ kind: "file" }, state)).viewport.disabled).toBeUndefined();
    expect(byId(browserMenuItems({ kind: "none" }, state)).viewport.disabled).toBe("No tab is open");
    const off = byId(browserMenuItems(page, { ...state, pages: false, shot: NOT_YET }));
    expect([off.links.disabled, off.cookies.disabled, off.clear.disabled, off["shot-full"].disabled]).toEqual(Array(4).fill(NOT_YET));
    expect(off.auto.disabled).toBeUndefined();
  });
  it("shows nothing as chosen, and the failure, when the settings couldn't be read", () => {
    const rows = byId(browserMenuItems(page, { ...state, settings: null, failed: true }));
    expect([rows.links.checked, rows.links.disabled, rows.auto.checked, rows.auto.disabled]).toEqual([false, UNREAD, false, UNREAD]);
    expect(rows.cookies.rows!.every((r) => "checked" in r && r.checked === false && r.disabled === UNREAD)).toBe(true);
  });
  it("claims no error while the settings are still loading", () => {
    const rows = byId(browserMenuItems(page, { ...state, settings: null, failed: false }));
    expect([rows.links.checked, rows.links.disabled, rows.auto.checked, rows.auto.disabled]).toEqual([false, undefined, false, undefined]);
    expect(rows.cookies.rows!.every((r) => "checked" in r && r.checked === false && r.disabled === undefined)).toBe(true);
  });
  it("Clear browsing data… says why where pages aren't available", () => {
    expect(byId(browserMenuItems(page, { ...state, pages: false })).clear.disabled).toBe(NOT_YET);
    expect(byId(browserMenuItems(page, state)).clear.disabled).toBeUndefined();
  });
});

describe("browserMenuChoice", () => {
  it("reads a chosen row", () => {
    expect(browserMenuChoice("shot-full")).toEqual({ kind: "shot", full: true });
    expect(browserMenuChoice("shot")).toEqual({ kind: "shot", full: false });
    expect(browserMenuChoice("viewport:responsive")).toEqual({ kind: "viewport", viewport: null });
    expect(browserMenuChoice("viewport:phone")).toEqual({ kind: "viewport", viewport: "phone" });
    expect(browserMenuChoice("cookies:never")).toEqual({ kind: "cookies", rule: "never" });
    expect(browserMenuChoice("links")).toEqual({ kind: "links" });
    expect(browserMenuChoice("auto")).toEqual({ kind: "auto" });
    for (const k of ["copy", "outside", "html", "clear"]) expect(browserMenuChoice(k)).toEqual({ kind: k });
    expect(browserMenuChoice("viewport")).toBeNull();
    expect(browserMenuChoice("cookies")).toBeNull();
    expect(browserMenuChoice("nope")).toBeNull();
  });
  it("names a switch's new state", () => {
    expect(toggledText("Open terminal links here", true)).toBe("Open terminal links here: on");
    expect(toggledText("Open HTML files agents write", false)).toBe("Open HTML files agents write: off");
  });
});

describe("menuTarget", () => {
  const tab = (kind: "browser" | "file", path: string) => ({ id: "t", kind, index: 0, history: [{ path }] }) as unknown as PaneTab;
  it("reads the selected tab", () => {
    expect(menuTarget(undefined)).toEqual({ kind: "none" });
    expect(menuTarget(tab("browser", "about:blank"))).toEqual({ kind: "blank" });
    expect(menuTarget(tab("browser", "https://a.test/"))).toEqual({ kind: "page", url: "https://a.test/" });
    expect(menuTarget(tab("file", "C:\\x\\a.html"))).toEqual({ kind: "file" });
  });
});
