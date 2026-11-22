import { useEffect, useRef, useState } from "react";
import type { FilterMode } from "./filterText";

export const LISTING = "Listing files…";
export const SEARCHING = "Searching…";
export const RING_DELAY_MS = 150;

/** The palette's words (`Palette.tsx`), with its fallback when there is no folder. */
export const listFailed = (folder: string | null): string => (folder ? `Can't list the files in ${folder}.` : "Can't list files for this session.");

/** What Files waits for, as the ring's line, or null. */
export function filesWait(o: { mode: FilterMode; names: "loading" | "error" | "ready"; searchBusy: boolean }): string | null {
  if (o.mode.kind === "names" && o.names === "loading") return LISTING;
  if (o.mode.kind === "search" && o.searchBusy) return SEARCHING;
  return null;
}

/** Turns `on` into "shown" only after `ms` of being on; off at once. */
export function createShownAfter(ms: number, set: (shown: boolean) => void): { update(on: boolean): void; dispose(): void } {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const clear = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };
  return {
    update(on) {
      if (!on) {
        clear();
        set(false);
      } else if (timer === null) {
        timer = setTimeout(() => {
          timer = null;
          set(true);
        }, ms);
      }
    },
    dispose: clear,
  };
}

export function useShownAfter(on: boolean, ms: number = RING_DELAY_MS): boolean {
  const [shown, setShown] = useState(false);
  const timing = useRef<ReturnType<typeof createShownAfter> | null>(null);
  useEffect(() => {
    const t = createShownAfter(ms, setShown);
    timing.current = t;
    return () => t.dispose();
  }, [ms]);
  useEffect(() => {
    timing.current?.update(on);
  }, [on]);
  return on && shown;
}

/** Whether focus goes to the filter box: the wait has just begun and focus was in the tree area when it did. */
export function focusToFilter(o: { waitBegan: boolean; focusWasInTree: boolean }): boolean {
  return o.waitBegan && o.focusWasInTree;
}
