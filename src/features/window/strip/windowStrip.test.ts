import { describe, expect, it, vi } from "vitest";
import type { StripPaint } from "./tiles";
import { focusWatch, stripShown, stripSync, type StripRead } from "./windowStrip";

const settle = () => new Promise<void>((done) => setTimeout(done, 0));
const PAINT = (n: number) => ({ height: n }) as unknown as StripPaint;

function reader(state: { shown: boolean; key: string | null }) {
  const paint = vi.fn(() => PAINT(state.key === null ? 0 : state.key.length));
  const read = (): StripRead => ({ shown: state.shown, key: state.key, paint });
  return { read, paint };
}

describe("stripShown", () => {
  it("shows only when nothing covers the page and the keyboard isn't on a button", () => {
    expect(stripShown(false, false)).toBe(true);
    expect(stripShown(true, false)).toBe(false);
    expect(stripShown(false, true)).toBe(false);
    expect(stripShown(true, true)).toBe(false);
  });
});

describe("stripSync", () => {
  it("sends the pictures once and then only what changed", async () => {
    const send = vi.fn(async () => {});
    const state = { shown: true, key: "a" };
    const { read, paint } = reader(state);
    const sync = stripSync(read, send);
    sync();
    await settle();
    sync();
    await settle();
    expect(send.mock.calls).toEqual([[true, PAINT(1)]]);
    state.shown = false;
    sync();
    await settle();
    expect(send.mock.calls[1]).toEqual([false, null]);
    expect(send).toHaveBeenCalledTimes(2);
    expect(paint).toHaveBeenCalledTimes(1);
  });

  it("paints again when the scale or a colour changes", async () => {
    const send = vi.fn(async () => {});
    const state = { shown: true, key: "a" };
    const { read, paint } = reader(state);
    const sync = stripSync(read, send);
    sync();
    await settle();
    state.key = "bb";
    sync();
    await settle();
    expect(send.mock.calls).toEqual([
      [true, PAINT(1)],
      [true, PAINT(2)],
    ]);
    expect(paint).toHaveBeenCalledTimes(2);
  });

  it("hides and tries again when the looks can't be read", async () => {
    const send = vi.fn(async () => {});
    const state = { shown: true, key: null as string | null };
    const { read } = reader(state);
    const sync = stripSync(read, send);
    sync();
    await settle();
    state.key = "a";
    sync();
    await settle();
    expect(send.mock.calls).toEqual([
      [false, null],
      [true, PAINT(1)],
    ]);
  });

  it("sends everything again after a failed send", async () => {
    const send = vi.fn(async () => {});
    send.mockRejectedValueOnce(new Error("no window"));
    const { read } = reader({ shown: true, key: "a" });
    const sync = stripSync(read, send);
    sync();
    await settle();
    await settle();
    sync();
    await settle();
    expect(send.mock.calls).toEqual([
      [true, PAINT(1)],
      [true, PAINT(1)],
    ]);
  });

  it("sends the newest state after a send that was overtaken", async () => {
    let finish: () => void = () => {};
    const send = vi.fn(async () => {});
    send.mockImplementationOnce(() => new Promise<void>((done) => (finish = done)));
    const state = { shown: true, key: "a" };
    const { read, paint } = reader(state);
    const sync = stripSync(read, send);
    sync();
    state.key = "bb";
    sync();
    expect(send).toHaveBeenCalledTimes(1);
    expect(paint).toHaveBeenCalledTimes(1);
    finish();
    await settle();
    expect(send.mock.calls).toEqual([
      [true, PAINT(1)],
      [true, PAINT(2)],
    ]);
  });
});

describe("focusWatch", () => {
  /** A hand-run "later": the test decides when the turn ends. */
  function later() {
    const waiting: (() => void)[] = [];
    const defer = (run: () => void) => {
      waiting.push(run);
      return () => void waiting.splice(waiting.indexOf(run), 1);
    };
    const flush = () => {
      for (const run of waiting.splice(0)) run();
    };
    return { defer, flush, waiting };
  }

  it("doesn't read the page between a button losing the keyboard and the next one getting it", () => {
    const sync = vi.fn();
    const { defer, flush } = later();
    const watch = focusWatch(sync, defer);
    // Tab from Minimize to Maximize: `focusout`, then `focusin`. A read between the two finds the keyboard on no button.
    watch.onOut();
    expect(sync).not.toHaveBeenCalled();
    watch.onIn();
    expect(sync).toHaveBeenCalledTimes(1);
    flush();
    expect(sync).toHaveBeenCalledTimes(2);
  });

  it("reads once the keyboard has left the buttons for good", () => {
    const sync = vi.fn();
    const { defer, flush } = later();
    const watch = focusWatch(sync, defer);
    watch.onOut();
    flush();
    expect(sync).toHaveBeenCalledTimes(1);
  });

  it("keeps one read waiting however many times focus leaves, and none after it stops", () => {
    const sync = vi.fn();
    const { defer, flush, waiting } = later();
    const watch = focusWatch(sync, defer);
    watch.onOut();
    watch.onOut();
    expect(waiting).toHaveLength(1);
    watch.stop();
    flush();
    expect(sync).not.toHaveBeenCalled();
  });
});
