import { NAMES } from "../lib/navigation/fixtures";
import { NO_SELECTION_TEXT, NavCursor } from "../features/window/NavCursor";
import type { Os } from "../lib/keymap";
import { sameTarget, type NavContext, type NavState } from "../lib/navigation/types";
import "./NavDemo.css";

const WIDTH = 888;
const HEIGHT = 640;

/**
 * A bare sketch of a window (top bar, five sessions in the sidebar, four panes) that the navigation state can be drawn on.
 * It is a review frame, not the app: it shows where the cursor is, what is selected, and what the status bar would say.
 */
export function NavDemo({ state, ctx, os = "windows" }: { state: NavState; ctx: NavContext; os?: Os }) {
  const cursorRect = state.cursor ? ctx.layout.find((l) => sameTarget(l.target, state.cursor))?.rect ?? null : null;
  const label = (() => {
    const t = state.cursor;
    if (!t) return "";
    if (t.kind === "row") return `Session ${t.number}, ${NAMES[t.number] ?? ""}`;
    if (t.kind === "top") return t.id;
    const n = ctx.panes[t.index];
    return n === null || n === undefined ? `Empty slot ${t.index + 1}` : `Session ${n}, ${NAMES[n] ?? ""}`;
  })();
  const selectedSession = state.selectedPane !== null ? ctx.panes[state.selectedPane] ?? null : null;
  return (
    <div className="nav-demo" style={{ inlineSize: WIDTH, blockSize: HEIGHT }}>
      {ctx.layout.map((l) => {
        const style = { insetInlineStart: l.rect.x, insetBlockStart: l.rect.y, inlineSize: l.rect.w, blockSize: l.rect.h };
        if (l.target.kind === "top") return <div key={`t${l.target.id}`} className="nav-demo__top" style={style}>{l.target.id}</div>;
        if (l.target.kind === "row") {
          const n = l.target.number;
          const shown = ctx.panes.includes(n);
          return (
            <div key={`r${n}`} className="nav-demo__row" data-on-screen={shown} style={style}>
              <span className="nav-demo__number">{n}</span>
              {NAMES[n]}
              {shown ? "" : " · not on screen"}
            </div>
          );
        }
        const i = l.target.index;
        const n = ctx.panes[i];
        return (
          <div key={`p${i}`} className="nav-demo__pane" data-selected={state.selectedPane === i} data-empty={n === null} style={style}>
            <div className="nav-demo__pane-head">
              {n === null || n === undefined ? "Empty slot" : `${n} · ${NAMES[n]}`}
              {state.selectedPane === i && <span className="nav-demo__selected">selected</span>}
            </div>
          </div>
        );
      })}
      <div className="nav-demo__status" style={{ insetBlockStart: HEIGHT - 24, inlineSize: WIDTH }}>
        {selectedSession === null ? NO_SELECTION_TEXT : `Session ${selectedSession}, ${NAMES[selectedSession] ?? ""}`}
      </div>
      <NavCursor rect={cursorRect} label={label} os={os} />
    </div>
  );
}
