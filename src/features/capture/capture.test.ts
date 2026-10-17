import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RowModel } from "../sidebar/types";
import { cancelAfterFocus, captureFailure, captureRefusal, imageBlob, watchFocusCancel } from "./capture";

const buffer = (...bytes: number[]): ArrayBuffer => new Uint8Array(bytes).buffer;

describe("imageBlob", () => {
  it("is null for an empty buffer", () => {
    expect(imageBlob(new ArrayBuffer(0))).toBeNull();
  });

  it("types a buffer that starts with the PNG signature as image/png", () => {
    const blob = imageBlob(buffer(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a));
    expect(blob?.type).toBe("image/png");
    expect(blob?.size).toBe(6);
  });

  it("types a buffer that starts with BM as image/bmp", () => {
    const blob = imageBlob(buffer(0x42, 0x4d, 1, 2, 3));
    expect(blob?.type).toBe("image/bmp");
    expect(blob?.size).toBe(5);
  });

  it("is null for bytes that are neither", () => {
    expect(imageBlob(buffer(0x47, 0x49, 0x46, 0x38))).toBeNull();
    expect(imageBlob(buffer(0x42))).toBeNull();
  });
});

describe("cancelAfterFocus", () => {
  it("waits until the window has had focus back for 1500 ms", () => {
    expect(cancelAfterFocus(1000, 2499)).toBe(false);
    expect(cancelAfterFocus(1000, 2500)).toBe(true);
  });
});

describe("watchFocusCancel", () => {
  let clock = 0;
  const now = () => clock;
  const wait = (ms: number) => {
    clock += ms;
    vi.advanceTimersByTime(ms);
  };
  beforeEach(() => {
    clock = 0;
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it("cancels once the window has had focus back for 1500 ms", () => {
    const target = new EventTarget();
    const onCancel = vi.fn();
    watchFocusCancel(onCancel, target, now);
    target.dispatchEvent(new Event("focus"));
    wait(1499);
    expect(onCancel).not.toHaveBeenCalled();
    wait(1);
    expect(onCancel).toHaveBeenCalledTimes(1);
    wait(5000);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("starts the count over when the focus is lost again before it fires", () => {
    const target = new EventTarget();
    const onCancel = vi.fn();
    watchFocusCancel(onCancel, target, now);
    target.dispatchEvent(new Event("focus"));
    wait(1000);
    target.dispatchEvent(new Event("blur"));
    wait(5000);
    expect(onCancel).not.toHaveBeenCalled();
    target.dispatchEvent(new Event("focus"));
    wait(1500);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("does nothing while the window never got focus back", () => {
    const onCancel = vi.fn();
    watchFocusCancel(onCancel, new EventTarget(), now);
    wait(60000);
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("stops: no listener and no timer is left once the capture ends", () => {
    const target = new EventTarget();
    const onCancel = vi.fn();
    const stop = watchFocusCancel(onCancel, target, now);
    target.dispatchEvent(new Event("focus"));
    wait(1000);
    stop();
    expect(vi.getTimerCount()).toBe(0);
    target.dispatchEvent(new Event("focus"));
    expect(vi.getTimerCount()).toBe(0);
    wait(5000);
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("still cancels when its timer fires a little early", () => {
    const target = new EventTarget();
    const onCancel = vi.fn();
    watchFocusCancel(onCancel, target, now);
    target.dispatchEvent(new Event("focus"));
    clock = 1499;
    vi.advanceTimersByTime(1500);
    expect(onCancel).not.toHaveBeenCalled();
    wait(1);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});

describe("captureFailure", () => {
  it("says a command's own sentence as it is, and never an empty one", () => {
    expect(captureFailure("A capture is already running.")).toBe("A capture is already running.");
    expect(captureFailure(new Error("Document is not focused."))).toBe("Document is not focused.");
    expect(captureFailure("")).toBe("Something went wrong.");
    expect(captureFailure(undefined)).toBe("Something went wrong.");
  });
});

describe("captureRefusal", () => {
  const rows = [
    { id: "t1", name: "api", status: "paused" },
    { id: "t2", name: "web", status: "idle" },
  ] as unknown as RowModel[];
  it("refuses before the snipping tool opens: no focused session, a paused or an ended one", () => {
    expect(captureRefusal(null, rows)).toBe("No session to add it to.");
    expect(captureRefusal("t9", rows)).toBe("No session to add it to.");
    expect(captureRefusal("t1", rows)).toBe("api is paused.");
  });
  it("lets a live session capture", () => {
    expect(captureRefusal("t2", rows)).toBeNull();
  });
});
