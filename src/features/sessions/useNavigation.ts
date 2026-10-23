import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { inMenu } from "../../components/ContextMenu/menuKeys";
import type { KeyEventLike } from "../../lib/bindings";
import type { Os } from "../../lib/keymap";
import { interpretBlur, interpretKeyDown, interpretKeyUp, type FocusKind } from "../../lib/navigation/keys";
import { run, type Result } from "../../lib/navigation/machine";
import { DEFAULT_NAV_MODIFIER, isNavModifierHeld } from "../../lib/navigation/modifier";
import { INITIAL_NAV, sameTarget, type NavContext, type NavEffect, type NavEvent, type NavLayout, type NavState, type Rect, type Target } from "../../lib/navigation/types";
import { isTextBox } from "../tasks/reviewOpen";
import { measureNavLayout } from "./navLayout";

/**
 * The navigation state of the one live window. It lives outside React because the terminal's key handler (TerminalView) asks
 * `claimsKey` synchronously, between the window's capture listener and xterm, and must see the state that listener just made.
 */
let current: NavState = INITIAL_NAV;

/** Replaces the window's navigation state (the hook does; tests reset it). */
export function setNavState(state: NavState): void {
  current = state;
}

/** True when the navigation layer takes this key from the terminal or the page: a chord it acts on, never a plain program key. */
export function claimsKey(e: KeyEventLike, os: Os, focus: FocusKind): boolean {
  return interpretKeyDown(e, { os, nav: DEFAULT_NAV_MODIFIER, focus, state: current })?.preventDefault === true;
}

/** Where the keyboard is: in a terminal, in a text box, on a control, or on nothing (the page, or the main area made focusable). */
export function focusKindOf(el: Element | null): FocusKind {
  if (!el) return "none";
  if (el.closest(".xterm")) return "terminal";
  if (isTextBox(el as HTMLElement)) return "editable";
  if (el.matches("button, a, select, summary, [role=option], [tabindex]") && !el.matches(".window__main")) return "control";
  return "none";
}

/** The panes the cursor can reach. A zoomed pane hides the others, so only it keeps its session; Tab cannot select one that is not shown. */
export function navPanesOf(shown: (number | null)[], zoomed: number | null): (number | null)[] {
  return zoomed === null ? shown : shown.map((n, i) => (i === zoomed ? n : null));
}

/** The navigation model run over a batch of events: what the hook does on each key, without React or a DOM. */
export function step(state: NavState, events: NavEvent[], ctx: NavContext): Result {
  return run(state, events, ctx);
}

const TOP_NAMES: Record<string, string> = { palette: "Command palette", layout: "Split layout", launcher: "New session" };

/** What the hint under the cursor names: "Session 3, api-server", "Empty slot 2", "Command palette". */
function labelOf(t: Target, ctx: NavContext): string {
  const session = (n: number) => (ctx.names?.[n] ? `Session ${n}, ${ctx.names[n]}` : `Session ${n}`);
  if (t.kind === "row") return session(t.number);
  if (t.kind === "top") return TOP_NAMES[t.id] ?? t.id;
  const n = ctx.panes[t.index] ?? null;
  return n === null ? `Empty slot ${t.index + 1}` : session(n);
}

export type NavCursorView = { rect: Rect; label: string };

type Options = {
  os: Os;
  /** Off while the launcher, an overlay or the Review sheet has the keys. */
  enabled: boolean;
  /** The session number in each pane on screen, in pane order (`null` for an empty one). */
  panes: Array<number | null>;
  /** The view's focused pane. */
  selectedPane: number | null;
  names: Record<number, string>;
  /** The window, which the cursor's rects are measured against. */
  root: RefObject<HTMLElement | null>;
  apply: (effect: NavEffect) => void;
};

