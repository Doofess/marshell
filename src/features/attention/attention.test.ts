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
    expect(attentionStep([e("a")], [e("a")], false, new Set())).toEqual({ count: 1, alert: false });
  });
  it("alerts when one entry of several is new", () => {
    expect(attentionStep([e("a")], [e("a"), e("b", "t2")], false, new Set(["t1"]))).toEqual({ count: 2, alert: true });
  });
  it("does not alert again when a tab's entry changes id, as when its held request replaces the waiting row (live check, 3d)", () => {
    expect(attentionStep([e("terminal-t1")], [e("p1")], false, new Set())).toEqual({ count: 1, alert: false });
  });
  it("alerts for a new request on a tab whose earlier one was answered", () => {
    expect(attentionStep([], [e("p2")], false, new Set())).toEqual({ count: 1, alert: true });
  });
  it("counts follow the entries", () => {
    expect(attentionStep([e("a"), e("b", "t2")], [], false, new Set())).toEqual({ count: 0, alert: false });
  });
});
