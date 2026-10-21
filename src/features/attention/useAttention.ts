import { useEffect, useRef } from "react";
import { setBadge } from "./attention";

/** Keeps the taskbar count in step with the needs-you count. Banners and sounds are the core's job, not the window's. */
export function useAttention(needsYou: number): void {
  const count = useRef<number | null>(null);

  useEffect(() => {
    if (needsYou !== count.current) {
      count.current = needsYou;
      void setBadge(needsYou);
    }
  }, [needsYou]);
}
