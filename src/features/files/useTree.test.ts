import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TreeAsk } from "../../generated/TreeAsk";
import type { TreeDir } from "../../generated/TreeDir";
import type { TreeEntry } from "../../generated/TreeEntry";
import type { TreeReply } from "../../generated/TreeReply";
import { type Listing, visibleRows } from "./treeModel";
import { applyReply, createTreeEngine, initialData, nextAsks, schedule, TREE_FULL_MS, TREE_POLL_MS, type TreeSource } from "./useTree";

const entry = (name: string, kind: TreeEntry["kind"] = "file"): TreeEntry => ({ name, kind, ignored: false });
const listed = (entries: TreeEntry[], stamp = 1): Listing => ({ entries, more: 0, stamp, state: "listed" });
const dirOf = (dir: string, entries: TreeEntry[], stamp = 1): TreeDir => ({ dir, state: "listed", stamp, entries, more: 0 });
const replyOf = (root: string, dirs: TreeDir[]): TreeReply => ({ root, dirs, changes: [] });

function rowsOf(listings: Map<string, Listing>, expanded: string[]) {
  return visibleRows(listings, new Set(expanded));
}

describe("applyReply's change marks", () => {
  it("a Same reply's marks replace the last ones", () => {
    const first = applyReply(initialData("s1", undefined), { session: "s1", root: null, expanded: new Set() }, { ...replyOf("C:\\p", [dirOf("", [entry("a")])]), changes: [{ path: "a", status: "modified" }] });
    expect(first.data.changes).toEqual([{ path: "a", status: "modified" }]);
    const same: TreeReply = { root: "C:\\p", dirs: [{ dir: "", state: "same", stamp: 1, entries: [], more: 0 }], changes: [{ path: "a", status: "modified" }, { path: "b", status: "added" }] };
    const second = applyReply(first.data, { session: "s1", root: "C:\\p", expanded: new Set() }, same);
    expect(second.data.changes).toEqual(same.changes);
    expect(second.data.listings.get("")?.entries).toHaveLength(1);
  });
});

describe("nextAsks", () => {
  it("nextAsks asks the root and every drawn open folder, with stamps unless full", () => {
    const listings = new Map<string, Listing>([
      ["", listed([entry("a", "folder"), entry("b", "folder"), entry("c.txt")], 5)],
      ["a", listed([], 7)],
    ]);
    const rows = rowsOf(listings, ["a", "b"]);
    // b was never listed, so it comes first; the rest keep row order.
    expect(nextAsks(listings, rows, false)).toEqual([
      [
        { dir: "b", stamp: null },
        { dir: "", stamp: 5 },
        { dir: "a", stamp: 7 },
      ],
    ]);
    expect(nextAsks(listings, rows, true)).toEqual([
      [
        { dir: "b", stamp: null },
        { dir: "", stamp: null },
        { dir: "a", stamp: null },
      ],
    ]);
  });

  it("nextAsks asks the root alone when nothing is listed yet", () => {
    expect(nextAsks(new Map(), [], false)).toEqual([[{ dir: "", stamp: null }]]);
  });

  it("nextAsks cuts 200 open folders into requests of 64, 64, 64 and 8", () => {
    const names = Array.from({ length: 199 }, (_, i) => `f${String(i).padStart(3, "0")}`);
    const listings = new Map<string, Listing>([["", listed(names.map((n) => entry(n, "folder")))]]);
    for (const n of names) listings.set(n, listed([entry("x.txt")]));
    const rows = rowsOf(listings, names);
    const sizes = nextAsks(listings, rows, false).map((c) => c.length);
    expect(sizes).toEqual([64, 64, 64, 8]);
  });

  it("an open folder under a closed one is not asked", () => {
    const listings = new Map<string, Listing>([
      ["", listed([entry("a", "folder")])],
      ["a", listed([entry("b", "folder")])],
      ["a/b", listed([entry("c.txt")])],
    ]);
    const rows = rowsOf(listings, ["a/b"]); // a itself is closed
    expect(nextAsks(listings, rows, false).flat().map((a) => a.dir)).toEqual([""]);
  });

  it("a gone listing is asked again without a stamp", () => {
    const listings = new Map<string, Listing>([["", { entries: [], more: 0, stamp: 0, state: "gone" }]]);
    expect(nextAsks(listings, [], false)).toEqual([[{ dir: "", stamp: null }]]);
  });
});

