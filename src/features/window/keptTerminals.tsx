import { useState, type ReactNode } from "react";
import type { RowModel } from "../sidebar/types";

/**
 * The sessions whose terminal is kept: those shown at least once, in the order first shown, minus any that left the list.
 * A terminal that stays mounted keeps its socket, its scrollback and its scroll position when the selection moves away and back.
 */
export function nextVisited(visited: string[], ids: string[], selectedId: string | null): string[] {
  const kept = visited.filter((id) => ids.includes(id));
  return selectedId !== null && ids.includes(selectedId) && !kept.includes(selectedId) ? [...kept, selectedId] : kept;
}

type StackProps = {
  rows: RowModel[];
  visited: string[];
  selectedId: string;
  /** Renders one session's terminal; `shown` is false while it is hidden behind the selected one. */
  render: (row: RowModel, shown: boolean) => ReactNode;
};

/** One wrapper per visited session, keyed by session id. Only the selected one is displayed; the rest carry `hidden`. */
export function TerminalStack({ rows, visited, selectedId, render }: StackProps) {
  return (
    <>
      {visited.map((id) => {
        const row = rows.find((r) => r.id === id);
        if (!row) return null;
        const shown = id === selectedId;
        return (
          <div key={id} className="window__terminal" hidden={!shown}>
            {render(row, shown)}
          </div>
        );
      })}
    </>
  );
}

/** The single window's terminals: every session shown so far stays mounted, and the selected one is on screen. */
export function KeptTerminals({ rows, selectedId, render }: { rows: RowModel[]; selectedId: string; render: StackProps["render"] }) {
  const [visited, setVisited] = useState<string[]>([]);
  const next = nextVisited(visited, rows.map((r) => r.id), selectedId);
  // Setting state while rendering is allowed for a component's own state; React renders again at once with the new list.
  if (next.length !== visited.length || next.some((id, i) => id !== visited[i])) setVisited(next);
  return <TerminalStack rows={rows} visited={next} selectedId={selectedId} render={render} />;
}
