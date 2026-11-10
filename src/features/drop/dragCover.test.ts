import { describe, expect, it } from "vitest";
import { createDragCover, PAGE_LEAVE_GRACE_MS } from "./dragCover";

/** Counts the covers held, and keeps the timers so a test can run or check them. */
function world() {
  let held = 0;
  const timers: { fn: () => void; ms: number; cancelled: boolean }[] = [];
  const cover = createDragCover(
    () => {
      held += 1;
      let released = false;
      return () => {
        if (released) return;
        released = true;
        held -= 1;
      };
    },
    (fn, ms) => {
      const t = { fn, ms, cancelled: false };
      timers.push(t);
      return () => {
        t.cancelled = true;
      };
    },
  );
  const runTimers = () => timers.splice(0).forEach((t) => !t.cancelled && t.fn());
  return { cover, held: () => held, timers, runTimers };
}

describe("createDragCover", () => {
  it("a drag that enters over a page and leaves the window uncovers after the grace", () => {
    const w = world();
    w.cover.pageEnter();
    expect(w.held()).toBe(1);
    w.cover.pageLeave();
    expect(w.held()).toBe(1);
    expect(w.timers.map((t) => t.ms)).toEqual([PAGE_LEAVE_GRACE_MS]);
    w.runTimers();
    expect(w.held()).toBe(0);
  });

  it("the app's own enter takes over from the page's, and its leave uncovers", () => {
    const w = world();
    w.cover.pageEnter();
    // The page is hidden under the drag, so it hears a leave; the app's webview hears the enter next.
    w.cover.pageLeave();
    w.cover.appEnter();
    expect(w.held()).toBe(1);
    w.runTimers();
    expect(w.held()).toBe(1);
    w.cover.appEnd();
    expect(w.held()).toBe(0);
  });

  it("the page's leave after the app's enter changes nothing", () => {
    const w = world();
    w.cover.pageEnter();
    w.cover.appEnter();
    w.cover.pageLeave();
    expect(w.timers).toEqual([]);
    expect(w.held()).toBe(1);
    w.cover.appEnd();
    expect(w.held()).toBe(0);
  });

  it("a drop on the page uncovers at once", () => {
    const w = world();
    w.cover.pageEnter();
    w.cover.pageDrop();
    expect(w.held()).toBe(0);
  });

  it("a drag back over the page inside the grace keeps the cover", () => {
    const w = world();
    w.cover.pageEnter();
    w.cover.pageLeave();
    w.cover.pageEnter();
    w.runTimers();
    expect(w.held()).toBe(1);
  });

  it("a drag that enters over the app holds one cover until it ends", () => {
    const w = world();
    w.cover.appEnter();
    w.cover.appEnter();
    expect(w.held()).toBe(1);
    w.cover.appEnd();
    w.cover.appEnd();
    expect(w.held()).toBe(0);
  });

  it("dispose lets go of everything and cancels the grace", () => {
    const w = world();
    w.cover.pageEnter();
    w.cover.pageLeave();
    w.cover.dispose();
    expect(w.held()).toBe(0);
    expect(w.timers.every((t) => t.cancelled)).toBe(true);
  });
});