describe("schedule", () => {
  const base = { inFlight: false, drawn: true, hasSource: true, visible: true, focused: true, now: 10_000, lastAsk: 9_000, lastFull: 9_000, urgent: false, refull: false };

  it("a_poll_waits_for_the_one_in_flight", () => {
    expect(schedule({ ...base, inFlight: true, urgent: true, refull: true, lastAsk: null })).toBe("none");
  });
  it("asks once when drawn, then every 2,000 ms", () => {
    expect(schedule({ ...base, lastAsk: null, lastFull: null })).toBe("full");
    expect(schedule({ ...base, now: 9_000 + TREE_POLL_MS - 1 })).toBe("none");
    expect(schedule({ ...base, now: 9_000 + TREE_POLL_MS })).toBe("poll");
  });
  it("asks afresh every 30,000 ms and when the window gets the focus back", () => {
    expect(schedule({ ...base, now: 9_000 + TREE_FULL_MS })).toBe("full");
    expect(schedule({ ...base, refull: true })).toBe("full");
  });
  it("asks at once on an expand", () => {
    expect(schedule({ ...base, urgent: true })).toBe("poll");
  });
  it("waits while hidden, unfocused or not drawn, except for the first ask", () => {
    expect(schedule({ ...base, now: 99_999, visible: false })).toBe("none");
    expect(schedule({ ...base, now: 99_999, focused: false })).toBe("none");
    expect(schedule({ ...base, now: 99_999, drawn: false })).toBe("none");
    expect(schedule({ ...base, now: 99_999, hasSource: false })).toBe("none");
    expect(schedule({ ...base, focused: false, lastAsk: null, lastFull: null })).toBe("full");
  });
});