/**
 * Keyboard navigation for the live window: hold Ctrl+Shift and Tab through the terminals, or press an arrow to move a cursor over
 * the panes, the session list and the top bar; releasing selects what it is on. Ctrl+Shift+Backspace selects nothing.
 * `send` runs the same events from elsewhere (the palette's rows).
 */
export function useNavigation(opts: Options): { cursor: NavCursorView | null; noneSelected: boolean; send: (events: NavEvent[]) => void } {
  const [cursor, setCursor] = useState<NavCursorView | null>(null);
  const [noneSelected, setNoneSelected] = useState(false);
  const optsRef = useRef(opts);
  optsRef.current = opts;
  const noneRef = useRef(noneSelected);
  noneRef.current = noneSelected;
  const layout = useRef<NavLayout>([]);

  const send = useCallback((events: NavEvent[]) => {
    if (events.length === 0) return;
    const o = optsRef.current;
    // Measured on the key press, never on render: the window may have changed since the last one.
    if (o.root.current && events.some((e) => e.type !== "modifier-down")) layout.current = measureNavLayout(o.root.current);
    const ctx: NavContext = { panes: o.panes, layout: layout.current, names: o.names };
    const from: NavState = { ...current, selectedPane: noneRef.current ? null : o.selectedPane };
    const { state, effects } = step(from, events, ctx);
    current = state;
    const at = state.cursor;
    const rect = at ? layout.current.find((l) => sameTarget(l.target, at))?.rect : undefined;
    setCursor(at && rect ? { rect, label: labelOf(at, ctx) } : null);
    for (const effect of effects) {
      if (effect.type === "deselect") setNoneSelected((noneRef.current = true));
      else if (effect.type === "select-pane" || effect.type === "bring-into-view") setNoneSelected((noneRef.current = false));
      o.apply(effect);
    }
  }, []);

  useEffect(() => {
    if (!opts.enabled) return;
    const keyDown = (chordPhase: boolean) => (e: KeyboardEvent) => {
      // Chords are read first (capture), so neither the terminal nor a list sees them; a plain key comes last (bubble), so a menu's
      // own Esc or Tab, which stops the key, wins.
      if (isNavModifierHeld(e, DEFAULT_NAV_MODIFIER) !== chordPhase || (!chordPhase && e.defaultPrevented)) return;
      // An open row menu owns its keys: a chord there is not navigation.
      if (inMenu(e.target)) return;
      const result = interpretKeyDown(e, { os: optsRef.current.os, nav: DEFAULT_NAV_MODIFIER, focus: focusKindOf(document.activeElement), state: current });
      if (!result) return;
      if (result.preventDefault) {
        e.preventDefault();
        e.stopPropagation();
      }
      send(result.events);
    };
    const capture = keyDown(true);
    const bubble = keyDown(false);
    const keyUp = (e: KeyboardEvent) => send(interpretKeyUp(e, { nav: DEFAULT_NAV_MODIFIER, state: current }));
    const blur = () => send(interpretBlur(current));
    // A click into a terminal selects it again.
    const focusIn = (e: FocusEvent) => {
      if (noneRef.current && e.target instanceof Element && focusKindOf(e.target) === "terminal") setNoneSelected((noneRef.current = false));
    };
    window.addEventListener("keydown", capture, true);
    window.addEventListener("keydown", bubble);
    window.addEventListener("keyup", keyUp, true);
    window.addEventListener("blur", blur);
    window.addEventListener("focusin", focusIn);
    return () => {
      window.removeEventListener("keydown", capture, true);
      window.removeEventListener("keydown", bubble);
      window.removeEventListener("keyup", keyUp, true);
      window.removeEventListener("blur", blur);
      window.removeEventListener("focusin", focusIn);
      // A sheet opening mid-hold takes the keys, and the key-up goes to it: the hold ends here, cancelled.
      current = { ...current, held: false, engaged: false, cursor: null };
      setCursor(null);
    };
  }, [opts.enabled, send]);

  return { cursor, noneSelected, send };
}
