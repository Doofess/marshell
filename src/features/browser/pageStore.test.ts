import { describe, expect, it } from "vitest";
import { pageReducer, type PageState } from "./pageStore";

const state = (key: string, extra: Partial<{ url: string; title: string }> = {}) =>
  ({ type: "state", key, url: "https://a.test/", title: "A", canBack: false, canForward: true, loading: true, ...extra }) as const;
const empty: ReadonlyMap<string, PageState> = new Map();

describe("pageReducer", () => {
  it("makes the page from a state event", () => {
    const p = pageReducer(empty, state("k")).get("k")!;
    expect(p).toMatchObject({ url: "https://a.test/", title: "A", canForward: true, loading: true, icon: null, focused: false, status: null });
  });

  it("keeps an icon that came before any state", () => {
    const m = pageReducer(pageReducer(empty, { type: "icon", key: "k", src: "data:x" }), state("k"));
    expect(m.get("k")!.icon).toBe("data:x");
  });

  it("sets the status on refused and the next state event clears it", () => {
    const m = pageReducer(pageReducer(empty, state("k")), { type: "refused", key: "k" });
    expect(m.get("k")!.status).toBe("refused");
    expect(pageReducer(m, state("k")).get("k")!.status).toBeNull();
  });

  it("sets the status on failed and the next state event clears it", () => {
    const m = pageReducer(pageReducer(empty, state("k")), { type: "failed", key: "k" });
    expect(m.get("k")).toMatchObject({ status: "failed", loading: false });
    expect(pageReducer(m, state("k")).get("k")!.status).toBeNull();
  });

  it("ignores failed for a page it doesn't hold", () => {
    expect(pageReducer(empty, { type: "failed", key: "k" }).size).toBe(0);
  });

  it("empties the store on gone", () => {
    const m = pageReducer(pageReducer(empty, state("a")), state("b"));
    expect(pageReducer(m, { type: "gone" }).size).toBe(0);
  });

  it("removes one page on closed", () => {
    const m = pageReducer(pageReducer(empty, state("a")), state("b"));
    expect([...pageReducer(m, { type: "closed", key: "a" }).keys()]).toEqual(["b"]);
  });

  it("focuses one page at a time", () => {
    let m = pageReducer(pageReducer(empty, state("a")), state("b"));
    m = pageReducer(m, { type: "focus", key: "a", focused: true });
    m = pageReducer(m, { type: "focus", key: "b", focused: true });
    expect(m.get("a")!.focused).toBe(false);
    expect(m.get("b")!.focused).toBe(true);
    m = pageReducer(m, { type: "focus", key: "a", focused: false });
    expect(m.get("b")!.focused).toBe(true);
  });
});