describe("applyReply", () => {
  const saved = { root: "C:\\p", expanded: ["a", "b"] };

  it("a_new_root_closes_every_folder", () => {
    const data = initialData("s1", saved);
    const withListings = { ...data, listings: new Map<string, Listing>([["a", listed([entry("x.txt")])]]) };
    const out = applyReply(withListings, { session: "s1", root: "C:\\p", expanded: new Set(withListings.expanded) }, replyOf("C:\\q", [dirOf("", [entry("a", "folder")]), dirOf("a", [entry("y.txt")])]));
    expect(out.save).toEqual({ root: "C:\\q", expanded: [] });
    expect(out.data.root).toBe("C:\\q");
    expect([...out.data.expanded]).toEqual([]);
    expect([...out.data.listings.keys()]).toEqual([""]);
  });

  it("learns the root and saves it, keeping the saved folders", () => {
    const data = initialData("s1", { root: "", expanded: ["a"] });
    const none = initialData("s1", undefined);
    const out = applyReply(none, { session: "s1", root: null, expanded: new Set(none.expanded) }, replyOf("C:\\p", [dirOf("", [entry("a", "folder")])]));
    expect(out.data.root).toBe("C:\\p");
    expect(out.save).toEqual({ root: "C:\\p", expanded: [] });
    expect(data.root).toBe("");
  });

  it("a same root with listings saves nothing", () => {
    const data = initialData("s1", saved);
    const out = applyReply(data, { session: "s1", root: "C:\\p", expanded: new Set(data.expanded) }, replyOf("C:\\p", [dirOf("", [entry("a", "folder"), entry("b", "folder")]), dirOf("a", []), dirOf("b", [])]));
    expect(out.save).toBeNull();
    expect(out.data.listings.get("a")?.state).toBe("listed");
    expect(rowsOf(new Map(out.data.listings), [...out.data.expanded]).map((r) => r.path)).toEqual(["a", "a/\u0000empty", "b", "b/\u0000empty"]);
  });

  it("a_gone_folder_is_dropped", () => {
    const data0 = initialData("s1", saved);
    const data = { ...data0, listings: new Map<string, Listing>([["", listed([entry("a", "folder"), entry("b", "folder")])], ["a", listed([entry("x.txt")])]]) };
    const out = applyReply(data, { session: "s1", root: "C:\\p", expanded: new Set(data.expanded) }, replyOf("C:\\p", [{ dir: "a", state: "gone", stamp: 0, entries: [], more: 0 }]));
    expect([...out.data.expanded]).toEqual(["b"]);
    expect(out.data.listings.has("a")).toBe(false);
    expect(out.save).toEqual({ root: "C:\\p", expanded: ["b"] });
  });

  it("a refused folder is dropped like a gone one", () => {
    const data = initialData("s1", saved);
    const out = applyReply(data, { session: "s1", root: "C:\\p", expanded: new Set(data.expanded) }, replyOf("C:\\p", [{ dir: "b", state: "refused", stamp: 0, entries: [], more: 0 }]));
    expect([...out.data.expanded]).toEqual(["a"]);
  });

  it("a same folder keeps its listing", () => {
    const data0 = initialData("s1", saved);
    const kept = listed([entry("x.txt")], 9);
    const data = { ...data0, listings: new Map<string, Listing>([["a", kept]]) };
    const out = applyReply(data, { session: "s1", root: "C:\\p", expanded: new Set(data.expanded) }, replyOf("C:\\p", [{ dir: "a", state: "same", stamp: 9, entries: [], more: 0 }]));
    expect(out.data.listings.get("a")).toBe(kept);
  });

  it("a reply that comes after the session changed is dropped", () => {
    const data = initialData("s2", saved);
    const out = applyReply(data, { session: "s1", root: "C:\\p", expanded: new Set(data.expanded) }, replyOf("C:\\p", [dirOf("", [entry("a", "folder")])]));
    expect(out.data).toBe(data);
    expect(out.save).toBeNull();
  });

  it("a reply asked under another root is dropped", () => {
    const data = initialData("s1", saved);
    const out = applyReply(data, { session: "s1", root: "C:\\old", expanded: new Set(data.expanded) }, replyOf("C:\\p", [dirOf("", [])]));
    expect(out.data).toBe(data);
    expect(out.save).toBeNull();
  });

  it("no folder shows the message and keeps the saved state", () => {
    const data = initialData("s1", saved);
    const out = applyReply(data, { session: "s1", root: "C:\\p", expanded: new Set(data.expanded) }, "no-folder");
    expect(out.data.noFolder).toBe(true);
    expect(out.save).toBeNull();
    expect([...out.data.expanded]).toEqual(["a", "b"]);
  });

  it("a folder that left its parent's listing leaves expanded, with what was under it", () => {
    const data0 = initialData("s1", { root: "C:\\p", expanded: ["a", "a/b", "c"] });
    const out = applyReply(data0, { session: "s1", root: "C:\\p", expanded: new Set(data0.expanded) }, replyOf("C:\\p", [dirOf("", [entry("c", "folder")])]));
    expect([...out.data.expanded]).toEqual(["c"]);
    expect(out.save).toEqual({ root: "C:\\p", expanded: ["c"] });
  });

  it("a folder opened while a reply was out is kept", () => {
    const data0 = initialData("s1", { root: "C:\\p", expanded: ["src", "src/new"] });
    const out = applyReply(data0, { session: "s1", root: "C:\\p", expanded: new Set(["src"]) }, replyOf("C:\\p", [dirOf("", [entry("src", "folder")]), dirOf("src", [entry("a.ts")])]));
    expect([...out.data.expanded]).toContain("src/new");
  });

  it("saves at most 500 open folders", () => {
    const many = Array.from({ length: 600 }, (_, i) => `d${i}`);
    expect(initialData("s1", { root: "r", expanded: many }).expanded.size).toBe(500);
  });
});

