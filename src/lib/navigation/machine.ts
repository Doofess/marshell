import { topName } from "./names";
import { moveCursor } from "./spatial";
import { sameTarget, type Dir, type NavContext, type NavEffect, type NavEvent, type NavState, type Target } from "./types";

/**
 * The keyboard navigation model, with no DOM in it: state and events in, new state and effects out.
 *
 * - Tab (with the navigation modifier) selects the terminal on screen, which starts the cursor. It never reaches the sidebar.
 * - Holding the modifier and pressing an arrow starts a cursor that moves anywhere: the terminal, the sidebar list, the top bar.
 *   Releasing the modifier selects what the cursor is on. Esc while it is out cancels it.
 * - A session chosen from the sidebar that is not on screen is brought onto the screen, then selected.
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

/** The panel of the side pane that has the keyboard now: it was selected, and it is still on screen. One that left the screen is forgotten. */
const sideSelected = (s: NavState, ctx: NavContext): string | null => (s.selectedSide !== null && ctx.side?.some((p) => p.id === s.selectedSide) ? s.selectedSide : null);

/** The selected slot, forgotten if the window no longer has that many slots, or if a panel of the side pane has the keyboard instead. */
const selectedOf = (s: NavState, ctx: NavContext) => (sideSelected(s, ctx) === null && s.selectedPane !== null && s.selectedPane < ctx.panes.length ? s.selectedPane : null);

/** A panel's spoken name ("Browser"); its id when the window doesn't list it. */
const sideName = (ctx: NavContext, panel: string) => ctx.side?.find((p) => p.id === panel)?.name ?? panel;

/**
 * The stop after `from` in Tab's order: the terminals on screen, then each panel of the side pane that is on screen (in the order
 * `ctx.side` lists them), then round to the first terminal. `from` is a slot (it may be empty or gone), a panel (it may be gone),
 * or null for nothing selected.
 */
function nextStop(from: Target | null, ctx: NavContext): Target | null {
  const stops: Target[] = [...visible(ctx).map((index): Target => ({ kind: "pane", index })), ...(ctx.side ?? []).map((p): Target => ({ kind: "side", panel: p.id }))];
  if (stops.length === 0) return null;
  if (!from) return stops[0]!;
  if (from.kind === "side") {
    const at = stops.findIndex((t) => sameTarget(t, from));
    return at < 0 ? stops[0]! : stops[(at + 1) % stops.length]!;
  }
  const after = from.kind === "pane" ? from.index : -1;
  return stops.find((t) => t.kind === "side" || (t.kind === "pane" && t.index > after)) ?? stops[0]!;
}

const rectOf = (ctx: NavContext, t: Target) => ctx.layout.find((l) => sameTarget(l.target, t))?.rect;

function selectPane(state: NavState, index: number, ctx: NavContext, from?: "row"): Result {
  const n = sessionOf(ctx, index);
  const text = n === null ? `Selected empty slot ${index + 1}` : `Selected ${sessionText(ctx, n)}`;
  const effect: NavEffect = from ? { type: "select-pane", index, from } : { type: "select-pane", index };
  return { state: { ...state, selectedPane: index, selectedSide: null, cursor: null }, effects: [effect, announce(text)] };
}

function selectSide(state: NavState, panel: string, ctx: NavContext): Result {
  return { state: { ...state, selectedSide: panel, cursor: null }, effects: [{ type: "select-side", panel }, announce(`Selected ${sideName(ctx, panel)}`)] };
}

/** Commit a target: select its terminal, bring its session into view, or activate its control. */
function commit(state: NavState, t: Target, ctx: NavContext): Result {
  if (t.kind === "pane") return selectPane(state, t.index, ctx);
  if (t.kind === "side") return selectSide(state, t.panel, ctx);
  if (t.kind === "top") return { state: { ...state, cursor: null }, effects: [{ type: "activate", id: t.id }, announce(`Activated ${topName(t.id)}`)] };
  const shown = ctx.panes.indexOf(t.number);
  if (shown >= 0) return selectPane(state, shown, ctx, "row");
  const sel = selectedOf(state, ctx);
  const empty = ctx.panes.indexOf(null);
  const pane = sel ?? (empty >= 0 ? empty : 0);
  return {
    state: { ...state, selectedPane: pane, selectedSide: null, cursor: null },
    effects: [{ type: "bring-into-view", number: t.number, pane }, announce(`Selected ${sessionText(ctx, t.number)}`)],
  };
}

function describeTarget(t: Target, ctx: NavContext): string {
  if (t.kind === "row") return `Cursor on ${sessionText(ctx, t.number)}`;
  if (t.kind === "top") return `Cursor on ${topName(t.id)}`;
  if (t.kind === "side") return `Cursor on ${sideName(ctx, t.panel)}`;
  const n = sessionOf(ctx, t.index);
  return n === null ? `Cursor on empty slot ${t.index + 1}` : `Cursor on ${sessionText(ctx, n)}`;
}

function moveWith(state: NavState, dir: Dir, ctx: NavContext): Result {
  const sel = selectedOf(state, ctx);
  const first = visible(ctx)[0] ?? null;
  const onSide = sideSelected(state, ctx);
  const origin: Target | null = state.cursor ?? (onSide !== null ? { kind: "side", panel: onSide } : sel !== null ? { kind: "pane", index: sel } : first !== null ? { kind: "pane", index: first } : null);
  if (!origin) return { state, effects: [announce("No terminal on screen")] };
  // Nothing was out yet and nothing was selected: the cursor appears on the first terminal, and the next arrow moves it.
  if (!state.cursor && sel === null && onSide === null) return { state: { ...state, cursor: origin }, effects: [announce(describeTarget(origin, ctx))] };
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
      // The order is the terminals on screen, then each panel of the side pane that is drawn, then round to the first terminal.
      const onSide = sideSelected(state, ctx);
      const current: Target | null = onSide !== null ? { kind: "side", panel: onSide } : sel !== null ? { kind: "pane", index: sel } : null;
      if (state.cursor) {
        const from = state.cursor.kind === "pane" || state.cursor.kind === "side" ? state.cursor : current;
        const next = nextStop(from, ctx);
        if (next === null) return { state, effects: [announce("No terminal on screen")] };
        return { state: { ...state, cursor: next }, effects: [announce(describeTarget(next, ctx))] };
      }
      const next = nextStop(current, ctx);
      if (next === null) return { state, effects: [announce("No terminal on screen")] };
      return commit(state, next, ctx);
    }
    case "arrow":
      return moveWith(state, ev.dir, ctx);
    case "select-none":
      return { state: { ...state, selectedPane: null, selectedSide: null, cursor: null }, effects: [{ type: "deselect" }, announce("No session selected")] };
    case "escape":
      if (state.cursor) return { state: { ...state, cursor: null }, effects: [announce("Cancelled")] };
      if (state.selectedPane !== null || sideSelected(state, ctx) !== null) return { state: { ...state, selectedPane: null, selectedSide: null }, effects: [{ type: "deselect" }, announce("No session selected")] };
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
