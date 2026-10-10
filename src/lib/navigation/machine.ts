import { moveCursor } from "./spatial";
import { sameTarget, type Dir, type NavContext, type NavEffect, type NavEvent, type NavState, type Target } from "./types";

/**
 * The keyboard navigation model, with no DOM in it: state and events in, new state and effects out.
 *
 * - Tab (with the navigation modifier) cycles the terminals on screen, in order, and wraps. It never reaches the sidebar.
 * - Holding the modifier and pressing an arrow starts a cursor that moves anywhere: other terminals, the sidebar list, the top bar.
 *   Releasing the modifier selects what the cursor is on. Esc while it is out cancels it.
 * - A session chosen from the sidebar that is not on screen is brought into the focused slot (the first empty slot when nothing is
 *   focused, the first slot when all are taken), then selected.
 * - Selecting nothing is a state: no terminal has the keyboard, and the header, accent and status bar show that.
 */
export type Result = { state: NavState; effects: NavEffect[] };

const sessionOf = (ctx: NavContext, i: number) => ctx.panes[i] ?? null;
const sessionText = (ctx: NavContext, n: number) => {
  const name = ctx.names?.[n];
  return name ? `session ${n}, ${name}` : `session ${n}`;
};
const announce = (text: string): NavEffect => ({ type: "announce", text });

/** The slots that hold a terminal, in order. Empty slots are skipped: there is nothing in them to select. */
const visible = (ctx: NavContext) => ctx.panes.flatMap((s, i) => (s === null ? [] : [i]));

/** The selected slot, forgotten if the window no longer has that many slots. */
const selectedOf = (s: NavState, ctx: NavContext) => (s.selectedPane !== null && s.selectedPane < ctx.panes.length ? s.selectedPane : null);

function nextVisible(from: number | null, ctx: NavContext): number | null {
  const list = visible(ctx);
  if (list.length === 0) return null;
  if (from === null) return list[0]!;
  return list.find((i) => i > from) ?? list[0]!;
}

const rectOf = (ctx: NavContext, t: Target) => ctx.layout.find((l) => sameTarget(l.target, t))?.rect;

function selectPane(state: NavState, index: number, ctx: NavContext): Result {
  const n = sessionOf(ctx, index);
  const text = n === null ? `Selected empty slot ${index + 1}` : `Selected ${sessionText(ctx, n)}`;
  return { state: { ...state, selectedPane: index, cursor: null }, effects: [{ type: "select-pane", index }, announce(text)] };
}

/** Commit a target: select its terminal, bring its session into view, or activate its control. */
function commit(state: NavState, t: Target, ctx: NavContext): Result {
  if (t.kind === "pane") return selectPane(state, t.index, ctx);
  if (t.kind === "top") return { state: { ...state, cursor: null }, effects: [{ type: "activate", id: t.id }, announce(`Activated ${t.id}`)] };
  const shown = ctx.panes.indexOf(t.number);
  if (shown >= 0) return selectPane(state, shown, ctx);
  const sel = selectedOf(state, ctx);
  const empty = ctx.panes.indexOf(null);
  const pane = sel ?? (empty >= 0 ? empty : 0);
  return {
    state: { ...state, selectedPane: pane, cursor: null },
    effects: [{ type: "bring-into-view", number: t.number, pane }, announce(`Selected ${sessionText(ctx, t.number)}`)],
  };
}

function describeTarget(t: Target, ctx: NavContext): string {
  if (t.kind === "row") return `Cursor on ${sessionText(ctx, t.number)}`;
  if (t.kind === "top") return `Cursor on ${t.id}`;
  const n = sessionOf(ctx, t.index);
  return n === null ? `Cursor on empty slot ${t.index + 1}` : `Cursor on ${sessionText(ctx, n)}`;
}

function moveWith(state: NavState, dir: Dir, ctx: NavContext): Result {
  const sel = selectedOf(state, ctx);
  const first = nextVisible(null, ctx);
  const origin: Target | null = state.cursor ?? (sel !== null ? { kind: "pane", index: sel } : first !== null ? { kind: "pane", index: first } : null);
  if (!origin) return { state, effects: [announce("No terminal on screen")] };
  // Nothing was out yet and nothing was selected: the cursor appears on the first terminal, and the next arrow moves it.
  if (!state.cursor && sel === null) return { state: { ...state, cursor: origin }, effects: [announce(describeTarget(origin, ctx))] };
  const rect = rectOf(ctx, origin);
  let to = rect ? moveCursor(rect, origin, dir, ctx.layout) : null;
  // Leaving a terminal to the left lands on its own session's row when the list has it, not on whichever row is level with its middle.
  if (origin.kind === "pane" && dir === "left" && to?.kind === "row") {
    const own = sessionOf(ctx, origin.index);
    if (own !== null && ctx.layout.some((l) => l.target.kind === "row" && l.target.number === own)) to = { kind: "row", number: own };
  }
  const cursor = to ?? origin;
  return { state: { ...state, cursor }, effects: to ? [announce(describeTarget(to, ctx))] : [] };
}

export function reduce(state: NavState, ev: NavEvent, ctx: NavContext): Result {
  const r = step(state, ev, ctx);
  const uses = ev.type === "tab" || ev.type === "arrow" || ev.type === "select-none";
  return uses && state.held ? { ...r, state: { ...r.state, engaged: true } } : r;
}

function step(state: NavState, ev: NavEvent, ctx: NavContext): Result {
  switch (ev.type) {
    case "modifier-down":
      return { state: { ...state, held: true, engaged: false }, effects: [] };
    case "modifier-up": {
      const cleared: NavState = { ...state, held: false, engaged: false, cursor: null };
      return state.cursor ? commit(cleared, state.cursor, ctx) : { state: cleared, effects: [] };
    }
    case "tab": {
      const sel = selectedOf(state, ctx);
      // With a cursor out, Tab carries it to the next terminal; the selection waits for the release.
      if (state.cursor) {
        const from = state.cursor.kind === "pane" ? state.cursor.index : sel;
        const next = nextVisible(from, ctx);
        if (next === null) return { state, effects: [announce("No terminal on screen")] };
        return { state: { ...state, cursor: { kind: "pane", index: next } }, effects: [announce(describeTarget({ kind: "pane", index: next }, ctx))] };
      }
      const next = nextVisible(sel, ctx);
      if (next === null) return { state, effects: [announce("No terminal on screen")] };
      return selectPane(state, next, ctx);
    }
    case "arrow":
      return moveWith(state, ev.dir, ctx);
    case "select-none":
      return { state: { ...state, selectedPane: null, cursor: null }, effects: [{ type: "deselect" }, announce("No session selected")] };
    case "escape":
      if (state.cursor) return { state: { ...state, cursor: null }, effects: [announce("Cancelled")] };
      if (state.selectedPane !== null) return { state: { ...state, selectedPane: null }, effects: [{ type: "deselect" }, announce("No session selected")] };
      return { state, effects: [] };
    case "choose":
      return commit({ ...state, cursor: null }, ev.target, ctx);
  }
}

/** Feed a list of events through `reduce`, collecting every effect in order. */
export function run(state: NavState, events: NavEvent[], ctx: NavContext): Result {
  let s = state;
  const effects: NavEffect[] = [];
  for (const ev of events) {
    const r = reduce(s, ev, ctx);
    s = r.state;
    effects.push(...r.effects);
  }
  return { state: s, effects };
}
