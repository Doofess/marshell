import { describe, expect, it } from "vitest";
import { syncPageHidden } from "./visibility";

function fakeDoc(state: "visible" | "hidden") {
  const listeners: Array<() => void> = [];
  const attrs = new Map<string, string>();
  const doc = {
    visibilityState: state as DocumentVisibilityState,
    documentElement: {
      setAttribute: (k: string, v: string) => attrs.set(k, v),
      removeAttribute: (k: string) => attrs.delete(k),
    } as unknown as HTMLElement,
    addEventListener: (_: string, fn: () => void) => listeners.push(fn),
    removeEventListener: (_: string, fn: () => void) => listeners.splice(listeners.indexOf(fn), 1),
  };
  return { doc, attrs, listeners };
}

describe("syncPageHidden", () => {
  it("marks a hidden page at once", () => {
    const { doc, attrs } = fakeDoc("hidden");
    syncPageHidden(doc);
    expect(attrs.has("data-page-hidden")).toBe(true);
  });
  it("follows visibility changes and unsubscribes", () => {
    const { doc, attrs, listeners } = fakeDoc("visible");
    const stop = syncPageHidden(doc);
    expect(attrs.has("data-page-hidden")).toBe(false);
    doc.visibilityState = "hidden";
    listeners.forEach((l) => l());
    expect(attrs.has("data-page-hidden")).toBe(true);
    stop();
    expect(listeners).toHaveLength(0);
  });
});
