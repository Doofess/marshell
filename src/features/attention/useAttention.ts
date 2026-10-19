import { useEffect, useRef } from "react";
import { alertNeedsYou, attentionStep, setBadge } from "./attention";

/** Keeps the taskbar count in step with the entries and flashes (with a chime unless the sound is off) for a new one while the window isn't focused. */
export function useAttention(entries: { id: string; tab: string }[], mutedTabs: Set<string>, soundOff: () => boolean): void {
  const focused = useRef(true);
  const prev = useRef<string[]>([]);
  const count = useRef<number | null>(null);
  const latest = useRef({ mutedTabs, soundOff });
  latest.current = { mutedTabs, soundOff };

  useEffect(() => {
    const on = () => (focused.current = true);
    const off = () => (focused.current = false);
    focused.current = document.hasFocus();
    window.addEventListener("focus", on);
    window.addEventListener("blur", off);
    return () => {
      window.removeEventListener("focus", on);
      window.removeEventListener("blur", off);
    };
  }, []);

  useEffect(() => {
    const step = attentionStep(prev.current, entries, focused.current, latest.current.mutedTabs);
    prev.current = entries.map((e) => e.id);
    if (step.count !== count.current) {
      count.current = step.count;
      void setBadge(step.count);
    }
    if (step.alert) void alertNeedsYou(!latest.current.soundOff());
  }, [entries]);
}
