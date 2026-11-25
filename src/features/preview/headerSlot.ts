import { createContext } from "react";

/**
 * The element a Files tab's tools are drawn into. `undefined`: no Files panel is around the tab; `null`: the panel is there and
 * its path bar isn't mounted yet.
 */
export const HeaderSlot = createContext<HTMLElement | null | undefined>(undefined);

export type SlotState = "absent" | "waiting" | "ready";

/** Tells no panel (`undefined`) from a panel whose path bar isn't mounted (`null`) and one whose bar is there. */
export function slotState(slot: HTMLElement | null | undefined): SlotState {
  return slot === undefined ? "absent" : slot === null ? "waiting" : "ready";
}

/** Where a tab draws its tools: its own header, the path bar's slot (only the tab on screen), or nowhere. */
export function headerPlace(o: { pathShown: boolean; active: boolean; slot: SlotState }): "header" | "slot" | "none" {
  if (!o.pathShown || o.slot === "absent") return "header";
  return o.active && o.slot === "ready" ? "slot" : "none";
}

/** Whether `to` is inside the pane or inside the slot that holds its tools. */
export function insidePane(pane: Pick<Node, "contains">, slot: Pick<Node, "contains"> | null, to: Node | null): boolean {
  return !!to && (pane.contains(to) || (slot?.contains(to) ?? false));
}
