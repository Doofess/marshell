import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type FocusEventHandler, type KeyboardEventHandler, type Ref } from "react";
import { useCoverPage } from "../../features/browser/pageCover";
import { usePopupFocus } from "../../lib/usePopupFocus";
import "./ContextMenu.css";
import { enteredRow, isAction, leavesMenu, menuKey, opensSubmenu, placeMenu, type MenuItem, type MenuRow } from "./menuKeys";
import { submenuPlace } from "./submenuPlace";

export type { MenuItem, MenuRow } from "./menuKeys";

type Props = {
  items: MenuItem[];
  /** The menu's accessible name. */
  label: string;
  /** Where it opened, in viewport pixels. */
  anchor: { x: number; y: number };
  onRun: (id: string) => void;
  onClose: (opts: { refocus: boolean }) => void;
};

const firstAction = (items: MenuItem[]) => items.findIndex(isAction);
/** The row a submenu opens on: its checked row, else its first. */
const firstInSubmenu = (rows: MenuItem[]) => {
  const checked = rows.findIndex((r) => isAction(r) && r.checked === true);
  return checked >= 0 ? checked : firstAction(rows);
};
/** The DOM row of the item at `index`: rows are the only children with a menuitem role, so index by row order. */
const rowAt = (list: HTMLElement | null, items: MenuItem[], index: number) => {
  const n = items.slice(0, index).filter(isAction).length;
  return list?.querySelectorAll<HTMLElement>('[role^="menuitem"]')[n];
};
const view = () => ({ w: window.innerWidth, h: window.innerHeight });

const rowRole = (row: MenuRow) => (row.checked === undefined ? "menuitem" : row.radio ? "menuitemradio" : "menuitemcheckbox");

/** The check of a checkbox row or the dot of a radio row, drawn in the row's text color; nothing is drawn while the row is off. */
function Mark({ row }: { row: MenuRow }) {
  return (
    <svg className="context-menu__mark" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
      {row.checked === true && (row.radio ? <circle cx="8" cy="8" r="3" /> : <path d="M3.5 8.5 6.75 11.75 12.5 4.75" />)}
    </svg>
  );
}

type ListProps = {
  items: MenuItem[];
  /** The list's accessible name: the menu's, or the row a submenu belongs to. */
  label: string;
  /** The index in `items` that holds focus. */
  active: number;
  /** The id of the row whose submenu is open, if any. */
  openId: string | null;
  onActive: (index: number) => void;
  onRun: (id: string) => void;
  /** A click, or a pointer resting, on a row that opens a submenu. */
  onOpen: (index: number) => void;
  listRef?: Ref<HTMLUListElement>;
  className?: string;
  style?: CSSProperties;
  onKeyDown?: KeyboardEventHandler;
  onBlur?: FocusEventHandler;
};

/** The rows of a menu or of one of its submenus: the part the menu and its submenu share. */
export function MenuList({ items, label, active, openId, onActive, onRun, onOpen, listRef, className, style, onKeyDown, onBlur }: ListProps) {
  const uid = useId();
  // The row the pointer last moved over, null once it left it.
  const rest = useRef<number | null>(null);
  // A menu with any checkable row gives every row the width of the mark, so the labels line up.
  const marked = items.some((i) => isAction(i) && i.checked !== undefined);
  return (
    <ul
      ref={listRef}
      className={className ? `context-menu ${className}` : "context-menu"}
      role="menu"
      aria-label={label}
      tabIndex={-1}
      style={style}
      onKeyDown={onKeyDown}
      // Focus wandering out (a pane or a box taking it) must not leave a stale menu up. Not refocused: focus has a new home.
      onBlur={onBlur}
      onContextMenu={(e) => e.preventDefault()}
    >
      {items.map((it, i) => {
        if (!isAction(it)) return <li key={`sep-${i}`} role="separator" className="context-menu__sep" />;
        const opens = opensSubmenu(it);
        const noteId = it.note === undefined ? undefined : `${uid}-note-${i}`;
        const checkable = it.checked !== undefined;
        return (
          <li
            key={it.id}
            role={rowRole(it)}
            tabIndex={-1}
            className={marked ? "context-menu__item--marked" : undefined}
            aria-keyshortcuts={it.shortcut}
            aria-disabled={it.disabled === undefined ? undefined : true}
            aria-checked={checkable ? it.checked : undefined}
            aria-haspopup={it.rows ? "menu" : undefined}
            aria-expanded={it.rows ? openId === it.id : undefined}
            aria-describedby={noteId}
            onClick={() => {
              if (it.disabled !== undefined) return;
              if (opens) onOpen(i);
              else onRun(it.id);
            }}
            onPointerMove={() => {
              const entered = enteredRow(rest.current, i);
              rest.current = i;
              if (active !== i) onActive(i);
              // A pointer coming to a row that opens a submenu opens it, once: a move over the row it already rests on leaves a
              // submenu closed with Left or Esc closed.
              if (entered && it.rows && it.disabled === undefined && openId !== it.id) onOpen(i);
            }}
            onPointerLeave={() => {
              if (rest.current === i) rest.current = null;
            }}
          >
            {marked && (checkable ? <Mark row={it} /> : <span className="context-menu__mark" aria-hidden="true" />)}
            <span className="context-menu__label">{it.label}</span>
            {it.disabled !== undefined && <span className="context-menu__reason">{it.disabled}</span>}
            {it.shortcut && (
              <span className="context-menu__shortcut" aria-hidden="true">
                {it.shortcut}
              </span>
            )}
            {it.rows && (
              <span className="context-menu__more" aria-hidden="true">
                ›
              </span>
            )}
            {it.note !== undefined && (
              <span id={noteId} className="context-menu__note">
                {it.note}
              </span>
            )}
          </li>
        );
      })}
    </ul>
  );
}

