import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPerfPolling } from "./perfPolling";

const controller = (poll: () => Promise<number>, onSnapshot: (snapshot: number) => void = vi.fn()) =>
  createPerfPolling({ poll, intervalMs: 1000, onSnapshot });

function deferred() {
  let resolve!: (value: number) => void;
  const promise = new Promise<number>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

describe("perf polling", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("polls at 0, 1000 and 2000 ms while visible", async () => {
    const poll = vi.fn(async () => 1);
    const polling = controller(poll);
    polling.setVisible(true);
    expect(poll).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(999);
    expect(poll).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(poll).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1000);
    expect(poll).toHaveBeenCalledTimes(3);
    polling.stop();
  });

  it("never overlaps a slow request", async () => {
    const pending = deferred();
    const poll = vi.fn(() => pending.promise);
    const polling = controller(poll);
    polling.setVisible(true);
    await vi.advanceTimersByTimeAsync(2000);
    expect(poll).toHaveBeenCalledTimes(1);
    pending.resolve(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(poll).toHaveBeenCalledTimes(2);
    polling.stop();
  });

  it("stops timers and ignores a response that arrives after stop", async () => {
    const pending = deferred();
    const poll = vi.fn(() => pending.promise);
    const onSnapshot = vi.fn();
    const polling = controller(poll, onSnapshot);
    polling.setVisible(true);
    polling.stop();
    pending.resolve(1);
    await vi.advanceTimersByTimeAsync(5000);
    expect(poll).toHaveBeenCalledTimes(1);
    expect(onSnapshot).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("keeps the previous snapshot after failure and recovers on the next tick", async () => {
    let snapshot: number | null = null;
    const onSnapshot = vi.fn((next: number) => { snapshot = next; });
    const poll = vi.fn<() => Promise<number>>().mockResolvedValueOnce(1).mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce(2);
    const polling = controller(poll, onSnapshot);
    polling.setVisible(true);
    await vi.advanceTimersByTimeAsync(0);
    expect(snapshot).toBe(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(snapshot).toBe(1);
    expect(onSnapshot).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(snapshot).toBe(2);
    polling.stop();
  });

  it("starts no polling when hidden and polls immediately on each show", async () => {
    const poll = vi.fn(async () => 1);
    const polling = controller(poll);
    polling.setVisible(false);
    await vi.advanceTimersByTimeAsync(2000);
    expect(poll).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
    polling.setVisible(true);
    expect(poll).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(0);
    polling.setVisible(false);
    await vi.advanceTimersByTimeAsync(2000);
    expect(poll).toHaveBeenCalledTimes(1);
    polling.setVisible(true);
    expect(poll).toHaveBeenCalledTimes(2);
    polling.stop();
  });

  it("does not overlap across hide and show or publish the old visibility's response", async () => {
    const pending = deferred();
    const poll = vi.fn<() => Promise<number>>().mockReturnValueOnce(pending.promise).mockResolvedValue(2);
    const onSnapshot = vi.fn();
    const polling = controller(poll, onSnapshot);
    polling.setVisible(true);
    polling.setVisible(false);
    polling.setVisible(true);
    expect(poll).toHaveBeenCalledTimes(1);
    pending.resolve(1);
    await vi.advanceTimersByTimeAsync(0);
    expect(poll).toHaveBeenCalledTimes(2);
    expect(onSnapshot.mock.calls).toEqual([[2]]);
    polling.stop();
  });
});
