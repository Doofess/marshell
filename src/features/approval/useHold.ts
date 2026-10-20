import { useCallback, useEffect, useRef, useState } from "react";
import { HOLD_MS, holdProgress } from "./arming";

/**
 * Hold-to-allow for risky requests: `begin()` on key or pointer down, `cancel()` on release. `onComplete` fires once
 * after 600 ms, provided the card is still armed and a keyboard hold still owns focus.
 */
export function useHold(onComplete: () => void, armed = true, hasFocus: () => boolean = () => true): { progress: number; begin: (source?: "keyboard" | "pointer") => void; cancel: () => void } {
  const [progress, setProgress] = useState(0);
  const startedAt = useRef<number | null>(null);
  const frame = useRef(0);
  const source = useRef<"keyboard" | "pointer">("keyboard");
  const latest = useRef({ onComplete, armed, hasFocus });
  latest.current = { onComplete, armed, hasFocus };

  const cancel = useCallback(() => {
    startedAt.current = null;
    cancelAnimationFrame(frame.current);
    setProgress(0);
  }, []);

  const tick = useCallback(() => {
    if (startedAt.current === null) return;
    if (!latest.current.armed || (source.current === "keyboard" && !latest.current.hasFocus())) {
      cancel();
      return;
    }
    const held = performance.now() - startedAt.current;
    setProgress(holdProgress(held));
    if (held >= HOLD_MS) {
      startedAt.current = null;
      latest.current.onComplete();
    } else frame.current = requestAnimationFrame(tick);
  }, [cancel]);

  const begin = useCallback((via: "keyboard" | "pointer" = "keyboard") => {
    if (!latest.current.armed || startedAt.current !== null) return;
    source.current = via;
    startedAt.current = performance.now();
    frame.current = requestAnimationFrame(tick);
  }, [tick]);

  useEffect(() => cancel, [cancel]);
  useEffect(() => { if (!armed) cancel(); }, [armed, cancel]);
  useEffect(() => {
    window.addEventListener("blur", cancel);
    return () => window.removeEventListener("blur", cancel);
  }, [cancel]);
  return { progress, begin, cancel };
}
