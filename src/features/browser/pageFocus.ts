// The pure rules for where the keyboard goes between the app and a page (S12 findings 4, 8 and 9, and 3b).
import type { FocusReason } from "./browserApi";
import { focusedPage } from "./pageStore";

/** The app's element on one side of the slot in the tab order, wrapping at both ends. Undefined when the slot isn't a stop. */
export function tabNeighbour<T>(all: T[], slot: T, direction: "next" | "previous"): T | undefined {
  const i = all.indexOf(slot);
  if (i < 0) return undefined;
  return direction === "next" ? all[(i + 1) % all.length] : all[(i - 1 + all.length) % all.length];
}

/** Focus reached the slot from an element: before it in the tab order the page starts at its first element, after it at its last. */
export function enterReason(slotIsBefore: boolean): Extract<FocusReason, "next" | "previous"> {
  return slotIsBefore ? "previous" : "next";
}

/**
 * After the window comes back to the front: the page gets the keyboard again when it had it, nothing in the app was pressed since,
 * nothing covers the page, and the app's document has focus (wry moved the keys to it, S12 finding 4).
 */
export function handBack(s: { pageHadKeys: boolean; pointerSince: boolean; covered: boolean; docHasFocus: boolean }): boolean {
  return s.pageHadKeys && !s.pointerSince && !s.covered && s.docHasFocus;
}

/**
 * After a key the app took from a page has run. Focus still on the slot: the page gets the keys back. A cover the key opened
 * decides when it goes. Focus anywhere else: the key moved it (to a terminal, another session), and the page's turn is over, so a
 * later return of the window doesn't hand the keys to a page the person left.
 */
export function afterKey(s: { onSlot: boolean; covered: boolean }): "giveBack" | "wait" | "end" {
  if (s.covered) return "wait";
  return s.onSlot ? "giveBack" : "end";
}

/** After a cover goes, for a page that had the keys when it came: back to the page unless the cover put focus somewhere in the app. */
export function afterCover(s: { onSlot: boolean; onNothing: boolean }): "giveBack" | "end" {
  return s.onSlot || s.onNothing ? "giveBack" : "end";
}

/**
 * Whether the window has the keyboard. `document.hasFocus()` is false while a page has it (S12 finding 8), so a page that has it
 * counts as the window being focused.
 */
export function windowFocused(doc: { hasFocus(): boolean } = document): boolean {
  return doc.hasFocus() || focusedPage() !== null;
}

/** The part of an element `onDragArea` reads. */
type Pressed = { getAttribute(name: string): string | null; closest(selector: string): unknown };

/** A press on the title bar's drag area moves the window; it is not the user picking something in the app. */
export function onDragArea(t: Pressed): boolean {
  const own = t.getAttribute("data-tauri-drag-region");
  if (own !== null && own !== "false") return true;
  return t.closest('[data-tauri-drag-region="deep"]') !== null && t.closest('button, a, input, [role="button"]') === null;
}

const SHIFTED: Record<string, [string, string]> = {
  Digit0: ["0", ")"],
  Digit1: ["1", "!"],
  Digit2: ["2", "@"],
  Digit3: ["3", "#"],
  Digit4: ["4", "$"],
  Digit5: ["5", "%"],
  Digit6: ["6", "^"],
  Digit7: ["7", "&"],
  Digit8: ["8", "*"],
  Digit9: ["9", "("],
  Backquote: ["`", "~"],
  Minus: ["-", "_"],
  Equal: ["=", "+"],
  BracketLeft: ["[", "{"],
  BracketRight: ["]", "}"],
  Backslash: ["\\", "|"],
  Semicolon: [";", ":"],
  Quote: ["'", '"'],
  Comma: [",", "<"],
  Period: [".", ">"],
  Slash: ["/", "?"],
};

/** `KeyboardEvent.key` for a physical key on a US layout; the keymap matches on `code`, so this only has to be plausible. */
export function keyForCode(code: string, shift: boolean): string {
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return shift ? letter[1]! : letter[1]!.toLowerCase();
  if (code === "Space") return " ";
  const pair = SHIFTED[code];
  if (pair) return pair[shift ? 1 : 0];
  return code;
}

// Focusing the slot to hand the keys to the app (a forwarded key) must not read as Tab arriving at the page, which would enter it.
let quiet = false;

/** Runs `f` (a `focus()` call) so that the slot's focus handler does not enter the page. */
export function quietly<T>(f: () => T): T {
  quiet = true;
  try {
    return f();
  } finally {
    quiet = false;
  }
}

/** True while `quietly` runs. */
export const isQuiet = (): boolean => quiet;
