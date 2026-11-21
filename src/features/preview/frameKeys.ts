/** The parts of a key event read here, so a test needs no DOM. */
export type FrameKey = Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "shiftKey" | "altKey" | "metaKey" | "repeat" | "defaultPrevented"> & { type: string };

const MODIFIERS: ReadonlySet<string> = new Set(["Control", "Shift", "Alt", "Meta"]);

/**
 * Whether a key pressed in a rendered HTML file's frame is passed on to the app. The frame is a document of its own, so the
 * app's shortcuts, which listen on the window, never see its keys. A press with Ctrl, Alt or the command key is a chord and goes
 * on, unless the pane's own handler already used it; every other press is the page's (letters, Tab, the arrows, Esc). A release
 * goes on for a modifier and for a key in `held`, the codes whose press was passed on: the app's key holds wait for both.
 */
export function forwardsToApp(e: FrameKey, held: ReadonlySet<string>): boolean {
  if (e.type === "keyup") return MODIFIERS.has(e.key) || held.has(e.code);
  return !e.defaultPrevented && (e.ctrlKey || e.altKey || e.metaKey);
}

/** The copy of a frame's key that the app's document gets: it bubbles to the window and can be cancelled. */
export function appKeyInit(e: FrameKey): KeyboardEventInit {
  return { key: e.key, code: e.code, ctrlKey: e.ctrlKey, shiftKey: e.shiftKey, altKey: e.altKey, metaKey: e.metaKey, repeat: e.repeat, bubbles: true, cancelable: true };
}
