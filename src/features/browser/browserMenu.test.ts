import { describe, expect, it } from "vitest";
import { browserMenuChoice, browserMenuItems, menuTarget } from "./browserMenu";
import type { MenuItem } from "../../components/ContextMenu/ContextMenu";
import type { PaneTab } from "../../generated/PaneTab";

type Row = Extract<MenuItem, { id: string }>;
const rows = (items: MenuItem[]) => items.filter((i): i is Row => "id" in i);
const row = (items: MenuItem[], label: string) => rows(items).find((r) => r.label === label)!;
const page = { kind: "page", url: "https://example.com/" } as const;

describe("browserMenuItems", () => {
  it("lists the four rows in order, with two separators", () => {
    const items = browserMenuItems(page, true);
    expect(items.map((i) => ("separator" in i ? "-" : i.label))).toEqual(["Copy link", "Open in the system browser", "-", "Open HTML file…", "-", "Clear browsing data…"]);
  });
  it("a page enables Copy link and Open in the system browser", () => {
    const items = browserMenuItems(page, true);
    expect(row(items, "Copy link").disabled).toBeUndefined();
    expect(row(items, "Open in the system browser").disabled).toBeUndefined();
  });
  it("a blank tab and no tab say No page is open", () => {
    for (const t of [{ kind: "blank" }, { kind: "none" }] as const) {
      const items = browserMenuItems(t, true);
      expect(row(items, "Copy link").disabled).toBe("No page is open");
      expect(row(items, "Open in the system browser").disabled).toBe("No page is open");
    }
  });
  it("an HTML file says it has no address", () => {
    const items = browserMenuItems({ kind: "file" }, true);
    expect(row(items, "Copy link").disabled).toBe("An HTML file has no address");
    expect(row(items, "Open in the system browser").disabled).toBe("An HTML file has no address");
  });
  it("Open HTML file… is never disabled", () => {
    for (const t of [{ kind: "none" }, { kind: "blank" }, { kind: "file" }, page] as const) {
      for (const avail of [true, false]) expect(row(browserMenuItems(t, avail), "Open HTML file…").disabled).toBeUndefined();
    }
  });
  it("Clear browsing data… says why where pages aren't available", () => {
    expect(row(browserMenuItems(page, false), "Clear browsing data…").disabled).toBe("Not available on this system yet");
    expect(row(browserMenuItems(page, true), "Clear browsing data…").disabled).toBeUndefined();
  });
  it("a row's id is its choice, and an unknown id is null", () => {
    const items = rows(browserMenuItems(page, true));
    expect(items.map((r) => browserMenuChoice(r.id))).toEqual(["copy", "outside", "html", "clear"]);
    expect(browserMenuChoice("nope")).toBeNull();
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
