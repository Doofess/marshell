import { useId, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import { selectedProps } from "../../../components/a11y/SelectedMark";
import { indexNodes, treeKey, visibleRows, type OutlineNode } from "./outline";
import type { SidebarView } from "./sidebarPref";

type Props = {
  /** The sidebar's id, for the button that shows and hides it. */
  id: string;
  /** The file's name, for the names of the tree and the list. */
  name: string;
  view: SidebarView;
  /** A view chosen; `takeKeyboard` when the control that chose it goes away with the change, so the view moves the keyboard into it. */
  onView: (v: SidebarView, takeKeyboard?: boolean) => void;
  /** The outline, or null while it loads. An empty one is a PDF with none. */
  outline: OutlineNode[] | null;
  /** The entry that holds the page in view. */
  section: string | null;
  /** The entries that are open. The view owns it, so a closed sidebar keeps what the person opened. */
  expanded: ReadonlySet<string>;
  onToggle: (id: string, open: boolean) => void;
  /** An entry was picked, by a click or by Enter. */
  onPick: (node: OutlineNode) => void;
  /** Esc: the keyboard goes back to the pages. */
  onLeave: () => void;
  /** The Pages view's list. */
  pages: ReactNode;
};

const VIEWS: { view: SidebarView; label: string }[] = [
  { view: "outline", label: "Outline" },
  { view: "pages", label: "Pages" },
];

const arrow = (open: boolean) => (
  <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
    <path d={open ? "M3 4.5 6 7.5 9 4.5" : "M4.5 3 7.5 6 4.5 9"} />
  </svg>
);

/** The outline and the pages of a PDF, as two views on one tab list. It is a landmark, not a dialog: the keyboard comes and goes freely. */
export function PdfSidebar({ id, name, view, onView, outline, section, expanded, onToggle, onPick, onLeave, pages }: Props) {
  const uid = useId();
  const tab = (v: SidebarView) => `${uid}-tab-${v}`;
  const panel = `${uid}-panel`;
  const tree = useRef<HTMLDivElement>(null);
  // The row that holds the Tab stop; until the keyboard has moved in the tree it is the section, or else the first row.
  const [at, setAt] = useState<string | null>(null);
  const rows = useMemo(() => (outline ? visibleRows(outline, expanded) : []), [outline, expanded]);
  const nodes = useMemo(() => (outline ? indexNodes(outline) : new Map<string, OutlineNode>()), [outline]);
  const rove = at !== null && rows.some((r) => r.id === at) ? at : rows.some((r) => r.id === section) ? section : (rows[0]?.id ?? null);

  const onTreeKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-id]");
    const from = row?.dataset.id;
    if (!from) return;
    const move = treeKey(rows, from, e.key);
    if (!move) return;
    e.preventDefault();
    if ("focus" in move) {
      setAt(move.focus);
      tree.current?.querySelector<HTMLElement>(`[data-id="${move.focus}"]`)?.focus();
    } else if ("expand" in move) onToggle(move.expand, true);
    else if ("collapse" in move) onToggle(move.collapse, false);
    else {
      const node = nodes.get(move.activate);
      if (node) onPick(node);
    }
  };

  const onTabKey = (e: KeyboardEvent<HTMLElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;
    const i = VIEWS.findIndex((v) => v.view === view);
    const to = e.key === "ArrowRight" ? (i + 1) % VIEWS.length : e.key === "ArrowLeft" ? (i + VIEWS.length - 1) % VIEWS.length : e.key === "Home" ? 0 : e.key === "End" ? VIEWS.length - 1 : -1;
    if (to < 0) return;
    e.preventDefault();
    onView(VIEWS[to]!.view);
    e.currentTarget.parentElement?.querySelectorAll<HTMLElement>('[role="tab"]')[to]?.focus();
  };

  return (
    <nav
      id={id}
      className="pdf-sidebar"
      aria-label="Outline and pages"
      // A click on the sidebar's empty space keeps the keyboard in it, so it isn't taken for the keyboard leaving.
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key !== "Escape" || e.ctrlKey || e.altKey || e.metaKey || e.shiftKey || e.nativeEvent.isComposing) return;
        e.preventDefault();
        onLeave();
      }}
    >
      <div className="pdf-sidebar__tabs" role="tablist" aria-label="Sidebar views">
        {VIEWS.map((v) => (
          <button key={v.view} type="button" role="tab" id={tab(v.view)} className="pdf-sidebar__tab" aria-selected={view === v.view} aria-controls={panel} tabIndex={view === v.view ? 0 : -1} onClick={() => onView(v.view)} onKeyDown={onTabKey}>
            {v.label}
          </button>
        ))}
      </div>
      <div className="pdf-sidebar__panel" role="tabpanel" id={panel} aria-labelledby={tab(view)}>
        {view === "pages" ? (
          pages
        ) : outline === null ? null : outline.length === 0 ? (
          <div className="pdf-sidebar__empty">
            <p className="pdf-sidebar__none">This PDF has no outline.</p>
            <button type="button" className="pdf-sidebar__show" onClick={() => onView("pages", true)}>
              Show pages
            </button>
          </div>
        ) : (
          <div ref={tree} className="pdf-sidebar__tree" role="tree" aria-label={`Outline of ${name}`} onKeyDown={onTreeKey}>
            {rows.map((r) => {
              const node = nodes.get(r.id)!;
              return (
                <div
                  key={r.id}
                  role="treeitem"
                  data-id={r.id}
                  tabIndex={r.id === rove ? 0 : -1}
                  aria-level={r.level}
                  aria-setsize={r.setsize}
                  aria-posinset={r.posinset}
                  aria-expanded={r.hasChildren ? r.expanded : undefined}
                  {...selectedProps(r.id === section, "current", "pdf-sidebar__row")}
                  style={{ "--depth": r.level - 1 } as CSSProperties}
                  data-dead={node.goal === null ? "true" : undefined}
                  onFocus={() => setAt(r.id)}
                  onClick={() => onPick(node)}
                >
                  <span
                    className="pdf-sidebar__arrow"
                    aria-hidden="true"
                    onClick={
                      r.hasChildren
                        ? (e) => {
                            e.stopPropagation();
                            onToggle(r.id, !r.expanded);
                          }
                        : undefined
                    }
                  >
                    {r.hasChildren ? arrow(r.expanded) : null}
                  </span>
                  <span className="pdf-sidebar__title">{r.title}</span>
                  {node.page !== null ? (
                    <span className="pdf-sidebar__page" aria-hidden="true">
                      {node.page}
                    </span>
                  ) : null}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </nav>
  );
}
