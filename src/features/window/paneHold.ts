import { useEffect, useState } from "react";
import { pageShown } from "../browser/usePageSlot";

/** The longest a hide holds the pane shown while its page freezes. The freeze was measured at about 52 ms; this leaves room. */
export const HOLD_CAP_MS = 150;

/**
 * Whether a change in what the pane is wanted as starts a hold. Only a hide does, and only while a page is on screen: the pane
 * stays shown until that page has frozen as a picture. A show never holds, so a hide undone at once finds the pane live.
 */
export function holdOnChange(wanted: boolean, pageShown: boolean): boolean {
  return !wanted && pageShown;
}

/** Whether a hold that has lasted `elapsed` ms is over: the freeze has hidden the page, or the cap is reached. */
export function holdEnds(pageShown: boolean, elapsed: number): boolean {
  return !pageShown || elapsed >= HOLD_CAP_MS;
}

/**
 * Whether the side pane is held shown after a hide was asked for. The hold starts during the render that sees the change, so no
 * frame draws the pane hidden first; it ends once the page has frozen (polled once per frame) or at the cap.
 */
export function usePaneHold(wanted: boolean): boolean {
  const [previous, setPrevious] = useState(wanted);
  const [held, setHeld] = useState(false);
  if (previous !== wanted) {
    setPrevious(wanted);
    setHeld(holdOnChange(wanted, pageShown()));
  }
  useEffect(() => {
    if (!held) return;
    const start = performance.now();
    let frame = 0;
    const poll = () => {
      if (holdEnds(pageShown(), performance.now() - start)) {
        setHeld(false);
        return;
      }
      frame = requestAnimationFrame(poll);
    };
    frame = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(frame);
  }, [held]);
  return held;
}
