import type { KeyboardEvent, MouseEvent } from "react";
import type { PortChip } from "../../generated/PortChip";
import { keyboardAnchor, opensRowMenu } from "../sidebar/rowMenu";
import { chipLabel, openChip } from "./ports";
import "./PortChips.css";

/** A menu the chips ask for: the chips it acts on, where it opens, and the button to give the focus back to. */
export type PortMenuRequest = { chips: PortChip[]; anchor: { x: number; y: number }; opener: HTMLElement | null };

type Props = {
  ports: readonly PortChip[];
  /** At most this many chips show; the rest sit behind a +N chip. */
  max?: number;
  onMenu?: (request: PortMenuRequest) => void;
};

/** Port chips: each is a button that opens its URL in the browser. Right-click, Shift+F10 and the Menu key open its menu. */
export function PortChips({ ports, max = Infinity, onMenu }: Props) {
  if (ports.length === 0) return null;
  const shown = ports.slice(0, max);
  const rest = ports.slice(shown.length);

  const ask = (chips: PortChip[], el: HTMLElement, at?: { x: number; y: number }) => {
    if (!onMenu) return;
    onMenu({ chips, anchor: at ?? keyboardAnchor(el.getBoundingClientRect()), opener: el });
  };
  const keys = (chips: PortChip[]) => (e: KeyboardEvent<HTMLElement>) => {
    if (!opensRowMenu(e)) return;
    e.preventDefault();
    ask(chips, e.currentTarget);
  };
  const pointer = (chips: PortChip[]) => (e: MouseEvent<HTMLElement>) => {
    if (!onMenu) return;
    e.preventDefault();
    ask(chips, e.currentTarget, { x: e.clientX, y: e.clientY });
  };

  return (
    <>
      {shown.map((c) => (
        <button
          key={c.port}
          type="button"
          className="port-chip"
          data-announced={c.announced || undefined}
          aria-label={`Open ${c.url} in the browser`}
          aria-haspopup={onMenu ? "menu" : undefined}
          onClick={() => void openChip(c.url)}
          onContextMenu={pointer([c])}
          onKeyDown={keys([c])}
        >
          {chipLabel(c)}
        </button>
      ))}
      {rest.length > 0 && (
        <button
          type="button"
          className="port-chip port-chip--more"
          aria-label={`${rest.length} more ${rest.length === 1 ? "port" : "ports"}`}
          aria-haspopup="menu"
          onClick={(e) => ask(rest, e.currentTarget)}
          onContextMenu={pointer(rest)}
          onKeyDown={keys(rest)}
        >
          +{rest.length}
        </button>
      )}
    </>
  );
}
