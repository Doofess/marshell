import { describe, expect, it } from "vitest";
import { dragIndex, stripKey } from "./stripKeys";

const ids = ["a", "b", "c"];
const key = (k: string, m: Partial<{ altKey: boolean; shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }> = {}) => ({ key: k, altKey: false, shiftKey: false, ctrlKey: false, metaKey: false, ...m });

describe("stripKey", () => {
  it("Right and Left select the neighbour and wrap", () => {
    expect(stripKey(key("ArrowRight"), ids, "a", false)).toEqual({ type: "select", id: "b" });
    expect(stripKey(key("ArrowRight"), ids, "c", false)).toEqual({ type: "select", id: "a" });
    expect(stripKey(key("ArrowLeft"), ids, "a", false)).toEqual({ type: "select", id: "c" });
    expect(stripKey(key("ArrowLeft"), ids, "b", false)).toEqual({ type: "select", id: "a" });
  });
  it("RTL swaps them", () => {
    expect(stripKey(key("ArrowRight"), ids, "b", true)).toEqual({ type: "select", id: "a" });
    expect(stripKey(key("ArrowLeft"), ids, "b", true)).toEqual({ type: "select", id: "c" });
    expect(stripKey(key("ArrowRight", { altKey: true, shiftKey: true }), ids, "b", true)).toEqual({ type: "move", id: "b", to: 0 });
  });
  it("Home and End select the ends", () => {
    expect(stripKey(key("Home"), ids, "b", false)).toEqual({ type: "select", id: "a" });
    expect(stripKey(key("End"), ids, "b", false)).toEqual({ type: "select", id: "c" });
  });
  it("Delete closes the focused tab", () => {
    expect(stripKey(key("Delete"), ids, "b", false)).toEqual({ type: "close", id: "b" });
  });
  it("Alt+Shift+Right moves it one place and stops at the end", () => {
    const m = { altKey: true, shiftKey: true };
    expect(stripKey(key("ArrowRight", m), ids, "a", false)).toEqual({ type: "move", id: "a", to: 1 });
    expect(stripKey(key("ArrowLeft", m), ids, "b", false)).toEqual({ type: "move", id: "b", to: 0 });
    expect(stripKey(key("ArrowRight", m), ids, "c", false)).toBeNull();
    expect(stripKey(key("ArrowLeft", m), ids, "a", false)).toBeNull();
  });
  it("any other key, or a key with Ctrl or Meta, is not the strip's", () => {
    expect(stripKey(key("x"), ids, "a", false)).toBeNull();
    expect(stripKey(key("ArrowRight", { ctrlKey: true }), ids, "a", false)).toBeNull();
    expect(stripKey(key("Delete", { metaKey: true }), ids, "a", false)).toBeNull();
    expect(stripKey(key("ArrowRight", { altKey: true }), ids, "a", false)).toBeNull();
    expect(stripKey(key("ArrowRight", { shiftKey: true }), ids, "a", false)).toBeNull();
    expect(stripKey(key("ArrowRight"), ids, "zzz", false)).toBeNull();
  });
});

describe("dragIndex", () => {
  const rects = [
    { left: 0, right: 100 },
    { left: 100, right: 200 },
    { left: 200, right: 300 },
  ];
  it("moves a tab once the pointer passes a neighbour's middle, and not before", () => {
    expect(dragIndex(rects, 120, 0)).toBe(0);
    expect(dragIndex(rects, 150, 0)).toBe(0);
    expect(dragIndex(rects, 151, 0)).toBe(1);
    expect(dragIndex(rects, 251, 0)).toBe(2);
    expect(dragIndex(rects, 49, 1)).toBe(0);
    expect(dragIndex(rects, 50, 1)).toBe(1);
    expect(dragIndex(rects, 160, 2)).toBe(2);
    expect(dragIndex(rects, 149, 2)).toBe(1);
  });
  it("works on rects measured right to left", () => {
    const rtl = [
      { left: 200, right: 300 },
      { left: 100, right: 200 },
      { left: 0, right: 100 },
    ];
    expect(dragIndex(rtl, 149, 0)).toBe(1);
    expect(dragIndex(rtl, 151, 0)).toBe(0);
    expect(dragIndex(rtl, 40, 0)).toBe(2);
  });
});
