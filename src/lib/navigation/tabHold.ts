/**
 * A rolled Ctrl+Shift+Tab (docs/PLAN.md §9, "Slice 6q-3 decisions" 14 and 15). Ctrl+Tab ("Next tab") is the navigation chord
 * Ctrl+Shift+Tab without its Shift, and a quick hand can put Tab down a few milliseconds before Shift. So a fresh Ctrl+Tab is held
 * until Tab comes up or TAB_HOLD_MS pass, and goes on as Ctrl+Shift+Tab if Shift went down with Ctrl meanwhile.
 */
export const TAB_HOLD_MS = 50;

export type HoldKey = { type: "keydown" | "keyup"; code: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean; repeat: boolean };
/** `release` sends the held chord now (`shift`: as Ctrl+Shift+Tab); `swallow` keeps this key from every other handler. */
export type HoldStep = { held: boolean; release: { shift: boolean } | null; swallow: boolean };

const SHIFT = /^Shift(Left|Right)$/;

/** Only Ctrl and Shift, with no Alt or Meta, make the navigation chord. */
const chordShift = (e: HoldKey): boolean => e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey;

/** What a key does to a Ctrl+Tab that may be held. */
export function tabHoldStep(held: boolean, e: HoldKey): HoldStep {
  if (!held) {
    const fresh = e.type === "keydown" && e.code === "Tab" && e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && !e.repeat;
    return { held: fresh, release: null, swallow: fresh };
  }
  if (e.type === "keyup") {
    // Ctrl coming up first changes nothing yet; the timer ends the hold.
    return e.code === "Tab" ? { held: false, release: { shift: chordShift(e) }, swallow: false } : { held: true, release: null, swallow: false };
  }
  if (e.code === "Tab" && e.repeat) return { held: true, release: null, swallow: true };
  // Shift turns the held key into Ctrl+Shift+Tab; any other key ends the hold as Ctrl+Tab, and then goes on itself.
  return { held: false, release: { shift: SHIFT.test(e.code) && chordShift(e) }, swallow: false };
}
