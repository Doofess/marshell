import type { KeyEventLike } from "../bindings";
import type { Os } from "../keymap";
import { isNavModifierHeld, type NavModifier } from "./modifier";
import type { Dir, NavEvent, NavState } from "./types";

/**
 * Where keyboard focus is when a key goes down:
 * - terminal: inside a terminal, where every plain key belongs to the program;
 * - editable: in a text box such as the prompt line, where arrows move the caret and Ctrl+Shift+arrows select words;
 * - control: on a button, list row or other control;
 * - none: on nothing in particular (the page itself).
 */
export type FocusKind = "terminal" | "editable" | "control" | "none";

export type KeyContext = { os: Os; nav: NavModifier; focus: FocusKind; state: NavState };
export type KeyResult = { events: NavEvent[]; preventDefault: boolean } | null;

const ARROWS: Record<string, Dir> = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" };
const MODIFIER_KEYS = /^(Control|Shift|Alt|Meta)(Left|Right)$/;

const handled = (events: NavEvent[], preventDefault: boolean): KeyResult => ({ events, preventDefault });

/**
 * What a key press means to the navigation layer, or null when it is none of its business and must reach the page or the terminal
 * program untouched. Plain Tab, plain Esc and plain arrows are the program's in a terminal or a text box: navigation lives on the
 * navigation modifier, which terminals never need. The only plain keys it takes are outside a terminal: Tab with nothing focused
 * (select the first terminal) and Esc with a terminal selected but focus on a control (select nothing).
 */
export function interpretKeyDown(e: KeyEventLike, ctx: KeyContext): KeyResult {
  const { state, focus } = ctx;
  const chord = isNavModifierHeld(e, ctx.nav);
  const start: NavEvent[] = chord && !state.held ? [{ type: "modifier-down" }] : [];

  if (chord) {
    if (MODIFIER_KEYS.test(e.code)) return start.length ? handled(start, false) : null;
    if (e.code === "Tab") return handled([...start, { type: "tab" }], true);
    if (e.code === "Backspace") return handled([...start, { type: "select-none" }], true);
    if (e.code === "Escape") return handled([...start, { type: "escape" }], true);
    const dir = ARROWS[e.code];
    if (dir) {
      // In a terminal or a text box Ctrl+Shift+arrow selects a word, so it is left alone until a Tab has started navigation.
      const inText = focus === "terminal" || focus === "editable";
      if (inText && !state.engaged && !state.cursor) return null;
      return handled([...start, { type: "arrow", dir }], true);
    }
    return null;
  }

  const plain = !e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey;
  if (!plain) return null;
  if (e.code === "Tab" && focus === "none") return handled([{ type: "tab" }], true);
  if (e.code === "Escape" && (focus === "control" || focus === "none") && (state.selectedPane !== null || state.cursor)) return handled([{ type: "escape" }], false);
  return null;
}

/** The modifier going up ends a hold: whatever the cursor is on is selected. */
export function interpretKeyUp(e: KeyEventLike, ctx: { nav: NavModifier; state: NavState }): NavEvent[] {
  if (!ctx.state.held) return [];
  if (!MODIFIER_KEYS.test(e.code)) return [];
  return isNavModifierHeld(e, ctx.nav) ? [] : [{ type: "modifier-up" }];
}

/** The window lost focus mid-hold: the key-up will never come, so the cursor is cancelled, not committed. */
export function interpretBlur(state: NavState): NavEvent[] {
  if (!state.held) return [];
  return state.cursor ? [{ type: "escape" }, { type: "modifier-up" }] : [{ type: "modifier-up" }];
}
