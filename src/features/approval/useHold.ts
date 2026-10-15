import { useCallback, useEffect, useRef, useState } from "react";
import { HOLD_MS, holdProgress } from "./arming";

/**
 * Hold-to-allow for risky requests: `begin()` on key or pointer down, `cancel()` on release. `onComplete` fires once
 * after 600 ms. Timing uses requestAnimationFrame, so it is exercised by the Interactive story, not by unit tests;
 * the numbers themselves are tested in arming.test.ts.
 */
export function useHold(onComplete: () => void): { progress: number; begin: () => void; cancel: () => void } {
  const [progress, setProgress] = useState(0);
  const startedAt = useRef<number | null>(null);
  const frame = useRef(0);

  const cancel = useCallback(() => {
    startedAt.current = null;
    cancelAnimationFrame(frame.current);
    setProgress(0);
  }, []);

  const tick = useCallback(() => {
    if (startedAt.current === null) return;
    const held = performance.now() - startedAt.current;
    setProgress(holdProgress(held));
    if (held >= HOLD_MS) {
      startedAt.current = null;
      onComplete();
    } else frame.current = requestAnimationFrame(tick);
  }, [onComplete]);

  const begin = useCallback(() => {
    if (startedAt.current !== null) return;
    startedAt.current = performance.now();
    frame.current = requestAnimationFrame(tick);
  }, [tick]);

  useEffect(() => cancel, [cancel]);
  return { progress, begin, cancel };
}