describe("the engine", () => {
  type Pending = { asks: TreeAsk[]; resolve(r: TreeReply | "no-folder"): void; reject(e: unknown): void };
  let pending: Pending[];
  let inFlight: number;
  let maxInFlight: number;
  let source: TreeSource;
  const env = { now: () => Date.now(), visible: () => true, focused: () => true };

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000_000);
    pending = [];
    inFlight = 0;
    maxInFlight = 0;
    source = {
      list: (asks) =>
        new Promise((resolve, reject) => {
          inFlight++;
          maxInFlight = Math.max(maxInFlight, inFlight);
          pending.push({
            asks,
            resolve: (r) => {
              inFlight--;
              resolve(r);
            },
            reject,
          });
        }),
    };
  });
  afterEach(() => vi.useRealTimers());

  const flush = async () => {
    for (let i = 0; i < 20; i++) await Promise.resolve();
  };
  const make = (over: Partial<Parameters<typeof createTreeEngine>[0]> = {}) => {
    const onSaved = vi.fn();
    const onChange = vi.fn();
    const engine = createTreeEngine({ session: "s1", saved: { root: "R", expanded: [] }, source, drawn: true, onSaved, onChange, env, ...over });
    return { engine, onSaved, onChange };
  };

  it("asks the root once and nothing more while that request is out", async () => {
    const { engine } = make();
    engine.tick();
    engine.tick();
    engine.focus();
    engine.expand("a");
    await flush();
    expect(pending).toHaveLength(1);
    expect(pending[0]!.asks).toEqual([{ dir: "", stamp: null }]);
    expect(maxInFlight).toBe(1);
    engine.dispose();
  });

  it("goes one request after another, 64 folders at a time", async () => {
    const names = Array.from({ length: 199 }, (_, i) => `f${String(i).padStart(3, "0")}`);
    const { engine } = make({ saved: { root: "R", expanded: names } });
    engine.tick();
    await flush();
    pending.shift()!.resolve(replyOf("R", [dirOf("", names.map((n) => entry(n, "folder")))]));
    await flush();
    const sizes: number[] = [];
    while (pending.length > 0) {
      expect(pending).toHaveLength(1);
      const p = pending.shift()!;
      sizes.push(p.asks.length);
      p.resolve(replyOf("R", p.asks.map((a) => dirOf(a.dir, [entry("x.txt")]))));
      await flush();
    }
    expect(sizes).toEqual([64, 64, 64, 7]);
    expect(maxInFlight).toBe(1);
    expect(engine.data().listings.size).toBe(200);
    engine.dispose();
  });

  it("polls every 2,000 ms and asks again with the stamps", async () => {
    const { engine } = make();
    engine.tick();
    await flush();
    pending.shift()!.resolve(replyOf("R", [dirOf("", [entry("a.txt")], 42)]));
    await flush();
    vi.setSystemTime(1_000_000 + TREE_POLL_MS + 10);
    engine.tick();
    await flush();
    expect(pending[0]!.asks).toEqual([{ dir: "", stamp: 42 }]);
    engine.dispose();
  });

  it("drops a reply that arrives after dispose, and sets and saves nothing", async () => {
    const { engine, onChange, onSaved } = make();
    engine.tick();
    await flush();
    const before = engine.data();
    engine.dispose();
    pending.shift()!.resolve(replyOf("C:\\other", [dirOf("", [entry("a.txt")])]));
    await flush();
    vi.advanceTimersByTime(5_000);
    expect(onChange).not.toHaveBeenCalled();
    expect(onSaved).not.toHaveBeenCalled();
    expect(engine.data()).toBe(before);
  });

  it("makes no request after dispose", async () => {
    const { engine } = make();
    engine.dispose();
    engine.tick();
    engine.focus();
    await flush();
    expect(pending).toHaveLength(0);
  });

  it("a failed request ends the round and the next poll tries again", async () => {
    const { engine } = make();
    engine.tick();
    await flush();
    pending.shift()!.reject(new Error("offline"));
    await flush();
    vi.setSystemTime(1_000_000 + TREE_POLL_MS + 10);
    engine.tick();
    await flush();
    expect(pending).toHaveLength(1);
    engine.dispose();
  });

  it("saves an expand 500 ms later, once", async () => {
    const { engine, onSaved } = make();
    engine.expand("a");
    engine.expand("a/b");
    expect(onSaved).not.toHaveBeenCalled();
    vi.advanceTimersByTime(499);
    expect(onSaved).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(onSaved).toHaveBeenCalledTimes(1);
    expect(onSaved).toHaveBeenCalledWith("s1", { root: "R", expanded: ["a", "a/b"] });
    engine.dispose();
  });

  it("flushes the debounced save for its own session on dispose", () => {
    const { engine, onSaved } = make();
    engine.expand("a");
    engine.dispose();
    expect(onSaved).toHaveBeenCalledTimes(1);
    expect(onSaved).toHaveBeenCalledWith("s1", { root: "R", expanded: ["a"] });
    vi.advanceTimersByTime(2_000);
    expect(onSaved).toHaveBeenCalledTimes(1);
  });

  it("collapse, collapseAll and reveal change what is open", () => {
    const { engine } = make({ saved: { root: "R", expanded: ["a", "b"] } });
    engine.reveal("c/d/e.txt");
    expect([...engine.data().expanded]).toEqual(["a", "b", "c", "c/d"]);
    engine.collapse("a");
    expect([...engine.data().expanded]).toEqual(["b", "c", "c/d"]);
    engine.collapseAll();
    expect([...engine.data().expanded]).toEqual([]);
    engine.dispose();
  });

  it("applies a reply and tells the owner", async () => {
    const { engine, onChange } = make();
    engine.tick();
    await flush();
    pending.shift()!.resolve(replyOf("R", [dirOf("", [entry("a.txt")])]));
    await flush();
    expect(onChange).toHaveBeenCalled();
    expect(engine.data().listings.get("")?.entries).toHaveLength(1);
    engine.dispose();
  });
});
