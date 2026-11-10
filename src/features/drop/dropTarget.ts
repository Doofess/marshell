import type { BrowserEvents } from "../browser/browserApi";
import type { RowModel } from "../sidebar/types";

export type DropTarget = { kind: "terminal"; tabId: string } | { kind: "pane" };

type Hit = { closest(sel: string): { getAttribute(n: string): string | null } | null };

/** Where a drop at this element lands: the terminal host it is inside, otherwise the side pane (the divider, a header, the sidebar, an empty pane). */
export function dropTarget(el: Hit | null): DropTarget {
  const host = el?.closest("[data-drop-tab]") ?? null;
  const tabId = host?.getAttribute("data-drop-tab") ?? null;
  return tabId !== null && tabId !== "" ? { kind: "terminal", tabId } : { kind: "pane" };
}

/**
 * A drop on a page, as the shell reports it: a drop on the side pane with the files that were dropped. Nothing for an enter, a
 * leave, or a drop with no files.
 */
export function pageDrop(d: BrowserEvents["drag"]):{ target: DropTarget; paths: string[] } | null {
  if (d.kind !== "drop" || d.paths === undefined || d.paths.length === 0) return null;
  return { target: { kind: "pane" }, paths: d.paths };
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

/** Who takes a drop or a pasted image on this tab. The shell panel's session has no sidebar row, but it is a live shell and takes it too. */
export function deliveryTo(
  tabId: string,
  rows: readonly RowModel[],
  panelTabId: string | undefined,
): { name: string; panel: true; refused?: undefined } | ReturnType<typeof dropDelivery> {
  if (tabId === panelTabId) return { name: "the shell panel", panel: true };
  return dropDelivery(rows.find((r) => r.id === tabId) ?? null);
}
