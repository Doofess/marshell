import { describe, expect, it, vi } from "vitest";
import { backgroundSync, type Rgb } from "./windowBackground";

const BLACK: Rgb = [0, 0, 0];
const WHITE: Rgb = [255, 255, 255];

/** Lets the sync's send settle. */
const settle = () => new Promise<void>((done) => setTimeout(done, 0));

describe("backgroundSync", () => {
  it("sends the page's background the first time", () => {
    const send = vi.fn(async () => {});
    backgroundSync(() => BLACK, send)();
    expect(send.mock.calls).toEqual([[BLACK]]);
  });

  it("sends an unchanged background once", () => {
    const send = vi.fn(async () => {});
    const sync = backgroundSync(() => [0, 0, 0], send);
    sync();
    sync();
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("sends again when the background changed", () => {
    const send = vi.fn(async () => {});
    let now = BLACK;
    const sync = backgroundSync(() => now, send);
    sync();
    now = WHITE;
    sync();
    expect(send.mock.calls).toEqual([[BLACK], [WHITE]]);
  });

  it("sends the same background again after a send that failed", async () => {
    const send = vi.fn(async () => {});
    send.mockRejectedValueOnce(new Error("no window"));
    const sync = backgroundSync(() => BLACK, send);
    sync();
    await settle();
    sync();
    expect(send).toHaveBeenCalledTimes(2);
  });

  it("keeps a newer background when an older send fails late", async () => {
    let failFirst: (e: Error) => void = () => {};
    const send = vi.fn(async () => {});
    send.mockImplementationOnce(() => new Promise<void>((_, reject) => (failFirst = reject)));
    let now = BLACK;
    const sync = backgroundSync(() => now, send);
    sync();
    now = WHITE;
    sync();
    failFirst(new Error("late"));
    await settle();
    sync();
    expect(send.mock.calls).toEqual([[BLACK], [WHITE]]);
  });

  it("sends nothing when the background can't be read", () => {
    const send = vi.fn(async () => {});
    backgroundSync(() => null, send)();
    expect(send).not.toHaveBeenCalled();
  });
});
