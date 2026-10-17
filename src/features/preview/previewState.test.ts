import { describe, expect, it } from "vitest";
import { INITIAL, currentOf, previewReducer, type PreviewAction, type PreviewEntry, type PreviewState } from "./previewState";

const e = (path: string, view: PreviewEntry["view"] = "rendered"): PreviewEntry => ({ path, view });
const run = (...actions: PreviewAction[]): PreviewState => actions.reduce(previewReducer, INITIAL);

describe("previewReducer", () => {
  it("starts closed with no history", () => {
    expect(INITIAL.open).toBe(false);
    expect(currentOf(INITIAL)).toBeUndefined();
  });
  it("opens an entry and shows it", () => {
    const s = run({ type: "open", entry: e("a") });
    expect(s.open).toBe(true);
    expect(currentOf(s)?.path).toBe("a");
  });
  it("goes back to a and forward to b", () => {
    const ab = run({ type: "open", entry: e("a") }, { type: "open", entry: e("b") });
    const back = previewReducer(ab, { type: "back" });
    expect(currentOf(back)?.path).toBe("a");
    expect(currentOf(previewReducer(back, { type: "forward" }))?.path).toBe("b");
  });
  it("stops at either end of the history", () => {
    const a = run({ type: "open", entry: e("a") });
    expect(previewReducer(a, { type: "back" })).toBe(a);
    expect(previewReducer(a, { type: "forward" })).toBe(a);
  });
  it("drops the forward history when a new file is opened after a Back", () => {
    let s = run({ type: "open", entry: e("a") }, { type: "open", entry: e("b") }, { type: "back" }, { type: "open", entry: e("c") });
    expect(s.entries.map((x) => x.path)).toEqual(["a", "c"]);
    s = previewReducer(s, { type: "forward" });
    expect(currentOf(s)?.path).toBe("c");
  });
  it("keeps the history when it closes, so the toggle reopens the last file", () => {
    const closed = run({ type: "open", entry: e("a") }, { type: "open", entry: e("c") }, { type: "close" });
    expect(closed.open).toBe(false);
    const reopened = previewReducer(closed, { type: "toggle" });
    expect(reopened.open).toBe(true);
    expect(currentOf(reopened)?.path).toBe("c");
    expect(previewReducer(reopened, { type: "toggle" }).open).toBe(false);
  });
  it("has nothing to toggle open before any file was opened", () => {
    expect(previewReducer(INITIAL, { type: "toggle" })).toBe(INITIAL);
  });
  it("changes only the current entry's view", () => {
    const s = run({ type: "open", entry: e("a") }, { type: "open", entry: e("b") }, { type: "setView", view: "source" });
    expect(s.entries.map((x) => x.view)).toEqual(["rendered", "source"]);
  });
  it("shows a file opened while closed", () => {
    const s = run({ type: "open", entry: e("a") }, { type: "close" }, { type: "open", entry: e("b") });
    expect(s.open).toBe(true);
    expect(currentOf(s)?.path).toBe("b");
  });
});
