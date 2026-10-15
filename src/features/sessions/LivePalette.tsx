import { useEffect, useRef, useState } from "react";
import type { Os } from "../../lib/keymap";
import { Palette } from "../palette/Palette";
import type { PaletteSession } from "../palette/fixtures";

/**
 * The command palette over the live window. It owns the typed text and the active row; the window decides what a command
 * does (`onRun` gets a command id, or `session:<id>`). Mounted only while open. Focus goes to the box and, on close, back to
 * where it was (usually the terminal).
 */
export function LivePalette({ sessions, os, onRun, onClose }: { sessions: PaletteSession[]; os: Os; onRun: (id: string) => void; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const before = document.activeElement;
    root.current?.querySelector<HTMLElement>(".palette__input")?.focus();
    return () => {
      if (before instanceof HTMLElement && before !== document.body && before.isConnected) before.focus();
    };
  }, []);
  return (
    <div ref={root} className="live-app__palette">
      <Palette
        view={{ query, activeIndex }}
        sessions={sessions}
        os={os}
        onQuery={(q) => {
          setQuery(q);
          setActiveIndex(0);
        }}
        onActive={setActiveIndex}
        onRun={onRun}
        onClose={onClose}
      />
    </div>
  );
}
