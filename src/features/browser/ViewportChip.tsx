import type { MouseEvent } from "react";
import { chipLabel, chipText, type ViewportId } from "./viewport";
import "./ViewportChip.css";

export type ViewportChipProps = {
  id: ViewportId;
  /** The width, in px, of the place the page is shown in. */
  slot: number;
  /** Opens the viewport menu at the button's bottom-left corner. */
  onOpen: (anchor: { x: number; y: number }) => void;
};

/** The size a tab's page is held to, at the end of the bar. A click, Enter or Space opens the menu that changes it. */
export function ViewportChip({ id, slot, onOpen }: ViewportChipProps) {
  const open = (e: MouseEvent<HTMLButtonElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    onOpen({ x: r.left, y: r.bottom });
  };
  return (
    <button type="button" className="viewport-chip" aria-haspopup="menu" aria-label={chipLabel(id, slot)} title="Change the viewport" onClick={open}>
      {chipText(id, slot)}
    </button>
  );
}
