import { describe, expect, it } from "vitest";
import { isAction, type MenuItem, type MenuRow } from "../../components/ContextMenu/menuKeys";
import type { FilesSettings } from "../../generated/FilesSettings";
import { filesMenuChoice, filesMenuItems, filesSaid } from "./filesMenu";

const settings: FilesSettings = {
  preview_tabs: true,
  auto_save: false,
  tab_indents: true,
  word_wrap: false,
  indent_guides: true,
  whitespace: false,
  scroll_past_end: true,
  tab_size: 4,
  tree_position: "left",
};
const rows = (items: MenuItem[]): MenuRow[] => items.filter(isAction);
const find = (items: MenuItem[], label: string): MenuRow => {
  const r = rows(items).find((i) => i.label === label);
  if (!r) throw new Error(`no row ${label}`);
  return r;
};
const read = (o: Partial<Parameters<typeof filesMenuItems>[0]> = {}) =>
  filesMenuItems({ settings, failed: false, treeShown: true, os: "windows", ...o });
const SETTING_LABELS = ["Preview tabs", "Auto save", "Tab key indents", "Word wrap", "Indent guides", "Show whitespace", "Scroll past end"];

describe("filesMenuItems", () => {
  it("lists the rows in order, with a rule between the groups", () => {
    expect(read().map((i) => ("separator" in i ? "-" : i.label))).toEqual([
      "Go to file…",
      "-",
      "Show file tree",
      "File tree position",
      "-",
      "Collapse all folders",
      "-",
      "Preview tabs",
      "Auto save",
      "Tab key indents",
      "-",
      "Word wrap",
      "Indent guides",
      "Show whitespace",
      "Scroll past end",
      "Tab size",
    ]);
  });
  it("Go to file… shows Ctrl+Shift+P, and ⌘P on macOS", () => {
    expect(find(read(), "Go to file…").shortcut).toBe("Ctrl+Shift+P");
    expect(find(read({ os: "mac" }), "Go to file…").shortcut).toBe("⌘P");
  });
  it("checks the rows from the settings", () => {
    const items = read();
    const on = ["Preview tabs", "Tab key indents", "Indent guides", "Scroll past end"];
    for (const l of SETTING_LABELS) expect(find(items, l).checked, l).toBe(on.includes(l));
  });
  it("Show file tree is checked by whether the tree is shown, even before the settings are read", () => {
    expect(find(read({ treeShown: true }), "Show file tree").checked).toBe(true);
    expect(find(read({ treeShown: false, settings: null }), "Show file tree").checked).toBe(false);
  });
  it("the position and the tab size are radio submenus", () => {
    const pos = find(read(), "File tree position");
    expect(pos.rows?.filter(isAction).map((r) => [r.label, r.checked, r.radio])).toEqual([
      ["Automatic", false, true],
      ["Left", true, true],
      ["Right", false, true],
    ]);
    const size = find(read(), "Tab size");
    expect(size.rows?.filter(isAction).map((r) => [r.label, r.checked, r.radio])).toEqual([
      ["2", false, true],
      ["4", true, true],
      ["8", false, true],
    ]);
  });
  it("until the settings are read no setting row is checked and none has a note", () => {
    const items = read({ settings: null });
    for (const l of SETTING_LABELS) {
      expect(find(items, l).checked, l).toBeUndefined();
      expect(find(items, l).note, l).toBeUndefined();
    }
    expect(find(items, "File tree position").rows?.filter(isAction).every((r) => r.checked === undefined)).toBe(true);
    expect(find(items, "Tab size").rows?.filter(isAction).every((r) => r.checked === undefined)).toBe(true);
  });
  it("a failed read says so on the setting rows", () => {
    const items = read({ settings: null, failed: true });
    for (const l of [...SETTING_LABELS, "Tab size"]) expect(find(items, l).note, l).toBe("Your settings couldn't be read");
    expect(find(items, "Go to file…").note).toBeUndefined();
    expect(find(items, "Collapse all folders").note).toBeUndefined();
  });
});

describe("filesMenuChoice", () => {
  it("reads every id the menu holds", () => {
    const all: MenuRow[] = [];
    const walk = (items: MenuItem[]) => rows(items).forEach((r) => (r.rows ? walk(r.rows) : all.push(r)));
    walk(read());
    expect(all).toHaveLength(16);
    for (const r of all) expect(filesMenuChoice(r.id), r.label).not.toBeNull();
  });
  it("maps the ids to their choices", () => {
    const id = (label: string, sub?: string) => {
      const top = find(read(), sub ?? label);
      return (sub ? find(top.rows!, label) : top).id;
    };
    expect(filesMenuChoice(id("Go to file…"))).toEqual({ kind: "goToFile" });
    expect(filesMenuChoice(id("Show file tree"))).toEqual({ kind: "tree" });
    expect(filesMenuChoice(id("Left", "File tree position"))).toEqual({ kind: "position", position: "left" });
    expect(filesMenuChoice(id("Automatic", "File tree position"))).toEqual({ kind: "position", position: "automatic" });
    expect(filesMenuChoice(id("Right", "File tree position"))).toEqual({ kind: "position", position: "right" });
    expect(filesMenuChoice(id("Collapse all folders"))).toEqual({ kind: "collapse" });
    const keys = { "Preview tabs": "preview_tabs", "Auto save": "auto_save", "Tab key indents": "tab_indents", "Word wrap": "word_wrap", "Indent guides": "indent_guides", "Show whitespace": "whitespace", "Scroll past end": "scroll_past_end" };
    for (const [label, key] of Object.entries(keys)) expect(filesMenuChoice(id(label)), label).toEqual({ kind: "setting", key });
    expect(filesMenuChoice(id("2", "Tab size"))).toEqual({ kind: "tabSize", size: 2 });
    expect(filesMenuChoice(id("4", "Tab size"))).toEqual({ kind: "tabSize", size: 4 });
    expect(filesMenuChoice(id("8", "Tab size"))).toEqual({ kind: "tabSize", size: 8 });
  });
  it("is null for an id it doesn't know", () => {
    for (const bad of ["nope", "tabsize:3", "position:top", "setting:other", "file-tree-position"]) expect(filesMenuChoice(bad), bad).toBeNull();
  });
});

describe("filesSaid", () => {
  it("says what changed", () => {
    expect(filesSaid({ kind: "tree" }, false)).toBe("Show file tree: off");
    expect(filesSaid({ kind: "tree" }, true)).toBe("Show file tree: on");
    expect(filesSaid({ kind: "position", position: "left" })).toBe("File tree position: Left");
    expect(filesSaid({ kind: "position", position: "automatic" })).toBe("File tree position: Automatic");
    expect(filesSaid({ kind: "setting", key: "auto_save" }, true)).toBe("Auto save: on");
    expect(filesSaid({ kind: "setting", key: "tab_indents" }, false)).toBe("Tab key indents: off");
    expect(filesSaid({ kind: "setting", key: "whitespace" }, true)).toBe("Show whitespace: on");
    expect(filesSaid({ kind: "tabSize", size: 2 })).toBe("Tab size: 2");
  });
});