/**
 * A context menu. The active item holds real DOM focus (roving), so a screen reader reads it as it moves. It is vendor-neutral:
 * the parent must render it outside any `[data-agent]` element. A row with `rows` opens a submenu beside it: Right, Enter, a
 * click or a pointer resting opens it on its checked row, Left or Esc closes it onto its row, and Tab closes the whole menu.
 */
export function ContextMenu({ items, label, anchor, onRun, onClose }: Props) {
  const wrap = useRef<HTMLDivElement>(null);
  const top = useRef<HTMLUListElement>(null);
  const sub = useRef<HTMLUListElement>(null);
  const [active, setActive] = useState(() => firstAction(items));
  const [openId, setOpenId] = useState<string | null>(null);
  const [subActive, setSubActive] = useState(0);
  const [pos, setPos] = useState(anchor);
  const [subPos, setSubPos] = useState({ x: 0, y: 0 });
  useCoverPage();

  const openRow = openId === null ? undefined : items.find((i): i is MenuRow => isAction(i) && i.id === openId);
  const subItems = openRow?.rows;

  // Measure once laid out, then pull the menu back inside the window.
  useLayoutEffect(() => {
    const r = top.current?.getBoundingClientRect();
    if (r) setPos(placeMenu(anchor, { w: r.width, h: r.height }, view()));
  }, [anchor]);
  // A submenu sits beside the menu, level with the row that opened it.
  useLayoutEffect(() => {
    if (!subItems || openRow === undefined) return;
    const m = top.current?.getBoundingClientRect();
    const s = sub.current?.getBoundingClientRect();
    const parent = rowAt(top.current, items, items.indexOf(openRow))?.getBoundingClientRect();
    if (m && s && parent) setSubPos(submenuPlace(m, parent.top, { w: s.width, h: s.height }, view()));
  }, [openRow, subItems, items, pos]);

  // Roving focus: the active item of the open list takes DOM focus.
  const activeItem = () => (subItems ? rowAt(sub.current, subItems, subActive) : rowAt(top.current, items, active));
  useEffect(() => {
    activeItem()?.focus();
  }, [active, subActive, openId, items]);
  // Focus pulled out of the menu comes back to the active item. Tab is left alone: it closes the menu without a refocus.
  usePopupFocus(wrap, activeItem, { tab: false });

  useEffect(() => {
    const away = (e: PointerEvent) => {
      if (!wrap.current?.contains(e.target as Node)) onClose({ refocus: false });
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose]);

  const openSub = (index: number) => {
    const it = items[index];
    if (!it || !isAction(it) || !opensSubmenu(it)) return;
    setActive(index);
    // A row with the app menu's bare `submenu` marker has no rows of its own to show.
    if (!it.rows) return;
    setSubActive(firstInSubmenu(it.rows));
    setOpenId(it.id);
  };
  const onBlur: FocusEventHandler = (e) => {
    if (wrap.current && leavesMenu(e.relatedTarget, wrap.current)) onClose({ refocus: false });
  };

  const onKey = (level: "top" | "sub"): KeyboardEventHandler => (e) => {
    // The open menu owns every key: none reaches the window's shortcuts, which would act on the focused session, not this row.
    // A chord is also kept from the webview (Ctrl+Shift+R would reload it), since the window's own handler no longer sees it.
    e.stopPropagation();
    if (e.ctrlKey || e.metaKey) e.preventDefault();
    const list = level === "top" ? items : subItems;
    if (!list) return;
    const r = menuKey(e.key, list, level === "top" ? active : subActive, e);
    if (!r) return;
    // Tab keeps its default so focus moves on from where Tab puts it.
    if (!(r.kind === "close" && !r.refocus)) e.preventDefault();
    if (r.kind === "move") (level === "top" ? setActive : setSubActive)(r.index);
    else if (r.kind === "run") onRun(r.id);
    else if (r.kind === "open") {
      if (level === "top") openSub(active);
    } else if (r.kind === "back") {
      if (level === "sub") setOpenId(null);
    } else if (level === "sub" && r.refocus) setOpenId(null);
    else onClose({ refocus: r.refocus });
  };

  return (
    <div ref={wrap} style={{ display: "contents" }}>
      <MenuList
        listRef={top}
        items={items}
        label={label}
        active={active}
        openId={openId}
        style={{ position: "fixed", insetInlineStart: pos.x, insetBlockStart: pos.y }}
        onKeyDown={onKey("top")}
        onBlur={onBlur}
        onActive={(i) => {
          setActive(i);
          // The pointer moving on to another row closes a submenu that belongs to the one it left.
          if (openId !== null) setOpenId(null);
        }}
        onRun={onRun}
        onOpen={openSub}
      />
      {subItems && openRow && (
        <MenuList
          listRef={sub}
          items={subItems}
          label={openRow.label}
          active={subActive}
          openId={null}
          style={{ position: "fixed", insetInlineStart: subPos.x, insetBlockStart: subPos.y }}
          onKeyDown={onKey("sub")}
          onBlur={onBlur}
          onActive={setSubActive}
          onRun={onRun}
          onOpen={() => {}}
        />
      )}
    </div>
  );
}
