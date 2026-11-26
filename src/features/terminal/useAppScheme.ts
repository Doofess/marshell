import { useSyncExternalStore } from "react";
import { appScheme, type Scheme } from "./terminalTheme";

export type SchemeRoot = HTMLElement;
export type SchemeMedia = {
  addEventListener(type: "change", cb: () => void): void;
  removeEventListener(type: "change", cb: () => void): void;
};
/** Calls back whenever the root's `data-theme` changes; the return value stops it (a `MutationObserver` in the app). */
export type WatchTheme = (root: SchemeRoot, cb: () => void) => () => void;

/** A store over the scheme the app shows: the root's `data-theme` and the system's `prefers-color-scheme`. */
export function schemeStore(root: SchemeRoot, media: SchemeMedia, watch: WatchTheme): { subscribe(cb: () => void): () => void; get(): Scheme } {
  return {
    subscribe(cb) {
      const stopTheme = watch(root, cb);
      media.addEventListener("change", cb);
      return () => {
        stopTheme();
        media.removeEventListener("change", cb);
      };
    },
    get: () => appScheme(root),
  };
}

const observe: WatchTheme = (root, cb) => {
  const observer = new MutationObserver(cb);
  observer.observe(root, { attributes: true, attributeFilter: ["data-theme"] });
  return () => observer.disconnect();
};

let store: ReturnType<typeof schemeStore> | undefined;
// Built on first use, so nothing touches `document` or `matchMedia` at module load or during a node render.
const shared = () => (store ??= schemeStore(document.documentElement, matchMedia("(prefers-color-scheme: dark)"), observe));
const subscribe = (cb: () => void) => shared().subscribe(cb);
const get = () => shared().get();

/** The scheme the app is showing, updated as the root's theme or the system's scheme changes. */
export function useAppScheme(): Scheme {
  return useSyncExternalStore(subscribe, get, () => "dark");
}
