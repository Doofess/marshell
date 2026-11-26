import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import { schemeStore, useAppScheme } from "./useAppScheme";

afterEach(() => vi.unstubAllGlobals());

/** Fakes for the three things the store touches; `computed` is what the root's `color-scheme` reads as. */
const fakes = (computed: string) => {
  const state = { computed, theme: null as null | (() => void), media: null as null | (() => void), stopped: 0 };
  vi.stubGlobal("getComputedStyle", () => ({ colorScheme: state.computed }));
  vi.stubGlobal("matchMedia", () => ({ matches: false }));
  const media = {
    addEventListener: (_: "change", cb: () => void) => void (state.media = cb),
    removeEventListener: () => void (state.stopped += 1),
  };
  const watch = (_root: unknown, cb: () => void) => {
    state.theme = cb;
    return () => void (state.stopped += 1);
  };
  return { state, root: {} as HTMLElement, media, watch };
};

describe("schemeStore", () => {
  it("the store answers with the root's scheme", () => {
    const f = fakes("dark");
    const store = schemeStore(f.root, f.media, f.watch);
    expect(store.get()).toBe("dark");
    f.state.computed = "light";
    expect(store.get()).toBe("light");
  });
  it("it tells its listener when the root's theme changes and when the system's scheme changes", () => {
    const f = fakes("");
    const store = schemeStore(f.root, f.media, f.watch);
    let told = 0;
    store.subscribe(() => void (told += 1));
    f.state.theme?.();
    f.state.media?.();
    expect(told).toBe(2);
  });
  it("unsubscribing stops both", () => {
    const f = fakes("");
    const store = schemeStore(f.root, f.media, f.watch);
    store.subscribe(() => {})();
    expect(f.state.stopped).toBe(2);
  });
});

describe("useAppScheme", () => {
  it("useAppScheme renders in node", () => {
    const C = () => createElement("i", null, useAppScheme());
    expect(() => renderToStaticMarkup(createElement(C))).not.toThrow();
  });
});
