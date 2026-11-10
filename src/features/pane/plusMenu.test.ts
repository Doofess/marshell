import { describe, expect, it } from "vitest";
import { plusChoice, plusMenuItems } from "./plusMenu";

const recent = ["C:\\work\\app\\README.md", "/home/me/notes.txt"];

describe("plusMenuItems", () => {
  it("starts with Open file… and Browse…", () => {
    const items = plusMenuItems(recent);
    expect(items[0]).toMatchObject({ label: "Open file…" });
    expect(items[1]).toMatchObject({ label: "Browse…" });
  });
  it("has the two items only when nothing was opened lately", () => {
    expect(plusMenuItems([])).toHaveLength(2);
  });
  it("lists a recent file by its name and folder", () => {
    const items = plusMenuItems(recent);
    expect(items).toHaveLength(4);
    expect(items[2]).toMatchObject({ label: "README.md — C:\\work\\app" });
    expect(items[3]).toMatchObject({ label: "notes.txt — /home/me" });
    expect(plusMenuItems(["a.txt"])[2]).toMatchObject({ label: "a.txt" });
  });
  it("gives every row its own id", () => {
    const ids = plusMenuItems(recent).map((i) => ("id" in i ? i.id : ""));
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("plusChoice", () => {
  it("maps each id back to its choice", () => {
    const ids = plusMenuItems(recent).map((i) => ("id" in i ? i.id : ""));
    expect(plusChoice(ids[0]!, recent)).toEqual({ kind: "openFile" });
    expect(plusChoice(ids[1]!, recent)).toEqual({ kind: "browse" });
    expect(plusChoice(ids[2]!, recent)).toEqual({ kind: "recent", path: recent[0] });
    expect(plusChoice(ids[3]!, recent)).toEqual({ kind: "recent", path: recent[1] });
  });
  it("is null for an unknown id and for a recent file no longer listed", () => {
    expect(plusChoice("nope", recent)).toBeNull();
    expect(plusChoice("recent:/gone.txt", recent)).toBeNull();
  });
});
