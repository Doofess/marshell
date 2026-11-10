import { createContext } from "react";

/**
 * Counts overlapping holds. `onRelease` runs once, when the last one ends, so a terminal that is being moved or resized by
 * a layout motion fits once at the end instead of once per frame. A release called twice counts once.
 */
export function fitHold(onRelease: () => void): { hold(): () => void; held(): boolean } {
  let count = 0;
  return {
    hold() {
      count++;
      let released = false;
      return () => {
        if (released) return;
        released = true;
        count--;
        if (count === 0) onRelease();
      };
    },
    held: () => count > 0,
  };
}

/** True while a layout motion runs (the window gives it; see `useLayoutMotion`). A terminal holds its fit for as long as it is. */
export const FitHoldContext = createContext(false);
