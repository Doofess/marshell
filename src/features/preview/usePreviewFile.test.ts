import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { POLL_MS, afterDeleted, contentState, createPoller, shownFile } from "./usePreviewFile";

describe("afterDeleted", () => {
  it("keeps the last pages of a deleted PDF", () => {
    const bytes = new ArrayBuffer(4);
    const pdf = { phase: "pdf", bytes, version: 2, hash: "h", readOnly: null } as const;
    expect(afterDeleted(pdf)).toEqual({ phase: "deleted", last: pdf });
  });
});

describe("shownFile", () => {
  const pdf = { phase: "pdf", bytes: new ArrayBuffer(4), version: 0, hash: "h", readOnly: null } as const;
  it("is loading while the state still belongs to the path before", () => {
    // The render where the tab goes to another file comes before the loader starts on it: B's pages must not show under A's entry.
    expect(shownFile(pdf, "C:\\p\\b.pdf", "C:\\p\\a.pdf")).toEqual({ phase: "loading" });
  });
  it("is the state itself for its own path", () => {
    expect(shownFile(pdf, "C:\\p\\a.pdf", "C:\\p\\a.pdf")).toBe(pdf);
  });
});

describe("contentState", () => {
  const ok = (hash: string) => ({ kind: "ok", bytes: new ArrayBuffer(4), modifiedMs: 1, hash }) as const;
  it("a pdf keeps its hash and read-only flag", () => {
    const content = ok("a".repeat(64));
    expect(contentState("pdf", content, "attribute", 3, "C:/p/a.pdf")).toEqual({ phase: "pdf", bytes: content.bytes, version: 3, hash: "a".repeat(64), readOnly: "attribute" });
    expect(contentState("pdf", content, null, 0, "C:/p/a.pdf")).toMatchObject({ hash: "a".repeat(64), readOnly: null });
  });
  it("a text keeps its hash and read-only flag", () => {
    const bytes = new TextEncoder().encode("hello").buffer as ArrayBuffer;
    expect(contentState("text", { kind: "ok", bytes, modifiedMs: 1, hash: "b" }, "own", 0, "C:/p/a.txt")).toMatchObject({ phase: "text", text: "hello", hash: "b", readOnly: "own" });
  });
});

describe("createPoller", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("a paused file is not polled", () => {
    const tick = vi.fn();
    const p = createPoller({ tick, isPaused: () => true, visible: () => true });
    p.start();
    vi.advanceTimersByTime(POLL_MS * 5);
    expect(tick).not.toHaveBeenCalled();
    p.stop();
  });
  it("resuming checks once, as the first look if there was none, then polls as before", () => {
    const tick = vi.fn();
    let paused = true;
    const p = createPoller({ tick, isPaused: () => paused, visible: () => true });
    p.start();
    vi.advanceTimersByTime(POLL_MS * 2);
    paused = false;
    p.resume();
    expect(tick).toHaveBeenCalledTimes(1);
    expect(tick).toHaveBeenLastCalledWith(true);
    vi.advanceTimersByTime(POLL_MS);
    expect(tick).toHaveBeenCalledTimes(2);
    expect(tick).toHaveBeenLastCalledWith(false);
    p.stop();
  });
  it("an unpaused file looks at once and then every second while visible", () => {
    const tick = vi.fn();
    let visible = true;
    const p = createPoller({ tick, isPaused: () => false, visible: () => visible });
    p.start();
    expect(tick).toHaveBeenCalledWith(true);
    visible = false;
    vi.advanceTimersByTime(POLL_MS * 3);
    expect(tick).toHaveBeenCalledTimes(1);
    visible = true;
    vi.advanceTimersByTime(POLL_MS);
    expect(tick).toHaveBeenCalledTimes(2);
    p.stop();
    vi.advanceTimersByTime(POLL_MS * 3);
    expect(tick).toHaveBeenCalledTimes(2);
  });
});
