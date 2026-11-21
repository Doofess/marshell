import { useEffect, useRef, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { AgentMark } from "../../components/AgentMark/AgentMark";
import { isAgentId } from "../../components/AgentMark/agents";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { agentLabel } from "../sessions/sessionActions";
import { clickGuard } from "./clickGuard";
import { PANEL_LABEL, type PanelId } from "./panels";
import { dragIndex, stripKey } from "./stripKeys";
import { splitName, type TabKind } from "./tabType";
import "./SidePane.css";

/** `kind` picks the tab's drawn icon, and whether its title is a file name that keeps its extension. */
export type StripTab = {
  id: string;
  title: string;
  path: string;
  kind: TabKind;
  /** Who `marshell open` opened it for: an agent id or `shell`; null for a tab the person opened. */
  openedBy: string | null;
  /** Opened by `marshell open` and not yet in front of the person: it carries the opener's mark. */
  unseen: boolean;
  /** The file has unsaved edits (a text tab): the tab shows a dot, and says so. */
  unsaved?: boolean;
  /** A preview tab: its name is italic and is read with ", preview"; the next preview replaces it. */
  preview?: boolean;
};

/** The tab's tooltip: its path or address, then who opened it, then whether its file has unsaved edits. */
export function tabTitle(t: Pick<StripTab, "path" | "openedBy"> & { unsaved?: boolean; preview?: boolean }): string {
  const edits = (t.preview === true ? " · Preview: double-click to keep" : "") + (t.unsaved === true ? " · Unsaved edits" : "");
  if (t.openedBy === null) return `${t.path}${edits}`;
  if (t.openedBy === "shell") return `${t.path} · Opened with marshell open${edits}`;
  return `${t.path} · Opened by ${agentLabel(t.openedBy)}${edits}`;
}

/** Before the type icon: the opener's own mark for a known agent, else a plain dot. Hidden from the reader; the tab's name says ", new". */
function NewMark({ openedBy }: { openedBy: string | null }) {
  if (openedBy !== null && isAgentId(openedBy)) {
    return (
      <span className="side-pane__mark" aria-hidden="true">
        <AgentMark agent={openedBy} size={12} />
      </span>
    );
  }
  return <span className="side-pane__new" aria-hidden="true" />;
}

/** A file with edits that are not saved: a plain dot, in the slot of the opener's mark. A shape, so it is not told by colour alone; the tab's name says ", unsaved". */
function UnsavedMark() {
  return <span className="side-pane__unsaved" aria-hidden="true" />;
}
export type SidePaneProps = {
  panel: PanelId;
  tabs: StripTab[];
  activeId: string | null;
  /** What the panel holds: its tabs' bodies or its empty state. */
  children: ReactNode;
  /** + : its name ("New tab" for Browser, "Open a file" for Files), whether it opens a menu, and why it can't be used. */
  plus: { label: string; menu: boolean; disabled?: string };
  /** The ⋯ button, for a panel that has a menu: its name ("Browser menu"). */
  menu?: { label: string };
  /** ⤢ : whether this panel fills the pane, and why it can't. */
  fill: { on: boolean; disabled?: string };
  /** The panel's own key, for the Hide button's tooltip: "Ctrl+Shift+A". */
  hideKeys: string;
  onSelect(id: string): void;
  onClose(id: string): void;
  /** A double-click on a tab keeps a preview tab. Optional until the Files panel passes it. */
  onKeep?(id: string): void;
  onMove(id: string, to: number): void;
  onPlus(anchor: { x: number; y: number }, opener: HTMLElement): void;
  onMenu?(anchor: { x: number; y: number }, opener: HTMLElement): void;
  onFill(): void;
  onHide(): void;
};

/** How far the pointer goes before a press on a tab becomes a drag. */
const DRAG_START_PX = 4;

const icon = (d: string) => (
  <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);

/** The tab's icon, 14 px, drawn in the current color: a globe for a web page, a page for text, the page with a band for a PDF, a frame for an image. */
function TabIcon({ kind }: { kind: TabKind }) {
  const page = "M3.5 1.5h4.5l3 3v8h-7.5Z M8 1.5v3h3";
  return (
    <svg className="side-pane__icon" data-kind={kind} width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.1" strokeLinecap="round" strokeLinejoin="round">
      {kind === "web" && (
        <>
          <circle cx="7" cy="7" r="5.5" />
          <path d="M1.5 7h11M7 1.5c1.7 1.5 2.5 3.4 2.5 5.5S8.7 11 7 12.5C5.3 11 4.5 9.1 4.5 7S5.3 3 7 1.5Z" />
        </>
      )}
      {kind === "text" && (
        <>
          <path d={page} />
          <path d="M5.5 7h3M5.5 9h3M5.5 11h1.5" />
        </>
      )}
      {kind === "pdf" && (
        <>
          <path d={page} />
          <path d="M5 8.5h4v2.5H5Z" />
        </>
      )}
      {kind === "image" && (
        <>
          <rect x="1.5" y="2.5" width="11" height="9" rx="1" />
          <circle cx="5" cy="5.5" r="1" />
          <path d="m2 11 3.5-3 2.5 2 2-1.5 2.5 2.5" />
        </>
      )}
    </svg>
  );
}

/** A file's name as two spans with no space between: the head shortens with an ellipsis, the tail keeps the extension. A web title is one span, cut at its end. */
function TabTitle({ kind, title }: { kind: TabKind; title: string }) {
  if (kind === "web") return <span className="side-pane__title">{title}</span>;
  const { head, tail } = splitName(title);
  return (
    <span className="side-pane__title">
      <span className="side-pane__name-head">{head}</span>
      <span className="side-pane__name-tail">{tail}</span>
    </span>
  );
}

type Drag = { id: string; pointerId: number; startX: number; started: boolean };

/** One panel's frame: its tab strip, ending with +, the menu (if it has one), Fill and Hide, then whatever the panel holds. */
export function SidePane({ panel, tabs, activeId, children, plus: plusSpec, menu, fill, hideKeys, onSelect, onClose, onKeep, onMove, onPlus, onMenu, onFill, onHide }: SidePaneProps) {
  const root = useRef<HTMLElement>(null);
  const plus = useRef<HTMLButtonElement>(null);
  // Where focus goes after the strip changes: a tab's id, or "active" for whichever tab became the selected one.
  const focusAfter = useRef<string | null>(null);
  const drag = useRef<Drag | null>(null);
  const swallowClick = useRef(clickGuard());

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
    // An unsaved tab is asked about first; the dialog takes the focus and gives it back, and the window moves it after the close.
    const asks = tabs.find((t) => t.id === id)?.unsaved === true;
    if (!asks && root.current?.querySelector(".side-pane__strip")?.contains(document.activeElement)) focusAfter.current = "active";
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
    if (d.started) swallowClick.current.arm();
  };

  return (
    <section ref={root} className="side-pane" aria-label={PANEL_LABEL[panel]} data-nav-side={panel}>
      <div className="side-pane__strip">
        {tabs.length > 0 && (
          <div className="side-pane__tabs" role="tablist" aria-label={`${PANEL_LABEL[panel]} tabs`}>
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
                    title={tabTitle(t)}
                    {...selectedProps(selected, "selected", t.preview === true ? "side-pane__tab side-pane__tab--preview" : "side-pane__tab")}
                    onDoubleClick={() => onKeep?.(t.id)}
                    onClick={() => {
                      // The click that ends a drag selects nothing.
                      if (swallowClick.current.take()) return;
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
                    {t.unseen ? <NewMark openedBy={t.openedBy} /> : t.unsaved === true && <UnsavedMark />}
                    <TabIcon kind={t.kind} />
                    <TabTitle kind={t.kind} title={t.title} />
                    {t.unseen && <span className="side-pane__new-word">, new</span>}
                    {t.unsaved === true && <span className="side-pane__new-word">, unsaved</span>}
                    {t.preview === true && <span className="side-pane__new-word">, preview</span>}
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
          data-pane-plus=""
          aria-label={plusSpec.label}
          aria-haspopup={plusSpec.menu ? "menu" : undefined}
          aria-disabled={plusSpec.disabled !== undefined ? "true" : undefined}
          title={plusSpec.disabled !== undefined ? `${plusSpec.label}: ${plusSpec.disabled}` : plusSpec.label}
          onClick={(e) => {
            if (plusSpec.disabled !== undefined) return;
            const r = e.currentTarget.getBoundingClientRect();
            onPlus({ x: r.left, y: r.bottom }, e.currentTarget);
          }}
        >
          {icon("M6 2.5v7M2.5 6h7")}
        </button>
        {menu && (
          <button
            type="button"
            className="side-pane__button"
            aria-label={menu.label}
            aria-haspopup="menu"
            title={menu.label}
            onClick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              onMenu?.({ x: r.left, y: r.bottom }, e.currentTarget);
            }}
          >
            {icon("M2.5 6h.01M6 6h.01M9.5 6h.01")}
          </button>
        )}
        <button
          type="button"
          className="side-pane__button"
          aria-label={`Fill the side pane with ${PANEL_LABEL[panel]}`}
          aria-pressed={fill.on}
          aria-disabled={fill.disabled !== undefined ? "true" : undefined}
          title={fill.disabled ?? (fill.on ? "Show the other panels" : `Fill the side pane with ${PANEL_LABEL[panel]}`)}
          onClick={() => {
            if (fill.disabled === undefined) onFill();
          }}
        >
          {icon("M7.5 1.5h3v3M10.5 1.5 7 5M4.5 10.5h-3v-3M1.5 10.5 5 7")}
        </button>
        <button type="button" className="side-pane__button" aria-label={`Hide ${PANEL_LABEL[panel]}`} title={`Hide ${PANEL_LABEL[panel]} (${hideKeys})`} onClick={onHide}>
          {icon("M3 6h6")}
        </button>
      </div>
      <div className="side-pane__body">{children}</div>
    </section>
  );
}
