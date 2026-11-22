import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchReply } from "../../generated/SearchReply";
import { createSearchScheduler, SEARCH_DEBOUNCE_MS, type SearchState } from "./useSearch";

const reply = (root: string): SearchReply => ({ root, files: [], cut: null });
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function setup() {
  const signals: AbortSignal[] = [];
  const resolvers: ((r: SearchReply) => void)[] = [];
  const run = vi.fn((_tab: string, _q: string, signal: AbortSignal) => {
    signals.push(signal);
    return new Promise<SearchReply | "no-folder">((res) => resolvers.push(res));
  });
  const states: SearchState[] = [];
  const s = createSearchScheduler(run, (st) => states.push(st));
  return { run, signals, resolvers, states, s };
}

describe("useSearch's scheduler", () => {
  it("a search runs 300 ms after the last key", () => {
    const { run, s } = setup();
    expect(SEARCH_DEBOUNCE_MS).toBe(300);
    s.schedule("t", "ab");
    vi.advanceTimersByTime(299);
    s.schedule("t", "abc");
    vi.advanceTimersByTime(299);
    expect(run).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(run).toHaveBeenCalledTimes(1);
    expect(run.mock.calls[0]?.[1]).toBe("abc");
  });

  it("an_older_search_is_aborted", async () => {
    const { signals, resolvers, states, s } = setup();
    s.schedule("t", "ab");
    vi.advanceTimersByTime(300);
    s.schedule("t", "abc");
    expect(signals[0]?.aborted).toBe(true);
    resolvers[0]?.(reply("old"));
    await Promise.resolve();
    expect(states.some((x) => x.reply?.root === "old")).toBe(false);
    vi.advanceTimersByTime(300);
    resolvers[1]?.(reply("new"));
    await Promise.resolve();
    await Promise.resolve();
    expect(states.at(-1)).toEqual({ reply: reply("new"), busy: false, failed: false });
  });

  it("a failure is said, an abort is not, and nothing is set after dispose", async () => {
    const run = vi.fn(
      (_t: string, _q: string, signal: AbortSignal) =>
        new Promise<SearchReply | "no-folder">((_, rej) => signal.addEventListener("abort", () => rej(new Error("aborted")))),
    );
    const states: SearchState[] = [];
    const s = createSearchScheduler(run, (st) => states.push(st));
    s.schedule("t", "ab");
    vi.advanceTimersByTime(300);
    s.dispose();
    await Promise.resolve();
    await Promise.resolve();
    expect(states.some((x) => x.failed)).toBe(false);
    const failing = createSearchScheduler(() => Promise.reject(new Error("boom")), (st) => states.push(st));
    failing.schedule("t", "ab");
    vi.advanceTimersByTime(300);
    await Promise.resolve();
    await Promise.resolve();
    expect(states.at(-1)).toEqual({ reply: null, busy: false, failed: true });
  });

  it("a null query goes idle", () => {
    const { states, s } = setup();
    s.schedule("t", null);
    expect(states.at(-1)).toEqual({ reply: null, busy: false, failed: false });
  });
});
