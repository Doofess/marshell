import { useEffect, useLayoutEffect, useRef, type CSSProperties } from "react";
import type { RowModel } from "../sidebar/types";
import { peekName } from "./usePeek";
import "./Peek.css";

/** Scrolls the screen to its last row: the newest output is what a glance is for, and 30 rows overflow the popover. */
export function showNewest(el: { scrollTop: number; scrollHeight: number }) {
  el.scrollTop = el.scrollHeight;
}

/**
 * A session's last rows, as plain monospace text in a non-modal popover beside its row. `rows` is null while loading (nothing
 * is drawn, so a fast read never flashes a spinner) and `failed` says the read did not work. `focusOnOpen` moves focus to the
 * text once it shows (Space); a hover leaves focus where it was. Esc closes.
 */
export function Peek({
  row,
  rows,
  failed = false,
  focusOnOpen = false,
  at,
  onClose,
  onPointerEnter,
  onPointerLeave,
}: {
  row: Pick<RowModel, "number" | "name">;
  rows: string[] | null;
  failed?: boolean;
  focusOnOpen?: boolean;
  /** Where to draw it: the window coordinates of its top-left corner. */
  at?: { x: number; y: number };
  onClose: () => void;
  onPointerEnter?: () => void;
  onPointerLeave?: () => void;
}) {
  const screen = useRef<HTMLPreElement & HTMLParagraphElement>(null);
  const shown = rows !== null || failed;
  useLayoutEffect(() => {
    if (screen.current) showNewest(screen.current);
  }, [rows]);
  useEffect(() => {
    if (focusOnOpen && shown) screen.current?.focus();
  }, [focusOnOpen, shown]);
  if (!shown) return null;
  const empty = !failed && (rows ?? []).every((r) => r.trim() === "");
  const style = at ? ({ "--peek-x": `${at.x}px`, "--peek-y": `${at.y}px` } as CSSProperties) : undefined;
  return (
    <div
      className="peek"
      role="dialog"
      aria-label={peekName(row)}
      style={style}
      onPointerEnter={onPointerEnter}
      onPointerLeave={onPointerLeave}
      onKeyDown={(e) => {
        if (e.key !== "Escape") return;
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }}
    >
      {failed ? (
        <p ref={screen} className="peek__note" tabIndex={0}>Can't read this session's screen</p>
      ) : empty ? (
        <p ref={screen} className="peek__note" tabIndex={0}>Nothing on screen yet</p>
      ) : (
        <pre ref={screen} className="peek__screen" tabIndex={0}>
          {rows!.join("\n")}
        </pre>
      )}
    </div>
  );
}
