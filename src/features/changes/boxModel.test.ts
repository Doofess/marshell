import { describe, expect, it } from "vitest";
import { type ReplyState, type ThreadBox, afterReply, canPost, closedThreadBox, openThreadBox, startPost, threadBoxAfter } from "./boxModel";
import type { Item } from "./model";

const box = (over: Partial<ThreadBox> = {}): ThreadBox => ({ kind: "reply", thread: "T1", text: "thanks", error: null, ...over });
const state = (over: Partial<ReplyState> = {}): ReplyState => ({ box: box(), texts: new Map(), out: new Set(), ...over });
const item = (kind: "file" | "thread" | "talk", id: string) => ({ kind, key: `${kind}:${id}` }) as unknown as Item;

describe("threadBoxAfter", () => {
  const items = [item("file", "a.ts"), item("thread", "T1"), item("thread", "T2"), item("talk", "N1")];
  it("is the thread's item index", () => {
    expect(threadBoxAfter(items, box())).toBe(1);
    expect(threadBoxAfter(items, box({ thread: "T2", kind: "note" }))).toBe(2);
    expect(threadBoxAfter(items, box({ thread: "N1", kind: "note" }))).toBe(3);
  });
  it("is -1 when the thread left the list", () => {
    expect(threadBoxAfter(items, box({ thread: "T9" }))).toBe(-1);
    expect(threadBoxAfter([], box())).toBe(-1);
  });
});

describe("afterReply", () => {
  it("a refused reply keeps the box and its text", () => {
    const s = state({ out: new Set(["T1"]), texts: new Map([["T1", "thanks"]]) });
    const next = afterReply(s, "T1", { ok: false, reason: "The pull request changed." });
    expect(next.box).toEqual(box({ error: "The pull request changed." }));
    expect(next.box?.text).toBe("thanks");
    expect(next.texts.get("T1")).toBe("thanks");
    expect(next.out.has("T1")).toBe(false);
    expect(canPost(next, "T1")).toBe(true);
  });
  it("a posted reply closes the box and forgets the text", () => {
    const s = state({ out: new Set(["T1", "T2"]), texts: new Map([["T1", "thanks"], ["T2", "later"]]) });
    const next = afterReply(s, "T1", { ok: true });
    expect(next.box).toBeNull();
    expect(next.texts.has("T1")).toBe(false);
    expect(next.texts.get("T2")).toBe("later");
    expect([...next.out]).toEqual(["T2"]);
  });
  it("a reply that lands after its box closed still forgets the text", () => {
    const other = box({ thread: "T2", text: "other" });
    const s = state({ box: other, out: new Set(["T1"]), texts: new Map([["T1", "thanks"]]) });
    const next = afterReply(s, "T1", { ok: true });
    expect(next.box).toBe(other);
    expect(next.texts.has("T1")).toBe(false);
    expect(next.out.size).toBe(0);
    const none = afterReply(state({ box: null, out: new Set(["T1"]), texts: new Map([["T1", "thanks"]]) }), "T1", { ok: true });
    expect(none.box).toBeNull();
    expect(none.texts.size).toBe(0);
  });
  it("a refusal after the box closed leaves the kept text and frees the thread", () => {
    const s = state({ box: null, out: new Set(["T1"]), texts: new Map([["T1", "thanks"]]) });
    const next = afterReply(s, "T1", { ok: false, reason: "x" });
    expect(next.box).toBeNull();
    expect(next.texts.get("T1")).toBe("thanks");
    expect(next.out.size).toBe(0);
  });
  it("a note box on the same thread is neither closed nor given the reply's error", () => {
    const note = box({ kind: "note", text: "mine" });
    expect(afterReply(state({ box: note, out: new Set(["T1"]) }), "T1", { ok: true }).box).toBe(note);
    expect(afterReply(state({ box: note, out: new Set(["T1"]) }), "T1", { ok: false, reason: "x" }).box).toBe(note);
  });
  it("never changes the state it is given", () => {
    const s = state({ out: new Set(["T1"]), texts: new Map([["T1", "t"]]) });
    afterReply(s, "T1", { ok: true });
    expect(s.box).not.toBeNull();
    expect(s.out.has("T1")).toBe(true);
    expect(s.texts.has("T1")).toBe(true);
  });
});

describe("canPost", () => {
  it("a thread with a reply out can't post again", () => {
    const s = startPost(state(), "T1");
    expect(canPost(state(), "T1")).toBe(true);
    expect(canPost(s, "T1")).toBe(false);
    expect(canPost(s, "T2")).toBe(true);
  });
  it("a thread with a reply out can't post again after its box closed and opened", () => {
    let s = startPost(state(), "T1");
    s = closedThreadBox(s);
    expect(s.box).toBeNull();
    s = openThreadBox(s, "reply", "T1", "");
    expect(s.box?.thread).toBe("T1");
    expect(canPost(s, "T1")).toBe(false);
  });
  it("startPost leaves the state it is given alone", () => {
    const s = state();
    startPost(s, "T1");
    expect(s.out.size).toBe(0);
  });
});

describe("opening and closing a thread box", () => {
  it("closing keeps a reply's text for its thread, and drops a blank one", () => {
    const kept = closedThreadBox(state({ box: box({ text: "half a thought" }) }));
    expect(kept.box).toBeNull();
    expect(kept.texts.get("T1")).toBe("half a thought");
    const blank = closedThreadBox(state({ box: box({ text: "  \n" }), texts: new Map([["T1", "old"]]) }));
    expect(blank.texts.has("T1")).toBe(false);
  });
  it("closing a note box keeps no reply text", () => {
    const next = closedThreadBox(state({ box: box({ kind: "note", text: "mine" }) }));
    expect(next.texts.size).toBe(0);
    expect(closedThreadBox(state({ box: null })).box).toBeNull();
  });
  it("opening a reply brings back the kept text; opening on another thread keeps the first's", () => {
    const s = state({ box: box({ text: "first" }), texts: new Map() });
    const next = openThreadBox(s, "reply", "T2", "");
    expect(next.box).toEqual({ kind: "reply", thread: "T2", text: "", error: null });
    expect(next.texts.get("T1")).toBe("first");
    const back = openThreadBox(next, "reply", "T1", "");
    expect(back.box?.text).toBe("first");
  });
  it("opening a note starts from the thread's note", () => {
    const next = openThreadBox(state({ box: null }), "note", "T1", "my note");
    expect(next.box).toEqual({ kind: "note", thread: "T1", text: "my note", error: null });
  });
});
