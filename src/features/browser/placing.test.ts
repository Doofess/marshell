import { describe, expect, it, vi } from "vitest";
import type { Report } from "./browserApi";
import { createPlacer } from "./placing";

const report = (w: number): Report => ({ rect: { x: 0, y: 0, width: 10, height: 10 }, win: { width: w, height: 600 }, rule: null, moving: false });
const deferred = () => {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => (resolve = r));
  return { promise, resolve };
};
const flush = () => new Promise((r) => setTimeout(r, 0));

describe("createPlacer", () => {
  it("makes a second report wait while one is in flight, and measures it late", async () => {
    const first = deferred();
    const send = vi.fn().mockReturnValueOnce(first.promise).mockResolvedValue(undefined);
    const placer = createPlacer(send, () => Promise.resolve());
    placer.report(() => report(1000));
    const second = vi.fn(() => report(1100));
    placer.report(second);
    expect(second).not.toHaveBeenCalled();
    first.resolve();
    await flush();
    expect(second).toHaveBeenCalled();
    expect(send).toHaveBeenCalledTimes(2);
  });

  it("sends two of three reports in flight", async () => {
    const first = deferred();
    const send = vi.fn().mockReturnValueOnce(first.promise).mockResolvedValue(undefined);
    const placer = createPlacer(send, () => Promise.resolve());
    const measures = [vi.fn(() => report(1000)), vi.fn(() => report(1001)), vi.fn(() => report(1002))];
    for (const m of measures) placer.report(m);
    first.resolve();
    await flush();
    expect(send).toHaveBeenCalledTimes(2);
    expect(measures[1]).not.toHaveBeenCalled();
    expect(measures[2]).toHaveBeenCalled();
  });

  it("makes a report at the same size wait for the paint", async () => {
    const paint = deferred();
    const send = vi.fn().mockResolvedValue(undefined);
    const placer = createPlacer(send, () => paint.promise);
    placer.report(() => report(1000));
    await flush();
    placer.report(() => report(1000));
    await flush();
    expect(send).toHaveBeenCalledTimes(1);
    paint.resolve();
    await flush();
    expect(send).toHaveBeenCalledTimes(2);
  });

  it("sends a report at a new size at once and drops the one waiting for a paint", async () => {
    const paint = deferred();
    const send = vi.fn().mockResolvedValue(undefined);
    const placer = createPlacer(send, () => paint.promise);
    placer.report(() => report(1000));
    await flush();
    placer.report(() => report(1000));
    placer.report(() => report(1200));
    await flush();
    expect(send).toHaveBeenCalledTimes(2);
    expect(send.mock.calls[1]![0].win.width).toBe(1200);
    paint.resolve();
    await flush();
    expect(send).toHaveBeenCalledTimes(2);
  });

  // During the app's own motion the window keeps its size and a report comes every frame, each before the last one's paint.
  it("sends a report each paint while one comes every frame at one size", async () => {
    // Each wait is its own frame, as `nextPaint` is: the second report's paint comes after the first one's.
    const paints: ReturnType<typeof deferred>[] = [];
    const send = vi.fn().mockResolvedValue(undefined);
    const placer = createPlacer(send, () => {
      const paint = deferred();
      paints.push(paint);
      return paint.promise;
    });
    const at = (x: number) => () => ({ ...report(1000), rect: { x, y: 0, width: 10, height: 10 } });
    placer.report(at(1));
    await flush();
    placer.report(at(2));
    placer.report(at(3));
    paints[0]!.resolve();
    await flush();
    expect(send).toHaveBeenCalledTimes(2);
    expect(send.mock.calls[1]![0].rect.x).toBe(3);
  });

  it("drops a report waiting for a paint when cancelled", async () => {
    const paint = deferred();
    const send = vi.fn().mockResolvedValue(undefined);
    const placer = createPlacer(send, () => paint.promise);
    placer.report(() => report(1000));
    await flush();
    const late = vi.fn(() => report(1000));
    placer.report(late);
    placer.cancel();
    paint.resolve();
    await flush();
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("drops a report queued behind one in flight when cancelled", async () => {
    const first = deferred();
    const send = vi.fn().mockReturnValueOnce(first.promise).mockResolvedValue(undefined);
    const placer = createPlacer(send, () => Promise.resolve());
    placer.report(() => report(1000));
    const queued = vi.fn(() => report(1100));
    placer.report(queued);
    placer.cancel();
    first.resolve();
    await flush();
    expect(queued).not.toHaveBeenCalled();
    expect(send).toHaveBeenCalledTimes(1);
  });
});
