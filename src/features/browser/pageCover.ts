import { useEffect, useSyncExternalStore } from "react";

// A page is a native webview over the main one, so a menu, a dialog or a drag drawn by the main webview would sit under it.
// Whatever draws one adds a cover while it is drawn; the page freezes as a picture while there is at least one.
let covers = 0;
const listeners = new Set<() => void>();

const emit = () => listeners.forEach((l) => l());

/** Adds one cover. The returned function removes it, once. */
export function coverPage(): () => void {
  covers += 1;
  if (covers === 1) emit();
  let released = false;
  return () => {
    if (released) return;
    released = true;
    covers -= 1;
    if (covers === 0) emit();
  };
}

export function pageCovered(): boolean {
  return covers > 0;
}

/** Calls `listener` when `pageCovered()` changes, not for every cover. Returns the unsubscribe function. */
export function subscribeCover(listener: () => void): () => void {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

/** True while anything (a menu, a dialog, a drag) is drawn over the page. */
export function usePageCovered(): boolean {
  return useSyncExternalStore(subscribeCover, pageCovered, () => false);
}

/** Covers the page while the calling component is mounted and `on`. */
export function useCoverPage(on = true): void {
  useEffect(() => {
    if (!on) return;
    return coverPage();
  }, [on]);
}
