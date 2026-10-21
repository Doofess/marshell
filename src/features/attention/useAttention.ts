import { useEffect, useRef } from "react";
import { badgeCount, setBadge } from "./attention";

/** Keeps the taskbar count in step with the entries. Banners and sounds are the core's job, not the window's. */
export function useAttention(entries: { id: string; tab: string }[]): void {
  const count = useRef<number | null>(null);

  useEffect(() => {
    const next = badgeCount(entries);
    if (next !== count.current) {
      count.current = next;
      void setBadge(next);
    }
  }, [entries]);
}
