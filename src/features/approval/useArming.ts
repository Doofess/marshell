import { useCallback, useEffect, useRef, useState } from "react";
import { ARM_DELAY_MS } from "./arming";

/** False for 500 ms after mount, after `resetKey` changes, and after `rearm()` (called on focus). */
export function useArming(resetKey: unknown): { armed: boolean; rearm: () => void } {
  const [armed, setArmed] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const rearm = useCallback(() => {
    setArmed(false);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setArmed(true), ARM_DELAY_MS);
  }, []);
  useEffect(() => {
    rearm();
    return () => window.clearTimeout(timer.current);
  }, [resetKey, rearm]);
  return { armed, rearm };
}
