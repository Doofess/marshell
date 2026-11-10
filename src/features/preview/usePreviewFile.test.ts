import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { POLL_MS, afterDeleted, createPoller } from "./usePreviewFile";

describe("afterDeleted", () => {
  it("keeps the last pages of a deleted PDF", () => {
    const bytes = new ArrayBuffer(4);
    const pdf = { phase: "pdf", bytes, version: 2 } as const;
    expect(afterDeleted(pdf)).toEqual({ phase: "deleted", last: pdf });
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
