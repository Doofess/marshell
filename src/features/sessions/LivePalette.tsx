import { useEffect, useMemo, useRef, useState } from "react";
import { commandShortcut } from "../../lib/appShortcuts";
import { COMMANDS, type Os } from "../../lib/keymap";
import { Palette } from "../palette/Palette";
import type { PaletteSession } from "../palette/fixtures";

/** Commands the window runs from the palette that have no key of their own: remove the focused pane, and open the layout menu. */
const PALETTE_ONLY = ["profiles", "appearance", "unsplit", "layout"];

/**
 * The commands the live window runs: the ones with a session shortcut, the pane commands, and the shell profiles and Appearance pages. Listing any other would
 * be a row that does nothing. (The palette itself is left out: choosing it from itself only closes it.)
 */
export const LIVE_COMMANDS = COMMANDS.filter((c) => PALETTE_ONLY.includes(c.id) || (c.id !== "palette" && commandShortcut(c.id) !== null));

/**
 * The command palette over the live window. It owns the typed text and the active row; the window decides what a command
 * does (`onRun` gets a command id, or `session:<id>`). Mounted only while open. Focus goes to the box and, on close, back to
 * where it was (usually the terminal).
 */
export function LivePalette({ sessions, os, labels, onRun, onClose }: { sessions: PaletteSession[]; os: Os; labels?: Record<string, string>; onRun: (id: string) => void; onClose: () => void }) {
  // A command whose words depend on the window's state (the sidebar command names what it will do next) gets its label from the window.
  const commands = useMemo(() => (labels ? LIVE_COMMANDS.map((c) => (labels[c.id] === undefined ? c : { ...c, label: labels[c.id]! })) : LIVE_COMMANDS), [labels]);
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
        commands={commands}
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
