import { useEffect, useRef } from "react";
import { alertNeedsYou, attentionStep, setBadge } from "./attention";

/** Keeps the taskbar count in step with the entries and flashes (with the sound unless it is off) for a new one while the window isn't focused. */
export function useAttention(entries: { id: string; tab: string }[], mutedTabs: Set<string>, soundOff: () => boolean): void {
  const prev = useRef<{ id: string; tab: string }[]>([]);
  const count = useRef<number | null>(null);
  const latest = useRef({ mutedTabs, soundOff });
  latest.current = { mutedTabs, soundOff };

  useEffect(() => {
    const step = attentionStep(prev.current, entries, document.hasFocus(), latest.current.mutedTabs);
    prev.current = entries;
    if (step.count !== count.current) {
      count.current = step.count;
      void setBadge(step.count);
    }
    if (step.alert) void alertNeedsYou(!latest.current.soundOff());
  }, [entries]);
}
