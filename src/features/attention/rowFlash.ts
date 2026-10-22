import { useEffect, useMemo, useRef, useState } from "react";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";
import type { RowModel } from "../sidebar/types";

export const FLASH_MS = 600;

export function attentionFlashes(prev: ReadonlyMap<string, GlyphKind> | null, rows: RowModel[]): string[] {
  if (prev === null) return [];
  return rows.filter((row) => !row.muted && prev.has(row.id) && prev.get(row.id) !== row.status && (
    row.status === "needs-permission" || row.status === "needs-question" || row.status === "done-unseen" || row.status === "error"
  )).map((row) => row.id);
}

type Flash = { timer?: ReturnType<typeof setTimeout>; frame?: number };
function cancel(flash: Flash) {
  if (flash.timer !== undefined) clearTimeout(flash.timer);
  if (flash.frame !== undefined) cancelAnimationFrame(flash.frame);
}

export function useRowFlash(rows: RowModel[]): RowModel[] {
  const prev = useRef<ReadonlyMap<string, GlyphKind> | null>(null);
  const runs = useRef(new Map<string, Flash>());
  const [flashing, setFlashing] = useState<ReadonlySet<string>>(() => new Set());

  useEffect(() => {
    const ids = attentionFlashes(prev.current, rows);
    prev.current = new Map(rows.map((row) => [row.id, row.status]));
    const present = new Set(rows.filter((row) => !row.muted).map((row) => row.id));
    const clear = (id: string) => setFlashing((current) => {
      const next = new Set(current);
      next.delete(id);
      return next;
    });
    for (const [id, run] of runs.current) {
      if (present.has(id)) continue;
      cancel(run);
      runs.current.delete(id);
      clear(id);
    }
    for (const id of ids) {
      const previous = runs.current.get(id);
      if (previous) cancel(previous);
      const run: Flash = {};
      runs.current.set(id, run);
      const start = () => {
        run.frame = undefined;
        setFlashing((current) => new Set(current).add(id));
        run.timer = setTimeout(() => {
          runs.current.delete(id);
          clear(id);
        }, FLASH_MS);
      };
      if (previous) {
        // Remove the animation for a frame before playing it again.
        clear(id);
        run.frame = requestAnimationFrame(start);
      } else {
        start();
      }
    }
  }, [rows]);

  useEffect(() => () => {
    for (const run of runs.current.values()) cancel(run);
    runs.current.clear();
    prev.current = null;
  }, []);

  // Memoised so the sidebar keeps the same rows between renders while nothing changes.
  return useMemo(() => rows.map((row) => ({ ...row, flash: !row.muted && flashing.has(row.id) })), [rows, flashing]);
}
