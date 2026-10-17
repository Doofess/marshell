import { useId, useRef, type KeyboardEvent } from "react";
import { commandShortcut, sessionShortcut } from "../../lib/appShortcuts";
import type { KeyEventLike } from "../../lib/bindings";
import type { Os } from "../../lib/keymap";
import type { NavModifier } from "../../lib/navigation/modifier";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { ShortcutSheet } from "../shortcuts/ShortcutSheet";
import "./LiveShortcuts.css";

/**
 * The keys the live window runs: every command it takes as a shortcut, the palette's own key, the navigation chords and the
 * session numbers among them. Any other key is not taught.
 */
const runs = (id: string) => commandShortcut(id) !== null;

/** Ctrl+Shift+O (Cmd+O) pressed while the sheet is up: the window's listener stands down for an overlay, so the sheet closes itself. */
export const closesShortcuts = (e: KeyEventLike, os: Os): boolean => sessionShortcut(e, os)?.kind === "cheatsheet";

/**
 * The keyboard shortcuts sheet over the live window (Ctrl+Shift+O, or the palette). A modal dialog: focus starts on it, Tab stays
 * inside, Escape or the same key again closes it, and focus goes back to where it was (usually the terminal).
 */
export function LiveShortcuts({ os, nav, onClose }: { os: Os; nav?: NavModifier; onClose: () => void }) {
  const dialog = useRef<HTMLDivElement>(null);
  const titleId = useId();
  useDialogFocus(dialog, dialog, onClose);
  const onKeyDown = (e: KeyboardEvent) => {
    if (!closesShortcuts(e, os)) return;
    e.preventDefault();
    onClose();
  };
  return (
    <div className="live-shortcuts">
      <div className="live-shortcuts__backdrop" onClick={onClose} />
      <div ref={dialog} className="live-shortcuts__sheet" role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1} onKeyDown={onKeyDown}>
        <ShortcutSheet os={os} nav={nav} runs={runs} titleId={titleId} />
      </div>
    </div>
  );
}
