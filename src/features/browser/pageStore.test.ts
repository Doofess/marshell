import { describe, expect, it } from "vitest";
import { dispatchPage, openPage, pageIcon, pageReducer, type PageState } from "./pageStore";

const state = (key: string, extra: Partial<{ url: string; title: string }> = {}) =>
  ({ type: "state", key, url: "https://a.test/", title: "A", canBack: false, canForward: true, loading: true, ...extra }) as const;
const empty: ReadonlyMap<string, PageState> = new Map();
/** The store once the app has asked the shell for these pages. */
const asked = (...keys: string[]) => keys.reduce((m, key) => pageReducer(m, { type: "opening", key }), empty);

describe("pageReducer", () => {
  it("holds a page the app asked for, which is not loaded until it reports its state", () => {
    const m = asked("k");
    expect(m.get("k")).toMatchObject({ url: "", live: false, status: null });
    const p = pageReducer(m, state("k")).get("k")!;
    expect(p).toMatchObject({ url: "https://a.test/", title: "A", canForward: true, loading: true, icon: null, focused: false, status: null, live: true });
  });

  it("asking again for a page it holds changes nothing", () => {
    const m = pageReducer(asked("k"), state("k"));
    expect(pageReducer(m, { type: "opening", key: "k" })).toBe(m);
  });

  // The shell's events and its answers come on separate channels. A page's last report can come after the app closed it, and
  // must not bring it back: its tab would show a page the shell no longer has, and never open it again.
  it("drops what a page it doesn't hold reports", () => {
    const m = asked("other");
    expect(pageReducer(m, state("k"))).toBe(m);
    expect(pageReducer(m, { type: "icon", key: "k", src: "data:x" })).toBe(m);
    expect(pageReducer(m, { type: "refused", key: "k" })).toBe(m);
    expect(pageReducer(m, { type: "focus", key: "k", focused: true })).toBe(m);
    const closed = pageReducer(pageReducer(asked("k"), state("k")), { type: "closed", key: "k" });
    expect(pageReducer(closed, state("k")).has("k")).toBe(false);
  });

  // The shell refuses an address before it makes the page (one of the app's own origins): the tab says so, and has no page.
  it("a refused open sets the status and leaves the page not loaded", () => {
    const m = pageReducer(asked("k"), { type: "refused", key: "k" });
    expect(m.get("k")).toMatchObject({ status: "refused", live: false });
  });

  // A page reports about:blank before its first address has loaded. The bar keeps showing the tab's saved address meanwhile.
  it("leaves the address alone while a blank page is loading", () => {
    const m = pageReducer(asked("k"), { ...state("k"), url: "about:blank", title: "" });
    expect(m.get("k")).toMatchObject({ url: "", loading: true, live: true });
    expect(pageReducer(m, { ...state("k"), url: "about:blank", title: "", loading: false }).get("k")!.url).toBe("about:blank");
  });

  it("keeps an icon that came before any state", () => {
    const m = pageReducer(pageReducer(asked("k"), { type: "icon", key: "k", src: "data:x" }), state("k"));
    expect(m.get("k")!.icon).toBe("data:x");
  });

  it("sets the status on refused and the next navigation clears it", () => {
    const rest = { ...state("k"), loading: false } as const;
    const m = pageReducer(pageReducer(asked("k"), rest), { type: "refused", key: "k" });
    expect(m.get("k")!.status).toBe("refused");
    expect(pageReducer(m, state("k")).get("k")!.status).toBeNull();
  });

  // A refused link or redirect is a cancelled navigation, and WebView2 reports its end like any other: the same address, not loading.
  it("keeps refused through the state events of the navigation that was refused", () => {
    const rest = { ...state("k"), loading: false } as const;
    const m = pageReducer(pageReducer(asked("k"), rest), { type: "refused", key: "k" });
    expect(pageReducer(m, rest).get("k")!.status).toBe("refused");
    expect(pageReducer(m, { ...rest, title: "A, later" }).get("k")!.status).toBe("refused");
  });

  // A refused redirect comes in the middle of a navigation that was let through: the page is still loading when it is refused,
  // and says so once more before it stops (seen in the 6q-3a live check).
  it("keeps refused through the rest of a navigation whose redirect was refused", () => {
    const loading = state("k");
    let m = pageReducer(pageReducer(asked("k"), loading), { type: "refused", key: "k" });
    m = pageReducer(m, loading);
    expect(m.get("k")!.status).toBe("refused");
    m = pageReducer(m, { ...loading, loading: false });
    expect(m.get("k")!.status).toBe("refused");
    expect(pageReducer(m, loading).get("k")!.status).toBeNull();
  });

  it("clears refused when the page goes to another address without a loading state", () => {
    const rest = { ...state("k"), loading: false } as const;
    const m = pageReducer(pageReducer(asked("k"), rest), { type: "refused", key: "k" });
    expect(pageReducer(m, { ...rest, url: "https://a.test/#part" }).get("k")!.status).toBeNull();
  });

  it("sets the status on failed and the next state event clears it", () => {
    const m = pageReducer(pageReducer(asked("k"), state("k")), { type: "failed", key: "k" });
    expect(m.get("k")).toMatchObject({ status: "failed", loading: false });
    expect(pageReducer(m, state("k")).get("k")!.status).toBeNull();
  });

  it("ignores failed for a page it doesn't hold", () => {
    expect(pageReducer(empty, { type: "failed", key: "k" }).size).toBe(0);
  });

  it("empties the store on gone", () => {
    const m = pageReducer(pageReducer(asked("a", "b"), state("a")), state("b"));
    expect(pageReducer(m, { type: "gone" }).size).toBe(0);
  });

  it("removes one page on closed", () => {
    const m = pageReducer(pageReducer(asked("a", "b"), state("a")), state("b"));
    expect([...pageReducer(m, { type: "closed", key: "a" }).keys()]).toEqual(["b"]);
  });

  it("focuses one page at a time", () => {
    let m = pageReducer(pageReducer(asked("a", "b"), state("a")), state("b"));
    m = pageReducer(m, { type: "focus", key: "a", focused: true });
    m = pageReducer(m, { type: "focus", key: "b", focused: true });
    expect(m.get("a")!.focused).toBe(false);
    expect(m.get("b")!.focused).toBe(true);
    m = pageReducer(m, { type: "focus", key: "a", focused: false });
    expect(m.get("b")!.focused).toBe(true);
  });
});

describe("openPage", () => {
  it("holds the page from the moment the shell is asked, so what the page reports is kept", async () => {
    dispatchPage({ type: "icon", key: "s/t", src: "data:early" });
    expect(pageIcon("s/t")).toBeNull();
    await openPage("s/t", "https://a.test/");
    dispatchPage({ type: "icon", key: "s/t", src: "data:x" });
    expect(pageIcon("s/t")).toBe("data:x");
    dispatchPage({ type: "gone" });
  });
});
