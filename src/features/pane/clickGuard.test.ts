import { describe, expect, it } from "vitest";
import { clickGuard } from "./clickGuard";

/** A scheduler that runs what it was given only when told to, as a timer would after the event. */
function later() {
  const queue: Array<() => void> = [];
  return { schedule: (run: () => void) => void queue.push(run), flush: () => queue.splice(0).forEach((run) => run()) };
}

describe("clickGuard", () => {
  it("swallows the click that ends a drag, once", () => {
    const t = later();
    const guard = clickGuard(t.schedule);
    guard.arm();
    expect(guard.take()).toBe(true);
    expect(guard.take()).toBe(false);
  });

  it("lets a click through when nothing armed it", () => {
    expect(clickGuard(later().schedule).take()).toBe(false);
  });

  it("forgets a drag whose click never came, so the next real click is not swallowed", () => {
    // A reorder moves the tab's node; the release can then land on another tab, and the click goes to neither.
    const t = later();
    const guard = clickGuard(t.schedule);
    guard.arm();
    t.flush();
    expect(guard.take()).toBe(false);
  });

  it("a second drag's click is still swallowed after the first one's timer ran", () => {
    const t = later();
    const guard = clickGuard(t.schedule);
    guard.arm();
    expect(guard.take()).toBe(true);
    guard.arm();
    expect(guard.take()).toBe(true);
    t.flush();
    expect(guard.take()).toBe(false);
  });
});
