import type { RowModel } from "../sidebar/types";

export type DropTarget = { kind: "terminal"; tabId: string } | { kind: "preview" };

type Hit = { closest(sel: string): { getAttribute(n: string): string | null } | null };

/** Where a drop at this element lands: the terminal host it is inside, otherwise the preview (the splitter, a header, the sidebar, an empty pane). */
export function dropTarget(el: Hit | null): DropTarget {
  const host = el?.closest("[data-drop-tab]") ?? null;
  const tabId = host?.getAttribute("data-drop-tab") ?? null;
  return tabId !== null && tabId !== "" ? { kind: "terminal", tabId } : { kind: "preview" };
}

/** Tauri reports a drop position in physical pixels; the page's coordinates are CSS pixels. */
export const toCss = (p: { x: number; y: number }, dpr: number): { x: number; y: number } => {
  const d = dpr > 0 ? dpr : 1;
  return { x: p.x / d, y: p.y / d };
};

/** Whether a dropped path can be pasted into this session: a paused or an ended one is refused (the 2.5b copy), any other live one takes it. */
export function dropDelivery(row: RowModel | null): { name: string; refused?: undefined } | { name: string; refused: string } | null {
  if (row === null) return null;
  if (row.status === "paused") return { name: row.name, refused: `${row.name} is paused.` };
  if (row.status === "ended") return { name: row.name, refused: `${row.name} has ended.` };
  return { name: row.name };
}
