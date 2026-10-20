import { useEffect, useRef, useState } from "react";

/** A hover opens Peek after this long (docs/PLAN.md "Sidebar rows"). */
export const hoverDelay = 350;
/** Moving from the row to Peek (or back) crosses a gap; closing waits this long so the crossing does not close it. */
const leaveGrace = 120;
/** The height Peek is kept inside the window for, and the gap it keeps from the window's edge. */
export const PEEK_HEIGHT = 320;
const EDGE = 8;

export const peekName = (row: { number: number; name: string }) => `Peek: Session ${row.number}, ${row.name}`;

/** The top edge for a popover `height` tall that wants to start at `top`, kept inside a window `viewH` tall. */
export const clampTop = (top: number, viewH: number, height: number) => Math.max(EDGE, Math.min(top, viewH - height - EDGE));

export type PeekState = {
  id: string;
  /** Space takes focus into Peek and stays open until Esc; a hover leaves focus alone and closes when the pointer leaves. */
  via: "key" | "hover";
  rows: string[] | null;
  failed: boolean;
  at: { x: number; y: number };
};

/** Which session's Peek is open, what it read, and the timers that open it on hover and close it on leaving. `read` gives a session's last rows. */
export function usePeek(read: ((id: string) => Promise<string[]>) | undefined) {
  const [state, setState] = useState<PeekState | null>(null);
  const hover = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const leave = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const seq = useRef(0);
  const readRef = useRef(read);
  readRef.current = read;
  const via = useRef<PeekState["via"] | null>(null);
  via.current = state?.via ?? null;

  const clear = () => {
    clearTimeout(hover.current);
    clearTimeout(leave.current);
  };
  useEffect(() => clear, []);

  const open = (id: string, how: PeekState["via"], el: HTMLElement) => {
    const reader = readRef.current;
    if (!reader) return;
    clear();
    const box = el.getBoundingClientRect();
    const at = { x: box.right + EDGE, y: clampTop(box.top, window.innerHeight, PEEK_HEIGHT) };
    const mine = ++seq.current;
    setState({ id, via: how, rows: null, failed: false, at });
    reader(id).then(
      (rows) => mine === seq.current && setState((s) => (s ? { ...s, rows } : s)),
      () => mine === seq.current && setState((s) => (s ? { ...s, failed: true } : s)),
    );
  };

  const close = () => {
    clear();
    seq.current++;
    setState(null);
  };

  const scheduleClose = () => {
    clearTimeout(hover.current);
    clearTimeout(leave.current);
    if (via.current === "hover") leave.current = setTimeout(close, leaveGrace);
  };

  return {
    state,
    close,
    /** Space on a focused row. */
    openFromKey: (id: string, el: HTMLElement) => open(id, "key", el),
    /** The pointer is over a row: opens after `hoverDelay` unless it leaves first. */
    hoverRow: (id: string, el: HTMLElement) => {
      clear();
      if (state?.id === id) return;
      hover.current = setTimeout(() => open(id, "hover", el), hoverDelay);
    },
    leaveRow: scheduleClose,
    hoverPeek: () => clearTimeout(leave.current),
    leavePeek: scheduleClose,
  };
}
