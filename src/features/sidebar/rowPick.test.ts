import { describe, expect, it } from "vitest";
import { selectsByCtrlSpace } from "./rowPick";

const key = (over: Partial<Parameters<typeof selectsByCtrlSpace>[0]>) => ({ key: " ", ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, target: null, ...over });

describe("selectsByCtrlSpace", () => {
  it("takes Ctrl+Space and nothing else", () => {
    expect(selectsByCtrlSpace(key({ ctrlKey: true }))).toBe(true);
    expect(selectsByCtrlSpace(key({ ctrlKey: true, altKey: true }))).toBe(false);
    expect(selectsByCtrlSpace(key({ ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(selectsByCtrlSpace(key({ metaKey: true }))).toBe(false);
    expect(selectsByCtrlSpace(key({}))).toBe(false);
    expect(selectsByCtrlSpace(key({ key: "a", ctrlKey: true }))).toBe(false);
  });
  it("leaves the key to the rename text box", () => {
    const target = { closest: (s: string) => (s === ".session-row__rename" ? {} : null) };
    expect(selectsByCtrlSpace(key({ ctrlKey: true, target }))).toBe(false);
  });
});
