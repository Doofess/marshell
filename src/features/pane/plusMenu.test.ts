import { describe, expect, it } from "vitest";
import { plusChoice, plusMenuItems } from "./plusMenu";

const recent = ["C:\\work\\app\\README.md", "/home/me/notes.txt"];

describe("plusMenuItems", () => {
  it('the first row is "New browser tab"', () => {
    const items = plusMenuItems(recent, true);
    expect(items[0]).toMatchObject({ label: "New browser tab" });
    expect(items[0]).not.toHaveProperty("disabled");
    expect(items[1]).toMatchObject({ label: "Open file…" });
    expect(items[2]).toMatchObject({ label: "Browse…" });
  });
  it("where browser tabs aren't available the row is disabled with its hint", () => {
    expect(plusMenuItems(recent, false)[0]).toMatchObject({ label: "New browser tab", disabled: "Not available on this system yet" });
  });
  it("has the three items only when nothing was opened lately", () => {
    expect(plusMenuItems([], true)).toHaveLength(3);
  });
  it("lists a recent file by its name and folder", () => {
    const items = plusMenuItems(recent, true);
    expect(items).toHaveLength(5);
    expect(items[3]).toMatchObject({ label: "README.md — C:\\work\\app" });
    expect(items[4]).toMatchObject({ label: "notes.txt — /home/me" });
    expect(plusMenuItems(["a.txt"], true)[3]).toMatchObject({ label: "a.txt" });
  });
  it("gives every row its own id", () => {
    const ids = plusMenuItems(recent, true).map((i) => ("id" in i ? i.id : ""));
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("plusChoice", () => {
  it("maps each id back to its choice", () => {
    const ids = plusMenuItems(recent, true).map((i) => ("id" in i ? i.id : ""));
    expect(plusChoice(ids[0]!, recent)).toEqual({ kind: "browserTab" });
    expect(plusChoice(ids[1]!, recent)).toEqual({ kind: "openFile" });
    expect(plusChoice(ids[2]!, recent)).toEqual({ kind: "browse" });
    expect(plusChoice(ids[3]!, recent)).toEqual({ kind: "recent", path: recent[0] });
    expect(plusChoice(ids[4]!, recent)).toEqual({ kind: "recent", path: recent[1] });
  });
  it("is null for an unknown id and for a recent file no longer listed", () => {
    expect(plusChoice("nope", recent)).toBeNull();
    expect(plusChoice("recent:/gone.txt", recent)).toBeNull();
  });
});
