import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { FOCUSABLE } from "../../lib/dialogFocus";
import { trapKey } from "../../lib/focusTrap";
import type { ApprovalRequest } from "../approval/types";
import type { RowModel } from "../sidebar/types";
import { boardRows, holdGroups, splitGroups } from "./board";
import { DeparturesBoard } from "./DeparturesBoard";
import { freezeOrder } from "./freeze";
import { activeIndex, cardFor, fitLiveBoard, gateRows, heldRows, openerUsable, refocusNeeded } from "./liveBoard";
import { reduceBoard, type CardFocus } from "./nav";
import "./LiveDepartures.css";

type Props = {
  rows: RowModel[];
  requests: ApprovalRequest[];
  reduced: boolean;
  layout?: ReadonlyMap<string, string>;
  renderCard: (r: ApprovalRequest, focus: CardFocus, restore: () => void) => ReactNode;
  onJump: (rowId: string) => void;
  onAllSessions: () => void;
  onClose: (focusTerminal: boolean) => void;
};
type Hold = { order: string[]; gate: string[] };

export function LiveDepartures({ rows, requests, reduced, layout, renderCard, onJump, onAllSessions, onClose }: Props) {
  const element = useRef<HTMLDivElement>(null);
  const opener = useRef<HTMLElement | null | undefined>(undefined);
  const leaving = useRef(false);
  const rowFocused = useRef(false);
  const kept = useRef(new Map<string, RowModel>());
  const [heightPx, setHeightPx] = useState(0);
  const [hold, setHold] = useState<Hold>();
  const [place, setPlace] = useState<{ id: string | null; index: number }>({ id: null, index: 0 });
  const [open, setOpen] = useState<{ rowId: string; requestId: string; focus: CardFocus } | null>(null);

  const restore = useCallback(() => {
    const root = element.current;
    if (!root || !root.isConnected || leaving.current) return;
    (root.querySelector<HTMLElement>('.board__row[tabindex="0"]') ?? root.querySelector<HTMLElement>(FOCUSABLE) ?? root).focus();
  }, []);

  const incoming = splitGroups(boardRows(gateRows(rows, requests)));
  const incomingIds = [...incoming.gate, ...incoming.others].map((r) => r.id);
  const frozen = hold ? freezeOrder(hold.order, incomingIds) : undefined;
  const held = hold && frozen ? { order: frozen.order, gate: [...hold.gate, ...incoming.gate.filter((r) => !hold.order.includes(r.id)).map((r) => r.id)] } : undefined;
  const stale = frozen?.stale ?? [];
  const liveRows = gateRows(heldRows(rows, kept.current, stale), requests);
  const sorted = splitGroups(boardRows(liveRows));
  const groups = held ? holdGroups(sorted, held) : sorted;
  const request = open ? cardFor(requests, open.rowId) : undefined;
  const card = request?.id === open?.requestId ? request : undefined;
  const cardContent = card && open ? renderCard(card, open.focus, restore) : null;
  const fit = fitLiveBoard(heightPx, groups, cardContent !== null && cardContent !== undefined ? open?.rowId : undefined, place.id ?? undefined);
  const shown = [...fit.gate, ...fit.others];
  const active = activeIndex(shown.map((r) => r.id), place.id, place.index);
  const activeId = shown[active]?.id ?? null;

  useEffect(() => {
    const root = element.current;
    if (!root) return;
    if (opener.current === undefined) opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    leaving.current = false;
    setHeightPx(root.getBoundingClientRect().height);
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setHeightPx(entry.contentRect.height);
    });
    observer.observe(root);
    root.focus();
    return () => observer.disconnect();
  }, [restore]);

  useEffect(() => {
    if (place.id !== activeId || place.index !== active) setPlace({ id: activeId, index: active });
    const focused = document.activeElement;
    if (heightPx > 0 && (rowFocused.current || focused === element.current || (focused instanceof HTMLElement && focused.matches(".board__row")))) restore();
  }, [activeId, active, place, heightPx, restore]);

  useLayoutEffect(() => {
    const root = element.current;
    if (root && refocusNeeded({ rowFocused: rowFocused.current, leaving: leaving.current, focusInside: root.contains(document.activeElement) })) restore();
  });

  useEffect(() => {
    if (!hold) return;
    for (const row of rows) kept.current.set(row.id, row);
    if (held && held.order.length !== hold.order.length) setHold(held);
  }, [rows, hold, held]);

  useEffect(() => {
    if (open && !card) {
      setOpen(null);
      restore();
    }
  }, [open, card, restore]);

  const latest = useRef({ incoming, rows });
  latest.current = { incoming, rows };
  const hasTable = liveRows.length > 0;
  useEffect(() => {
    const table = element.current?.querySelector(".board__table");
    if (!table) return;
    const enter = () => {
      const { incoming: current, rows: sources } = latest.current;
      const order = [...current.gate, ...current.others].map((r) => r.id);
      const byId = new Map(sources.map((r) => [r.id, r]));
      kept.current = new Map(order.flatMap((id) => {
        const row = byId.get(id);
        return row ? [[id, row] as const] : [];
      }));
      setHold({ order, gate: current.gate.map((r) => r.id) });
    };
    const leave = () => {
      setHold(undefined);
      kept.current.clear();
    };
    table.addEventListener("pointerenter", enter);
    table.addEventListener("pointerleave", leave);
    return () => {
      table.removeEventListener("pointerenter", enter);
      table.removeEventListener("pointerleave", leave);
    };
  }, [hasTable]);

  const jump = (id: string) => {
    if (stale.includes(id)) return;
    leaving.current = true;
    onJump(id);
  };
  const allSessions = () => {
    leaving.current = true;
    onAllSessions();
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.code === "Tab") {
      const controls = [...e.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE)];
      const action = trapKey({ key: e.code, shiftKey: e.shiftKey }, controls.length, controls.indexOf(document.activeElement as HTMLElement));
      e.preventDefault();
      e.stopPropagation();
      if (action?.kind === "focus") controls[action.index]?.focus();
      return;
    }
    if (e.defaultPrevented || e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.repeat && ["Escape", "Enter", "KeyY", "KeyN"].includes(e.code)) return;
    if (e.code === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      if (open) {
        setOpen(null);
        restore();
      } else {
        leaving.current = true;
        const usable = openerUsable(opener.current, document.body);
        onClose(!usable);
        if (usable) opener.current?.focus();
      }
      return;
    }
    if (!(e.target instanceof HTMLElement) || !e.target.matches(".board__row")) return;
    const state = { active, card: open?.rowId ?? null, focus: open?.focus };
    const result = reduceBoard(state, e.code, shown);
    if (result.state === state && !result.jump) return;
    e.preventDefault();
    e.stopPropagation();
    if (result.jump) {
      jump(result.jump);
      return;
    }
    setPlace({ id: shown[result.state.active]?.id ?? null, index: result.state.active });
    if (result.state.card && result.state.focus) {
      const pending = cardFor(requests, result.state.card);
      if (pending) setOpen({ rowId: result.state.card, requestId: pending.id, focus: result.state.focus });
    }
  };

  return (
    <div ref={element} className="departures" role="dialog" aria-modal="true" aria-label="Departures" tabIndex={-1} onKeyDown={onKeyDown}
      onFocusCapture={(e) => {
        rowFocused.current = e.target instanceof HTMLElement && e.target.matches(".board__row");
        if (rowFocused.current) {
          const id = e.target.getAttribute("data-row-id");
          const index = shown.findIndex((r) => r.id === id);
          if (index >= 0) setPlace((previous) => previous.id === id && previous.index === index ? previous : { id, index });
        }
      }} onBlurCapture={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) rowFocused.current = false;
      }}>
      <DeparturesBoard rows={liveRows} heightPx={heightPx} active={active} layout={layout} live reduced={reduced} hold={held} stale={stale}
        card={card ? open?.rowId : undefined} keep={place.id ?? undefined} renderCard={() => cardContent}
        onRowClick={jump} onAllSessions={allSessions} />
    </div>
  );
}
