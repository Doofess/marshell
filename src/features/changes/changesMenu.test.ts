import { describe, expect, it } from "vitest";
import { isAction, type MenuItem, type MenuRow } from "../../components/ContextMenu/menuKeys";
import { changesMenuItems, headerMenuItems, lineMenuItems, openLineOf, openOf } from "./changesMenu";
import type { Item, Row, Section } from "./model";

const section = (rows: Row[], status: Section["status"] = "modified"): Section => ({ path: "a.ts", status, added: 1, removed: 1, rows, folded: false });
const rowItem = (file: Section, at: number): Item => ({ kind: "row", key: `row:${file.path}:${at}`, file, at });
const rowsOf = (items: MenuItem[]): MenuRow[] => items.filter(isAction);
const labels = (items: MenuItem[]) => items.map((i) => (isAction(i) ? i.label : "-"));
const on = { settings: { hide_whitespace: true, wrap: false, pull_requests: true }, failed: false, review: "Ask Claude Code to review (types /code-review)", hasFiles: true };

describe("changesMenuItems", () => {
  it("leaves review out for a plain shell", () => {
    expect(labels(changesMenuItems({ ...on, review: null }))).toEqual(["Refresh", "-", "Fold all files", "Unfold all files", "-", "Hide whitespace changes", "Wrap long lines"]);
    expect(labels(changesMenuItems(on))[1]).toBe(on.review);
  });
  it("makes the switches checked rows", () => {
    const rows = rowsOf(changesMenuItems(on));
    expect(rows.find((r) => r.id === "setting:hide_whitespace")?.checked).toBe(true);
    expect(rows.find((r) => r.id === "setting:wrap")?.checked).toBe(false);
  });
  it("disables the switches while the settings are unread and reads defaults after a failed read", () => {
    const loading = rowsOf(changesMenuItems({ ...on, settings: null }));
    expect(loading.find((r) => r.id === "setting:wrap")?.disabled).toBeTruthy();
    const failed = rowsOf(changesMenuItems({ ...on, settings: null, failed: true }));
    expect(failed.find((r) => r.id === "setting:wrap")?.checked).toBe(false);
    expect(failed.find((r) => r.id === "setting:wrap")?.disabled).toBeTruthy();
  });
  it("disables folding all without files", () => {
    const rows = rowsOf(changesMenuItems({ ...on, hasFiles: false }));
    expect(rows.find((r) => r.id === "fold-all")?.disabled).toBeTruthy();
    expect(rows.find((r) => r.id === "unfold-all")?.disabled).toBeTruthy();
    expect(rows.find((r) => r.id === "refresh")?.disabled).toBeUndefined();
  });
});

describe("lineMenuItems", () => {
  const file = section([
    { kind: "hunk", text: "@@ -1,2 +1,2 @@" },
    { kind: "del", text: "const a", oldNo: 1 },
    { kind: "add", text: "let a", newNo: 1 },
    { kind: "context", text: "b", oldNo: 2, newNo: 2 },
  ]);
  it("disables Comment on a hunk", () => {
    expect(rowsOf(lineMenuItems(rowItem(file, 0))).find((i) => i.label === "Comment")?.disabled).toBeTruthy();
    expect(rowsOf(lineMenuItems(rowItem(file, 2))).find((i) => i.label === "Comment")?.disabled).toBeUndefined();
  });
  it("names the copy by how many lines", () => {
    expect(labels(lineMenuItems(rowItem(file, 2)))).toEqual(["Comment", "Open in Files at this line", "Copy line"]);
    expect(labels(lineMenuItems(rowItem(file, 2), 3))[2]).toBe("Copy lines");
  });
  it("cannot open a deleted file", () => {
    const gone = section(file.rows, "deleted");
    expect(rowsOf(lineMenuItems(rowItem(gone, 2))).find((i) => i.id === "open")?.disabled).toBeTruthy();
  });
});

describe("openLineOf", () => {
  const rows: Row[] = [
    { kind: "hunk", text: "@@" },
    { kind: "context", text: "x", oldNo: 4, newNo: 4 },
    { kind: "del", text: "gone", oldNo: 5 },
    { kind: "add", text: "new", newNo: 5 },
    { kind: "del", text: "last", oldNo: 6 },
  ];
  it("gives a row its own new line", () => expect(openLineOf(rows, 3)).toBe(5));
  it("gives a removed row the next new line, else the one before", () => {
    expect(openLineOf(rows, 2)).toBe(5);
    expect(openLineOf(rows, 4)).toBe(5);
  });
  it("gives a hunk the first new line after it", () => expect(openLineOf(rows, 0)).toBe(4));
  it("has no line when no row has one", () => expect(openLineOf([{ kind: "del", text: "x", oldNo: 1 }], 0)).toBeUndefined());
});

describe("openOf", () => {
  const rows: Row[] = [
    { kind: "hunk", text: "@@" },
    { kind: "del", text: "gone", oldNo: 5 },
    { kind: "add", text: "new", newNo: 5 },
  ];
  const file = section(rows);
  it("opens what the menu's Open opens: a header at no line, a row at its line or a removed row's next one", () => {
    expect(openOf({ kind: "file", key: "file:a.ts", file })).toEqual({ file, line: undefined });
    expect(openOf(rowItem(file, 1))).toEqual({ file, line: 5 });
  });
  it("refuses a deleted file with the menu's reason, from its header or a row", () => {
    const deleted = section(rows, "deleted");
    expect(openOf({ kind: "file", key: "file:a.ts", file: deleted })).toEqual({ refused: "The file is deleted, so there is nothing to open." });
    expect(openOf(rowItem(deleted, 1))).toEqual({ refused: "The file is deleted, so there is nothing to open." });
  });
  it("opens nothing for a missing item", () => expect(openOf(undefined)).toBeNull());
});

describe("headerMenuItems", () => {
  it("offers Open in Files, Fold or Unfold, and Copy path", () => {
    const open = section([{ kind: "add", text: "x", newNo: 1 }]);
    expect(labels(headerMenuItems(open))).toEqual(["Open in Files", "Fold", "Copy path"]);
    expect(labels(headerMenuItems({ ...open, folded: true }))[1]).toBe("Unfold");
  });
  it("cannot open a deleted file", () => {
    expect(rowsOf(headerMenuItems(section([], "deleted"))).find((i) => i.id === "open")?.disabled).toBeTruthy();
  });
});
