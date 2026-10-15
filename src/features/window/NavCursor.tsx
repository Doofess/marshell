import type { Os } from "../../lib/keymap";
import { navModifierLabel, DEFAULT_NAV_MODIFIER, type NavModifier } from "../../lib/navigation/modifier";
import type { Rect } from "../../lib/navigation/types";
import "./NavCursor.css";

/** The line under the cursor: what releasing does, and the one key that selects nothing. */
export function navHint(os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER): string {
  return `Release ${navModifierLabel(os, nav)} to select · Backspace for none`;
}

/** What the status bar says while no terminal is selected, so an empty selection never looks like a broken window. */
export const NO_SELECTION_TEXT = "No session selected · Tab to select a terminal";

/**
 * The navigation cursor: a tinted fill and a bar on the start edge over whatever the arrows are on, drawn only while the
 * modifier is held. It is a picture (aria-hidden); the live region below says where it is. Place it inside a positioned
 * container (the window) with `rect` measured against that container.
 */
export function NavCursor({ rect, label, os, nav }: { rect: Rect | null; label: string; os: Os; nav?: NavModifier }) {
  if (!rect) return null;
  return (
    <>
      <div className="nav-cursor" aria-hidden="true" data-label={label} style={{ insetInlineStart: rect.x, insetBlockStart: rect.y, inlineSize: rect.w, blockSize: rect.h }} />
      <p className="nav-hint" aria-hidden="true">
        <span className="nav-hint__target">{label}</span>
        <span className="nav-hint__keys">{navHint(os, nav)}</span>
      </p>
    </>
  );
}

/** The one live region of the navigation layer: "Selected session 6, billing", "No session selected". Empty until something is said. */
export function NavAnnouncer({ text }: { text: string }) {
  return (
    <div className="nav-announcer" role="status" aria-live="polite" aria-atomic="true" data-nav-announcer>
      {text}
    </div>
  );
}
