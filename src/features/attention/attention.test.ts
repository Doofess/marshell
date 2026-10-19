import { describe, expect, it } from "vitest";
import { attentionStep } from "./attention";

const e = (id: string, tab = "t1") => ({ id, tab });

describe("attentionStep", () => {
  it("alerts for a new entry while the window is not focused", () => {
    expect(attentionStep([], [e("a")], false, new Set())).toEqual({ count: 1, alert: true });
  });
  it("does not alert while focused", () => {
    expect(attentionStep([], [e("a")], true, new Set())).toEqual({ count: 1, alert: false });
  });
  it("does not alert for a muted tab", () => {
    expect(attentionStep([], [e("a", "t1")], false, new Set(["t1"]))).toEqual({ count: 1, alert: false });
  });
  it("does not alert for the same entries again", () => {
    expect(attentionStep(["a"], [e("a")], false, new Set())).toEqual({ count: 1, alert: false });
  });
  it("alerts when one entry of several is new", () => {
    expect(attentionStep(["a"], [e("a"), e("b", "t2")], false, new Set(["t1"]))).toEqual({ count: 2, alert: true });
  });
  it("counts follow the entries", () => {
    expect(attentionStep(["a", "b"], [], false, new Set())).toEqual({ count: 0, alert: false });
  });
});
