import { useEffect, useRef, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import { dragIndex, stripKey } from "./stripKeys";
import "./SidePane.css";

export type StripTab = { id: string; title: string; path: string };
export type SidePaneProps = {
  tabs: StripTab[];
  activeId: string | null;
  os: Os;
  /** `<KeptTabs/>` or `<EmptyPane/>`. */
  children: ReactNode;
  onSelect(id: string): void;
  onClose(id: string): void;
  onMove(id: string, to: number): void;
  onHide(): void;
  onPlus(anchor: { x: number; y: number }, opener: HTMLElement): void;
};

/** How far the pointer goes before a press on a tab becomes a drag. */
const DRAG_START_PX = 4;

const icon = (d: string) => (
  <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);

type Drag = { id: string; pointerId: number; startX: number; started: boolean };

/** The side pane's frame: the tab strip with its + and hide buttons, then whatever the pane holds. */
export function SidePane({ tabs, activeId, os, children, onSelect, onClose, onMove, onHide, onPlus }: SidePaneProps) {
  const root = useRef<HTMLElement>(null);
  const plus = useRef<HTMLButtonElement>(null);
  // Where focus goes after the strip changes: a tab's id, or "active" for whichever tab became the selected one.
  const focusAfter = useRef<string | null>(null);
  const drag = useRef<Drag | null>(null);
  const swallowClick = useRef(false);

  const tabEl = (id: string) => root.current?.querySelector<HTMLElement>(`[role="tab"][id="pane-tab-${id}"]`) ?? null;
  const focusTab = (id: string) => tabEl(id)?.focus();

  // The selected tab stays in view; and after a close or a move, focus goes where it should.
  useEffect(() => {
    if (activeId !== null) tabEl(activeId)?.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [activeId]);
  useEffect(() => {
    const want = focusAfter.current;
    if (want === null) return;
    focusAfter.current = null;
    const id = want === "active" ? activeId : want;
    if (id !== null && tabEl(id)) focusTab(id);
    else plus.current?.focus();
  });

  const tabbable = activeId !== null && tabs.some((t) => t.id === activeId) ? activeId : tabs[0]?.id;
  const close = (id: string) => {
    // Focus follows only when it was in the strip: a middle click from the terminal closes the tab and leaves focus there.
    if (root.current?.querySelector(".side-pane__strip")?.contains(document.activeElement)) focusAfter.current = "active";
    onClose(id);
  };

  const onKey = (e: ReactKeyboardEvent<HTMLElement>, id: string) => {
    const rtl = getComputedStyle(e.currentTarget).direction === "rtl";
    const k = stripKey(e, tabs.map((t) => t.id), id, rtl);
    if (!k) return;
    e.preventDefault();
    if (k.type === "select") {
      onSelect(k.id);
      focusTab(k.id);
    } else if (k.type === "close") close(k.id);
    else {
      focusAfter.current = k.id;
      onMove(k.id, k.to);
    }
  };

  const onDown = (e: ReactPointerEvent<HTMLElement>, id: string) => {
    if (e.button === 1) {
      e.preventDefault(); // no autoscroll
      return;
    }
    if (e.button !== 0) return;
    drag.current = { id, pointerId: e.pointerId, startX: e.clientX, started: false };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onDragMove = (e: ReactPointerEvent<HTMLElement>) => {
    const d = drag.current;
    if (!d || d.pointerId !== e.pointerId) return;
    // A reorder moves the tab's node, which drops the pointer capture, so the release can land outside the strip unseen:
    // a move with the button up ends the drag.
    if ((e.buttons & 1) === 0) {
      drag.current = null;
      return;
    }
    if (!d.started) {
      if (Math.abs(e.clientX - d.startX) < DRAG_START_PX) return;
      d.started = true;
    }
    const wraps = Array.from(root.current?.querySelectorAll<HTMLElement>("[data-tab-wrap]") ?? []);
    const from = tabs.findIndex((t) => t.id === d.id);
    if (from < 0 || wraps.length !== tabs.length) return;
    const to = dragIndex(wraps.map((w) => w.getBoundingClientRect()), e.clientX, from);
    if (to !== from) {
      focusAfter.current = d.id;
      onMove(d.id, to);
    }
  };
  const onDragEnd = (e: ReactPointerEvent<HTMLElement>) => {
    const d = drag.current;
    if (!d || d.pointerId !== e.pointerId) return;
    drag.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    if (d.started) swallowClick.current = true;
  };

  return (
    <section ref={root} className="side-pane" aria-label="Side pane" data-nav-side="" data-drop-pane="">
      <div className="side-pane__strip">
        {tabs.length > 0 && (
          <div className="side-pane__tabs" role="tablist" aria-label="Tabs">
            {tabs.map((t) => {
              const selected = t.id === activeId;
              return (
                <div key={t.id} className="side-pane__tab-wrap" role="presentation" data-tab-wrap="" data-selected={selected ? "true" : undefined}>
                  <button
                    type="button"
                    role="tab"
                    id={`pane-tab-${t.id}`}
                    aria-controls={`pane-panel-${t.id}`}
                    tabIndex={t.id === tabbable ? 0 : -1}
                    title={t.path}
                    {...selectedProps(selected, "selected", "side-pane__tab")}
                    onClick={() => {
                      if (swallowClick.current) {
                        swallowClick.current = false;
                        return;
                      }
                      onSelect(t.id);
                    }}
                    onKeyDown={(e) => onKey(e, t.id)}
                    onPointerDown={(e) => onDown(e, t.id)}
                    onPointerMove={onDragMove}
                    onPointerUp={onDragEnd}
                    onPointerCancel={onDragEnd}
                    onAuxClick={(e) => {
                      if (e.button !== 1) return;
                      e.preventDefault();
                      close(t.id);
                    }}
                  >
                    <span className="side-pane__title">{t.title}</span>
                  </button>
                  <button type="button" className="side-pane__close" tabIndex={-1} aria-label={`Close ${t.title}`} onClick={() => close(t.id)}>
                    {icon("M3 3l6 6M9 3 3 9")}
                  </button>
                </div>
              );
            })}
          </div>
        )}
        <span className="side-pane__spacer" />
        <button
          ref={plus}
          type="button"
          className="side-pane__button"
          aria-label="New tab"
          aria-haspopup="menu"
          title="New tab"
          onClick={(e) => {
            const r = e.currentTarget.getBoundingClientRect();
            onPlus({ x: r.left, y: r.bottom }, e.currentTarget);
          }}
        >
          {icon("M6 2.5v7M2.5 6h7")}
        </button>
        <button type="button" className="side-pane__button" aria-label="Hide the side pane" title={`Hide the side pane (${shortcutLabel(commandById("preview").binding!, os)})`} onClick={onHide}>
          {icon("M4.5 2.5 8 6 4.5 9.5")}
        </button>
      </div>
      <div className="side-pane__body">{children}</div>
    </section>
  );
}
