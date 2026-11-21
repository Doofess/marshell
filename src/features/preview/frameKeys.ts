/** The parts of a key event read here, so a test needs no DOM. */
export type FrameKey = Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "shiftKey" | "altKey" | "metaKey" | "repeat" | "defaultPrevented"> & { type: string; getModifierState?: (key: string) => boolean };

const MODIFIERS: ReadonlySet<string> = new Set(["Control", "Shift", "Alt", "Meta"]);

/**
 * Whether a key pressed in a rendered HTML file's frame is passed on to the app. The frame is a document of its own, so the
 * app's shortcuts, which listen on the window, never see its keys. A press with Ctrl, Alt or the command key is a chord and goes
 * on, unless the pane's own handler already used it; every other press is the page's (letters, Tab, the arrows, Esc). A release
 * goes on for a modifier and for a key in `held`, the codes whose press was passed on: the app's key holds wait for both.
 */
export function forwardsToApp(e: FrameKey, held: ReadonlySet<string>): boolean {
  if (e.type === "keyup") return MODIFIERS.has(e.key) || held.has(e.code);
  // AltGr reports Ctrl and Alt together; it types a character, so it is the page's.
  if (e.getModifierState?.("AltGraph") === true) return false;
  return !e.defaultPrevented && (e.ctrlKey || e.altKey || e.metaKey);
}

/** `forwardsToApp` with its bookkeeping: a forwarded press adds its code to `held`, a forwarded release removes it. True when the key goes on to the app. */
export function trackHeld(e: FrameKey, held: Set<string>): boolean {
  if (!forwardsToApp(e, held)) return false;
  if (e.type === "keydown") held.add(e.code);
  else held.delete(e.code);
  return true;
}

/** The copy of a frame's key that the app's document gets: it bubbles to the window and can be cancelled. */
export function appKeyInit(e: FrameKey): KeyboardEventInit {
  return { key: e.key, code: e.code, ctrlKey: e.ctrlKey, shiftKey: e.shiftKey, altKey: e.altKey, metaKey: e.metaKey, repeat: e.repeat, bubbles: true, cancelable: true };
}
